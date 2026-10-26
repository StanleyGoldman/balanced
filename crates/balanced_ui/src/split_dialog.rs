// SPDX-License-Identifier: GPL-3.0-or-later WITH AdditionRef-Balanced-Plugin-Exception

//! The Split dialog's form: a transaction's split lines and the total they add up to,
//! edited until Done hands them back to the Register's editing row.

use balanced_core::{AccountId, CurrencyCode, LedgerView, Money, MoneyError, TransactionId};
use balanced_i18n::{AmountParseError, AmountReader, AmountStyle, Localizer};
use eframe::egui::{
    self, Align, Button, Id, Key, Label, Layout, Modifiers, RichText, ScrollArea, Sense, TextEdit,
    vec2,
};

use crate::amount_field;
use crate::combo_field::{self, ComboText, ListScope, text_typed};
use crate::combobox::{CategoryTarget, FieldChoices, ListRow};
use crate::field_keys::FieldKeys;
use crate::formatted_amount::{FormattedAmount, i18n_currency};
use crate::i18n::fl;
use crate::register_model::{
    AmountSide, RegisterRow, RowId, SplitLineRow, SplitLines, target_text,
};
use crate::theme::Theme;

/// What the Split dialog opens on: the row whose lines it edits, as the Register shows
/// it, pending changes and all.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SplitStart {
    row: RowId,
    currency: CurrencyCode,
    /// The row's amount, which only the draft row can be without.
    amount: Option<Money>,
    lines: Vec<SplitLineRow>,
}

impl SplitStart {
    /// The Split dialog on `row`: its lines when it is split, else one line with its
    /// category or the account its transfer goes to, if it has either, for its whole
    /// amount.
    pub(crate) fn of(row: &RegisterRow) -> Self {
        let amount = row.amount();
        let whole = amount.unwrap_or_else(|| Money::zero(row.currency()));
        let lines = if row.is_split() {
            row.split_lines().to_vec()
        } else if let Some(category) = row.category_id() {
            vec![SplitLineRow::new(
                CategoryTarget::Category(category),
                None,
                "",
                whole,
            )]
        } else if let Some(account) = row.transfer_account() {
            let kept = row.other_side().map(|other_side| other_side.transaction);
            vec![SplitLineRow::new(
                CategoryTarget::Transfer(account),
                kept,
                "",
                whole,
            )]
        } else {
            Vec::new()
        };
        Self {
            row: row.id(),
            currency: row.currency(),
            amount,
            lines,
        }
    }

    /// The lines as the row holds them, with its amount, or `None` while it has
    /// neither.
    fn split(&self) -> Option<SplitLines> {
        let amount = self.amount?;
        Some(SplitLines::new(amount, self.lines.clone()))
    }
}

/// The lines Done hands back to the row they were opened on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SplitDone {
    pub(crate) row: RowId,
    pub(crate) lines: SplitLines,
}

/// What Done made of the form.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum DoneOutcome {
    /// The lines don't add up, or can't be read; the form shows why.
    Refused,
    /// The lines are as the Dialog opened on, so the row keeps what it had.
    Unchanged,
    /// The row takes these lines.
    Changed(SplitDone),
}

/// The other side a line had when the Dialog opened, and the account holding it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct KeptSide {
    account: AccountId,
    other_side: TransactionId,
}

/// A line's place in the form, counting from 1, as the user reads it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct LineNumber(usize);

impl LineNumber {
    /// The number of the line at `index`, counting from 0.
    fn at(index: usize) -> Self {
        Self(index + 1)
    }
}

/// Which amount field Done couldn't read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AmountField {
    Total,
    Line(LineNumber),
}

/// One line of the form, as typed.
#[derive(Debug, Clone)]
struct LineForm {
    /// Tells the line's fields apart from other lines', whichever are removed.
    key: usize,
    category: ComboText,
    /// The other side the line had when the Dialog opened, which it keeps while its
    /// category still names that account.
    kept: Option<KeptSide>,
    memo: String,
    amount: String,
}

/// Why Done didn't close the Dialog, shown below the lines until a field changes.
#[derive(Debug, Clone, PartialEq, Eq)]
enum SplitError {
    /// The line has text but no category.
    NoCategory(LineNumber),
    /// No category or account is named `name`, which the line holds.
    UnknownCategory { line: LineNumber, name: String },
    /// The field's amount can't be read.
    Amount {
        field: AmountField,
        error: AmountParseError,
    },
    /// The lines add up to more than an amount can hold.
    Sum(MoneyError),
    /// No line has a category.
    NoLines,
    /// The lines add up to `lines`, not to `total`; both in the row's direction.
    Mismatch { lines: Money, total: Money },
}

