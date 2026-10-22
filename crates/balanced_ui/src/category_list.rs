// SPDX-License-Identifier: GPL-3.0-or-later WITH AdditionRef-Balanced-Plugin-Exception

//! The Category List: a heavy Dialog showing every category as a tree, with its type,
//! how many transactions and split lines use each and their total, and every account a
//! transaction can transfer to. A filter row above the tree narrows it by type, by
//! usage and by name. Buttons beside the tree create, edit and delete categories; each
//! change applies as soon as its confirm is accepted, as one undo step.

use std::collections::{BTreeMap, HashSet};

use balanced_core::{
    AccountId, Category, CategoryId, CategoryType, DomainCommand, DomainError, Generation,
    LedgerView, Usage,
};
use balanced_i18n::{AmountStyle, Localizer};
use eframe::egui::accesskit::{self, Role};
use eframe::egui::style::ScrollStyle;
use eframe::egui::{
    self, Align, Button, ComboBox, EventFilter, Id, Key, Layout, Modifiers, Rect, Response,
    RichText, Sense, TextEdit, TextStyle, TextWrapMode, WidgetInfo, WidgetText, WidgetType, pos2,
    vec2,
};
use egui_extras::{Column, Size, StripBuilder, TableBuilder, TableRow};
use icu_locale::{LanguageIdentifier, Locale};

use crate::command::{AppCommand, CommandSender};
use crate::dialog::{DialogOutcome, FocusReturn, HeavyDialog, LightDialog, OpenDialog};
use crate::edit_category::{self, EditCategoryForm, EditCategoryOutcome};
use crate::formatted_amount::FormattedAmount;
use crate::i18n::fl;
use crate::new_category::{self, NewCategoryForm, NewCategoryOutcome};
use crate::register_model::{category_text, transfer_text, type_label};
use crate::theme::Theme;
use crate::triangle_button::{Fold, fold_triangle};

/// What the Category List holds while it is open.
#[derive(Debug, Default)]
pub(crate) struct CategoryListForm {
    /// The rows of the last snapshot shown, rebuilt when it is out of date.
    model: Option<ListModel>,
    /// The categories whose subcategories are hidden.
    folded: HashSet<CategoryId>,
    /// Which rows the filter row lets the tree show.
    filter: Filter,
    /// The index in the model of each row shown, top to bottom: every row the filter
    /// keeps but those under a folded category. `None` once the model, the filter or
    /// the folds change, so each index is in range of the model it was built from.
    shown: Option<Vec<usize>>,
    /// The selected row, which is the tree's one Tab stop.
    selected: Option<RowKey>,
    /// Whether the selected row takes keyboard focus once it is drawn.
    focus_selected: bool,
    /// The offset the table scrolls to on the next frame, to show the selected row.
    scroll_to: Option<f32>,
    /// How far the table was scrolled on the last frame, and how tall its rows' view
    /// was.
    scrolled: Option<Scrolled>,
    /// The New Category Dialog, while it is open over the list.
    new_category: Option<OpenDialog<NewCategoryForm>>,
    /// The Edit Category Dialog, while it is open over the list.
    edit: Option<OpenDialog<EditCategoryForm>>,
    /// The delete confirm, while it is open over the list.
    delete: Option<OpenDialog<DeleteConfirm>>,
    /// Where focus goes now that a Dialog over the list has closed, until a frame
    /// restores it.
    focus_return: Option<FocusReturn>,
}

/// The delete confirm's form: which category it deletes, with every category under it,
/// and where their transactions go.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct DeleteConfirm {
    category: CategoryId,
    /// The category the transactions move to, once one is picked (Reassign).
    target: Option<CategoryId>,
    /// Whether the target list is open, or the press that closed it is still held.
    targets_held: bool,
    /// Whether the target list takes keyboard focus when next drawn, as it does when
    /// the confirm opens with Delete disabled.
    focus_targets: bool,
}

impl DeleteConfirm {
    fn new(category: CategoryId) -> Self {
        Self {
            category,
            target: None,
            targets_held: false,
            focus_targets: true,
        }
    }

    /// The target list, which leaves out the categories in `branch`: every other
    /// category, by its path. It takes keyboard focus the first time it is drawn.
    fn target_picker(
        &mut self,
        ui: &mut egui::Ui,
        i18n: &Localizer,
        view: &LedgerView,
        branch: &[CategoryId],
    ) {
        let mut paths: Vec<(CategoryId, String)> = view
            .categories()
            .iter()
            .map(Category::id)
            .filter(|id| !branch.contains(id))
            .filter_map(|id| Some((id, category_text(view, id)?)))
            .collect();
        paths.sort_by(|(_, a), (_, b)| i18n.compare_text(a, b));
        let chosen = self
            .target
            .and_then(|target| category_text(view, target))
            .unwrap_or_else(|| fl!(i18n, "dialog-delete-category-choose"));
        let picker = ui.horizontal(|ui| {
            let label = ui.label(fl!(i18n, "dialog-delete-category-move-to"));
            ComboBox::from_id_salt("category_list_delete_target")
                .selected_text(chosen)
                .width(ui.available_width())
                .show_ui(ui, |ui| {
                    for (id, path) in paths {
                        ui.selectable_value(&mut self.target, Some(id), path);
                    }
                })
                .response
                .labelled_by(label.id)
        });
        if std::mem::take(&mut self.focus_targets) {
            picker.inner.request_focus();
        }
        let open = ComboBox::is_open(ui.ctx(), picker.inner.id);
        let held = self.targets_held && ui.input(|input| input.pointer.any_down());
        self.targets_held = open || held;
    }
}

