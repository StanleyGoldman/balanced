//! The Register: one account's transactions in a table, one row each, with the running
//! balance after each.

use std::collections::{BTreeMap, HashMap};

use balanced_core::{
    AccountId, AccountSummary, DomainCommand, DomainError, LedgerView, TransactionEdit,
    TransactionId,
};
use balanced_i18n::Localizer;
use eframe::egui::accesskit::{self, Role};
use eframe::egui::text::{CCursor, CCursorRange};
use eframe::egui::text_edit::TextEditState;
use eframe::egui::{
    self, Align, CursorIcon, Event, EventFilter, Id, Key, Layout, Margin, Modifiers, Popup, Rect,
    Response, ScrollArea, Sense, Shape, Stroke, StrokeKind, TextEdit, TextStyle, TextWrapMode,
    WidgetInfo, WidgetText, WidgetType, pos2, vec2,
};
use egui_extras::{Column, TableBody, TableBuilder};

use crate::command::{AppCommand, CommandSender, StatusMessage};
use crate::i18n::fl;
use crate::register_model::{RegisterModel, RegisterRow, RegisterSort, SortDirection, SortKey};
use crate::theme::Theme;

/// The narrowest Payee and Category get before the Register scrolls sideways.
const MIN_FLEX_WIDTH: f32 = 120.0;

/// The narrowest a resized column gets.
const MIN_COLUMN_WIDTH: f32 = 24.0;

/// The width of the handle inside a heading's right edge that resizes its column.
const RESIZE_HANDLE_WIDTH: f32 = 8.0;

/// The radius of each dot of the Actions menu's ⋯, and the space between their centres.
const ACTION_DOT_RADIUS: f32 = 1.5;
const ACTION_DOT_SPACING: f32 = 4.5;

/// The sort arrow beside the sorted column's heading: its width, its height, and the
/// space between it and the heading.
const SORT_ARROW_WIDTH: f32 = 7.0;
const SORT_ARROW_HEIGHT: f32 = 4.0;
const SORT_ARROW_GAP: f32 = 4.0;

/// A Register column, in the default left-to-right order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum RegisterColumn {
    Date,
    Number,
    Payee,
    Category,
    Memo,
    Amount,
    Balance,
    /// The untitled column holding each row's Actions menu.
    Actions,
}

impl RegisterColumn {
    /// Every column, in the order the Register shows them. A new column goes here as well
    /// as in the matches below, which the compiler checks; it can't check this list.
    const ALL: [Self; 8] = [
        Self::Date,
        Self::Number,
        Self::Payee,
        Self::Category,
        Self::Memo,
        Self::Amount,
        Self::Balance,
        Self::Actions,
    ];

    /// The column's stable id, which the saved layout names it by.
    fn id(self) -> &'static str {
        match self {
            Self::Date => "date",
            Self::Number => "num",
            Self::Payee => "payee",
            Self::Category => "category",
            Self::Memo => "memo",
            Self::Amount => "amount",
            Self::Balance => "balance",
            Self::Actions => "actions",
        }
    }

    /// The column's width before the user resizes it, for a column that resizes.
    /// Payee and Category share the width the others leave, and the Actions column
    /// fits its button, so neither resizes.
    fn default_width(self) -> Option<f32> {
        match self {
            Self::Date => Some(80.0),
            Self::Number => Some(48.0),
            Self::Memo => Some(140.0),
            Self::Amount => Some(90.0),
            Self::Balance => Some(100.0),
            Self::Payee | Self::Category | Self::Actions => None,
        }
    }

    /// The column's heading. The Actions column has none.
    fn title(self, i18n: &Localizer) -> String {
        match self {
            Self::Date => fl!(i18n, "register-column-date"),
            Self::Number => fl!(i18n, "register-column-number"),
            Self::Payee => fl!(i18n, "register-column-payee"),
            Self::Category => fl!(i18n, "register-column-category"),
            Self::Memo => fl!(i18n, "register-column-memo"),
            Self::Amount => fl!(i18n, "register-column-amount"),
            Self::Balance => fl!(i18n, "register-column-balance"),
            Self::Actions => String::new(),
        }
    }

    /// What clicking the column's heading sorts by. Number, Balance and Actions don't
    /// sort.
    fn sort_key(self) -> Option<SortKey> {
        match self {
            Self::Date => Some(SortKey::Date),
            Self::Payee => Some(SortKey::Payee),
            Self::Category => Some(SortKey::Category),
            Self::Memo => Some(SortKey::Memo),
            Self::Amount => Some(SortKey::Amount),
            Self::Number | Self::Balance | Self::Actions => None,
        }
    }

    /// How wide the column is: as `widths` says for a column that resizes. Payee and
    /// Category share the width the others leave, down to [`MIN_FLEX_WIDTH`]; below
    /// that the Register scrolls sideways.
    fn width(self, theme: &Theme, widths: &ColumnWidths) -> Column {
        let column = match self {
            Self::Payee | Self::Category => Column::remainder().at_least(MIN_FLEX_WIDTH),
            Self::Actions => Column::exact(theme.row_height() + Theme::EDGE_INSET),
            Self::Date | Self::Number | Self::Memo | Self::Amount | Self::Balance => {
                Column::exact(widths.width(self).unwrap_or(MIN_COLUMN_WIDTH))
            }
        };
        column.clip(true)
    }

    /// Whether the cell cursor can rest in the column. Balance is worked out from the
    /// other cells, and Actions holds a menu.
    fn holds_cursor(self) -> bool {
        match self {
            Self::Date
            | Self::Number
            | Self::Payee
            | Self::Category
            | Self::Memo
            | Self::Amount => true,
            Self::Balance | Self::Actions => false,
        }
    }

    /// The field the column's cell on `row` edits as text, or `None` when the cell
    /// can't be edited as text. The opening balance's Payee cell shows a caption, not a
    /// payee.
    fn text_field(self, row: &RegisterRow) -> Option<TextField> {
        match self {
            Self::Number => Some(TextField::Number),
            Self::Payee if !row.is_opening_balance() => Some(TextField::Payee),
            Self::Memo => Some(TextField::Memo),
            // TODO(#91): edit Date in the segmented date field.
            // TODO(#92): edit Payee and Category in their comboboxes.
            // TODO(#90): edit Amount in the amount field.
            Self::Payee
            | Self::Date
            | Self::Category
            | Self::Amount
            | Self::Balance
            | Self::Actions => None,
        }
    }

    /// Where the column's text sits: amounts on the right, the rest on the left.
    fn align(self) -> Align {
        match self {
            Self::Amount | Self::Balance => Align::Max,
            Self::Date
            | Self::Number
            | Self::Payee
            | Self::Category
            | Self::Memo
            | Self::Actions => Align::Min,
        }
    }

    /// Space between the cell's edges and its text: [`Theme::EDGE_INSET`] on the
    /// Register's outer edges, none elsewhere.
    fn insets(self) -> Insets {
        let edge = Theme::EDGE_INSET;
        match self {
            Self::Date => Insets {
                left: edge,
                right: 0.0,
            },
            Self::Actions => Insets {
                left: 0.0,
                right: edge,
            },
            Self::Number
            | Self::Payee
            | Self::Category
            | Self::Memo
            | Self::Amount
            | Self::Balance => Insets {
                left: 0.0,
                right: 0.0,
            },
        }
    }
}

