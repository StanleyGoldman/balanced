// SPDX-License-Identifier: GPL-3.0-or-later WITH AdditionRef-Balanced-Plugin-Exception

//! The Category List: a heavy Dialog showing every category as a tree, with its type,
//! how many transactions and split lines use each and their total, and every account a
//! transaction can transfer to. Buttons beside the tree create and delete categories;
//! each change applies as soon as its confirm is accepted, as one undo step.

use std::collections::{BTreeMap, HashSet};

use balanced_core::{
    AccountId, Category, CategoryId, DomainCommand, DomainError, Generation, LedgerView, Usage,
};
use balanced_i18n::{AmountStyle, Localizer};
use eframe::egui::accesskit::{self, Role};
use eframe::egui::style::ScrollStyle;
use eframe::egui::{
    self, Align, Button, EventFilter, Id, Key, Layout, Modifiers, Rect, Response, RichText, Sense,
    TextStyle, TextWrapMode, WidgetInfo, WidgetText, WidgetType, pos2, vec2,
};
use egui_extras::{Column, Size, StripBuilder, TableBuilder, TableRow};
use icu_locale::{LanguageIdentifier, Locale};

use crate::command::{AppCommand, CommandSender};
use crate::dialog::{DialogOutcome, FocusReturn, HeavyDialog, LightDialog, OpenDialog};
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
    /// The index in the model of each row shown, top to bottom: every row but those
    /// under a folded category. `None` once the model or the folds change, so each
    /// index is in range of the model it was built from.
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
    /// The delete confirm, while it is open over the list.
    delete: Option<OpenDialog<DeleteConfirm>>,
    /// Where focus goes now that a Dialog over the list has closed, until a frame
    /// restores it.
    focus_return: Option<FocusReturn>,
}

/// The delete confirm's form: which unused category, with none under it, it deletes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct DeleteConfirm {
    category: CategoryId,
}

/// What a click in the button column asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ButtonAction {
    New,
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
    /// The Type cell's text: a category's type, or `None` for an account.
    category_type: Option<String>,
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
            let category_type = Some(type_label(i18n, category.category_type()));
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

    /// The index of each row shown while the categories in `folded` are folded.
    fn shown(&self, folded: &HashSet<CategoryId>) -> Vec<usize> {
        let mut shown = Vec::with_capacity(self.rows.len());
        // The depth of the folded category whose subcategories are being passed over.
        let mut passing: Option<usize> = None;
        for (index, row) in self.rows.iter().enumerate() {
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
}

impl ListRow {
    fn new(
        i18n: &Localizer,
        key: RowKey,
        depth: usize,
        name: String,
        has_children: bool,
        category_type: Option<String>,
        usage: &Usage,
    ) -> Self {
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
        let description = match &category_type {
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
    /// Hands the New Category Dialog open over the list the ledger's answer to the
    /// category it asked for: it closes once the category is created, and shows a
    /// refusal itself. Returns the answer when no Dialog over the list waits on it.
    pub(crate) fn applied(
        &mut self,
        ctx: &egui::Context,
        applied: Result<(), DomainError>,
    ) -> Option<Result<(), DomainError>> {
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
            let shown = model.shown(&self.folded);
            // A row a fold hides is no longer selected.
            let still_shown = |key| shown.iter().any(|&index| model.rows[index].key == key);
            self.selected = self.selected.filter(|&key| still_shown(key));
            shown
        });
        let mut asked = None;
        let status_height = ui.text_style_height(&TextStyle::Body);
        StripBuilder::new(ui)
            .size(Size::remainder())
            .size(Size::exact(status_height))
            .vertical(|mut strip| {
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
        match asked {
            None => {}
            Some(ButtonAction::New) => {
                let form = NewCategoryForm::new(String::new());
                self.new_category = Some(OpenDialog::new(ui.ctx(), form));
            }
            Some(ButtonAction::Delete(category)) => {
                self.delete = Some(OpenDialog::new(ui.ctx(), DeleteConfirm { category }));
            }
        }
        self.model = Some(model);
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
                        if let Some(category_type) = &row.category_type {
                            ui.label(category_type);
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

    /// The button column: New… always, and Delete… for a selected category nothing
    /// uses, with none under it. Edit…, Merge… and Hide wait on their own Dialogs.
    /// Returns what a click asked for.
    fn buttons(
        &self,
        ui: &mut egui::Ui,
        i18n: &Localizer,
        model: &ListModel,
    ) -> Option<ButtonAction> {
        // TODO(#488): Delete… a used category or a parent, with Reassign.
        let deletable = self
            .selected
            .and_then(|key| model.rows.iter().find(|row| row.key == key))
            .and_then(|row| match row.key {
                RowKey::Category(id) if !row.used && !row.has_children => Some(id),
                RowKey::Category(_) | RowKey::Transfer(_) => None,
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
            // TODO(#490): Edit… a category.
            button(fl!(i18n, "dialog-category-list-edit"), false);
            // TODO(#489): Merge… the selected categories.
            button(fl!(i18n, "dialog-category-list-merge"), false);
            // TODO(#491): Hide and Unhide a category.
            button(fl!(i18n, "dialog-category-list-hide"), false);
            if button(
                fl!(i18n, "dialog-category-list-delete"),
                deletable.is_some(),
            ) {
                asked = deletable.map(ButtonAction::Delete);
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
    let outcome = LightDialog::new(
        Id::new("category_list_delete"),
        fl!(i18n, "dialog-delete-category-delete"),
        fl!(i18n, "dialog-cancel"),
        theme,
    )
    .show(ctx, dialog, |ui, _confirm| {
        ui.label(fl!(i18n, "dialog-delete-category-message", name = name));
    });
    match outcome {
        DialogOutcome::Open => return,
        DialogOutcome::Accepted => {
            let command = DomainCommand::DeleteCategory { category };
            commands.send(AppCommand::Domain(command));
        }
        DialogOutcome::Cancelled => {}
        DialogOutcome::Back | DialogOutcome::Discarded => {
            unreachable!("a confirm has no Back or Discard button")
        }
    }
    form.focus_return = form.delete.take().map(|dialog| dialog.close(ctx));
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