/// Which rows the filter row lets the tree show: those Show picks whose names hold the
/// Search text, and every category above one of them.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct Filter {
    show: Show,
    /// The Search box's text, as typed.
    search: String,
}

impl Filter {
    /// Whether `row` is one the filter picks, before the categories above a pick are
    /// added. `search` is the Search text, trimmed and lowercased.
    fn picks(&self, row: &ListRow, search: &str) -> bool {
        let shown = match self.show {
            Show::All => true,
            Show::Income => row.category_type == Some(CategoryType::Income),
            Show::Expense => row.category_type == Some(CategoryType::Expense),
            Show::Used => row.used,
            Show::Unused => !row.used,
        };
        shown && row.name.to_lowercase().contains(search)
    }
}

/// The rows the Show dropdown picks.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
enum Show {
    #[default]
    All,
    /// Income categories, and no transfer rows.
    Income,
    /// Expense categories, and no transfer rows.
    Expense,
    /// Rows that a transaction or split line uses, counting those under a category.
    Used,
    /// Rows that nothing uses.
    Unused,
}

impl Show {
    const ALL: [Self; 5] = [
        Self::All,
        Self::Income,
        Self::Expense,
        Self::Used,
        Self::Unused,
    ];

    fn label(self, i18n: &Localizer) -> String {
        match self {
            Self::All => fl!(i18n, "dialog-category-list-show-all"),
            Self::Income => fl!(i18n, "dialog-category-list-show-income"),
            Self::Expense => fl!(i18n, "dialog-category-list-show-expense"),
            Self::Used => fl!(i18n, "dialog-category-list-show-used"),
            Self::Unused => fl!(i18n, "dialog-category-list-show-unused"),
        }
    }
}

/// What a click in the button column asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ButtonAction {
    New,
    Edit(CategoryId),
    Delete(CategoryId),
}

/// How far the table is scrolled, and how tall its rows' view is.
#[derive(Debug, Clone, Copy)]
struct Scrolled {
    offset: f32,
    height: f32,
}

/// What a click or a key in the table asks for, done once the table is drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TableAction {
    /// Select the row, and give it focus.
    Select(RowKey),
    /// Fold the category, or unfold it, and select it.
    Toggle(CategoryId),
    /// Fold or unfold the category, as ← and → do.
    SetFold(CategoryId, Fold),
    /// Select the row shown below the selected one, as ↓ does.
    Down,
    /// Select the row shown above the selected one, as ↑ does.
    Up,
}

/// How a row is drawn this frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RowState {
    fold: Fold,
    selected: bool,
    /// Whether it is the tree's Tab stop: the selected row, or the first row while none
    /// shown is selected.
    tab_stop: bool,
    /// Whether it takes keyboard focus this frame.
    take_focus: bool,
}

/// Which row of the Category List a row is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum RowKey {
    Category(CategoryId),
    /// An account, which a transaction names as `[Account]` to transfer to it.
    Transfer(AccountId),
}

/// One row of the Category List, formatted for drawing.
#[derive(Debug)]
struct ListRow {
    key: RowKey,
    /// How many categories it sits under: 0 at the top of the tree.
    depth: usize,
    name: String,
    /// Whether any category sits under it.
    has_children: bool,
    /// A category's type, or `None` for an account.
    category_type: Option<CategoryType>,
    /// The Type cell's text, or `None` for an account.
    type_text: Option<String>,
    /// Whether any transaction or split line uses it, or a category under it.
    used: bool,
    /// The Txns cell's text.
    count: String,
    /// The Total cell's amount, or `None` when nothing uses the row or the total
    /// overflows.
    total: Option<FormattedAmount>,
    /// The type, count and total, as a screen reader reads them after the name.
    description: String,
}

/// Every row of the Category List for one snapshot: the categories in tree order, each
/// category's subcategories after it sorted by name, then the accounts in Account bar
/// order.
#[derive(Debug)]
struct ListModel {
    source: Source,
    rows: Vec<ListRow>,
}

/// What a [`ListModel`] was built from: a snapshot, formatted in a locale.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Source {
    generation: Generation,
    text_language: LanguageIdentifier,
    formatting_locale: Locale,
}

impl Source {
    fn of(view: &LedgerView, i18n: &Localizer) -> Self {
        Self {
            generation: view.generation(),
            text_language: i18n.text_language().clone(),
            formatting_locale: i18n.formatting_locale().clone(),
        }
    }
}