/// The widths of the Register columns that resize, one set shared by every Register.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ColumnWidths {
    widths: HashMap<RegisterColumn, f32>,
}

impl Default for ColumnWidths {
    fn default() -> Self {
        Self::from_saved(&BTreeMap::new())
    }
}

impl ColumnWidths {
    /// The widths `saved` holds under column ids, raised to [`MIN_COLUMN_WIDTH`]. A
    /// column `saved` leaves out takes its default width, and an unknown id is ignored.
    pub(crate) fn from_saved(saved: &BTreeMap<String, f32>) -> Self {
        let widths = RegisterColumn::ALL
            .into_iter()
            .filter_map(|column| {
                let default = column.default_width()?;
                let width = saved
                    .get(column.id())
                    .copied()
                    .filter(|width| width.is_finite())
                    .map_or(default, |width| width.max(MIN_COLUMN_WIDTH));
                Some((column, width))
            })
            .collect();
        Self { widths }
    }

    /// Every width under its column's id, for saving.
    pub(crate) fn to_saved(&self) -> BTreeMap<String, f32> {
        self.widths
            .iter()
            .map(|(column, width)| (column.id().to_owned(), *width))
            .collect()
    }

    /// The width of `column`, unless it doesn't resize.
    fn width(&self, column: RegisterColumn) -> Option<f32> {
        self.widths.get(&column).copied()
    }

    /// The handle on the right edge of `column`'s heading, in the heading cell `ui`,
    /// for a column that resizes. Dragging it resizes the column from the next frame.
    fn resize_handle(&mut self, ui: &egui::Ui, i18n: &Localizer, column: RegisterColumn) {
        let Some(width) = self.widths.get_mut(&column) else {
            return;
        };
        // The cell clips what it holds, so the handle stays inside its right edge.
        let cell = ui.max_rect();
        let edge = cell.right();
        let handle = Rect::from_x_y_ranges(edge - RESIZE_HANDLE_WIDTH..=edge, cell.y_range());
        let id = ui.id().with("register_resize");
        let response = ui
            .interact(handle, id, Sense::drag())
            .on_hover_cursor(CursorIcon::ResizeColumn);
        let label = fl!(i18n, "register-resize-column", column = column.title(i18n));
        response.widget_info(|| WidgetInfo::labeled(WidgetType::Other, ui.is_enabled(), &label));
        ui.ctx()
            .accesskit_node_builder(response.id, |node| node.set_role(Role::Splitter));
        if response.dragged() {
            *width = (*width + response.drag_delta().x).max(MIN_COLUMN_WIDTH);
            ui.ctx().request_repaint();
        }
        if response.hovered() || response.dragged() {
            let stroke = ui.style().interact(&response).fg_stroke;
            ui.ctx()
                .layer_painter(ui.layer_id())
                .vline(edge, cell.y_range(), stroke);
        }
    }
}

/// Space between a cell's left and right edges and its text, in points.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Insets {
    left: f32,
    right: f32,
}

/// A transaction field a Register cell edits as plain text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TextField {
    Number,
    Payee,
    Memo,
}

impl TextField {
    /// The field's saved text on `row`.
    fn saved(self, row: &RegisterRow) -> &str {
        match self {
            Self::Number => row.number(),
            Self::Payee => row.payee(),
            Self::Memo => row.memo(),
        }
    }

