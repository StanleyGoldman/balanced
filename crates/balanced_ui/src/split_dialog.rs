// SPDX-License-Identifier: GPL-3.0-or-later WITH AdditionRef-Balanced-Plugin-Exception

//! The Split dialog's form: a transaction's split lines and the total they add up to,
//! edited until Done hands them back to the Register's editing row.

use balanced_core::{AccountId, CurrencyCode, LedgerView, Money, TransactionId};
use balanced_i18n::{AmountParseError, AmountReader, AmountStyle, Localizer};
use eframe::egui::{
    self, Align, Button, EventFilter, Id, Key, Label, Layout, Modifiers, RichText, ScrollArea,
    Sense, TextEdit, vec2,
};

use crate::amount_field;
use crate::combo_field::{self, ComboText, ListScope, text_typed};
use crate::combobox::{CategoryTarget, FieldChoices, ListRow};
use crate::formatted_amount::{FormattedAmount, i18n_currency};
use crate::i18n::fl;
use crate::register_model::{
    AmountSide, RegisterRow, RowId, SplitLineRow, SplitLines, target_text,
};
use crate::theme::Theme;

/// How wide the Category field of a line is, its ⏷ button included.
const CATEGORY_WIDTH: f32 = 260.0;

/// How wide the Memo field of a line is.
const MEMO_WIDTH: f32 = 200.0;

/// How wide the Amount fields are.
const AMOUNT_WIDTH: f32 = 110.0;

/// How wide each line's Remove button is.
const REMOVE_WIDTH: f32 = 80.0;

/// How many lines show before they scroll.
const LINES_SHOWN: usize = 10;

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
        let one = |target, other_side| SplitLineRow {
            target,
            other_side,
            memo: String::new(),
            amount: whole,
        };
        let lines = if row.is_split() {
            row.split_lines().to_vec()
        } else if let Some(category) = row.category_id() {
            vec![one(CategoryTarget::Category(category), None)]
        } else if let Some(account) = row.transfer_account() {
            let kept = row.other_side().map(|other_side| other_side.transaction);
            vec![one(CategoryTarget::Transfer(account), kept)]
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
}

/// One line of the form, as typed.
#[derive(Debug, Clone)]
struct LineForm {
    /// Tells the line's fields apart from other lines', whichever are removed.
    key: usize,
    category: ComboText,
    /// The other side the line had when the dialog opened, in this account, which it
    /// keeps while its category still names the account.
    kept: Option<(AccountId, TransactionId)>,
    memo: String,
    amount: String,
}