impl ListModel {
    fn new(view: &LedgerView, i18n: &Localizer) -> Self {
        let mut children: BTreeMap<Option<CategoryId>, Vec<&Category>> = BTreeMap::new();
        for category in view.categories() {
            children
                .entry(category.parent())
                .or_default()
                .push(category);
        }
        for siblings in children.values_mut() {
            siblings.sort_by(|a, b| i18n.compare_text(a.name(), b.name()));
        }
        let mut rows = Vec::with_capacity(view.categories().len() + view.accounts().len());
        // Depth first, each category before its subcategories.
        let mut pending: Vec<(&Category, usize)> = children
            .get(&None)
            .into_iter()
            .flatten()
            .rev()
            .map(|&category| (category, 0))
            .collect();
        while let Some((category, depth)) = pending.pop() {
            let key = RowKey::Category(category.id());
            let usage = view.category_usage(category.id());
            let under = children.get(&Some(category.id()));
            let name = category.name().to_owned();
            let has_children = under.is_some();
            let category_type = Some(category.category_type());
            rows.push(ListRow::new(
                i18n,
                key,
                depth,
                name,
                has_children,
                category_type,
                usage,
            ));
            pending.extend(
                under
                    .into_iter()
                    .flatten()
                    .rev()
                    .map(|&child| (child, depth + 1)),
            );
        }
        for summary in view.accounts() {
            let account = summary.account();
            let usage = view.transfer_usage(account.id());
            let name = transfer_text(account.name());
            let key = RowKey::Transfer(account.id());
            rows.push(ListRow::new(i18n, key, 0, name, false, None, usage));
        }
        Self {
            source: Source::of(view, i18n),
            rows,
        }
    }

    /// The index of each row shown through `filter` while the categories in `folded`
    /// are folded.
    fn shown(&self, filter: &Filter, folded: &HashSet<CategoryId>) -> Vec<usize> {
        let kept = self.kept(filter);
        let kept: Vec<bool> = (0..self.rows.len())
            .map(|index| kept.picked[index] || kept.above_pick[index])
            .collect();
        let mut shown = Vec::with_capacity(self.rows.len());
        // The depth of the folded category whose subcategories are being passed over.
        // A row the filter leaves out has none kept under it, so it never starts a fold.
        let mut passing: Option<usize> = None;
        for (index, row) in self.rows.iter().enumerate() {
            if !kept[index] {
                continue;
            }
            let under_fold = matches!(row.key, RowKey::Category(_))
                && passing.is_some_and(|depth| row.depth > depth);
            if under_fold {
                continue;
            }
            passing = match row.key {
                RowKey::Category(id) if folded.contains(&id) => Some(row.depth),
                RowKey::Category(_) | RowKey::Transfer(_) => None,
            };
            shown.push(index);
        }
        shown
    }

    /// Whether `filter` keeps each row: those it picks, and every category above one.
    fn kept(&self, filter: &Filter) -> Kept {
        let search = filter.search.trim().to_lowercase();
        let mut kept = Kept {
            picked: vec![false; self.rows.len()],
            above_pick: vec![false; self.rows.len()],
        };
        // The index of each category above the row, by depth. Rows come depth first, so
        // each row's parent is the last row seen one level up.
        let mut above: Vec<usize> = Vec::new();
        for (index, row) in self.rows.iter().enumerate() {
            above.truncate(row.depth);
            if filter.picks(row, &search) {
                kept.picked[index] = true;
                for &parent in &above {
                    kept.above_pick[parent] = true;
                }
            }
            match row.key {
                RowKey::Category(_) => above.push(index),
                RowKey::Transfer(_) => {}
            }
        }
        kept
    }

    /// The categories above a row that `filter` picks.
    fn above_picks(&self, filter: &Filter) -> impl Iterator<Item = CategoryId> {
        let kept = self.kept(filter);
        self.rows
            .iter()
            .zip(kept.above_pick)
            .filter_map(|(row, above_pick)| match row.key {
                RowKey::Category(id) if above_pick => Some(id),
                RowKey::Category(_) | RowKey::Transfer(_) => None,
            })
    }
}

/// Which rows a filter keeps, by index in the model.
struct Kept {
    /// The rows the filter picks.
    picked: Vec<bool>,
    /// The categories above a row the filter picks.
    above_pick: Vec<bool>,
}

impl ListRow {
    fn new(
        i18n: &Localizer,
        key: RowKey,
        depth: usize,
        name: String,
        has_children: bool,
        category_type: Option<CategoryType>,
        usage: &Usage,
    ) -> Self {
        let type_text = category_type.map(|category_type| type_label(i18n, category_type));
        let count = usage.count();
        let total = match usage.total() {
            Ok(total) if count > 0 => Some(FormattedAmount::new(i18n, total, AmountStyle::Symbol)),
            Ok(_) => None,
            Err(error) => {
                tracing::warn!(?error, ?key, "a Category List total can't be summed");
                None
            }
        };
        let description = if let Some(total) = &total {
            fl!(
                i18n,
                "dialog-category-list-usage",
                count = count,
                total = total.text().to_owned()
            )
        } else {
            fl!(i18n, "dialog-category-list-usage-count", count = count)
        };
        let description = match &type_text {
            Some(category_type) => fl!(
                i18n,
                "dialog-category-list-typed-usage",
                category_type = category_type.clone(),
                usage = description
            ),
            None => description,
        };
        Self {
            key,
            depth,
            name,
            has_children,
            category_type,
            type_text,
            used: count > 0,
            count: i18n.format_number(
                i64::try_from(count).expect("a ledger holds fewer than 2^63 transactions"),
            ),
            total,
            description,
        }
    }
}