    /// The edit that saves `text`, already trimmed, in the field. An empty number
    /// clears it.
    fn edit(self, text: &str) -> TransactionEdit {
        match self {
            Self::Number => TransactionEdit::Number((!text.is_empty()).then(|| text.to_owned())),
            Self::Payee => TransactionEdit::Payee(text.to_owned()),
            Self::Memo => TransactionEdit::Memo(text.to_owned()),
        }
    }
}

/// One cell of the Register: a transaction's row and a column.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct CellAddress {
    row: TransactionId,
    column: RegisterColumn,
}

/// Where the cell cursor goes when a key moves it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Move {
    Up,
    Down,
    Left,
    Right,
    /// Right, wrapping to the first cell of the next row.
    Next,
    /// To the next cell that can be edited, wrapping from row to row.
    NextEditable,
    /// To the previous cell that can be edited, wrapping from row to row.
    PreviousEditable,
}

/// The cell being edited and its unsaved text.
#[derive(Debug, Clone, PartialEq, Eq)]
struct CellEdit {
    cell: CellAddress,
    text: String,
}

impl CellEdit {
    /// The id of the cell's `TextEdit`.
    fn editor_id(&self) -> Id {
        Id::new(("register_editor", self.cell))
    }
}

/// The rows fully in view in the last frame, which keyboard scrolling keeps the cell
/// cursor among.
#[derive(Debug, Clone, Copy, PartialEq)]
struct RowsInView {
    first: usize,
    last: usize,
    /// The height of the Register's body.
    height: f32,
    /// The distance from one row's top to the next one's.
    pitch: f32,
}

impl RowsInView {
    /// The scroll offset that brings row `index` fully into view, or `None` when it
    /// already is.
    #[expect(
        clippy::cast_precision_loss,
        reason = "row indices stay far below 2^24, where f32 stops counting whole numbers"
    )]
    fn offset_showing(self, index: usize) -> Option<f32> {
        if index < self.first {
            Some(index as f32 * self.pitch)
        } else if index > self.last {
            Some(((index + 1) as f32 * self.pitch - self.height).max(0.0))
        } else {
            None
        }
    }
}

/// The open account's Register: its cached rows, its sort, the cell cursor, and the
/// cell being edited. The cursor's row is the selected row. Opening another account
/// sorts it by date and clears the cursor. None of it outlives the run.
#[derive(Debug, Default)]
pub(crate) struct Register {
    /// The rows of the last account shown, rebuilt when they are out of date.
    model: Option<RegisterModel>,
    sort: RegisterSort,
    /// Whether the sort changed since the rows were last shown, so the cursor's row,
    /// if any, scrolls to the middle of the Register.
    centre_pending: bool,
    cursor: Option<CellAddress>,
    edit: Option<CellEdit>,
    /// The modifiers held with each Tab press `App::raw_input_hook` took for the
    /// Register since the last frame.
    tabs: Vec<Modifiers>,
    rows_in_view: Option<RowsInView>,
}

impl Register {
    /// Whether keys go to the Register: the cell cursor is in it, and nothing holds
    /// keyboard focus but the Register's own cell editor.
    pub(crate) fn owns_keyboard(&self, ctx: &egui::Context) -> bool {
        self.cursor.is_some()
            && ctx.memory(|memory| match memory.focused() {
                None => true,
                Some(focused) => self
                    .edit
                    .as_ref()
                    .is_some_and(|edit| edit.editor_id() == focused),
            })
    }

    /// Queues a Tab press, with `modifiers` held, for the next frame.
    pub(crate) fn press_tab(&mut self, modifiers: Modifiers) {
        self.tabs.push(modifiers);
    }

    /// Draws the Register of `summary`, an account in `view`, filling the space left in
    /// `ui`. Only the rows in view are laid out.
    pub(crate) fn show(
        &mut self,
        ui: &mut egui::Ui,
        i18n: &Localizer,
        theme: &Theme,
        commands: &CommandSender,
        summary: &AccountSummary,
        view: &LedgerView,
        widths: &mut ColumnWidths,
    ) {
        let account = summary.account().id();
        if self
            .model
            .as_ref()
            .is_some_and(|model| model.account() != account)
        {
            self.sort = RegisterSort::default();
            self.centre_pending = false;
            self.cursor = None;
            self.edit = None;
            self.rows_in_view = None;
        }
        let model = match self.model.take() {
            Some(model) if model.is_current(account, view, self.sort, i18n) => model,
            _ => RegisterModel::new(summary, view, self.sort, i18n),
        };
        let keys = KeyContext {
            account,
            closed: summary.account().is_closed(),
            rows: model.rows(),
            commands,
        };
        let key_scroll_offset = self.handle_keys(ui.ctx(), &keys);
        let row_height = theme.row_height();
        let pitch = row_height + ui.spacing().item_spacing.y;
        let centre_pending = std::mem::take(&mut self.centre_pending);
        let centre_on = self
            .cursor
            .filter(|_| centre_pending)
            .and_then(|cursor| model.rows().iter().position(|row| row.id() == cursor.row));
        ScrollArea::horizontal()
            .id_salt(("register_sideways", account))
            .auto_shrink(false)
            .show(ui, |ui| {
                let viewport = ui.clip_rect();
                let register_id = ui.id();
                let scroll_offset = key_scroll_offset
                    .or_else(|| centre_on.map(|index| centring_offset(ui, row_height, index)));
                let mut table = TableBuilder::new(ui)
                    .id_salt(("register", account))
                    .striped(theme.alternating_rows())
                    .resizable(false)
                    .cell_layout(Layout::left_to_right(Align::Center))
                    .auto_shrink(false)
                    .max_scroll_height(f32::INFINITY);
                for column in RegisterColumn::ALL {
                    table = table.column(column.width(theme, widths));
                }
                if let Some(offset) = scroll_offset {
                    table = table.vertical_scroll_offset(offset);
                }
                table
                    .header(row_height, |mut header| {
                        for column in RegisterColumn::ALL {
                            header.col(|ui| {
                                let heading = Heading {
                                    register_id,
                                    column,
                                    sort: self.sort,
                                    viewport,
                                };
                                if let Some(key) = heading.show(ui, i18n, theme) {
                                    self.sort = self.sort.clicked(key);
                                    self.centre_pending = true;
                                    // The rows sort and scroll on the next frame.
                                    ui.ctx().request_repaint();
                                }
                                widths.resize_handle(ui, i18n, column);
                            });
                        }
                    })
                    .body(|body| {
                        let frame = BodyFrame {
                            i18n,
                            theme,
                            commands,
                            rows: model.rows(),
                            register_id,
                            viewport: Rect::NOTHING,
                        };
                        self.body(body, frame, row_height, pitch);
                    });
            });
        self.model = Some(model);
    }

