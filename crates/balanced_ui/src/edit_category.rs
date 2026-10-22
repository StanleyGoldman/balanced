// SPDX-License-Identifier: GPL-3.0-or-later WITH AdditionRef-Balanced-Plugin-Exception

//! The Edit Category Dialog's form: a category's name, the category it sits under, and,
//! for a top-level category, its type. OK sends every change as one Command, so they
//! undo in one step.

use balanced_core::{Category, CategoryId, CategoryType, DomainCommand, DomainError, LedgerView};
use balanced_i18n::Localizer;
use eframe::egui::{self, ComboBox, Id, Label, RichText, TextEdit};

use crate::command::{AppCommand, CommandSender, StatusMessage};
use crate::dialog::{DialogOutcome, HeavyDialog, OpenDialog};
use crate::i18n::fl;
use crate::register_model::{category_text, type_label};
use crate::theme::Theme;

/// The types a top-level category can take, in the order the Dialog offers them.
const TYPE_CHOICES: [CategoryType; 2] = [CategoryType::Expense, CategoryType::Income];

/// What the Edit Category Dialog holds while it is open.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EditCategoryForm {
    /// The category as it was when the Dialog opened.
    category: Category,
    name: String,
    /// The category it goes under, or `None` for the top of the tree.
    parent: Option<CategoryId>,
    /// The type it takes while it goes at the top of the tree.
    category_type: CategoryType,
    /// Why the ledger refused the last changes, shown until a field changes.
    error: Option<DomainError>,
    /// Whether the form waits on the ledger's answer to its changes.
    submitted: bool,
    /// Whether the Name field takes keyboard focus on the next frame.
    focus_name: bool,
}

impl EditCategoryForm {
    /// The form holding `category` as it is, its Name field taking focus.
    pub(crate) fn new(category: &Category) -> Self {
        Self {
            category: category.clone(),
            name: category.name().to_owned(),
            parent: category.parent(),
            category_type: category.category_type(),
            error: None,
            submitted: false,
            focus_name: true,
        }
    }

    /// The Commands making the form's changes, in order: the new name and the new
    /// parent, as one Command when both change so the name is checked only against the
    /// new parent's categories, then the new type of a category at the top of the tree.
    fn changes(&self) -> Vec<DomainCommand> {
        let id = self.category.id();
        let renamed = self.name.trim() != self.category.name();
        let moved = self.parent != self.category.parent();
        let mut changes = Vec::new();
        match (renamed, moved) {
            (true, true) => changes.push(DomainCommand::EditCategory {
                category: id,
                name: self.name.clone(),
                parent: self.parent,
            }),
            (true, false) => changes.push(DomainCommand::RenameCategory {
                category: id,
                name: self.name.clone(),
            }),
            (false, true) => changes.push(DomainCommand::MoveCategory {
                category: id,
                parent: self.parent,
            }),
            (false, false) => {}
        }
        if self.parent.is_none() && self.category_type != self.category.category_type() {
            changes.push(DomainCommand::SetCategoryType {
                category: id,
                category_type: self.category_type,
            });
        }
        changes
    }

    /// Shows why the ledger refused the changes, and gives the Name field focus.
    pub(crate) fn rejected(&mut self, error: DomainError) {
        self.submitted = false;
        self.error = Some(error);
        self.focus_name = true;
    }