impl CategoryListForm {
    /// Hands the New Category or Edit Category Dialog open over the list the ledger's
    /// answer to the change it asked for: it closes once the change is made, and shows a
    /// refusal itself. Returns the answer when no Dialog over the list waits on it.
    pub(crate) fn applied(
        &mut self,
        ctx: &egui::Context,
        applied: Result<(), DomainError>,
    ) -> Option<Result<(), DomainError>> {
        if let Some(dialog) = &mut self.edit {
            match applied {
                Ok(()) => self.focus_return = self.edit.take().map(|dialog| dialog.close(ctx)),
                Err(error) => dialog.form.rejected(error),
            }
            return None;
        }
        let Some(dialog) = &mut self.new_category else {
            return Some(applied);
        };
        match applied {
            Ok(()) => self.focus_return = self.new_category.take().map(|dialog| dialog.close(ctx)),
            Err(error) => dialog.form.rejected(error),
        }
        None
    }

    /// Draws the tree and its usage beside the button column, above the status line,
    /// filling `ui`.
    fn show(&mut self, ui: &mut egui::Ui, i18n: &Localizer, theme: &Theme, view: &LedgerView) {
        if let Some(focus) = self.focus_return.take() {
            self.focus_return = focus.restore_in(ui.ctx(), dialog_id());
        }
        let model = match self.model.take() {
            Some(model) if model.source == Source::of(view, i18n) => model,
            _ => {
                self.shown = None;
                ListModel::new(view, i18n)
            }
        };
        let shown = self.shown.take().unwrap_or_else(|| {
            let shown = model.shown(&self.filter, &self.folded);
            // A row a fold or the filter hides is no longer selected.
            let still_shown = |key| shown.iter().any(|&index| model.rows[index].key == key);
            self.selected = self.selected.filter(|&key| still_shown(key));
            shown
        });
        let mut asked = None;
        let filter = self.filter.clone();
        let status_height = ui.text_style_height(&TextStyle::Body);
        let filter_height = ui.spacing().interact_size.y;
        StripBuilder::new(ui)
            .size(Size::exact(filter_height))
            .size(Size::remainder())
            .size(Size::exact(status_height))
            .vertical(|mut strip| {
                strip.cell(|ui| self.filter_row(ui, i18n));
                strip.strip(|builder| {
                    builder
                        .size(Size::remainder())
                        .size(Size::exact(Theme::CATEGORY_LIST_BUTTONS_WIDTH))
                        .horizontal(|mut strip| {
                            strip.cell(|ui| self.table(ui, i18n, theme, &model, shown));
                            strip.cell(|ui| asked = self.buttons(ui, i18n, &model));
                        });
                });
                strip.cell(|ui| Self::status_line(ui, i18n));
            });
        // The table has drawn the rows the old filter showed; the next frame shows the
        // new filter's.
        if self.filter != filter {
            self.shown = None;
            // A new search opens every fold above a match, so the match shows.
            if self.filter.search != filter.search && !self.filter.search.trim().is_empty() {
                for id in model.above_picks(&self.filter) {
                    self.folded.remove(&id);
                }
            }
        }
        match asked {
            None => {}
            Some(ButtonAction::New) => {
                let form = NewCategoryForm::new(String::new());
                self.new_category = Some(OpenDialog::new(ui.ctx(), form));
            }
            Some(ButtonAction::Edit(category)) => {
                // The list is drawn from `view`, so its categories are in it.
                if let Some(category) = view.category(category) {
                    let form = EditCategoryForm::new(category);
                    self.edit = Some(OpenDialog::new(ui.ctx(), form));
                }
            }
            Some(ButtonAction::Delete(category)) => {
                self.delete = Some(OpenDialog::new(ui.ctx(), DeleteConfirm::new(category)));
            }
        }
        self.model = Some(model);
    }

    /// The filter row above the tree: the Show dropdown and the Search box.
    fn filter_row(&mut self, ui: &mut egui::Ui, i18n: &Localizer) {
        ui.horizontal(|ui| {
            let label = ui.label(fl!(i18n, "dialog-category-list-show"));
            ComboBox::from_id_salt("category_list_show")
                .selected_text(self.filter.show.label(i18n))
                .show_ui(ui, |ui| {
                    for show in Show::ALL {
                        ui.selectable_value(&mut self.filter.show, show, show.label(i18n));
                    }
                })
                .response
                .labelled_by(label.id);
            let label = ui.label(fl!(i18n, "dialog-category-list-search"));
            ui.add(
                TextEdit::singleline(&mut self.filter.search)
                    .hint_text(fl!(i18n, "dialog-category-list-search", "placeholder"))
                    .desired_width(Theme::SEARCH_BOX_WIDTH),
            )
            .labelled_by(label.id);
        });
    }

