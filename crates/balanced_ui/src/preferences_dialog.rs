//! The Preferences Dialog's form: a draft of the Preferences, edited one group at a
//! time. The App shows the draft while the Dialog is open.

use balanced_i18n::Localizer;
use eframe::egui::accesskit::Role;
use eframe::egui::{self, ComboBox};

use crate::i18n::fl;
use crate::preferences::{FontFace, Gridlines, NetWorthPlacement, Preferences, Scheme};
use crate::route::OpenIn;

/// What the Preferences Dialog holds while it is open.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PreferencesForm {
    draft: Preferences,
    group: Group,
}

/// A group of Preferences, listed down the left of the Dialog.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Group {
    Display,
    Register,
    AccountBar,
    Accounts,
}

impl Group {
    const ALL: [Self; 4] = [
        Self::Display,
        Self::Register,
        Self::AccountBar,
        Self::Accounts,
    ];

    fn label(self, i18n: &Localizer) -> String {
        match self {
            Self::Display => fl!(i18n, "dialog-preferences-group-display"),
            Self::Register => fl!(i18n, "dialog-preferences-group-register"),
            Self::AccountBar => fl!(i18n, "dialog-preferences-group-account-bar"),
            Self::Accounts => fl!(i18n, "dialog-preferences-group-accounts"),
        }
    }
}

impl PreferencesForm {
    /// The form starting from `preferences`, on its first group.
    pub(crate) fn new(preferences: Preferences) -> Self {
        Self {
            draft: preferences,
            group: Group::Display,
        }
    }

    /// The Preferences as the form has them now.
    pub(crate) fn draft(&self) -> &Preferences {
        &self.draft
    }

    /// Draws the group list, which AccessKit reads as a tab list, and beside it the
    /// Preferences of the chosen group.
    pub(crate) fn show(&mut self, ui: &mut egui::Ui, i18n: &Localizer) {
        let before = self.draft.clone();
        ui.horizontal_top(|ui| {
            ui.vertical(|ui| {
                ui.ctx()
                    .accesskit_node_builder(ui.unique_id(), |node| node.set_role(Role::TabList));
                for group in Group::ALL {
                    let chosen = self.group == group;
                    let response = ui.selectable_label(chosen, group.label(i18n));
                    ui.ctx().accesskit_node_builder(response.id, |node| {
                        node.set_role(Role::Tab);
                        node.set_selected(chosen);
                    });
                    if response.clicked() {
                        self.group = group;
                    }
                }
            });
            ui.separator();
            ui.vertical(|ui| match self.group {
                Group::Display => self.display(ui, i18n),
                Group::Register => self.register(ui, i18n),
                Group::AccountBar => self.account_bar(ui, i18n),
                Group::Accounts => self.accounts(ui, i18n),
            });
        });
        if self.draft != before {
            // The App shows the changed draft in the next frame.
            ui.ctx().request_repaint();
        }
    }

    /// The Display group: the look of every window.
    fn display(&mut self, ui: &mut egui::Ui, i18n: &Localizer) {
        let draft = &mut self.draft;
        ui.horizontal(|ui| {
            ui.label(fl!(i18n, "dialog-preferences-scheme"));
            ui.radio_value(
                &mut draft.scheme,
                Scheme::Light,
                fl!(i18n, "dialog-preferences-scheme-light"),
            );
            ui.radio_value(
                &mut draft.scheme,
                Scheme::Dark,
                fl!(i18n, "dialog-preferences-scheme-dark"),
            );
        });
        ComboBox::from_label(fl!(i18n, "dialog-preferences-font-face"))
            .selected_text(font_face_label(draft.font_face, i18n))
            .show_ui(ui, |ui| {
                for face in FontFace::ALL {
                    ui.selectable_value(&mut draft.font_face, face, font_face_label(face, i18n));
                }
            });
        ui.checkbox(
            &mut draft.dense_rows,
            fl!(i18n, "dialog-preferences-dense-rows"),
        );
        ui.checkbox(
            &mut draft.monospace_money,
            fl!(i18n, "dialog-preferences-monospace-money"),
        );
        ui.checkbox(
            &mut draft.coloured_negatives,
            fl!(i18n, "dialog-preferences-coloured-negatives"),
        );
    }

    /// The Register group: how every Register's rows read.
    fn register(&mut self, ui: &mut egui::Ui, i18n: &Localizer) {
        let draft = &mut self.draft;
        ui.checkbox(
            &mut draft.alternating_rows,
            fl!(i18n, "dialog-preferences-alternating-rows"),
        );
        ui.label(fl!(i18n, "dialog-preferences-gridlines"));
        ui.horizontal(|ui| {
            for gridlines in Gridlines::ALL {
                ui.radio_value(
                    &mut draft.gridlines,
                    gridlines,
                    gridlines_label(gridlines, i18n),
                );
            }
        });
    }

    /// The Account bar group: what the Account bar shows.
    fn account_bar(&mut self, ui: &mut egui::Ui, i18n: &Localizer) {
        let draft = &mut self.draft;
        ui.checkbox(
            &mut draft.group_totals,
            fl!(i18n, "dialog-preferences-group-totals"),
        );
        ui.label(fl!(i18n, "dialog-preferences-net-worth"));
        ui.radio_value(
            &mut draft.net_worth_placement,
            NetWorthPlacement::AccountBar,
            fl!(i18n, "dialog-preferences-net-worth-account-bar"),
        );
        ui.radio_value(
            &mut draft.net_worth_placement,
            NetWorthPlacement::StatusBar,
            fl!(i18n, "dialog-preferences-net-worth-status-bar"),
        );
    }

    /// The Accounts group: where an account's Register opens.
    fn accounts(&mut self, ui: &mut egui::Ui, i18n: &Localizer) {
        let draft = &mut self.draft;
        ui.label(fl!(i18n, "dialog-preferences-open-account-in"));
        ui.radio_value(
            &mut draft.open_account_in,
            OpenIn::CurrentRegister,
            fl!(i18n, "dialog-preferences-open-account-in-current"),
        );
        ui.radio_value(
            &mut draft.open_account_in,
            OpenIn::NewTab,
            fl!(i18n, "dialog-preferences-open-account-in-new-tab"),
        );
    }
}

/// The name of `gridlines`, as the Register group shows it.
fn gridlines_label(gridlines: Gridlines, i18n: &Localizer) -> String {
    match gridlines {
        Gridlines::Off => fl!(i18n, "dialog-preferences-gridlines-off"),
        Gridlines::Horizontal => fl!(i18n, "dialog-preferences-gridlines-horizontal"),
        Gridlines::Vertical => fl!(i18n, "dialog-preferences-gridlines-vertical"),
        Gridlines::Both => fl!(i18n, "dialog-preferences-gridlines-both"),
    }
}

/// The name of `face`, as the font list shows it.
fn font_face_label(face: FontFace, i18n: &Localizer) -> String {
    match face {
        FontFace::Egui => fl!(i18n, "dialog-preferences-font-face-egui"),
        FontFace::NotoSans => fl!(i18n, "dialog-preferences-font-face-noto-sans"),
        FontFace::LiberationSans => fl!(i18n, "dialog-preferences-font-face-liberation-sans"),
    }
}