/// What the Split dialog holds while it is open.
#[derive(Debug, Clone)]
pub(crate) struct SplitForm {
    row: RowId,
    currency: CurrencyCode,
    /// The way the row's money moves. Amounts are typed in this direction, and a
    /// negative one goes the other way.
    side: AmountSide,
    /// Reads amounts with the formatting locale the Dialog opened in.
    reader: AmountReader,
    choices: FieldChoices,
    /// The lines the Dialog opened on, which Done compares the typed ones against.
    start: Option<SplitLines>,
    total: String,
    lines: Vec<LineForm>,
    next_key: usize,
    error: Option<SplitError>,
    /// The field that takes keyboard focus on the next frame.
    focus: Option<Id>,
}

impl SplitForm {
    /// The form on `start`, a row of `account` in `view`, with its amounts formatted
    /// by `i18n`. The first line's Category field takes focus, and a row with no lines
    /// starts with a blank one.
    pub(crate) fn new(
        start: &SplitStart,
        view: &LedgerView,
        account: AccountId,
        i18n: &Localizer,
    ) -> Self {
        let side = start.amount.map_or(AmountSide::Payment, AmountSide::of);
        let shown = |money: Money| {
            if money.amount().is_zero() {
                String::new()
            } else {
                FormattedAmount::new(i18n, side.typed(money), AmountStyle::Bare)
                    .text()
                    .to_owned()
            }
        };
        let mut form = Self {
            row: start.row,
            currency: start.currency,
            side,
            reader: i18n.amount_reader(i18n_currency(start.currency)),
            choices: FieldChoices::new(view, account, i18n),
            start: start.split(),
            total: start.amount.map(shown).unwrap_or_default(),
            lines: Vec::new(),
            next_key: 0,
            error: None,
            focus: None,
        };
        for line in &start.lines {
            let kept = match (line.target(), line.other_side()) {
                (CategoryTarget::Transfer(account), Some(other_side)) => Some(KeptSide {
                    account,
                    other_side,
                }),
                (CategoryTarget::Transfer(_), None) | (CategoryTarget::Category(_), _) => None,
            };
            let key = form.next_key();
            form.lines.push(LineForm {
                key,
                category: ComboText::new(target_text(view, line.target()).unwrap_or_default()),
                kept,
                memo: line.memo().to_owned(),
                amount: shown(line.amount()),
            });
        }
        if form.lines.is_empty() {
            form.add_blank_line();
        }
        form.focus = form.lines.first().map(|line| category_id(line.key));
        form
    }

    /// The key the next line added takes.
    fn next_key(&mut self) -> usize {
        let key = self.next_key;
        self.next_key += 1;
        key
    }

    /// Adds a line with every field empty, at the end.
    fn add_blank_line(&mut self) {
        let key = self.next_key();
        self.lines.push(LineForm {
            key,
            category: ComboText::new(String::new()),
            kept: None,
            memo: String::new(),
            amount: String::new(),
        });
    }

    /// The row's lines as typed, once they add up to the total, else why not, which
    /// the form then shows. Lines left blank are dropped.
    pub(crate) fn done(&mut self) -> DoneOutcome {
        match self.read() {
            Ok(lines) if self.start.as_ref() == Some(&lines) => DoneOutcome::Unchanged,
            Ok(lines) => DoneOutcome::Changed(SplitDone {
                row: self.row,
                lines,
            }),
            Err(error) => {
                self.error = Some(error);
                DoneOutcome::Refused
            }
        }
    }

    /// Reads the lines and the total, and checks the lines add up to it.
    fn read(&self) -> Result<SplitLines, SplitError> {
        let total = self
            .read_amount(&self.total)
            .map_err(|error| SplitError::Amount {
                field: AmountField::Total,
                error,
            })?;
        let mut lines = Vec::new();
        for (index, line) in self.lines.iter().enumerate() {
            let number = LineNumber::at(index);
            let name = line.category.text().trim();
            if name.is_empty() && line.memo.trim().is_empty() && line.amount.trim().is_empty() {
                continue;
            }
            if name.is_empty() {
                return Err(SplitError::NoCategory(number));
            }
            let target = *self
                .choices
                .categories()
                .named(name)
                .ok_or_else(|| SplitError::UnknownCategory {
                    line: number,
                    name: name.to_owned(),
                })?
                .value();
            let amount = self
                .read_amount(&line.amount)
                .map_err(|error| SplitError::Amount {
                    field: AmountField::Line(number),
                    error,
                })?;
            let other_side = line
                .kept
                .filter(|kept| target == CategoryTarget::Transfer(kept.account))
                .map(|kept| kept.other_side);
            lines.push(SplitLineRow::new(
                target,
                other_side,
                line.memo.trim(),
                amount,
            ));
        }
        if lines.is_empty() {
            return Err(SplitError::NoLines);
        }
        let sum = lines
            .iter()
            .try_fold(Money::zero(self.currency), |sum, line| {
                sum.checked_add(line.amount())
            })
            .map_err(SplitError::Sum)?;
        if sum != total {
            return Err(SplitError::Mismatch {
                lines: self.side.typed(sum),
                total: self.side.typed(total),
            });
        }
        Ok(SplitLines::new(total, lines))
    }