    /// Draws the rows of `model` at the indices `shown`, and does what a click or a key
    /// in them asks for.
    fn table(
        &mut self,
        ui: &mut egui::Ui,
        i18n: &Localizer,
        theme: &Theme,
        model: &ListModel,
        shown: Vec<usize>,
    ) {
        let tab_stop = self
            .selected
            .or_else(|| shown.first().map(|&index| model.rows[index].key));
        let mut action = None;
        // A solid scroll bar takes its own width beside the columns, so it never covers
        // Total.
        ui.style_mut().spacing.scroll = ScrollStyle::solid();
        let row_height = theme.row_height();
        let pitch = row_height + ui.spacing().item_spacing.y;
        let mut table = TableBuilder::new(ui)
            .id_salt("category_list_table")
            .striped(true)
            .resizable(false)
            .auto_shrink(false)
            .max_scroll_height(f32::INFINITY)
            .sense(Sense::CLICK)
            .cell_layout(Layout::left_to_right(Align::Center))
            .column(Column::remainder().clip(true))
            .column(Column::exact(Theme::CATEGORY_LIST_TYPE_WIDTH))
            .column(Column::exact(Theme::CATEGORY_LIST_COUNT_WIDTH))
            .column(Column::exact(Theme::CATEGORY_LIST_TOTAL_WIDTH));
        if let Some(offset) = self.scroll_to.take() {
            table = table.vertical_scroll_offset(offset);
        }
        let scrolled = table
            .header(row_height, |header| headings(header, i18n))
            .body(|body| {
                body.rows(row_height, shown.len(), |mut table_row| {
                    // egui asks only for rows below `shown.len()`.
                    let row = &model.rows[shown[table_row.index()]];
                    let selected = self.selected == Some(row.key);
                    let state = RowState {
                        fold: match row.key {
                            RowKey::Category(id) if self.folded.contains(&id) => Fold::Folded,
                            RowKey::Category(_) | RowKey::Transfer(_) => Fold::Open,
                        },
                        selected,
                        tab_stop: tab_stop == Some(row.key),
                        take_focus: selected && self.focus_selected,
                    };
                    table_row.set_selected(selected);
                    table_row.col(|ui| {
                        if let Some(asked) = name_cell(ui, i18n, row, state) {
                            action = Some(asked);
                        }
                    });
                    table_row.col(|ui| {
                        if let Some(type_text) = &row.type_text {
                            ui.label(type_text);
                        }
                    });
                    table_row.col(|ui| {
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            ui.label(&row.count);
                        });
                    });
                    table_row.col(|ui| {
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if let Some(total) = &row.total {
                                ui.label(total.styled(theme));
                            }
                        });
                    });
                    if state.take_focus {
                        self.focus_selected = false;
                    }
                    if action.is_none() && table_row.response().clicked() {
                        action = Some(TableAction::Select(row.key));
                    }
                });
            });
        self.scrolled = Some(Scrolled {
            offset: scrolled.state.offset.y,
            height: scrolled.inner_rect.height(),
        });
        self.shown = Some(shown);
        if let Some(action) = action {
            self.act(model, action, pitch);
        }
    }

    /// The button column: New… always, and Edit… and Delete… for a selected category.
    /// Merge… and Hide wait on their own Dialogs. Returns what a click asked for.
    fn buttons(
        &self,
        ui: &mut egui::Ui,
        i18n: &Localizer,
        model: &ListModel,
    ) -> Option<ButtonAction> {
        let selected = self
            .selected
            .and_then(|key| model.rows.iter().find(|row| row.key == key));
        let editable = selected.and_then(|row| match row.key {
            RowKey::Category(id) => Some(id),
            RowKey::Transfer(_) => None,
        });
        let mut asked = None;
        ui.vertical(|ui| {
            let width = ui.available_width() - ui.spacing().item_spacing.x;
            let mut button = |text: String, enabled: bool| {
                ui.add_enabled(enabled, Button::new(text).min_size(vec2(width, 0.0)))
                    .clicked()
            };
            if button(fl!(i18n, "dialog-category-list-new"), true) {
                asked = Some(ButtonAction::New);
            }
            if button(fl!(i18n, "dialog-category-list-edit"), editable.is_some()) {
                asked = editable.map(ButtonAction::Edit);
            }
            // TODO(#489): Merge… the selected categories.
            button(fl!(i18n, "dialog-category-list-merge"), false);
            // TODO(#491): Hide and Unhide a category.
            button(fl!(i18n, "dialog-category-list-hide"), false);
            if button(fl!(i18n, "dialog-category-list-delete"), editable.is_some()) {
                asked = editable.map(ButtonAction::Delete);
            }
        });
        asked
    }

    /// The status line below the tree: changes apply at once.
    fn status_line(ui: &mut egui::Ui, i18n: &Localizer) {
        ui.label(RichText::new(fl!(i18n, "dialog-category-list-status")).weak());
    }

    /// Does what a click or a key in the table asked for, in `model`, whose rows are
    /// `pitch` apart.
    fn act(&mut self, model: &ListModel, action: TableAction, pitch: f32) {
        let shown = self.shown.as_deref().unwrap_or_default();
        let place = |key| shown.iter().position(|&index| model.rows[index].key == key);
        match action {
            TableAction::Select(key) => self.select(key),
            TableAction::Toggle(id) => {
                if !self.folded.remove(&id) {
                    self.folded.insert(id);
                }
                self.shown = None;
                self.select(RowKey::Category(id));
            }
            TableAction::SetFold(id, fold) => {
                let changed = match fold {
                    Fold::Open => self.folded.remove(&id),
                    Fold::Folded => self.folded.insert(id),
                };
                if changed {
                    self.shown = None;
                }
            }
            // The selected row is one of `shown`, so `shown` has a row at `at` and its
            // neighbours are in range.
            TableAction::Down => {
                if let Some(at) = self.selected.and_then(place) {
                    self.step_to(model, (at + 1).min(shown.len() - 1), pitch);
                }
            }
            TableAction::Up => {
                if let Some(at) = self.selected.and_then(place) {
                    self.step_to(model, at.saturating_sub(1), pitch);
                }
            }
        }
    }

    /// Selects the row shown at `place`, scrolling it into view, in `model`, whose
    /// rows are `pitch` apart.
    fn step_to(&mut self, model: &ListModel, place: usize, pitch: f32) {
        let Some(&index) = self.shown.as_deref().and_then(|shown| shown.get(place)) else {
            return;
        };
        // `shown` holds indices into the model it was built from.
        self.select(model.rows[index].key);
        self.scroll_to = self
            .scrolled
            .and_then(|view| row_in_view(view, place, pitch));
    }

    /// Selects the row `key`, which takes keyboard focus once it is drawn.
    fn select(&mut self, key: RowKey) {
        self.selected = Some(key);
        self.focus_selected = true;
    }
}

