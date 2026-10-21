// SPDX-License-Identifier: GPL-3.0-or-later WITH AdditionRef-Balanced-Plugin-Exception

//! The Category List: a heavy Dialog showing every category as a tree, with how many
//! transactions and split lines use each and their total, and every account a
//! transaction can transfer to.

use std::collections::{BTreeMap, HashSet};

use balanced_core::{AccountId, Category, CategoryId, Generation, LedgerView, Usage};
use balanced_i18n::{AmountStyle, Localizer};
use eframe::egui::accesskit::{self, Role};
use eframe::egui::style::ScrollStyle;
use eframe::egui::{
    self, Align, EventFilter, Id, Key, Layout, Modifiers, Rect, Response, RichText, Sense,
    TextStyle, TextWrapMode, WidgetInfo, WidgetText, WidgetType, pos2, vec2,
};
use egui_extras::{Column, TableBuilder};
use icu_locale::Locale;

use crate::dialog::{DialogOutcome, HeavyDialog, OpenDialog};
use crate::formatted_amount::FormattedAmount;
use crate::i18n::fl;
use crate::register_model::transfer_text;
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
    /// under a folded category. `None` once the model or the folds change.
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
    /// The Txns cell's text.
    count: String,
    /// The Total cell's amount, or `None` when nothing uses the row or the total
    /// overflows.
    total: Option<FormattedAmount>,
    /// The count and total, as a screen reader reads them after the name.
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
    formatting_locale: Locale,
}

impl Source {
    fn of(view: &LedgerView, i18n: &Localizer) -> Self {
        Self {
            generation: view.generation(),
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
            let mut row = ListRow::new(i18n, key, depth, category.name().to_owned(), usage);
            row.has_children = under.is_some();
            rows.push(row);
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
            rows.push(ListRow::new(
                i18n,
                RowKey::Transfer(account.id()),
                0,
                name,
                usage,
            ));
        }
        Self {
            source: Source::of(view, i18n),
            rows,
        }
    }
}

impl ListRow {
    fn new(i18n: &Localizer, key: RowKey, depth: usize, name: String, usage: &Usage) -> Self {
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
        Self {
            key,
            depth,
            name,
            has_children: false,
            count: i18n.format_number(i64::try_from(count).unwrap_or(i64::MAX)),
            total,
            description,
        }
    }
}

impl ListModel {
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

impl CategoryListForm {
    /// Draws the tree and its usage, filling `ui`.
    fn show(&mut self, ui: &mut egui::Ui, i18n: &Localizer, theme: &Theme, view: &LedgerView) {
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
            .column(Column::exact(Theme::CATEGORY_LIST_COUNT_WIDTH))
            .column(Column::exact(Theme::CATEGORY_LIST_TOTAL_WIDTH));
        if let Some(offset) = self.scroll_to.take() {
            table = table.vertical_scroll_offset(offset);
        }
        let scrolled = table
            .header(row_height, |mut header| {
                header.col(|ui| {
                    ui.strong(fl!(i18n, "dialog-category-list-category"));
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
            })
            .body(|body| {
                body.rows(row_height, shown.len(), |mut table_row| {
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
            self.act(&model, action, pitch);
        }
        self.model = Some(model);
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
            TableAction::Down | TableAction::Up => {
                let Some(at) = self.selected.and_then(place) else {
                    return;
                };
                let to = match action {
                    TableAction::Down => (at + 1).min(shown.len() - 1),
                    TableAction::Up | TableAction::Select(_) | TableAction::Toggle(_) => {
                        at.saturating_sub(1)
                    }
                    TableAction::SetFold(..) => at,
                };
                self.select(model.rows[shown[to]].key);
                self.scroll_to = self.scrolled.and_then(|view| row_in_view(view, to, pitch));
            }
        }
    }

    /// Selects the row `key`, which takes keyboard focus once it is drawn.
    fn select(&mut self, key: RowKey) {
        self.selected = Some(key);
        self.focus_selected = true;
    }
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
        action = row_keys(ui, row).or(action);
    }
    if row.has_children
        && fold_triangle_button(ui, i18n, row, slot, state.fold).clicked()
        && let RowKey::Category(id) = row.key
    {
        action = Some(TableAction::Toggle(id));
    }
    action
}

/// The keys the focused name of `row` takes from egui's focus moves: ↑ and ↓ move the
/// selection, and ← and → fold and unfold a row with categories under it.
fn row_keys(ui: &egui::Ui, row: &ListRow) -> Option<TableAction> {
    let id = Id::new(("category_list_row", row.key));
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

/// Draws the Category List from `view`.
pub(crate) fn show(
    ctx: &egui::Context,
    i18n: &Localizer,
    theme: &Theme,
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
    match outcome {
        DialogOutcome::Open => CategoryListOutcome::Open,
        DialogOutcome::Accepted | DialogOutcome::Cancelled => CategoryListOutcome::Closed,
        DialogOutcome::Back | DialogOutcome::Discarded => {
            unreachable!("a heavy Dialog has no Back or Discard button")
        }
    }
}
