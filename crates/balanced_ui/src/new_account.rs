// SPDX-License-Identifier: GPL-3.0-or-later WITH AdditionRef-Balanced-Plugin-Exception

//! The New Account wizard: a light Dialog that steps through an account's type, its
//! name and its opening balance, then reviews them before creating the account.
//!
//! Enter, or Next, moves to the next step once the step's input is valid. Invalid
//! input keeps the step, with the reason below its field. Finish sends the Command
//! that creates the account, and the wizard stays open until it is applied, showing
//! the reason inline if the ledger refuses it.

use balanced_core::{AccountKind, CurrencyCode, DomainCommand, DomainError, Money};
use balanced_i18n::{AmountParseError, AmountReader, AmountStyle, Localizer};
use eframe::egui::{self, Id, Key, Label, Modifiers, Rect, RichText, Sense, TextEdit, vec2};
use icu_calendar::{Date, Iso};

use crate::accounts_model::kind_label;
use crate::amount_field::{self, Preview};
use crate::command::{AppCommand, CommandSender, StatusMessage};
use crate::date_field::{self, DateEdit};
use crate::dialog::{DialogOutcome, LightDialog, OpenDialog};
use crate::formatted_amount::{FormattedAmount, i18n_currency};
use crate::i18n::fl;
use crate::keypad::{self, Keypad};
use crate::theme::Theme;

/// The wizard's steps, in order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Step {
    Type,
    Details,
    OpeningBalance,
    Review,
}

impl Step {
    /// The step before this one, or `None` on the first.
    fn previous(self) -> Option<Self> {
        match self {
            Self::Type => None,
            Self::Details => Some(Self::Type),
            Self::OpeningBalance => Some(Self::Details),
            Self::Review => Some(Self::OpeningBalance),
        }
    }

    /// The step's heading, which says how far through the wizard it is.
    fn heading(self, i18n: &Localizer) -> String {
        match self {
            Self::Type => fl!(i18n, "dialog-new-account-step-type"),
            Self::Details => fl!(i18n, "dialog-new-account-step-details"),
            Self::OpeningBalance => fl!(i18n, "dialog-new-account-step-opening-balance"),
            Self::Review => fl!(i18n, "dialog-new-account-step-review"),
        }
    }

    /// Whether the step has a field that takes keyboard focus when the step is shown.
    /// On the other steps, the default button takes it.
    fn has_field(self) -> bool {
        match self {
            Self::Details | Self::OpeningBalance => true,
            Self::Type | Self::Review => false,
        }
    }
}

/// The opening balance as the Review step shows it and Finish creates it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct OpeningBalance {
    amount: Money,
    date: Date<Iso>,
}

/// The keypad under the opening balance, and what follows its closing.
#[derive(Debug, Clone, PartialEq, Eq)]
enum BalanceKeypad {
    Closed,
    Open(Keypad),
    /// A pointer press outside closed it and is still held. Its click closes nothing
    /// else, not even the Dialog when it lands on the dimmed window.
    Closing,
    /// It handed its result back, and the date field takes keyboard focus on the next
    /// frame. A widget given focus in the frame of a click elsewhere loses it again,
    /// so it waits a frame.
    HandedBack,
}

impl BalanceKeypad {
    /// Whether a click on the dimmed window goes to the keypad, not the Dialog.
    fn holds_clicks(&self) -> bool {
        match self {
            Self::Open(_) | Self::Closing => true,
            Self::Closed | Self::HandedBack => false,
        }
    }
}

/// Where the create Command is, once Finish has sent it.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Submission {
    /// Finish hasn't sent it, or the user went back since.
    NotSent,
    /// Finish sent it, and the wizard waits for it to be applied.
    Sent,
    /// The ledger refused it, for the reason the Review step shows.
    Refused(DomainError),
}