/// The table's headings: Category, Type, Txns and Total.
fn headings(mut header: TableRow<'_, '_>, i18n: &Localizer) {
    header.col(|ui| {
        ui.strong(fl!(i18n, "dialog-category-list-category"));
    });
    header.col(|ui| {
        ui.strong(fl!(i18n, "dialog-category-list-type"));
    });
    header.col(|ui| {
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.strong(fl!(i18n, "dialog-category-list-txns"));
        });
    });
    header.col(|ui| {
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.strong(fl!(i18n, "dialog-category-list-total"));
        });
    });
}

/// The offset that brings the row at `index`, of rows `pitch` apart, into a `view`
/// scrolled as it is, or `None` when the row is already in it.
fn row_in_view(view: Scrolled, index: usize, pitch: f32) -> Option<f32> {
    #[expect(
        clippy::cast_precision_loss,
        reason = "a list never holds enough rows to lose precision"
    )]
    let top = index as f32 * pitch;
    let bottom = top + pitch;
    if top < view.offset {
        Some(top)
    } else if bottom > view.offset + view.height {
        Some(bottom - view.height)
    } else {
        None
    }
}

/// The Category cell of `row`, drawn as `state` says: its fold triangle, when a
/// category sits under it, and its name, indented to its depth. A screen reader reads
/// the name as one tree item, with its level, its usage, whether it is selected and
/// whether it is expanded. While the name has focus, ↑ and ↓ move the selection, and ←
/// and → fold and unfold the row. Returns what a click or a key asked for.
fn name_cell(
    ui: &mut egui::Ui,
    i18n: &Localizer,
    row: &ListRow,
    state: RowState,
) -> Option<TableAction> {
    let cell = ui.max_rect();
    #[expect(
        clippy::cast_precision_loss,
        reason = "a tree is never deep enough to lose precision"
    )]
    let indent = row.depth as f32 * Theme::CATEGORY_LIST_INDENT;
    let icon_width = ui.spacing().icon_width;
    let slot = Rect::from_min_size(
        pos2(cell.left() + indent, cell.top()),
        vec2(icon_width, cell.height()),
    );
    let name_rect = Rect::from_x_y_ranges(
        (slot.right() + ui.spacing().item_spacing.x)..=cell.right(),
        cell.y_range(),
    );
    // Only the Tab stop takes focus from Tab; the arrow keys move between the rows.
    let sense = if state.tab_stop {
        Sense::click()
    } else {
        Sense::CLICK
    };
    let response = ui.interact(name_rect, Id::new(("category_list_row", row.key)), sense);
    if state.take_focus {
        response.request_focus();
    }
    let rect = response.rect;
    ui.ctx().accesskit_node_builder(response.id, |node| {
        // egui gives only a focusable widget its bounds, and only the Tab stop is one.
        node.set_bounds(accesskit::Rect {
            x0: rect.min.x.into(),
            y0: rect.min.y.into(),
            x1: rect.max.x.into(),
            y1: rect.max.y.into(),
        });
        node.set_role(Role::TreeItem);
        node.set_label(row.name.clone());
        node.set_description(row.description.clone());
        node.set_level(row.depth + 1);
        node.set_selected(state.selected);
        if row.has_children {
            node.set_expanded(state.fold == Fold::Open);
        }
    });
    let text = match row.key {
        RowKey::Category(_) => RichText::new(&row.name),
        RowKey::Transfer(_) => RichText::new(&row.name).weak(),
    };
    let galley = WidgetText::from(text).into_galley(
        ui,
        Some(TextWrapMode::Truncate),
        name_rect.width(),
        TextStyle::Body,
    );
    let top = name_rect.center().y - galley.size().y / 2.0;
    let color = ui.visuals().text_color();
    ui.painter()
        .galley(pos2(name_rect.left(), top), galley, color);
    ui.advance_cursor_after_rect(cell);
    let mut action = None;
    if response.clicked() || response.gained_focus() {
        action = Some(TableAction::Select(row.key));
    }
    if response.has_focus() {
        action = row_keys(ui, response.id, row).or(action);
    }
    if row.has_children
        && fold_triangle_button(ui, i18n, row, slot, state.fold).clicked()
        && let RowKey::Category(id) = row.key
    {
        action = Some(TableAction::Toggle(id));
    }
    action
}

