//! The New Account wizard: a light Dialog that steps through an account's type, its
//! name and its opening balance, then reviews them before creating the account.
//!
//! Enter, or Next, moves to the next step once the step's input is valid. Invalid
//! input keeps the step, with the reason below its field. Finish sends the Command
//! that creates the account, and the wizard stays open until it is applied, showing
//! the reason inline if the ledger refuses it.

use balanced_core::{AccountKind, CurrencyCode, DomainCommand, DomainError, Money};
use balanced_i18n::{AmountParseError, AmountReader, AmountStyle, Localizer};
use eframe::egui::{self, Id, Label, RichText, Sense, TextEdit, vec2};
use icu_calendar::{Date, Iso};

use crate::amount_field::{self, Preview};
use crate::command::{AppCommand, CommandSender, StatusMessage};
use crate::date_field::{self, DateEdit};
use crate::dialog::{DialogOutcome, LightDialog, OpenDialog};
use crate::formatted_amount::{FormattedAmount, i18n_currency};
use crate::i18n::fl;
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
    /// Reads the opening balance in the formatting locale the wizard opened in.
    reader: AmountReader,
    /// The ledger's currency, which the new account holds.
    currency: CurrencyCode,
    /// The opening balance's date, starting on the day the wizard opened.
    date: DateEdit,
    /// The day the wizard opened, the date field's saved date.
    opened_on: Date<Iso>,
    /// The opening balance and its date, read when the Opening balance step is left.
    read: Option<(Money, Date<Iso>)>,
    /// Whether the step's field takes keyboard focus on the next frame.
    focus_field: bool,
    /// Whether Finish has sent the create Command, which the wizard waits on.
    submitted: bool,
    /// Why the ledger refused the create Command.
    rejected: Option<DomainError>,
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
            reader: i18n.amount_reader(i18n_currency(currency)),
            currency,
            date: DateEdit::new(today),
            opened_on: today,
            read: None,
            focus_field: false,
            submitted: false,
            rejected: None,
        }
    }

    /// Notes that the ledger refused the create Command with `error`, which the
    /// Review step shows. Finish can send it again.
    pub(crate) fn refused(&mut self, error: DomainError) {
        self.submitted = false;
        self.rejected = Some(error);
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
                    (Ok(balance), Some(date)) => {
                        self.read = Some((balance, date));
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
                if self.submitted {
                    return;
                }
                let Some((opening_balance, opened)) = self.read else {
                    return;
                };
                self.submitted = true;
                self.rejected = None;
                commands.send(AppCommand::SubmitForm(DomainCommand::CreateAccount {
                    name: self.name.trim().to_owned(),
                    kind: self.kind,
                    opened,
                    opening_balance,
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
        self.focus_field = step.has_field();
        self.rejected = None;
    }

    fn body(&mut self, ui: &mut egui::Ui, i18n: &Localizer, theme: &Theme, today: Date<Iso>) {
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
            error(ui, fl!(i18n, "dialog-new-account-name-blank"));
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
        let label = ui.label(fl!(i18n, "dialog-new-account-balance"));
        if ui.memory(|memory| memory.has_focus(id)) {
            amount_field::refuse_characters(ui.ctx(), &self.reader);
        }
        let zero = FormattedAmount::new(i18n, Money::zero(self.currency), AmountStyle::Bare);
        let field = ui.add(
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
        if let Some(reason) = self.balance_error {
            error(
                ui,
                amount_field::error_text(i18n, reason, &self.reader, self.currency),
            );
        } else if let Some(preview) = Preview::of(i18n, &self.reader, self.currency, &self.balance)
        {
            ui.label(preview.text(i18n));
        }
        ui.label(fl!(i18n, "dialog-new-account-date"));
        let (rect, _) = ui.allocate_exact_size(
            vec2(ui.available_width(), theme.row_height()),
            Sense::hover(),
        );
        self.date
            .show(ui, i18n, theme, today, Id::new("new_account_date"), rect);
    }

    fn review_step(&self, ui: &mut egui::Ui, i18n: &Localizer) {
        let Some((balance, date)) = self.read else {
            return;
        };
        let balance = FormattedAmount::new(i18n, balance, AmountStyle::Symbol);
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
        if let Some(rejected) = &self.rejected {
            error(ui, StatusMessage::Rejected(rejected.clone()).text(i18n));
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
    );
    if step.previous().is_some() {
        wizard = wizard.with_back(fl!(i18n, "dialog-new-account-back"));
    }
    let outcome = wizard.show(ctx, dialog, |ui, form| form.body(ui, i18n, theme, today));
    let form = &mut dialog.form;
    match outcome {
        DialogOutcome::Open => return WizardOutcome::Open,
        DialogOutcome::Cancelled => return WizardOutcome::Cancelled,
        DialogOutcome::Accepted => form.next(i18n, commands),
        DialogOutcome::Back => {
            if let Some(previous) = step.previous() {
                form.go_to(previous);
            }
        }
    }
    if form.step != step && !form.step.has_field() {
        dialog.focus_default_button();
    }
    WizardOutcome::Open
}

/// The name of `kind` in the Type step's list and on the Review step.
fn kind_label(i18n: &Localizer, kind: AccountKind) -> String {
    match kind {
        AccountKind::Checking => fl!(i18n, "dialog-new-account-kind-checking"),
        AccountKind::Savings => fl!(i18n, "dialog-new-account-kind-savings"),
        AccountKind::CreditCard => fl!(i18n, "dialog-new-account-kind-credit-card"),
        AccountKind::Investment => fl!(i18n, "dialog-new-account-kind-investment"),
        AccountKind::Retirement => fl!(i18n, "dialog-new-account-kind-retirement"),
        AccountKind::Asset => fl!(i18n, "dialog-new-account-kind-asset"),
        AccountKind::Liability => fl!(i18n, "dialog-new-account-kind-liability"),
    }
}

/// Shows why the step's input was refused, below its field.
fn error(ui: &mut egui::Ui, text: String) {
    let colour = ui.visuals().error_fg_color;
    ui.label(RichText::new(text).color(colour));
}