/// The wizard's form: what each step holds so far, and where it is.
#[derive(Debug, Clone)]
pub(crate) struct NewAccountForm {
    step: Step,
    kind: AccountKind,
    name: String,
    /// Whether Next was refused because the name is blank, shown until it changes.
    blank_name: bool,
    /// The typed opening balance. Left empty, it is zero.
    balance: String,
    /// Why Next refused the typed opening balance, shown until it changes.
    balance_error: Option<AmountParseError>,
    /// The keypad under the opening balance.
    keypad: BalanceKeypad,
    /// Reads the opening balance in the formatting locale the wizard opened in.
    reader: AmountReader,
    /// The ledger's currency, which the new account holds.
    currency: CurrencyCode,
    /// The opening balance's date, starting on the day the wizard opened.
    date: DateEdit,
    /// The day the wizard opened, the date field's saved date.
    opened_on: Date<Iso>,
    /// The opening balance, read when the Opening balance step is left.
    reviewed: Option<OpeningBalance>,
    /// Whether the step's field takes keyboard focus on the next frame.
    focus_field: bool,
    submission: Submission,
}

impl NewAccountForm {
    /// The wizard on its first step, for an account in `currency`, reading amounts in
    /// `i18n`'s formatting locale, with the opening balance dated `today`.
    pub(crate) fn new(i18n: &Localizer, currency: CurrencyCode, today: Date<Iso>) -> Self {
        Self {
            step: Step::Type,
            kind: AccountKind::Checking,
            name: String::new(),
            blank_name: false,
            balance: String::new(),
            balance_error: None,
            keypad: BalanceKeypad::Closed,
            reader: i18n.amount_reader(i18n_currency(currency)),
            currency,
            date: DateEdit::new(Some(today)),
            opened_on: today,
            reviewed: None,
            focus_field: false,
            submission: Submission::NotSent,
        }
    }

    /// Notes that the ledger refused the create Command with `error`, which the
    /// Review step shows. Finish can send it again.
    pub(crate) fn refused(&mut self, error: DomainError) {
        self.submission = Submission::Refused(error);
    }

    /// Moves on from the current step if its input is valid, or keeps it with the
    /// reason shown. On the Review step, sends the Command that creates the account.
    fn next(&mut self, i18n: &Localizer, commands: &CommandSender) {
        match self.step {
            Step::Type => self.go_to(Step::Details),
            Step::Details => {
                if self.name.trim().is_empty() {
                    self.blank_name = true;
                    self.focus_field = true;
                } else {
                    self.go_to(Step::OpeningBalance);
                }
            }
            Step::OpeningBalance => {
                let balance = self.read_balance();
                let date = match self.date.commit(i18n) {
                    date_field::Commit::Unchanged => Some(self.opened_on),
                    date_field::Commit::Changed(date) => Some(date),
                    date_field::Commit::Rejected => None,
                };
                match (balance, date) {
                    (Ok(amount), Some(date)) => {
                        self.reviewed = Some(OpeningBalance { amount, date });
                        self.go_to(Step::Review);
                    }
                    (Err(error), _) => {
                        self.balance_error = Some(error);
                        self.focus_field = true;
                    }
                    (Ok(_), None) => {}
                }
            }
            Step::Review => {
                if self.submission == Submission::Sent {
                    return;
                }
                let Some(OpeningBalance { amount, date }) = self.reviewed else {
                    return;
                };
                self.submission = Submission::Sent;
                commands.send(AppCommand::SubmitForm(DomainCommand::CreateAccount {
                    name: self.name.trim().to_owned(),
                    kind: self.kind,
                    opened: date,
                    opening_balance: amount,
                }));
            }
        }
    }

    /// The typed opening balance, or zero when nothing is typed.
    fn read_balance(&self) -> Result<Money, AmountParseError> {
        let text = self.balance.trim();
        if text.is_empty() {
            return Ok(Money::zero(self.currency));
        }
        let amount = self.reader.read(text)?.amount();
        Ok(Money::new(amount, self.currency)
            .expect("the reader gives amounts at the currency's decimal places"))
    }

    fn go_to(&mut self, step: Step) {
        self.step = step;
        self.keypad = BalanceKeypad::Closed;
        self.focus_field = step.has_field();
        self.submission = Submission::NotSent;
    }

