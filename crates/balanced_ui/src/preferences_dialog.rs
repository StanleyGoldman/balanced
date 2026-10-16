//! The Preferences Dialog's form: a draft of the Preferences, edited one group at a
//! time. The App shows the draft while the Dialog is open.

use balanced_i18n::Localizer;
use eframe::egui::{self, ComboBox};

use crate::i18n::fl;
use crate::preferences::{FontFace, Preferences, Scheme};

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
}

impl Group {
    const ALL: [Self; 1] = [Self::Display];

    fn label(self, i18n: &Localizer) -> String {
        match self {
            Self::Display => fl!(i18n, "dialog-preferences-group-display"),
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

    /// Draws the group list, and beside it the Preferences of the chosen group.
    pub(crate) fn show(&mut self, ui: &mut egui::Ui, i18n: &Localizer) {
        let before = self.draft.clone();
        ui.horizontal_top(|ui| {
            ui.vertical(|ui| {
                for group in Group::ALL {
                    if ui
                        .selectable_label(self.group == group, group.label(i18n))
                        .clicked()
                    {
                        self.group = group;
                    }
                }
            });
            ui.separator();
            ui.vertical(|ui| match self.group {
                Group::Display => self.display(ui, i18n),
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
}

/// The name of `face`, as the font list shows it.
fn font_face_label(face: FontFace, i18n: &Localizer) -> String {
    match face {
        FontFace::Egui => fl!(i18n, "dialog-preferences-font-face-egui"),
        FontFace::NotoSans => fl!(i18n, "dialog-preferences-font-face-noto-sans"),
        FontFace::LiberationSans => fl!(i18n, "dialog-preferences-font-face-liberation-sans"),
    }
}