    /// Lays out the rows in view of the Register's body, and notes which of them are
    /// fully in view for keyboard scrolling. `frame`'s viewport is filled in here.
    fn body(&mut self, mut body: TableBody<'_>, frame: BodyFrame<'_>, row_height: f32, pitch: f32) {
        let viewport = body.ui_mut().clip_rect();
        let frame = BodyFrame { viewport, ..frame };
        let rows = frame.rows;
        let mut in_view: Option<(usize, usize)> = None;
        body.rows(row_height, rows.len(), |mut table_row| {
            let index = table_row.index();
            // `rows` lays out indices below `rows.len()` only.
            let row = &rows[index];
            table_row.set_selected(self.cursor.is_some_and(|cursor| cursor.row == row.id()));
            for (column_index, column) in RegisterColumn::ALL.into_iter().enumerate() {
                table_row.col(|ui| {
                    if column_index == 0 && fully_in(viewport, ui.max_rect()) {
                        in_view = Some(in_view.map_or((index, index), |(first, _)| (first, index)));
                    }
                    self.body_cell(ui, &frame, row, column_index, column);
                });
            }
        });
        self.rows_in_view = in_view.map(|(first, last)| RowsInView {
            first,
            last,
            height: viewport.height(),
            pitch,
        });
    }

    /// Draws one body cell, `column` (the `column_index`th) of `row`: its text, or its
    /// editor while it is edited, and its gridlines.
    fn body_cell(
        &mut self,
        ui: &mut egui::Ui,
        frame: &BodyFrame<'_>,
        row: &RegisterRow,
        column_index: usize,
        column: RegisterColumn,
    ) {
        let BodyFrame {
            i18n,
            theme,
            commands,
            rows,
            register_id,
            viewport,
        } = *frame;
        let address = CellAddress {
            row: row.id(),
            column,
        };
        let cell = Cell {
            register_id,
            row,
            column,
            selected: self.cursor.is_some_and(|cursor| cursor.row == row.id()),
            cursor: self.cursor == Some(address),
            viewport,
        };
        if cell.show(ui, i18n, theme).clicked() {
            self.place_cursor(row.id(), column);
        }
        if let Some(edit) = self.edit.as_mut().filter(|edit| edit.cell == address)
            && editor(ui, edit, heading_id(register_id, column)).lost_focus()
        {
            let edit = self.edit.take().expect("the edit was just shown");
            commit(&edit, rows, commands);
        }
        let rules = theme.cell_rules(ui.max_rect(), column_index, ui.ctx().pixels_per_point());
        ui.ctx()
            .layer_painter(ui.layer_id())
            .with_clip_rect(theme.rule_clip(ui.clip_rect(), viewport))
            .extend(rules);
    }

    /// Puts the cell cursor on a click in `column` of transaction `row`. A click in a
    /// column the cursor can't rest in keeps the cursor's column.
    fn place_cursor(&mut self, row: TransactionId, column: RegisterColumn) {
        let column = if column.holds_cursor() {
            column
        } else {
            self.cursor
                .map_or(RegisterColumn::Date, |cursor| cursor.column)
        };
        self.cursor = Some(CellAddress { row, column });
    }

