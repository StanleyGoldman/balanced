// SPDX-License-Identifier: GPL-3.0-or-later WITH AdditionRef-Balanced-Plugin-Exception

//! Go to date's form: a box that is the date field, with its keys, its 📅 button and
//! its calendar.

use balanced_i18n::Localizer;
use eframe::egui::{self, Id, Sense, vec2};
use icu_calendar::{Date, Iso};

use crate::date_field::{self, DateEdit};
use crate::i18n::fl;
use crate::theme::Theme;

/// What Go to date holds while it is open.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GoToDateForm {
    /// The date the box opened on.
    seed: Date<Iso>,
    date: DateEdit,
    /// Whether the box takes keyboard focus on the next frame.
    focus_field: bool,
}

impl GoToDateForm {
    /// The form with its box on `seed`, taking focus.
    pub(crate) fn new(seed: Date<Iso>) -> Self {
        Self {
            seed,
            date: DateEdit::new(Some(seed)),
            focus_field: true,
        }
    }

    /// The date to go to, or `None` when the box holds no date, which it then says.
    pub(crate) fn submit(&mut self, i18n: &Localizer) -> Option<Date<Iso>> {
        match self.date.commit(i18n) {
            date_field::Commit::Unchanged => Some(self.seed),
            date_field::Commit::Changed(date) => Some(date),
            date_field::Commit::Rejected => {
                self.focus_field = true;
                None
            }
        }
    }

    /// Draws the box, `width` wide, under its label.
    pub(crate) fn show(
        &mut self,
        ui: &mut egui::Ui,
        i18n: &Localizer,
        theme: &Theme,
        today: Date<Iso>,
        width: f32,
    ) {
        let label = ui.label(fl!(i18n, "dialog-go-to-date-date"));
        let (rect, _) = ui.allocate_exact_size(vec2(width, theme.row_height()), Sense::hover());
        let field = self
            .date
            .show(ui, i18n, theme, today, Id::new("go_to_date_field"), rect)
            .labelled_by(label.id);
        if std::mem::take(&mut self.focus_field) {
            field.request_focus();
        }
    }
}