    /// The signed amount `text` reads as, typed in the row's direction. Blank text is
    /// zero.
    fn read_amount(&self, text: &str) -> Result<Money, AmountParseError> {
        let text = text.trim();
        if text.is_empty() {
            return Ok(Money::zero(self.currency));
        }
        let calculation = self.reader.read(text)?;
        let money = Money::new(calculation.amount(), self.currency)
            .expect("the reader gives amounts at the currency's decimal places");
        Ok(self.side.typed(money))
    }

    /// How wide the Dialog's body is in `ctx`: a line's fields and its Remove button.
    pub(crate) fn body_width(ctx: &egui::Context) -> f32 {
        let gap = ctx.global_style().spacing.item_spacing.x;
        Theme::SPLIT_CATEGORY_WIDTH
            + Theme::SPLIT_MEMO_WIDTH
            + Theme::SPLIT_AMOUNT_WIDTH
            + Theme::SPLIT_REMOVE_WIDTH
            + 3.0 * gap
    }

    /// Draws the lines, scrolling past [`Theme::SPLIT_LINES_SHOWN`] of them, then Add
    /// line, the total and why Done was refused, in `theme`. Category fields complete
    /// typed text when `inline_completion` is on.
    pub(crate) fn show(
        &mut self,
        ui: &mut egui::Ui,
        i18n: &Localizer,
        theme: &Theme,
        inline_completion: bool,
    ) {
        let height = theme.row_height();
        let left = Layout::left_to_right(Align::Center);
        let right = Layout::right_to_left(Align::Center);
        ui.horizontal(|ui| {
            let category = fl!(i18n, "register-column-category");
            heading(ui, category, Theme::SPLIT_CATEGORY_WIDTH, height, left);
            let memo = fl!(i18n, "register-column-memo");
            heading(ui, memo, Theme::SPLIT_MEMO_WIDTH, height, left);
            let amount = fl!(i18n, "dialog-split-amount");
            heading(ui, amount, Theme::SPLIT_AMOUNT_WIDTH, height, right);
        });
        let pitch = ui.spacing().interact_size.y.max(height) + ui.spacing().item_spacing.y;
        #[expect(
            clippy::cast_precision_loss,
            reason = "SPLIT_LINES_SHOWN is a handful of lines"
        )]
        let lines_height = Theme::SPLIT_LINES_SHOWN as f32 * pitch;
        let mut removed = None;
        let fields = LineFields {
            i18n,
            theme,
            inline_completion,
        };
        ScrollArea::vertical()
            .max_height(lines_height)
            .min_scrolled_height(0.0)
            .auto_shrink([false, true])
            .show(ui, |ui| {
                for index in 0..self.lines.len() {
                    if self.line(ui, fields, index) {
                        removed = Some(index);
                    }
                }
            });
        if let Some(index) = removed {
            self.lines.remove(index);
            self.error = None;
        }
        if ui.button(fl!(i18n, "dialog-split-add-line")).clicked() {
            self.add_blank_line();
            self.focus = self.lines.last().map(|line| category_id(line.key));
            self.error = None;
        }
        self.total_field(ui, i18n, height);
        if let Some(error) = &self.error {
            let text = self.error_text(i18n, error);
            ui.add(Label::new(
                RichText::new(text).color(ui.visuals().error_fg_color),
            ));
        }
    }

    /// Draws the Total field, under the lines' amounts, labelled at its left.
    fn total_field(&mut self, ui: &mut egui::Ui, i18n: &Localizer, height: f32) {
        ui.horizontal(|ui| {
            let gap = ui.spacing().item_spacing.x;
            let label = ui
                .allocate_ui_with_layout(
                    vec2(
                        Theme::SPLIT_CATEGORY_WIDTH + Theme::SPLIT_MEMO_WIDTH + gap,
                        height,
                    ),
                    Layout::right_to_left(Align::Center),
                    |ui| ui.label(fl!(i18n, "dialog-split-total")),
                )
                .inner;
            let total_id = Id::new("split_total");
            if ui.memory(|memory| memory.has_focus(total_id)) {
                amount_field::refuse_characters(ui.ctx(), &self.reader);
            }
            let total = amount_text_field(
                ui,
                &mut self.total,
                total_id,
                &fl!(i18n, "dialog-split-total"),
            )
            .labelled_by(label.id);
            if total.changed() {
                self.error = None;
            }
        });
    }

    /// Draws line `index`, one of the form's: its Category, Memo and Amount fields and
    /// its Remove button. Returns whether Remove was clicked.
    fn line(&mut self, ui: &mut egui::Ui, fields: LineFields<'_>, index: usize) -> bool {
        let LineFields { i18n, .. } = fields;
        let number = LineNumber::at(index).0;
        ui.horizontal(|ui| {
            self.category_field(ui, fields, index);
            // `show` draws only indices below the number of lines.
            let line = &mut self.lines[index];
            let memo = text_field(
                ui,
                TextEdit::singleline(&mut line.memo)
                    .id(Id::new(("split_memo", line.key)))
                    .desired_width(Theme::SPLIT_MEMO_WIDTH),
                &fl!(i18n, "dialog-split-line-memo", line = number),
            );
            let amount_id = Id::new(("split_amount", line.key));
            if ui.memory(|memory| memory.has_focus(amount_id)) {
                amount_field::refuse_characters(ui.ctx(), &self.reader);
            }
            let amount = amount_text_field(
                ui,
                &mut line.amount,
                amount_id,
                &fl!(i18n, "dialog-split-line-amount", line = number),
            );
            if memo.changed() || amount.changed() {
                self.error = None;
            }
            let remove = ui.add_sized(
                vec2(Theme::SPLIT_REMOVE_WIDTH, ui.spacing().interact_size.y),
                Button::new(fl!(i18n, "dialog-split-remove-line")),
            );
            let label = fl!(i18n, "dialog-split-remove-line", "a11y", line = number);
            ui.ctx()
                .accesskit_node_builder(remove.id, |node| node.set_label(label));
            remove.clicked()
        })
        .inner
    }

    /// Draws line `index`'s Category field with its ⏷ button and list, taking the keys
    /// its list answers first.
    fn category_field(&mut self, ui: &mut egui::Ui, fields: LineFields<'_>, index: usize) {
        let LineFields {
            i18n,
            theme,
            inline_completion,
        } = fields;
        let height = ui.spacing().interact_size.y;
        let gap = ui.spacing().item_spacing.x;
        let choices = &self.choices;
        let list = |text: &str| choices.categories().list(text);
        // `show` draws only indices below the number of lines.
        let line = &mut self.lines[index];
        let id = category_id(line.key);
        if ui.memory(|memory| memory.has_focus(id)) {
            list_keys(ui.ctx(), &mut line.category, list);
        }
        let response = ui.add_sized(
            vec2(Theme::SPLIT_CATEGORY_WIDTH - height - gap, height),
            TextEdit::singleline(line.category.text_mut())
                .id(id)
                // Esc and the arrows reach `list_keys` before egui takes the field's
                // focus or moves it on them.
                .event_filter(FieldKeys::new().with_vertical_arrows().into()),
        );
        let number = LineNumber::at(index).0;
        let label = fl!(i18n, "dialog-split-line-category", line = number);
        ui.ctx()
            .accesskit_node_builder(response.id, |node| node.set_label(label));
        if self.focus == Some(id) {
            response.request_focus();
            self.focus = None;
        }
        if response.changed() {
            self.error = None;
            let completed = line.category.forget_completion();
            line.category.open_list(ListScope::Matching, list);
            if inline_completion && text_typed(ui.ctx()) {
                line.category.complete_typing(ui.ctx(), id, list);
            } else if completed {
                line.category.highlight_none();
            }
        }
        let (button, _) = ui.allocate_exact_size(vec2(height, height), Sense::hover());
        if combo_field::list_button(ui, i18n, button, id).clicked() {
            if line.category.lists_all() {
                line.category.close_list();
            } else {
                line.category.open_list(ListScope::All, list);
            }
            ui.memory_mut(|memory| memory.request_focus(id));
        } else if response.lost_focus() {
            line.category.take_highlighted(list);
        }
        let rows = line.category.rows(list);
        let pressed = line.category.show_list(
            ui,
            i18n,
            id.with("list"),
            &rows,
            response.rect,
            theme.row_height(),
        );
        if let Some(ListRow::Choice(text)) = pressed {
            line.category.set_text(text);
            line.category.close_list();
        }
    }

    /// Why Done didn't close the Dialog, in words.
    fn error_text(&self, i18n: &Localizer, error: &SplitError) -> String {
        let format = |money: Money| {
            FormattedAmount::new(i18n, money, AmountStyle::Bare)
                .text()
                .to_owned()
        };
        match error {
            SplitError::NoCategory(line) => {
                fl!(i18n, "dialog-split-no-category", line = line.0)
            }
            SplitError::UnknownCategory { line, name } => fl!(
                i18n,
                "dialog-split-unknown-category",
                line = line.0,
                name = name.clone()
            ),
            SplitError::Amount { field, error } => {
                let error = amount_field::error_text(i18n, *error, &self.reader, self.currency);
                match field {
                    AmountField::Line(line) => {
                        fl!(
                            i18n,
                            "dialog-split-line-error",
                            line = line.0,
                            error = error
                        )
                    }
                    AmountField::Total => fl!(i18n, "dialog-split-total-error", error = error),
                }
            }
            SplitError::Sum(error) => {
                debug_assert_eq!(
                    *error,
                    MoneyError::Overflow,
                    "the lines are all in the row's currency"
                );
                let error = fl!(i18n, "field-amount-out-of-range");
                fl!(i18n, "dialog-split-total-error", error = error)
            }
            SplitError::NoLines => fl!(i18n, "dialog-split-no-lines"),
            SplitError::Mismatch { lines, total } => {
                let difference = total
                    .checked_sub(*lines)
                    .map(|difference| format(AmountSide::Deposit.signed(difference)))
                    .unwrap_or_default();
                fl!(
                    i18n,
                    "dialog-split-mismatch",
                    lines = format(*lines),
                    total = format(*total),
                    difference = difference
                )
            }
        }
    }
}