    /// Acts on this frame's keys while the Register owns the keyboard, and returns
    /// the scroll offset that brings the cell cursor into view when it has left it.
    ///
    /// Enter, Tab and Esc are taken before the cell editor is added, which would
    /// otherwise act on them first.
    fn handle_keys(&mut self, ctx: &egui::Context, keys: &KeyContext<'_>) -> Option<f32> {
        if !self.owns_keyboard(ctx) {
            self.tabs.clear();
            return None;
        }
        let cursor = self.cursor?;
        let tab = (!self.tabs.is_empty()).then(|| self.tabs.remove(0));
        if !self.tabs.is_empty() {
            // Each Tab acts on the cell the one before it opened, so the rest wait.
            ctx.request_repaint();
        }
        let tab = tab.map(|modifiers| {
            if modifiers.shift {
                Move::PreviousEditable
            } else {
                Move::NextEditable
            }
        });
        if let Some(edit) = self.edit.take() {
            let (escape, enter) = ctx.input_mut(|input| {
                (
                    input.consume_key(Modifiers::NONE, Key::Escape),
                    input.consume_key(Modifiers::NONE, Key::Enter),
                )
            });
            if escape {
                ctx.memory_mut(|memory| memory.surrender_focus(edit.editor_id()));
                return None;
            }
            if enter || tab.is_some() {
                commit(&edit, keys.rows, keys.commands);
                ctx.memory_mut(|memory| memory.surrender_focus(edit.editor_id()));
                let step = if enter { Move::Next } else { tab? };
                let moved = self.move_cursor(cursor, step, keys.rows)?;
                // Tab opens the cell it moved to. With nowhere to move it opens nothing,
                // since this frame's rows don't hold the text just committed yet.
                if !enter {
                    self.start_edit(ctx, keys, EditStart::Saved);
                }
                return moved.scroll_offset;
            }
            self.edit = Some(edit);
            return None;
        }
        let pressed = ctx.input_mut(|input| {
            [
                (Key::ArrowUp, Move::Up),
                (Key::ArrowDown, Move::Down),
                (Key::ArrowLeft, Move::Left),
                (Key::ArrowRight, Move::Right),
            ]
            .into_iter()
            .find(|&(key, _)| input.consume_key(Modifiers::NONE, key))
            .map(|(_, step)| step)
        });
        if let Some(step) = pressed.or(tab) {
            let moved = self.move_cursor(cursor, step, keys.rows)?;
            if tab.is_some() {
                self.start_edit(ctx, keys, EditStart::Saved);
            }
            return moved.scroll_offset;
        }
        if ctx.input_mut(|input| input.consume_key(Modifiers::NONE, Key::Enter)) {
            if field_at(keys.rows, cursor).is_some() {
                self.start_edit(ctx, keys, EditStart::Saved);
                return None;
            }
            return self
                .move_cursor(cursor, Move::Next, keys.rows)?
                .scroll_offset;
        }
        let typed = ctx.input(|input| {
            input
                .events
                .iter()
                .any(|event| matches!(event, Event::Text(text) if !text.is_empty()))
        });
        if typed {
            // The editor takes the typed text in this same frame.
            self.start_edit(ctx, keys, EditStart::Empty);
        }
        None
    }

    /// Moves the cell cursor from `cursor` by `step`. Returns `None`, leaving the
    /// cursor where it is, when there is nowhere to go.
    fn move_cursor(
        &mut self,
        cursor: CellAddress,
        step: Move,
        rows: &[RegisterRow],
    ) -> Option<Moved> {
        let columns: Vec<RegisterColumn> = RegisterColumn::ALL
            .into_iter()
            .filter(|column| column.holds_cursor())
            .collect();
        let row = rows.iter().position(|row| row.id() == cursor.row)?;
        let column = columns.iter().position(|&column| column == cursor.column)?;
        let (row, column) = match step {
            Move::Up => (row.checked_sub(1)?, column),
            Move::Down => (row + 1, column),
            Move::Left => (row, column.checked_sub(1)?),
            Move::Right => (row, column + 1),
            Move::Next => {
                if column + 1 < columns.len() {
                    (row, column + 1)
                } else {
                    // TODO(#93): Enter on the last row's last cell goes to the draft row.
                    (row + 1, 0)
                }
            }
            Move::NextEditable | Move::PreviousEditable => {
                let width = columns.len();
                let start = row * width + column;
                let editable = |position: usize| {
                    let row = rows.get(position / width)?;
                    // The remainder of a division by the width is a column index.
                    columns[position % width].text_field(row)
                };
                let position = if step == Move::NextEditable {
                    (start + 1..rows.len() * width)
                        .find(|&position| editable(position).is_some())?
                } else {
                    (0..start)
                        .rev()
                        .find(|&position| editable(position).is_some())?
                };
                (position / width, position % width)
            }
        };
        let address = CellAddress {
            row: rows.get(row)?.id(),
            column: *columns.get(column)?,
        };
        self.cursor = Some(address);
        Some(Moved {
            scroll_offset: self
                .rows_in_view
                .and_then(|rows_in_view| rows_in_view.offset_showing(row)),
        })
    }

    /// Opens the cell under the cursor for editing, starting from `start`, when it can
    /// be edited as text. A closed account's cells can't be: the Status bar says so.
    fn start_edit(&mut self, ctx: &egui::Context, keys: &KeyContext<'_>, start: EditStart) {
        let Some(cursor) = self.cursor else {
            return;
        };
        let Some((row, field)) = field_at(keys.rows, cursor) else {
            return;
        };
        if keys.closed {
            let error = DomainError::AccountClosed {
                account: keys.account,
            };
            keys.commands
                .send(AppCommand::ShowStatus(StatusMessage::Rejected(error)));
            return;
        }
        let text = match start {
            EditStart::Saved => field.saved(row).to_owned(),
            EditStart::Empty => String::new(),
        };
        let edit = CellEdit { cell: cursor, text };
        let id = edit.editor_id();
        // The text cursor starts at the end, wherever an earlier edit left it.
        let mut state = TextEditState::load(ctx, id).unwrap_or_default();
        state
            .cursor
            .set_char_range(Some(CCursorRange::one(CCursor::new(
                edit.text.chars().count(),
            ))));
        state.store(ctx, id);
        ctx.memory_mut(|memory| memory.request_focus(id));
        self.edit = Some(edit);
    }
}

/// A move of the cell cursor that happened.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Moved {
    /// The scroll offset that brings the cursor into view, when it has left it.
    scroll_offset: Option<f32>,
}

