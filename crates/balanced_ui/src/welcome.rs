//! The welcome screen: what the App shows while no data file is open, at first run or
//! when the data file opened last was refused.

use balanced_i18n::Localizer;
use eframe::egui::{self, CentralPanel, Id};

use crate::command::{AppCommand, CommandSender};
use crate::data_file::{DataFilePicker, DataFileRefusal};
use crate::dialog::{DialogOutcome, LightDialog, OpenDialog};
use crate::i18n::fl;
use crate::theme::Theme;

/// An empty window under the welcome Dialog, which offers to create or open a data
/// file. A refusal shows first, and the welcome Dialog once it is closed.
#[derive(Debug)]
pub(crate) struct Welcome {
    /// The welcome Dialog's choice of picker.
    dialog: OpenDialog<DataFilePicker>,
    /// The Dialog saying why a data file was refused, while it is open.
    refusal: Option<OpenDialog<DataFileRefusal>>,
}

impl Welcome {
    /// The welcome screen, opening now in `ctx`, with Create chosen.
    pub(crate) fn new(ctx: &egui::Context) -> Self {
        Self {
            dialog: OpenDialog::new(ctx, DataFilePicker::New),
            refusal: None,
        }
    }

    /// Shows why a data file was refused, before the welcome Dialog comes back.
    pub(crate) fn refuse(&mut self, ctx: &egui::Context, refusal: DataFileRefusal) {
        self.refusal = Some(OpenDialog::new(ctx, refusal));
        self.dialog.focus_default_button();
    }

    /// Draws the empty window and the Dialog on top. Continue shows the chosen picker,
    /// which leaves the welcome Dialog open, so cancelling the picker comes back to it.
    /// Quit, or closing the Dialog, quits.
    pub(crate) fn show(
        &mut self,
        ui: &mut egui::Ui,
        i18n: &Localizer,
        theme: &Theme,
        commands: &CommandSender,
    ) {
        CentralPanel::default().show(ui, |_| {});
        let ctx = ui.ctx();
        if let Some(refusal) = &mut self.refusal {
            if DataFileRefusal::show_dialog(ctx, i18n, theme, refusal) {
                self.refusal = None;
            }
            return;
        }
        let outcome = LightDialog::new(
            Id::new("welcome_dialog"),
            fl!(i18n, "dialog-welcome-continue"),
            fl!(i18n, "dialog-welcome-quit"),
            theme,
        )
        .show(ctx, &mut self.dialog, |ui, picker| {
            ui.label(fl!(i18n, "dialog-welcome-message"));
            ui.radio_value(
                picker,
                DataFilePicker::New,
                fl!(i18n, "dialog-welcome-create"),
            );
            ui.radio_value(
                picker,
                DataFilePicker::Open,
                fl!(i18n, "dialog-welcome-open"),
            );
        });
        match outcome {
            DialogOutcome::Open => {}
            DialogOutcome::Accepted => {
                commands.send(AppCommand::ChooseDataFile(self.dialog.form));
            }
            DialogOutcome::Cancelled => commands.send(AppCommand::Quit),
            DialogOutcome::Back => unreachable!("the welcome Dialog has no Back button"),
        }
    }
}