    fn body(&mut self, ui: &mut egui::Ui, i18n: &Localizer, theme: &Theme, today: Date<Iso>) {
        // Every step takes the height of the tallest, the Type step's heading and list
        // of kinds, so the buttons stay put from step to step.
        let rows = 1 + AccountKind::ALL.len();
        #[expect(
            clippy::cast_precision_loss,
            reason = "a handful of rows is exact in f32"
        )]
        let height = rows as f32 * (ui.spacing().interact_size.y + ui.spacing().item_spacing.y);
        ui.set_min_height(height);
        ui.add(Label::new(RichText::new(self.step.heading(i18n)).strong()));
        ui.add_space(ui.spacing().item_spacing.y);
        match self.step {
            Step::Type => self.type_step(ui, i18n),
            Step::Details => self.details_step(ui, i18n),
            Step::OpeningBalance => self.opening_balance_step(ui, i18n, theme, today),
            Step::Review => self.review_step(ui, i18n),
        }
        self.focus_field = false;
    }

    fn type_step(&mut self, ui: &mut egui::Ui, i18n: &Localizer) {
        for kind in AccountKind::ALL {
            ui.radio_value(&mut self.kind, kind, kind_label(i18n, kind));
        }
    }

    fn details_step(&mut self, ui: &mut egui::Ui, i18n: &Localizer) {
        let label = ui.label(fl!(i18n, "dialog-new-account-name"));
        let field = ui.add(
            TextEdit::singleline(&mut self.name)
                .id(Id::new("new_account_name"))
                .desired_width(f32::INFINITY),
        );
        let field = field.labelled_by(label.id);
        if self.focus_field {
            field.request_focus();
        }
        if field.changed() {
            self.blank_name = false;
        }
        if self.blank_name {
            let blank = StatusMessage::Rejected(DomainError::BlankAccountName);
            refusal(ui, blank.text(i18n));
        }
    }

    fn opening_balance_step(
        &mut self,
        ui: &mut egui::Ui,
        i18n: &Localizer,
        theme: &Theme,
        today: Date<Iso>,
    ) {
        let id = Id::new("new_account_balance");
        let date_id = Id::new("new_account_date");
        if self.keypad == BalanceKeypad::HandedBack {
            self.keypad = BalanceKeypad::Closed;
            ui.memory_mut(|memory| memory.request_focus(date_id));
        }
        let label = ui.label(fl!(i18n, "dialog-new-account-balance"));
        if ui.memory(|memory| memory.has_focus(id)) {
            amount_field::refuse_characters(ui.ctx(), &self.reader);
            if ui.input_mut(|input| input.consume_key(Modifiers::ALT, Key::ArrowDown)) {
                self.keypad = BalanceKeypad::Open(Keypad::new(&self.balance));
            }
        }
        let (row, _) = ui.allocate_exact_size(
            vec2(ui.available_width(), theme.row_height()),
            Sense::hover(),
        );
        let button = Rect::from_x_y_ranges(row.right() - row.height()..=row.right(), row.y_range());
        let zero = FormattedAmount::new(i18n, Money::zero(self.currency), AmountStyle::Bare);
        let field = ui.put(
            Rect::from_x_y_ranges(row.left()..=button.left(), row.y_range()),
            TextEdit::singleline(&mut self.balance)
                .id(id)
                .hint_text(zero.text())
                .desired_width(f32::INFINITY),
        );
        let field = field.labelled_by(label.id);
        if self.focus_field {
            field.request_focus();
        }
        if field.changed() {
            self.balance_error = None;
        }
        if keypad::button(ui, i18n, id, button).clicked() {
            self.keypad = match self.keypad {
                BalanceKeypad::Open(_) => {
                    keypad::focus_at_end(ui.ctx(), id, &self.balance);
                    BalanceKeypad::Closed
                }
                BalanceKeypad::Closed | BalanceKeypad::Closing | BalanceKeypad::HandedBack => {
                    BalanceKeypad::Open(Keypad::new(&self.balance))
                }
            };
        }
        self.show_keypad(ui.ctx(), i18n, theme, id, row, button);
        if let Some(reason) = self.balance_error {
            refusal(
                ui,
                amount_field::error_text(i18n, reason, &self.reader, self.currency),
            );
        } else if let Some(preview) = Preview::of(i18n, &self.reader, self.currency, &self.balance)
        {
            ui.label(preview.text(i18n));
        }
        let label = ui.label(fl!(i18n, "dialog-new-account-date"));
        let (rect, _) = ui.allocate_exact_size(
            vec2(ui.available_width(), theme.row_height()),
            Sense::hover(),
        );
        self.date
            .show(ui, i18n, theme, today, date_id, rect)
            .labelled_by(label.id);
    }

    /// Draws the opening balance's keypad, while it is open, under the field with `id`
    /// at `row` with its 🖩 button at `button`. A result it hands back goes into the
    /// field, and focus moves on to the date on the next frame; closing it gives the
    /// field back focus.
    fn show_keypad(
        &mut self,
        ctx: &egui::Context,
        i18n: &Localizer,
        theme: &Theme,
        id: Id,
        row: Rect,
        button: Rect,
    ) {
        let BalanceKeypad::Open(keypad) = &mut self.keypad else {
            return;
        };
        match keypad.show(
            ctx,
            i18n,
            theme,
            &self.reader,
            self.currency,
            id,
            row,
            button,
        ) {
            keypad::Outcome::Open => {}
            keypad::Outcome::Cancelled => {
                self.keypad = if ctx.input(|input| input.pointer.any_pressed()) {
                    BalanceKeypad::Closing
                } else {
                    BalanceKeypad::Closed
                };
                keypad::focus_at_end(ctx, id, &self.balance);
            }
            keypad::Outcome::Taken(text) => {
                self.keypad = BalanceKeypad::HandedBack;
                self.balance = text;
                self.balance_error = None;
                ctx.request_repaint();
            }
        }
    }

    fn review_step(&self, ui: &mut egui::Ui, i18n: &Localizer) {
        let Some(OpeningBalance { amount, date }) = self.reviewed else {
            return;
        };
        let balance = FormattedAmount::new(i18n, amount, AmountStyle::Symbol);
        ui.label(fl!(
            i18n,
            "dialog-new-account-review-type",
            kind = kind_label(i18n, self.kind)
        ));
        ui.label(fl!(
            i18n,
            "dialog-new-account-review-name",
            name = self.name.trim()
        ));
        ui.label(fl!(
            i18n,
            "dialog-new-account-review-balance",
            amount = balance.text(),
            date = i18n.format_date(date)
        ));
        if let Submission::Refused(error) = &self.submission {
            refusal(ui, StatusMessage::Rejected(error.clone()).text(i18n));
        }
    }
}