    /// Draws the Name field, the Parent picker and the type choice, which only a
    /// category at the top of the tree takes, with the ledger's refusal below them while
    /// there is one.
    fn show(&mut self, ui: &mut egui::Ui, i18n: &Localizer, view: &LedgerView) {
        let before = (self.name.clone(), self.parent, self.category_type);
        let label = ui.label(fl!(i18n, "dialog-edit-category-name"));
        let field = ui
            .add(
                TextEdit::singleline(&mut self.name)
                    .desired_width(f32::INFINITY)
                    .id(ui.make_persistent_id("dialog_edit_category_name")),
            )
            .labelled_by(label.id);
        if std::mem::take(&mut self.focus_name) {
            field.request_focus();
        }
        self.parent_picker(ui, i18n, view);
        let top_level = self.parent.is_none();
        // A subcategory shows the type it takes from the category it goes under.
        let mut shown_type = match self.parent.and_then(|parent| view.category(parent)) {
            Some(parent) => parent.category_type(),
            None => self.category_type,
        };
        ui.horizontal(|ui| {
            ui.label(fl!(i18n, "dialog-edit-category-type"));
            ui.add_enabled_ui(top_level, |ui| {
                for category_type in TYPE_CHOICES {
                    ui.radio_value(
                        &mut shown_type,
                        category_type,
                        type_label(i18n, category_type),
                    );
                }
            });
        });
        if top_level {
            self.category_type = shown_type;
        }
        if (self.name.clone(), self.parent, self.category_type) != before {
            self.error = None;
        }
        if let Some(error) = &self.error {
            let text = StatusMessage::Rejected(error.clone()).text(i18n);
            ui.add(Label::new(
                RichText::new(text).color(ui.visuals().error_fg_color),
            ));
        }
    }

    /// The Parent picker: the top of the tree, then every other category by its path.
    fn parent_picker(&mut self, ui: &mut egui::Ui, i18n: &Localizer, view: &LedgerView) {
        let top_level = fl!(i18n, "dialog-edit-category-top-level");
        let mut paths: Vec<(CategoryId, String)> = view
            .categories()
            .iter()
            .map(Category::id)
            .filter(|&id| id != self.category.id())
            .filter_map(|id| Some((id, category_text(view, id)?)))
            .collect();
        paths.sort_by(|(_, a), (_, b)| i18n.compare_text(a, b));
        let chosen = match self.parent {
            Some(parent) => category_text(view, parent).unwrap_or_default(),
            None => top_level.clone(),
        };
        ComboBox::from_label(fl!(i18n, "dialog-edit-category-parent"))
            .selected_text(chosen)
            .width(ui.available_width() / 2.0)
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut self.parent, None, top_level);
                for (id, path) in paths {
                    ui.selectable_value(&mut self.parent, Some(id), path);
                }
            });
    }
}

/// What became of the Edit Category Dialog this frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EditCategoryOutcome {
    /// It stays open, waiting on the ledger after OK.
    Open,
    /// OK with nothing changed, or Cancel.
    Closed,
}

/// Draws the Edit Category Dialog over the Category List, from `view`. OK sends the
/// changes as one Command, and the Dialog stays open until the ledger answers.
pub(crate) fn show(
    ctx: &egui::Context,
    i18n: &Localizer,
    theme: &Theme,
    commands: &CommandSender,
    dialog: &mut OpenDialog<EditCategoryForm>,
    view: &LedgerView,
) -> EditCategoryOutcome {
    let outcome = HeavyDialog::new(
        Id::new("edit_category_dialog"),
        fl!(i18n, "dialog-edit-category-title"),
        fl!(i18n, "dialog-ok"),
        fl!(i18n, "dialog-cancel"),
        fl!(i18n, "dialog-close"),
        theme,
    )
    .show(ctx, dialog, |ui, form| form.show(ui, i18n, view));
    match outcome {
        DialogOutcome::Open => {}
        DialogOutcome::Accepted => {
            let form = &mut dialog.form;
            if form.submitted {
                return EditCategoryOutcome::Open;
            }
            let changes = form.changes();
            if changes.is_empty() {
                return EditCategoryOutcome::Closed;
            }
            form.submitted = true;
            commands.send(AppCommand::SubmitDialog(DomainCommand::Compound(changes)));
        }
        DialogOutcome::Back | DialogOutcome::Discarded => {
            unreachable!("a heavy Dialog has no Back or Discard button")
        }
        DialogOutcome::Cancelled => return EditCategoryOutcome::Closed,
    }
    EditCategoryOutcome::Open
}