/// Why Done didn't close the Dialog, shown below the lines until a field changes.
#[derive(Debug, Clone, PartialEq, Eq)]
enum SplitError {
    /// Line `line`, counting from 1, has text but no category.
    NoCategory { line: usize },
    /// No category or account is named `name`, which line `line` holds.
    UnknownCategory { line: usize, name: String },
    /// Line `line`'s amount, or the total when `None`, can't be read.
    Amount {
        line: Option<usize>,
        error: AmountParseError,
    },
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
            total: start.amount.map(shown).unwrap_or_default(),
            lines: Vec::new(),
            next_key: 0,
            error: None,
            focus: None,
            choices: FieldChoices::new(view, account, i18n),
        };
        for line in &start.lines {
            let account = match line.target {
                CategoryTarget::Transfer(account) => Some(account),
                CategoryTarget::Category(_) => None,
            };
            form.push_line(LineForm {
                key: 0,
                category: ComboText::new(target_text(view, line.target).unwrap_or_default()),
                kept: account.zip(line.other_side),
                memo: line.memo.clone(),
                amount: shown(line.amount),
            });
        }
        if form.lines.is_empty() {
            form.push_line(blank_line());
        }
        form.focus = form.lines.first().map(|line| category_id(line.key));
        form
    }

    /// Adds `line` at the end, giving it the next key.
    fn push_line(&mut self, line: LineForm) {
        let key = self.next_key;
        self.next_key += 1;
        self.lines.push(LineForm { key, ..line });
    }

    /// The row's lines as typed, once they add up to the total, else `None`, showing
    /// why. Lines left blank are dropped.
    pub(crate) fn done(&mut self) -> Option<(RowId, SplitLines)> {
        match self.read() {
            Ok(split) => Some((self.row, split)),
            Err(error) => {
                self.error = Some(error);
                None
            }
        }
    }

    /// Reads the lines and the total, and checks the lines add up to it.
    fn read(&self) -> Result<SplitLines, SplitError> {
        let total = self
            .read_amount(&self.total)
            .map_err(|error| SplitError::Amount { line: None, error })?;
        let mut lines = Vec::new();
        for (index, line) in self.lines.iter().enumerate() {
            let number = index + 1;
            let name = line.category.text.trim();
            if name.is_empty() && line.memo.trim().is_empty() && line.amount.trim().is_empty() {
                continue;
            }
            if name.is_empty() {
                return Err(SplitError::NoCategory { line: number });
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
                    line: Some(number),
                    error,
                })?;
            let other_side = line
                .kept
                .filter(|&(account, _)| target == CategoryTarget::Transfer(account))
                .map(|(_, other_side)| other_side);
            lines.push(SplitLineRow {
                target,
                other_side,
                memo: line.memo.trim().to_owned(),
                amount,
            });
        }
        if lines.is_empty() {
            return Err(SplitError::NoLines);
        }
        let sum = lines
            .iter()
            .try_fold(Money::zero(self.currency), |sum, line| {
                sum.checked_add(line.amount)
            })
            .map_err(|_| SplitError::Amount {
                line: None,
                error: AmountParseError::OutOfRange,
            })?;
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
        CATEGORY_WIDTH + MEMO_WIDTH + AMOUNT_WIDTH + REMOVE_WIDTH + 3.0 * gap
    }

    /// Draws the lines, scrolling past [`LINES_SHOWN`] of them, then Add line, the
    /// total and why Done was refused, in `theme`. Category fields complete typed text
    /// when `inline_completion` is on.
    pub(crate) fn show(
        &mut self,
        ui: &mut egui::Ui,
        i18n: &Localizer,
        theme: &Theme,
        inline_completion: bool,
    ) {
        let height = theme.row_height();
        ui.horizontal(|ui| {
            heading(
                ui,
                fl!(i18n, "register-column-category"),
                CATEGORY_WIDTH,
                height,
                Align::Min,
            );
            heading(
                ui,
                fl!(i18n, "register-column-memo"),
                MEMO_WIDTH,
                height,
                Align::Min,
            );
            heading(
                ui,
                fl!(i18n, "dialog-split-amount"),
                AMOUNT_WIDTH,
                height,
                Align::Max,
            );
        });
        let pitch = ui.spacing().interact_size.y.max(height) + ui.spacing().item_spacing.y;
        #[expect(
            clippy::cast_precision_loss,
            reason = "LINES_SHOWN is a handful of lines"
        )]
        let lines_height = LINES_SHOWN as f32 * pitch;
        let mut removed = None;
        ScrollArea::vertical()
            .max_height(lines_height)
            .min_scrolled_height(0.0)
            .auto_shrink([false, true])
            .show(ui, |ui| {
                for index in 0..self.lines.len() {
                    if self.line(ui, i18n, theme, index, inline_completion) {
                        removed = Some(index);
                    }
                }
            });
        if let Some(index) = removed {
            self.lines.remove(index);
            self.error = None;
        }
        if ui.button(fl!(i18n, "dialog-split-add-line")).clicked() {
            self.push_line(blank_line());
            self.focus = self.lines.last().map(|line| category_id(line.key));
            self.error = None;
        }
        ui.horizontal(|ui| {
            let gap = ui.spacing().item_spacing.x;
            let label = ui
                .allocate_ui_with_layout(
                    vec2(CATEGORY_WIDTH + MEMO_WIDTH + gap, height),
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
        if let Some(error) = &self.error {
            let text = self.error_text(i18n, error);
            ui.add(Label::new(
                RichText::new(text).color(ui.visuals().error_fg_color),
            ));
        }
    }

    /// Draws line `index`: its Category, Memo and Amount fields and its Remove button.
    /// Returns whether Remove was clicked.
    fn line(
        &mut self,
        ui: &mut egui::Ui,
        i18n: &Localizer,
        theme: &Theme,
        index: usize,
        inline_completion: bool,
    ) -> bool {
        let number = index + 1;
        ui.horizontal(|ui| {
            self.category_field(ui, i18n, theme, index, inline_completion);
            let key = self.lines[index].key;
            let memo = text_field(
                ui,
                TextEdit::singleline(&mut self.lines[index].memo)
                    .id(Id::new(("split_memo", key)))
                    .desired_width(MEMO_WIDTH),
                &fl!(i18n, "dialog-split-line-memo", line = number),
            );
            let amount_id = Id::new(("split_amount", key));
            if ui.memory(|memory| memory.has_focus(amount_id)) {
                amount_field::refuse_characters(ui.ctx(), &self.reader);
            }
            let amount = amount_text_field(
                ui,
                &mut self.lines[index].amount,
                amount_id,
                &fl!(i18n, "dialog-split-line-amount", line = number),
            );
            if memo.changed() || amount.changed() {
                self.error = None;
            }
            let remove = ui.add_sized(
                vec2(REMOVE_WIDTH, ui.spacing().interact_size.y),
                Button::new(fl!(i18n, "dialog-split-remove-line")),
            );
            let label = fl!(i18n, "dialog-split-remove-line-a11y", line = number);
            ui.ctx()
                .accesskit_node_builder(remove.id, |node| node.set_label(label));
            remove.clicked()
        })
        .inner
    }

    /// Draws line `index`'s Category field with its ⏷ button and list, taking the keys
    /// its list answers first.
    fn category_field(
        &mut self,
        ui: &mut egui::Ui,
        i18n: &Localizer,
        theme: &Theme,
        index: usize,
        inline_completion: bool,
    ) {
        let height = ui.spacing().interact_size.y;
        let gap = ui.spacing().item_spacing.x;
        let choices = &self.choices;
        let list = |text: &str| choices.categories().list(text);
        let line = &mut self.lines[index];
        let id = category_id(line.key);
        if ui.memory(|memory| memory.has_focus(id)) {
            list_keys(ui.ctx(), &mut line.category, list);
        }
        let response = ui.add_sized(
            vec2(CATEGORY_WIDTH - height - gap, height),
            TextEdit::singleline(&mut line.category.text)
                .id(id)
                // Esc and the arrows reach `list_keys` before egui takes the field's
                // focus or moves it on them.
                .event_filter(EventFilter {
                    escape: true,
                    vertical_arrows: true,
                    ..Default::default()
                }),
        );
        let number: usize = index + 1;
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
        let field = response.rect;
        if combo_field::list_button(ui, i18n, button, id).clicked() {
            if line.category.lists_all() {
                line.category.close_list();
            } else {
                line.category.open_list(ListScope::All, list);
            }
            ui.memory_mut(|memory| memory.request_focus(id));
        } else if response.lost_focus() {
            take_highlighted(&mut line.category, list);
        }
        let rows = line.category.rows(list);
        let pressed =
            line.category
                .show_list(ui, i18n, id.with("list"), &rows, field, theme.row_height());
        if let Some(ListRow::Choice(text)) = pressed {
            text.clone_into(&mut line.category.text);
            line.category.close_list();
        }
    }

    /// Why Done didn't close the Dialog, in words.
    fn error_text(&self, i18n: &Localizer, error: &SplitError) -> String {
        let currency = self.currency;
        let format = |money: Money| {
            FormattedAmount::new(i18n, money, AmountStyle::Bare)
                .text()
                .to_owned()
        };
        match *error {
            SplitError::NoCategory { line } => {
                fl!(i18n, "dialog-split-no-category", line = line)
            }
            SplitError::UnknownCategory { line, ref name } => fl!(
                i18n,
                "dialog-split-unknown-category",
                line = line,
                name = name.clone()
            ),
            SplitError::Amount { line, error } => {
                let error = amount_field::error_text(i18n, error, &self.reader, currency);
                if let Some(line) = line {
                    fl!(i18n, "dialog-split-line-error", line = line, error = error)
                } else {
                    fl!(i18n, "dialog-split-total-error", error = error)
                }
            }
            SplitError::NoLines => fl!(i18n, "dialog-split-no-lines"),
            SplitError::Mismatch { lines, total } => {
                let difference = total
                    .checked_sub(lines)
                    .map(|difference| format(AmountSide::Deposit.signed(difference)))
                    .unwrap_or_default();
                fl!(
                    i18n,
                    "dialog-split-mismatch",
                    lines = format(lines),
                    total = format(total),
                    difference = difference
                )
            }
        }
    }
}

/// A line with every field empty.
fn blank_line() -> LineForm {
    LineForm {
        key: 0,
        category: ComboText::new(String::new()),
        kept: None,
        memo: String::new(),
        amount: String::new(),
    }
}

/// The id of the Category field of the line with `key`.
fn category_id(key: usize) -> Id {
    Id::new(("split_category", key))
}

/// A column heading `width` wide and `height` high, its text at the left edge, or at
/// the right edge for `Align::Max`.
fn heading(ui: &mut egui::Ui, title: String, width: f32, height: f32, align: Align) {
    let layout = match align {
        Align::Max => Layout::right_to_left(Align::Center),
        Align::Min | Align::Center => Layout::left_to_right(Align::Center),
    };
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
            .desired_width(AMOUNT_WIDTH)
            .horizontal_align(Align::Max),
        label,
    )
}