/// What became of the wizard this frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WizardOutcome {
    /// It stays open, perhaps waiting on its create Command.
    Open,
    /// The user cancelled it.
    Cancelled,
}

/// Draws the wizard `dialog`, reading today's date as `today`.
pub(crate) fn show(
    ctx: &egui::Context,
    i18n: &Localizer,
    theme: &Theme,
    commands: &CommandSender,
    dialog: &mut OpenDialog<NewAccountForm>,
    today: Date<Iso>,
) -> WizardOutcome {
    let step = dialog.form.step;
    let accept = match step {
        Step::Review => fl!(i18n, "dialog-new-account-finish"),
        Step::Type | Step::Details | Step::OpeningBalance => {
            fl!(i18n, "dialog-new-account-next")
        }
    };
    let mut wizard = LightDialog::new(
        Id::new("new_account_dialog"),
        accept,
        fl!(i18n, "dialog-cancel"),
        theme,
    );
    if step.previous().is_some() {
        wizard = wizard.with_back(fl!(i18n, "dialog-new-account-back"));
    }
    wizard = wizard.with_body_popup(dialog.form.keypad.holds_clicks());
    let outcome = wizard.show(ctx, dialog, |ui, form| form.body(ui, i18n, theme, today));
    let form = &mut dialog.form;
    if form.keypad == BalanceKeypad::Closing && !ctx.input(|input| input.pointer.any_down()) {
        form.keypad = BalanceKeypad::Closed;
    }
    match outcome {
        DialogOutcome::Open => return WizardOutcome::Open,
        DialogOutcome::Cancelled => return WizardOutcome::Cancelled,
        DialogOutcome::Accepted => form.next(i18n, commands),
        DialogOutcome::Back => {
            if let Some(previous) = step.previous() {
                form.go_to(previous);
            }
        }
        DialogOutcome::Discarded => unreachable!("the wizard has no Discard button"),
    }
    if form.step != step && !form.step.has_field() {
        dialog.focus_default_button();
    }
    WizardOutcome::Open
}

/// Shows why the step's input, or Finish, was refused, below what was refused.
fn refusal(ui: &mut egui::Ui, text: String) {
    let colour = ui.visuals().error_fg_color;
    ui.label(RichText::new(text).color(colour));
}
