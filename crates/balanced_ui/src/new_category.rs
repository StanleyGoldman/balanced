// SPDX-License-Identifier: GPL-3.0-or-later WITH AdditionRef-Balanced-Plugin-Exception

//! The New Category Dialog's form. Its name is read as a path: the text after the last
//! `:` names the new category, and the text before it the category it goes under.

use balanced_core::{Category, CategoryId, CategoryType, DomainCommand, DomainError, LedgerView};
use balanced_i18n::Localizer;
use eframe::egui::{self, Label, RichText, TextEdit};

use crate::command::StatusMessage;
use crate::i18n::fl;
use crate::register_model::{CATEGORY_SEPARATOR, category_text, type_label};

/// The types a top-level category can take, in the order the Dialog offers them: Expense,
/// the one chosen at first, then Income.
const TYPE_CHOICES: [CategoryType; 2] = [CategoryType::Expense, CategoryType::Income];

/// What the New Category Dialog holds while it is open.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NewCategoryForm {
    name: String,
    /// The type a top-level category takes. A subcategory takes its parent's instead.
    category_type: CategoryType,
    error: Option<FormError>,
    /// The path of the category the form asked the ledger for, while it waits for the
    /// answer.
    submitted: Option<String>,
    /// Whether the Name field takes keyboard focus on the next frame.
    focus_name: bool,
}

/// Why the form's category wasn't created, shown below its Name field until the name
/// changes.
#[derive(Debug, Clone, PartialEq, Eq)]
enum FormError {
    /// No category has the path typed before the last `:`.
    NoParent(String),
    /// The ledger refused the category.
    Rejected(DomainError),
}

impl NewCategoryForm {
    /// The form prefilled with `name`, its Name field taking focus.
    pub(crate) fn new(name: String) -> Self {
        Self {
            name,
            category_type: CategoryType::Expense,
            error: None,
            submitted: None,
            focus_name: true,
        }
    }

    /// The Command creating the category the name says, in `view`, which the form
    /// then waits on. `None` while it already waits, or when no category has the path
    /// before the last `:`, which the form then shows.
    pub(crate) fn submit(&mut self, view: &LedgerView) -> Option<DomainCommand> {
        if self.submitted.is_some() {
            return None;
        }
        let typed = self.name.trim();
        let (parent, name) = match typed.rsplit_once(CATEGORY_SEPARATOR) {
            None => (None, typed),
            Some((parent, name)) => {
                let parent = parent.trim();
                let Some(found) = named_category(view, parent) else {
                    self.error = Some(FormError::NoParent(parent.to_owned()));
                    self.focus_name = true;
                    return None;
                };
                (Some(found), name.trim())
            }
        };
        let path = match parent.and_then(|parent| category_text(view, parent)) {
            Some(parent) => format!("{parent}{CATEGORY_SEPARATOR}{name}"),
            None => name.to_owned(),
        };
        self.submitted = Some(path);
        Some(DomainCommand::CreateCategory {
            name: name.to_owned(),
            parent,
            category_type: self.category_type,
        })
    }

    /// Takes the path of the category the form asked for, once the ledger has
    /// created it.
    pub(crate) fn take_created_path(&mut self) -> Option<String> {
        self.submitted.take()
    }

    /// Shows why the ledger refused the category, and gives the Name field focus.
    pub(crate) fn rejected(&mut self, error: DomainError) {
        self.submitted = None;
        self.error = Some(FormError::Rejected(error));
        self.focus_name = true;
    }

    /// Draws the Name field, with the error below it while there is one, else a hint,
    /// and the choice of Income or Expense while the name is of a top-level category.
    pub(crate) fn show(&mut self, ui: &mut egui::Ui, i18n: &Localizer) {
        let label = ui.label(fl!(i18n, "dialog-new-category-name"));
        let field = ui
            .add(
                TextEdit::singleline(&mut self.name)
                    .desired_width(f32::INFINITY)
                    .id(ui.make_persistent_id("dialog_new_category_name")),
            )
            .labelled_by(label.id);
        if std::mem::take(&mut self.focus_name) {
            field.request_focus();
        }
        if field.changed() {
            self.error = None;
        }
        match &self.error {
            Some(error) => {
                let text = match error {
                    FormError::NoParent(parent) => fl!(
                        i18n,
                        "dialog-new-category-no-parent",
                        parent = parent.clone()
                    ),
                    FormError::Rejected(error) => StatusMessage::Rejected(error.clone()).text(i18n),
                };
                ui.add(Label::new(
                    RichText::new(text).color(ui.visuals().error_fg_color),
                ));
            }
            None => {
                ui.add(Label::new(
                    RichText::new(fl!(i18n, "dialog-new-category-hint")).weak(),
                ));
            }
        }
        if !self.name.contains(CATEGORY_SEPARATOR) {
            ui.horizontal(|ui| {
                ui.label(fl!(i18n, "dialog-new-category-type"));
                for category_type in TYPE_CHOICES {
                    ui.radio_value(
                        &mut self.category_type,
                        category_type,
                        type_label(i18n, category_type),
                    );
                }
            });
        }
    }
}

/// The category of `view` whose path is `path`, ignoring case.
fn named_category(view: &LedgerView, path: &str) -> Option<CategoryId> {
    let path = path.to_lowercase();
    view.categories()
        .iter()
        .map(Category::id)
        .find(|&id| category_text(view, id).is_some_and(|text| text.to_lowercase() == path))
}