/// Acts on this frame's keys for the focused Category field `combo`, whose list rows
/// `list` gives: Alt+↓ lists every entry, ↓ and ↑ move the highlight, and while the
/// list is open Esc closes it and Enter takes its highlighted row. The keys it acts on
/// go no further, so Esc and Enter close the Dialog only while the list is closed.
fn list_keys<'a>(
    ctx: &egui::Context,
    combo: &mut ComboText,
    list: impl Fn(&str) -> Vec<ListRow<'a>>,
) {
    let rows = combo.rows(&list).len();
    let open = rows > 0;
    let (alt_down, down, up, escape, enter) = ctx.input_mut(|input| {
        // Alt+↓ first, since a bare ↓ also matches it.
        let alt_down = input.consume_key(Modifiers::ALT, Key::ArrowDown);
        (
            alt_down,
            input.consume_key(Modifiers::NONE, Key::ArrowDown),
            input.consume_key(Modifiers::NONE, Key::ArrowUp),
            open && input.consume_key(Modifiers::NONE, Key::Escape),
            open && input.consume_key(Modifiers::NONE, Key::Enter),
        )
    });
    if alt_down {
        combo.open_list(ListScope::All, &list);
    } else if down && open {
        combo.highlight_next(rows);
    } else if down {
        combo.open_list(ListScope::Matching, &list);
    } else if up {
        combo.highlight_previous();
    }
    if escape {
        combo.close_list();
        combo.drop_completion();
    } else if enter {
        take_highlighted(combo, &list);
    }
}

/// Takes the highlighted row of `combo`'s list, if it is open with one highlighted,
/// and closes the list.
fn take_highlighted<'a>(combo: &mut ComboText, list: impl FnOnce(&str) -> Vec<ListRow<'a>>) {
    if let Some(ListRow::Choice(text)) = combo.highlighted(list) {
        text.clone_into(&mut combo.text);
    }
    combo.close_list();
}