/// The row of `cell` and the field it edits as text, or `None` when its row is gone or
/// it can't be edited as text.
fn field_at(rows: &[RegisterRow], cell: CellAddress) -> Option<(&RegisterRow, TextField)> {
    let row = rows.iter().find(|row| row.id() == cell.row)?;
    Some((row, cell.column.text_field(row)?))
}

/// What a cell edit starts from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EditStart {
    /// The cell's saved text, as Enter and Tab open it.
    Saved,
    /// Nothing, as typing opens it: the editor takes the typed text.
    Empty,
}

/// What every body cell is drawn with in one frame.
#[derive(Clone, Copy)]
struct BodyFrame<'a> {
    i18n: &'a Localizer,
    theme: &'a Theme,
    commands: &'a CommandSender,
    rows: &'a [RegisterRow],
    register_id: Id,
    /// The visible part of the Register's body.
    viewport: Rect,
}

/// Whether `rect` lies wholly within `viewport` from top to bottom, give or take half a
/// point.
fn fully_in(viewport: Rect, rect: Rect) -> bool {
    let view = viewport.y_range().expand(0.5);
    view.contains(rect.top()) && view.contains(rect.bottom())
}

/// What the Register's keys act on.
struct KeyContext<'a> {
    account: AccountId,
    closed: bool,
    rows: &'a [RegisterRow],
    commands: &'a CommandSender,
}

/// Saves `edit`'s text, trimmed, by sending one edit Command, unless it matches the
/// saved text.
fn commit(edit: &CellEdit, rows: &[RegisterRow], commands: &CommandSender) {
    let Some((row, field)) = field_at(rows, edit.cell) else {
        return;
    };
    let text = edit.text.trim();
    if text != field.saved(row) {
        commands.send(AppCommand::Domain(DomainCommand::EditTransaction {
            transaction: row.id(),
            edit: field.edit(text),
        }));
    }
}

/// The id of `column`'s heading, which labels the cell editor in that column.
fn heading_id(register_id: Id, column: RegisterColumn) -> Id {
    register_id.with(("register_heading", column))
}

/// The `TextEdit` over the cell being edited. It keeps Esc, so the Register can cancel
/// the edit before egui drops the field's focus.
fn editor(ui: &mut egui::Ui, edit: &mut CellEdit, heading: Id) -> Response {
    let rect = ui.max_rect();
    let Insets { left, right } = edit.cell.column.insets();
    let inner = Rect::from_x_y_ranges(rect.left() + left..=rect.right() - right, rect.y_range());
    let id = edit.editor_id();
    ui.put(
        inner,
        TextEdit::singleline(&mut edit.text)
            .id(id)
            .margin(Margin::ZERO)
            .desired_width(inner.width())
            .event_filter(EventFilter {
                escape: true,
                ..Default::default()
            }),
    )
    .labelled_by(heading)
}

/// The scroll offset that puts row `index` in the middle of a Register filling `ui`,
/// or as near as the top allows.
fn centring_offset(ui: &egui::Ui, row_height: f32, index: usize) -> f32 {
    // The body is what the header row leaves, and each row takes its height and one gap.
    let gap = ui.spacing().item_spacing.y;
    let body = ui.available_height() - row_height - gap;
    #[expect(
        clippy::cast_precision_loss,
        reason = "row indices stay far below 2^24, where f32 is exact"
    )]
    let top = index as f32 * (row_height + gap);
    (top - (body - row_height) / 2.0).max(0.0)
}

/// A column heading on the header band.
struct Heading {
    /// The Register's id, which the heading's id derives from.
    register_id: Id,
    column: RegisterColumn,
    /// The Register's sort, which puts an arrow on the sorted column's heading.
    sort: RegisterSort,
    /// The visible part of the Register.
    viewport: Rect,
}

impl Heading {
    /// Draws the heading. The band's fill covers the cell and half the gap around it,
    /// clipped to the Register's viewport so it never paints past it. A sortable heading
    /// highlights under the pointer, and returns what it sorts by when clicked.
    fn show(&self, ui: &egui::Ui, i18n: &Localizer, theme: &Theme) -> Option<SortKey> {
        let Self {
            register_id,
            column,
            sort,
            viewport,
        } = *self;
        let rect = ui.max_rect();
        let key = column.sort_key();
        let id = heading_id(register_id, column);
        let mut fill = theme.header_fill();
        let response = match key {
            Some(_) => {
                let response = ui.interact(rect, id, Sense::click());
                if response.hovered() {
                    fill = ui.visuals().widgets.hovered.weak_bg_fill;
                }
                response.on_hover_cursor(CursorIcon::PointingHand)
            }
            None => ui.interact(rect, id, Sense::hover()),
        };
        let band = rect.expand2(ui.spacing().item_spacing / 2.0);
        ui.ctx()
            .layer_painter(ui.layer_id())
            .with_clip_rect(band.intersect(viewport))
            .rect_filled(band, 0.0, fill);
        let direction = key
            .filter(|&key| key == sort.key())
            .map(|_| sort.direction());
        let title = column.title(i18n);
        response.widget_info(|| WidgetInfo::labeled(WidgetType::Other, ui.is_enabled(), &title));
        ui.ctx().accesskit_node_builder(response.id, |node| {
            node.set_role(Role::ColumnHeader);
            if let Some(direction) = direction {
                node.set_sort_direction(direction.into());
            }
        });
        let text = paint_text(
            ui,
            rect,
            column,
            WidgetText::from(egui::RichText::new(title).strong()),
        );
        if let (Some(direction), Some(text)) = (direction, text) {
            sort_arrow(ui, column, text, direction);
        }
        key.filter(|_| response.clicked())
    }
}