/// The keys the focused name of `row`, widget `id`, takes from egui's focus moves: ↑
/// and ↓ move the selection, and ← and → fold and unfold a row with categories under
/// it.
fn row_keys(ui: &egui::Ui, id: Id, row: &ListRow) -> Option<TableAction> {
    ui.memory_mut(|memory| {
        memory.set_focus_lock_filter(
            id,
            EventFilter {
                horizontal_arrows: true,
                vertical_arrows: true,
                ..EventFilter::default()
            },
        );
    });
    let pressed = |key| ui.input_mut(|input| input.consume_key(Modifiers::NONE, key));
    let folding = match row.key {
        RowKey::Category(id) if row.has_children => Some(id),
        RowKey::Category(_) | RowKey::Transfer(_) => None,
    };
    if pressed(Key::ArrowDown) {
        Some(TableAction::Down)
    } else if pressed(Key::ArrowUp) {
        Some(TableAction::Up)
    } else if pressed(Key::ArrowLeft) {
        folding.map(|id| TableAction::SetFold(id, Fold::Folded))
    } else if pressed(Key::ArrowRight) {
        folding.map(|id| TableAction::SetFold(id, Fold::Open))
    } else {
        None
    }
}

/// The fold triangle of `row` in `slot`, folded as `fold` says. Only a click folds it,
/// so it is no Tab stop; ← and → fold the selected row.
fn fold_triangle_button(
    ui: &egui::Ui,
    i18n: &Localizer,
    row: &ListRow,
    slot: Rect,
    fold: Fold,
) -> Response {
    let triangle = ui.interact(slot, Id::new(("category_list_fold", row.key)), Sense::CLICK);
    let label = match fold {
        Fold::Open => fl!(
            i18n,
            "dialog-category-list-collapse",
            name = row.name.clone()
        ),
        Fold::Folded => fl!(i18n, "dialog-category-list-expand", name = row.name.clone()),
    };
    triangle.widget_info(|| WidgetInfo::labeled(WidgetType::Button, ui.is_enabled(), &label));
    let icon_size = ui.spacing().icon_width / 2.0;
    let icon = Rect::from_center_size(slot.center(), vec2(icon_size, icon_size));
    let color = ui.style().interact(&triangle).text_color();
    ui.painter().add(fold_triangle(icon, fold, color));
    triangle
}

/// What became of the Category List this frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CategoryListOutcome {
    Open,
    Closed,
}

fn dialog_id() -> Id {
    Id::new("category_list_dialog")
}

/// Draws the Category List from `view`, and any Dialog open over it. Each change it
/// asks for goes to the ledger as soon as its Dialog is accepted.
pub(crate) fn show(
    ctx: &egui::Context,
    i18n: &Localizer,
    theme: &Theme,
    commands: &CommandSender,
    dialog: &mut OpenDialog<CategoryListForm>,
    view: &LedgerView,
) -> CategoryListOutcome {
    let outcome = HeavyDialog::closing(
        dialog_id(),
        fl!(i18n, "dialog-category-list-title"),
        fl!(i18n, "dialog-category-list-close"),
        fl!(i18n, "dialog-close"),
        theme,
    )
    .with_size(Theme::CATEGORY_LIST_SIZE)
    .body_fills()
    .show(ctx, dialog, |ui, form| form.show(ui, i18n, theme, view));
    // Drawn after the list, so they are raised above it.
    show_delete(ctx, i18n, theme, commands, &mut dialog.form, view);
    show_new_category(ctx, i18n, theme, commands, &mut dialog.form, view);
    show_edit(ctx, i18n, theme, commands, &mut dialog.form, view);
    match outcome {
        DialogOutcome::Open => CategoryListOutcome::Open,
        DialogOutcome::Accepted | DialogOutcome::Cancelled => CategoryListOutcome::Closed,
        DialogOutcome::Back | DialogOutcome::Discarded => {
            unreachable!("a heavy Dialog has no Back or Discard button")
        }
    }
}