/// What every line's fields are drawn with.
#[derive(Clone, Copy)]
struct LineFields<'a> {
    i18n: &'a Localizer,
    theme: &'a Theme,
    /// Whether Category fields complete typed text.
    inline_completion: bool,
}

/// The id of the Category field of the line with `key`.
fn category_id(key: usize) -> Id {
    Id::new(("split_category", key))
}

/// A column heading `width` wide and `height` high, laid out by `layout`.
fn heading(ui: &mut egui::Ui, title: String, width: f32, height: f32, layout: Layout) {
    ui.allocate_ui_with_layout(vec2(width, height), layout, |ui| {
        ui.set_min_width(width);
        ui.label(RichText::new(title).strong());
    });
}

/// Adds `field`, which a screen reader reads as `label`.
fn text_field(ui: &mut egui::Ui, field: TextEdit<'_>, label: &str) -> egui::Response {
    let response = ui.add(field);
    let label = label.to_owned();
    ui.ctx()
        .accesskit_node_builder(response.id, |node| node.set_label(label));
    response
}

/// An amount field holding `text`, with `id`, its text at the right as the Register
/// shows amounts, which a screen reader reads as `label`.
fn amount_text_field(ui: &mut egui::Ui, text: &mut String, id: Id, label: &str) -> egui::Response {
    text_field(
        ui,
        TextEdit::singleline(text)
            .id(id)
            .desired_width(Theme::SPLIT_AMOUNT_WIDTH)
            .horizontal_align(Align::Max),
        label,
    )
}

/// Acts on this frame's keys for the focused Category field `combo`, whose list rows
/// `list` gives: the arrows as [`ComboText::arrow_keys`] takes them, and while the
/// list is open, Esc closes it and Enter takes its highlighted row. The keys it acts on
/// go no further, so Esc and Enter close the Dialog only while the list is closed.
fn list_keys<'a>(
    ctx: &egui::Context,
    combo: &mut ComboText,
    list: impl Fn(&str) -> Vec<ListRow<'a>>,
) {
    let open = !combo.rows(&list).is_empty();
    combo.arrow_keys(ctx, &list);
    let (escape, enter) = ctx.input_mut(|input| {
        (
            open && input.consume_key(Modifiers::NONE, Key::Escape),
            open && input.consume_key(Modifiers::NONE, Key::Enter),
        )
    });
    if escape {
        combo.close_list();
        combo.drop_completion();
    } else if enter {
        combo.take_highlighted(&list);
    }
}