impl From<SortDirection> for accesskit::SortDirection {
    fn from(direction: SortDirection) -> Self {
        match direction {
            SortDirection::Ascending => Self::Ascending,
            SortDirection::Descending => Self::Descending,
        }
    }
}

/// Paints the sort arrow beside a heading whose text is at `text`: after it, or before
/// it in a right-aligned column. It points up when the sort ascends.
fn sort_arrow(ui: &egui::Ui, column: RegisterColumn, text: Rect, direction: SortDirection) {
    let x = match column.align() {
        Align::Max => text.left() - SORT_ARROW_GAP - SORT_ARROW_WIDTH / 2.0,
        Align::Min | Align::Center => text.right() + SORT_ARROW_GAP + SORT_ARROW_WIDTH / 2.0,
    };
    let centre = pos2(x, text.center().y);
    let (half_width, half_height) = (SORT_ARROW_WIDTH / 2.0, SORT_ARROW_HEIGHT / 2.0);
    let (tip, base) = match direction {
        SortDirection::Ascending => (-half_height, half_height),
        SortDirection::Descending => (half_height, -half_height),
    };
    ui.painter().add(Shape::convex_polygon(
        vec![
            centre + vec2(-half_width, base),
            centre + vec2(half_width, base),
            centre + vec2(0.0, tip),
        ],
        ui.visuals().text_color(),
        Stroke::NONE,
    ));
}

/// One cell of one transaction's row.
struct Cell<'a> {
    /// The Register's id, which every cell's id derives from with its transaction's id.
    register_id: Id,
    row: &'a RegisterRow,
    column: RegisterColumn,
    selected: bool,
    /// Whether the cell cursor is on this cell.
    cursor: bool,
    /// The visible part of the Register's body.
    viewport: Rect,
}

impl Cell<'_> {
    /// Draws the cell, the whole of which is one click target.
    fn show(&self, ui: &egui::Ui, i18n: &Localizer, theme: &Theme) -> Response {
        let Self {
            register_id,
            row,
            column,
            selected,
            cursor,
            viewport,
        } = *self;
        let rect = ui.max_rect();
        let id = register_id.with(("register_cell", row.id(), column));
        let response = ui.interact(rect, id, Sense::click());
        let text: WidgetText = match column {
            RegisterColumn::Date => row.date().into(),
            RegisterColumn::Number => row.number().into(),
            RegisterColumn::Payee => row.payee().into(),
            RegisterColumn::Category => row.category().into(),
            RegisterColumn::Memo => row.memo().into(),
            RegisterColumn::Amount => row.amount().styled(theme).into(),
            RegisterColumn::Balance => row
                .balance()
                .map_or_else(WidgetText::default, |balance| balance.styled(theme).into()),
            RegisterColumn::Actions => {
                // The row's band across the whole Register, for telling whether the
                // pointer is on this row.
                let band = Rect::from_x_y_ranges(
                    viewport.x_range(),
                    rect.y_range().expand(ui.spacing().item_spacing.y / 2.0),
                );
                let hovered = ui
                    .ctx()
                    .pointer_hover_pos()
                    .is_some_and(|pointer| band.contains(pointer) && viewport.contains(pointer));
                let open = Popup::is_id_open(ui.ctx(), Popup::default_response_id(&response));
                if selected || hovered || open {
                    actions_menu(ui, i18n, &response);
                } else {
                    // Hidden, the menu leaves an empty cell that selects the row.
                    cell_info(ui, &response, selected, "");
                }
                return response;
            }
        };
        let label = text.text().to_owned();
        response.widget_info(|| {
            WidgetInfo::selected(
                WidgetType::SelectableLabel,
                ui.is_enabled(),
                selected,
                &label,
            )
        });
        ui.ctx().accesskit_node_builder(response.id, |node| {
            node.set_role(Role::Cell);
            if cursor {
                node.set_selected(true);
            }
        });
        paint_text(ui, rect, column, text);
        if cursor {
            ui.painter()
                .rect_stroke(rect, 0.0, theme.cell_cursor_stroke(), StrokeKind::Inside);
        }
        response
    }
}

/// Reports a cell to AccessKit as a table cell labelled `label`, toggled when its row is
/// selected.
fn cell_info(ui: &egui::Ui, response: &Response, selected: bool, label: &str) {
    response.widget_info(|| {
        WidgetInfo::selected(
            WidgetType::SelectableLabel,
            ui.is_enabled(),
            selected,
            label,
        )
    });
    ui.ctx()
        .accesskit_node_builder(response.id, |node| node.set_role(Role::Cell));
}

/// Paints `text` in `rect`, inset and aligned as `column` says, truncated to fit.
/// Returns where the text went, unless `rect` is out of view.
fn paint_text(ui: &egui::Ui, rect: Rect, column: RegisterColumn, text: WidgetText) -> Option<Rect> {
    if !ui.is_rect_visible(rect) {
        return None;
    }
    let Insets { left, right } = column.insets();
    let inner = Rect::from_x_y_ranges(rect.left() + left..=rect.right() - right, rect.y_range());
    let galley = text.into_galley(
        ui,
        Some(TextWrapMode::Truncate),
        inner.width().max(0.0),
        TextStyle::Body,
    );
    let x = match column.align() {
        Align::Max => inner.right() - galley.size().x,
        Align::Min | Align::Center => inner.left(),
    };
    let top = rect.center().y - galley.size().y / 2.0;
    let painted = Rect::from_min_size(pos2(x, top), galley.size());
    ui.painter()
        .galley(painted.min, galley, ui.visuals().text_color());
    Some(painted)
}