/// The delete confirm, while it is open over the list. Delete has first focus, so
/// Enter confirms. It closes as its Command is sent, so a refusal goes to the Status
/// bar.
fn show_delete(
    ctx: &egui::Context,
    i18n: &Localizer,
    theme: &Theme,
    commands: &CommandSender,
    form: &mut CategoryListForm,
    view: &LedgerView,
) {
    let Some(dialog) = &mut form.delete else {
        return;
    };
    let category = dialog.form.category;
    // The ledger can't change while the confirm blocks the list.
    let name = category_text(view, category).expect("the confirm asks about a category");
    let branch = view.branch(category);
    let used = view.category_usage(category).count() > 0;
    let outcome = LightDialog::new(
        Id::new("category_list_delete"),
        fl!(i18n, "dialog-delete-category-delete"),
        fl!(i18n, "dialog-cancel"),
        theme,
    )
    .with_accept_enabled(!used || dialog.form.target.is_some())
    .with_body_popup(dialog.form.targets_held)
    .show(ctx, dialog, |ui, confirm| {
        ui.label(delete_message(i18n, view, &branch, name));
        if used {
            confirm.target_picker(ui, i18n, view, &branch);
        }
    });
    match outcome {
        DialogOutcome::Open => return,
        DialogOutcome::Accepted => {
            let command = delete_command(view, &branch, dialog.form.target);
            commands.send(AppCommand::Domain(command));
        }
        DialogOutcome::Cancelled => {}
        DialogOutcome::Back | DialogOutcome::Discarded => {
            unreachable!("a confirm has no Back or Discard button")
        }
    }
    form.focus_return = form.delete.take().map(|dialog| dialog.close(ctx));
}

/// The Command that deletes the categories in `branch`, a category followed by those
/// under it level by level, each after every category under it. It first moves the
/// transactions of each used one to `target`, when there is one. It is one undo step.
fn delete_command(
    view: &LedgerView,
    branch: &[CategoryId],
    target: Option<CategoryId>,
) -> DomainCommand {
    let deepest_first = || branch.iter().rev().copied();
    let deletes = deepest_first().map(|category| DomainCommand::DeleteCategory { category });
    let Some(to) = target else {
        return DomainCommand::Compound(deletes.collect());
    };
    let reassigns = deepest_first()
        .filter(|&from| view.category_usage(from).count() > 0)
        .map(|from| DomainCommand::ReassignCategory { from, to });
    DomainCommand::Compound(reassigns.chain(deletes).collect())
}

/// What the delete confirm says about the categories in `branch`, the first of them the
/// one deleted, whose full path is `name`: how many transactions use them and their
/// total, or that none does, and how many go with it.
fn delete_message(
    i18n: &Localizer,
    view: &LedgerView,
    branch: &[CategoryId],
    name: String,
) -> String {
    let (&category, under) = branch.split_first().expect("a branch holds its category");
    let subcategories = under.len();
    let usage = view.category_usage(category);
    let count = usage.count();
    if count == 0 && subcategories == 0 {
        return fl!(i18n, "dialog-delete-category-message", name = name);
    }
    if count == 0 {
        return fl!(
            i18n,
            "dialog-delete-category-unused-parent",
            name = name,
            subcategories = subcategories
        );
    }
    match usage.total() {
        Ok(total) => fl!(
            i18n,
            "dialog-delete-category-used",
            name = name,
            subcategories = subcategories,
            count = count,
            total = FormattedAmount::new(i18n, total, AmountStyle::Symbol)
                .text()
                .to_owned()
        ),
        Err(error) => {
            tracing::warn!(
                ?error,
                ?category,
                "a deleted category's total can't be summed"
            );
            fl!(
                i18n,
                "dialog-delete-category-used-count",
                name = name,
                subcategories = subcategories,
                count = count
            )
        }
    }
}

/// The New Category Dialog, while it is open over the list.
fn show_new_category(
    ctx: &egui::Context,
    i18n: &Localizer,
    theme: &Theme,
    commands: &CommandSender,
    form: &mut CategoryListForm,
    view: &LedgerView,
) {
    let Some(dialog) = &mut form.new_category else {
        return;
    };
    match new_category::show(ctx, i18n, theme, commands, dialog, view) {
        NewCategoryOutcome::Open => {}
        NewCategoryOutcome::Cancelled => {
            form.focus_return = form.new_category.take().map(|dialog| dialog.close(ctx));
        }
    }
}

/// The Edit Category Dialog, while it is open over the list.
fn show_edit(
    ctx: &egui::Context,
    i18n: &Localizer,
    theme: &Theme,
    commands: &CommandSender,
    form: &mut CategoryListForm,
    view: &LedgerView,
) {
    let Some(dialog) = &mut form.edit else {
        return;
    };
    match edit_category::show(ctx, i18n, theme, commands, dialog, view) {
        EditCategoryOutcome::Open => {}
        EditCategoryOutcome::Closed => {
            form.focus_return = form.edit.take().map(|dialog| dialog.close(ctx));
        }
    }
}