/// The row's Actions menu: a ⋯ button on the right of the Actions cell, which opens
/// the menu below it. The default fonts lack ⋯, so its dots are painted.
fn actions_menu(ui: &egui::Ui, i18n: &Localizer, response: &Response) {
    let label = fl!(i18n, "register-actions");
    response.widget_info(|| WidgetInfo::labeled(WidgetType::Button, ui.is_enabled(), &label));
    let rect = response.rect;
    if ui.is_rect_visible(rect) {
        let size = rect.height();
        let button = Rect::from_center_size(
            pos2(
                rect.right() - Theme::EDGE_INSET - size / 2.0,
                rect.center().y,
            ),
            vec2(size, size),
        );
        let visuals = ui.style().interact(response);
        if response.hovered() || response.is_pointer_button_down_on() {
            ui.painter().rect_filled(button, 0.0, visuals.weak_bg_fill);
        }
        for offset in [-ACTION_DOT_SPACING, 0.0, ACTION_DOT_SPACING] {
            ui.painter().circle_filled(
                button.center() + vec2(offset, 0.0),
                ACTION_DOT_RADIUS,
                visuals.text_color(),
            );
        }
    }
    // TODO(#94): add the row's commands, starting with Delete….
    Popup::menu(response).show(|_ui| {});
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::mpsc::Receiver;

    use balanced_core::Ledger;
    use egui_kittest::Harness;
    use egui_kittest::kittest::Queryable as _;

    use super::*;

    /// A Register showing Everyday Checking, with the Commands it sends.
    struct Shown {
        register: Register,
        i18n: Localizer,
        theme: Theme,
        commands: CommandSender,
        view: Arc<LedgerView>,
        widths: ColumnWidths,
    }

    impl Shown {
        fn everyday(&self) -> &AccountSummary {
            self.view
                .accounts()
                .iter()
                .find(|summary| summary.account().name() == "Everyday Checking")
                .expect("the stub has Everyday Checking")
        }

        /// The id of Everyday Checking's transaction with `payee`.
        fn transaction(&self, payee: &str) -> TransactionId {
            self.everyday()
                .register()
                .iter()
                .find(|entry| entry.transaction().payee() == payee)
                .expect("Everyday Checking has the payee")
                .transaction()
                .id()
        }
    }

    fn register() -> (Harness<'static, Shown>, Receiver<AppCommand>) {
        let (commands, receiver) = CommandSender::new(egui::Context::default());
        let shown = Shown {
            register: Register::default(),
            i18n: Localizer::for_tests(),
            theme: Theme::default(),
            commands,
            view: Ledger::stub().snapshot(),
            widths: ColumnWidths::default(),
        };
        let harness = Harness::builder()
            .with_size(vec2(1000.0, 400.0))
            .build_ui_state(
                |ui, shown: &mut Shown| {
                    let Shown {
                        register,
                        i18n,
                        theme,
                        commands,
                        view,
                        widths,
                    } = shown;
                    let summary = view
                        .accounts()
                        .iter()
                        .find(|summary| summary.account().name() == "Everyday Checking")
                        .expect("the stub has Everyday Checking");
                    register.show(ui, i18n, theme, commands, summary, view, widths);
                },
                shown,
            );
        (harness, receiver)
    }

    fn click_cell(harness: &mut Harness<'static, Shown>, label: &str) {
        harness.get_by_role_and_label(Role::Cell, label).click();
        harness.run();
    }

    fn press(harness: &mut Harness<'static, Shown>, key: Key) {
        harness.key_press(key);
        harness.run();
    }

    #[test]
    fn a_commit_that_changes_nothing_sends_nothing() {
        let (mut harness, receiver) = register();
        click_cell(&mut harness, "Fresh Market");
        press(&mut harness, Key::Enter);

        press(&mut harness, Key::Enter);

        assert_eq!(receiver.try_iter().collect::<Vec<_>>(), []);
    }

    #[test]
    fn a_commit_sends_one_edit_with_the_text_trimmed() {
        let (mut harness, receiver) = register();
        click_cell(&mut harness, "Fresh Market");
        harness.event(Event::Text(" Corner Shop  ".to_owned()));
        harness.run();

        press(&mut harness, Key::Enter);

        let fresh_market = harness.state().transaction("Fresh Market");
        assert_eq!(
            receiver.try_iter().collect::<Vec<_>>(),
            [AppCommand::Domain(DomainCommand::EditTransaction {
                transaction: fresh_market,
                edit: TransactionEdit::Payee("Corner Shop".to_owned()),
            })]
        );
    }

    #[test]
    fn a_cleared_number_saves_as_no_number() {
        let (mut harness, receiver) = register();
        click_cell(&mut harness, "1041");
        press(&mut harness, Key::Enter);
        for _ in "1041".chars() {
            press(&mut harness, Key::Backspace);
        }

        press(&mut harness, Key::Enter);

        let rent = harness.state().transaction("Maple Property Management");
        assert_eq!(
            receiver.try_iter().collect::<Vec<_>>(),
            [AppCommand::Domain(DomainCommand::EditTransaction {
                transaction: rent,
                edit: TransactionEdit::Number(None),
            })]
        );
    }
}
