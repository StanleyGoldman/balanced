//! The Register: one account's transactions in a table, one row each, with the running
//! balance after each.

use std::collections::{BTreeMap, HashMap};

use balanced_core::{
    Account, AccountId, AccountSummary, CurrencyCode, DomainCommand, DomainError, Generation,
    LedgerView, Money, TransactionEdit, TransactionId,
};
use balanced_i18n::{AmountParseError, AmountReader, Localizer};
use eframe::egui::accesskit::{self, Role};
use eframe::egui::emath::GuiRounding as _;
use eframe::egui::text::{CCursor, CCursorRange};
use eframe::egui::text_edit::TextEditState;
use eframe::egui::{
    self, Align, Align2, Area, Button, Checkbox, CursorIcon, Event, EventFilter, Frame, Id, Key,
    Label, Layout, Margin, Modifiers, Order, Popup, PopupCloseBehavior, Rangef, Rect, Response,
    ScrollArea, Sense, Shape, Stroke, StrokeKind, TextEdit, TextStyle, TextWrapMode, WidgetInfo,
    WidgetText, WidgetType, pos2, vec2,
};
use egui_extras::{Column, TableBody, TableBuilder};
use icu_calendar::{Date, Iso};

use crate::amount_field::{self, Preview};
use crate::combobox::{FieldChoices, ListRow};
use crate::command::{AppCommand, CommandSender, DialogKind, StatusMessage};
use crate::date_field::{self, DateEdit};
use crate::draft_row::{Draft, Missing};
use crate::formatted_amount::{FormattedAmount, i18n_currency};
use crate::i18n::fl;
use crate::preferences::Preferences;
use crate::register_columns::{RegisterColumn, RegisterColumns};
use crate::register_model::{
    RegisterModel, RegisterRow, RegisterSort, RowId, SortDirection, SortKey, TodayDivider,
};
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

/// The narrowest a combobox list gets, so its entries stay readable under a narrow cell.
const LIST_MIN_WIDTH: f32 = 240.0;

/// How many rows a combobox list shows before it scrolls.
const LIST_MAX_ROWS: usize = 10;

/// The sort arrow beside the sorted column's heading: its width, its height, and the
/// space between it and the heading.
const SORT_ARROW_WIDTH: f32 = 7.0;
const SORT_ARROW_HEIGHT: f32 = 4.0;
const SORT_ARROW_GAP: f32 = 4.0;

impl RegisterColumn {
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
            Self::Date | Self::Number | Self::Memo | Self::Amount | Self::Balance => Column::exact(
                widths
                    .width(self)
                    .expect("every column that resizes has a width"),
            ),
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

    /// The field the column's cell on `row` edits, or `None` when the cell can't be
    /// edited. The opening balance's Payee cell shows a caption, not a payee, only a
    /// row in one category, or the draft row, edits its Category cell, and a split
    /// transaction's amount is the sum of its lines.
    fn field(self, row: &RegisterRow) -> Option<CellField> {
        let text = |field| Some(CellField::Text(field));
        match self {
            Self::Date => Some(CellField::Date),
            Self::Number => text(TextField::Number),
            Self::Payee if !row.is_opening_balance() => text(TextField::Payee),
            Self::Category if row.takes_category() => text(TextField::Category),
            Self::Memo => text(TextField::Memo),
            Self::Amount if !row.is_split() => Some(CellField::Amount),
            Self::Payee | Self::Category | Self::Amount | Self::Balance | Self::Actions => None,
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

/// A transaction field a Register cell edits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CellField {
    /// The date, in the segmented date field.
    Date,
    /// Plain text, or text naming an entry.
    Text(TextField),
    /// The amount, typed as written or as arithmetic.
    Amount,
}

/// A transaction field a Register cell edits as text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TextField {
    Number,
    Payee,
    /// The one category of a transaction, by its path.
    Category,
    Memo,
}

impl TextField {
    /// The field's saved text on `row`.
    fn saved(self, row: &RegisterRow) -> &str {
        match self {
            Self::Number => row.number(),
            Self::Payee => row.payee(),
            Self::Category => row.category(),
            Self::Memo => row.memo(),
        }
    }

    /// Whether the field offers a combobox list of entries as you type.
    fn offers_list(self) -> bool {
        match self {
            Self::Payee | Self::Category => true,
            Self::Number | Self::Memo => false,
        }
    }

    /// The edit that saves `text`, already trimmed, in the field of `row`, or `None`
    /// when it changes nothing. An empty number clears it. A category is named by its
    /// path, ignoring case, and fails when no category in `choices` has that name.
    fn edit(
        self,
        row: &RegisterRow,
        text: &str,
        choices: &FieldChoices,
    ) -> Result<Option<TransactionEdit>, NoSuchCategory> {
        if text == self.saved(row) {
            return Ok(None);
        }
        let edit = match self {
            Self::Number => TransactionEdit::Number((!text.is_empty()).then(|| text.to_owned())),
            Self::Payee => TransactionEdit::Payee(text.to_owned()),
            Self::Memo => TransactionEdit::Memo(text.to_owned()),
            Self::Category => {
                let category = *choices
                    .categories()
                    .named(text)
                    .ok_or(NoSuchCategory)?
                    .value();
                if row.category_id() == Some(category) {
                    return Ok(None);
                }
                TransactionEdit::Category(category)
            }
        };
        Ok(Some(edit))
    }
}

/// Why a Category cell's text can't be saved: no category has that name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("no category has the typed name")]
struct NoSuchCategory;

/// One cell of the Register: a row and a column.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct CellAddress {
    row: RowId,
    column: RegisterColumn,
}

impl CellAddress {
    /// The draft row's cell in `column`.
    fn draft(column: RegisterColumn) -> Self {
        Self {
            row: RowId::Draft,
            column,
        }
    }
}

/// The draft row, formatted, and the draft it was formatted from.
#[derive(Debug)]
struct DraftRow {
    draft: Draft,
    row: RegisterRow,
}

/// The Register's rows: the transactions' in the order of the sort, then the draft
/// row, which an account that can't change has none of. The Today divider is drawn
/// between two rows, on a line of its own, and is no row.
#[derive(Debug, Clone, Copy)]
struct Rows<'a> {
    transactions: &'a [RegisterRow],
    /// The index of the row the Today divider sits above, while it is drawn.
    divider: Option<usize>,
    draft: Option<&'a RegisterRow>,
}

/// One line of the Register's body: a row, with its index among the rows, or the
/// Today divider.
#[derive(Debug, Clone, Copy)]
enum Line<'a> {
    Row { index: usize, row: &'a RegisterRow },
    TodayDivider,
}

impl<'a> Rows<'a> {
    /// The rows of `model`, with its Today divider, then `draft`.
    fn new(model: &'a RegisterModel, draft: Option<&'a RegisterRow>) -> Self {
        Self {
            transactions: model.rows(),
            divider: model.divider(),
            draft,
        }
    }

    /// How many lines the rows take: one each, and one for the Today divider.
    fn lines(self) -> usize {
        self.len() + usize::from(self.divider.is_some())
    }

    /// The line at `index`, top line first.
    fn line(self, index: usize) -> Option<Line<'a>> {
        let index = match self.divider {
            Some(divider) if index == divider => return Some(Line::TodayDivider),
            Some(divider) if index > divider => index - 1,
            Some(_) | None => index,
        };
        let row = self.get(index)?;
        Some(Line::Row { index, row })
    }

    /// The index of the line the row at `index` is drawn on.
    fn line_of(self, index: usize) -> usize {
        index + usize::from(self.divider.is_some_and(|divider| index >= divider))
    }

    fn len(self) -> usize {
        self.transactions.len() + usize::from(self.draft.is_some())
    }

    /// The row at `index`, top row first.
    fn get(self, index: usize) -> Option<&'a RegisterRow> {
        self.transactions
            .get(index)
            .or_else(|| self.draft.filter(|_| index == self.transactions.len()))
    }

    /// The row with `id`, unless it is gone.
    fn find(self, id: RowId) -> Option<&'a RegisterRow> {
        match id {
            RowId::Transaction(_) => self.transactions.iter().find(|row| row.id() == id),
            RowId::Draft => self.draft,
        }
    }

    /// The index of the row with `id`, unless it is gone.
    fn position(self, id: RowId) -> Option<usize> {
        match id {
            RowId::Transaction(_) => self.transactions.iter().position(|row| row.id() == id),
            RowId::Draft => self.draft.map(|_| self.transactions.len()),
        }
    }
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

/// The cell being edited, the field it edits, its unsaved text, and its combobox list
/// while that is open.
#[derive(Debug, Clone)]
struct CellEdit {
    cell: CellAddress,
    field: EditField,
    text: String,
    list: Option<ListState>,
    /// Why the last commit was refused, shown in a bubble below the cell until the
    /// text changes.
    error: Option<AmountParseError>,
    /// The result of the arithmetic typed so far, for the bubble below the cell, or
    /// `None` while the text is no arithmetic that can be read.
    preview: Option<Preview>,
}

/// The field a cell edit saves its text in.
#[derive(Debug, Clone)]
enum EditField {
    Text(TextField),
    /// The amount, in the transaction's currency, read with the formatting locale the
    /// edit started in.
    Amount {
        reader: AmountReader,
        currency: CurrencyCode,
    },
}

impl EditField {
    /// Whether the field offers a combobox list of entries as you type.
    fn offers_list(&self) -> bool {
        match self {
            Self::Text(field) => field.offers_list(),
            Self::Amount { .. } => false,
        }
    }
}

/// The cell being edited, in the editor its field takes.
#[derive(Debug, Clone)]
enum Edit {
    Text(CellEdit),
    Date(DateCellEdit),
}

impl Edit {
    fn cell(&self) -> CellAddress {
        match self {
            Self::Text(edit) => edit.cell,
            Self::Date(edit) => edit.cell,
        }
    }

    /// The id of the cell's editor.
    fn editor_id(&self) -> Id {
        editor_id(self.cell())
    }

    /// Whether the last commit was refused, so the edit stays open with its message.
    fn is_rejected(&self) -> bool {
        match self {
            Self::Text(_) => false,
            Self::Date(edit) => edit.date.is_rejected(),
        }
    }
}

/// The id of the editor over `cell`.
fn editor_id(cell: CellAddress) -> Id {
    Id::new(("register_editor", cell))
}

/// The Date cell being edited, and the date field's state.
#[derive(Debug, Clone, PartialEq, Eq)]
struct DateCellEdit {
    cell: CellAddress,
    date: DateEdit,
}

impl CellEdit {
    /// The edit after a commit refused by `error`, which it shows until the text
    /// changes.
    fn refused(self, error: AmountParseError) -> Self {
        Self {
            error: Some(error),
            ..self
        }
    }

    /// The id of the cell's `TextEdit`.
    fn editor_id(&self) -> Id {
        editor_id(self.cell)
    }

    /// Opens the field's list of `scope`, if it offers one. A list of matches starts
    /// with its top row highlighted, and a list of every entry with the entry the text
    /// names, else its top row.
    fn open_list(&mut self, scope: ListScope, choices: &FieldChoices) {
        if !self.field.offers_list() {
            return;
        }
        self.list = Some(ListState {
            scope,
            highlight: Some(0),
            reveal: true,
            offset: 0.0,
        });
        let named = match scope {
            ListScope::Matching => None,
            ListScope::All => self.list_rows(choices).iter().position(
                |row| matches!(row, ListRow::Choice(text) if text.eq_ignore_ascii_case(self.text.trim())),
            ),
        };
        if let (Some(list), Some(named)) = (&mut self.list, named) {
            list.highlight = Some(named);
        }
    }

    /// The rows of the list, from `choices`, or none while it is closed.
    fn list_rows<'a>(&self, choices: &'a FieldChoices) -> Vec<ListRow<'a>> {
        let Some(list) = self.list else {
            return Vec::new();
        };
        let text = match list.scope {
            ListScope::Matching => self.text.as_str(),
            ListScope::All => "",
        };
        match &self.field {
            EditField::Text(TextField::Payee) => choices.payees().list(text),
            EditField::Text(TextField::Category) => {
                let categories = choices.categories();
                let mut rows = categories.list(text);
                rows.extend(categories.create_row(text));
                rows
            }
            EditField::Text(TextField::Number | TextField::Memo) | EditField::Amount { .. } => {
                Vec::new()
            }
        }
    }

    /// The highlighted row of the list, if it is open with one highlighted.
    fn highlighted<'a>(&self, choices: &'a FieldChoices) -> Option<ListRow<'a>> {
        let highlight = self.list?.highlight?;
        self.list_rows(choices).into_iter().nth(highlight)
    }

    /// Moves the highlight one row down the list of `rows` rows, stopping at the last.
    fn highlight_next(&mut self, rows: usize) {
        if let Some(list) = &mut self.list {
            let last = rows.saturating_sub(1);
            list.highlight = Some(list.highlight.map_or(0, |row| (row + 1).min(last)));
            list.reveal = true;
        }
    }

    /// Moves the highlight one row up the list. Up from the top row clears it, so
    /// Enter keeps the typed text.
    fn highlight_previous(&mut self) {
        if let Some(list) = &mut self.list {
            list.highlight = list.highlight.and_then(|row| row.checked_sub(1));
            list.reveal = true;
        }
    }
}

/// Which entries a combobox list shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ListScope {
    /// Those matching the typed text, as typing opens it.
    Matching,
    /// Every entry, as ⏷ and Alt+↓ open it.
    All,
}

/// An open combobox list: which entries it lists, its highlighted row, and how far
/// it is scrolled.
#[derive(Debug, Clone, Copy, PartialEq)]
struct ListState {
    scope: ListScope,
    /// The row Enter takes, or `None` to keep the typed text.
    highlight: Option<usize>,
    /// Whether the highlight moved since the list was last drawn, so the list scrolls
    /// to show it.
    reveal: bool,
    /// The list's scroll offset when it was last drawn.
    offset: f32,
}

impl ListState {
    /// The scroll offset that shows the highlighted row, row `pitch` apart, in a list
    /// `height` high, when the highlight moved out of view; else `None`.
    fn reveal_offset(self, pitch: f32, height: f32) -> Option<f32> {
        let highlight = self.highlight.filter(|_| self.reveal)?;
        #[expect(
            clippy::cast_precision_loss,
            reason = "list rows stay far below 2^24, where f32 stops counting whole numbers"
        )]
        let top = highlight as f32 * pitch;
        if top < self.offset {
            Some(top)
        } else if top + pitch > self.offset + height {
            Some(top + pitch - height)
        } else {
            None
        }
    }
}

/// The lines fully in view in the last frame, counted as [`Rows::line`] counts them,
/// which keyboard scrolling keeps the cell cursor among.
#[derive(Debug, Clone, Copy, PartialEq)]
struct LinesInView {
    first: usize,
    last: usize,
    /// The height of the Register's body.
    height: f32,
    /// The distance from one line's top to the next one's.
    pitch: f32,
}

impl LinesInView {
    /// The scroll offset that brings line `index` fully into view, or `None` when it
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

/// The open account's Register: its cached rows, its sort, the cell cursor, the cell
/// being edited, and the draft row. The cursor's row is the selected row. Opening
/// another account sorts it by date, empties the draft row and puts the cursor on it.
/// None of it outlives the run.
#[derive(Debug, Default)]
pub(crate) struct Register {
    /// The rows of the last account shown, rebuilt when they are out of date.
    model: Option<RegisterModel>,
    sort: RegisterSort,
    /// Whether the sort changed since the rows were last shown, so the cursor's row,
    /// if any, scrolls to the middle of the Register.
    centre_pending: bool,
    cursor: Option<CellAddress>,
    edit: Option<Edit>,
    /// What the Payee and Category comboboxes offer, rebuilt when it is out of date.
    choices: Option<FieldChoices>,
    /// The modifiers held with each Tab press `App::raw_input_hook` took for the
    /// Register since the last frame.
    tabs: Vec<Modifiers>,
    lines_in_view: Option<LinesInView>,
    /// Whether the edit refused to let the keyboard leave its cell this frame.
    refused_leave: bool,
    /// What the draft row holds.
    draft: Draft,
    /// The draft row, formatted, with the draft it shows. It is rebuilt when the draft
    /// changes or the transactions' rows are rebuilt. `None` while the account can't
    /// change.
    draft_row: Option<DraftRow>,
    /// The generation of the snapshot a post was sent against, until a later snapshot
    /// shows whether the ledger applied it. A rejected post keeps the draft.
    posted_at: Option<Generation>,
    /// The cell a refused draft row still needs, outlined with a bubble saying so
    /// until it is filled.
    missing: Option<Missing>,
    /// Whether the draft row is to scroll into view on the next frame.
    reveal_draft: bool,
    /// The columns shown, left to right, as the column chooser last left them.
    shown: Vec<RegisterColumn>,
    /// The Category cell edit waiting on the New Category Dialog its Create row opened.
    awaiting_category: Option<AwaitingCategory>,
}

impl Register {
    /// Whether keys go to the Register: the cell cursor is in it, no Dialog is open
    /// over it, and nothing holds keyboard focus but the Register's own cell editor.
    pub(crate) fn owns_keyboard(&self, ctx: &egui::Context) -> bool {
        self.cursor.is_some()
            && ctx.memory(|memory| memory.top_modal_layer().is_none())
            && ctx.memory(|memory| match memory.focused() {
                None => true,
                Some(focused) => self
                    .edit
                    .as_ref()
                    .is_some_and(|edit| edit.editor_id() == focused),
            })
    }

    /// The selected transaction's row, while `account`'s Register shows it. The draft
    /// row is no transaction's.
    pub(crate) fn selected_row(&self, account: AccountId) -> Option<&RegisterRow> {
        let RowId::Transaction(id) = self.cursor?.row else {
            return None;
        };
        self.row(account, id)
    }

    /// Transaction `id`'s row, while `account`'s Register shows it.
    pub(crate) fn row(&self, account: AccountId, id: TransactionId) -> Option<&RegisterRow> {
        let model = self
            .model
            .as_ref()
            .filter(|model| model.account() == account)?;
        let id = RowId::Transaction(id);
        model.rows().iter().find(|row| row.id() == id)
    }

    /// The index of the line this frame scrolls to the middle of the Register, if any:
    /// the draft row's when it is to come into view, else the cursor's row's after a
    /// sort. The draft row is the last row, so centring it scrolls to the bottom.
    fn line_to_centre(&mut self, rows: Rows<'_>) -> Option<usize> {
        let centre_pending = std::mem::take(&mut self.centre_pending);
        let row = if std::mem::take(&mut self.reveal_draft) {
            rows.position(RowId::Draft)
        } else {
            self.cursor
                .filter(|_| centre_pending)
                .and_then(|cursor| rows.position(cursor.row))
        };
        row.map(|row| rows.line_of(row))
    }

    /// Catches up with what changed since the last frame: another account, `account`,
    /// starts the Register over with the cell cursor on its draft row, unless it is
    /// `closed`; and a post that `view` shows applied empties the draft row.
    fn catch_up(&mut self, account: AccountId, closed: bool, view: &LedgerView) {
        if self.model.as_ref().map(RegisterModel::account) != Some(account) {
            self.start_over();
            if !closed {
                self.go_to_draft_row();
            }
        }
        if let Some(sent_at) = self.posted_at.take()
            && view.generation() != sent_at
        {
            self.draft = Draft::default();
            self.missing = None;
            self.go_to_draft_row();
        }
    }

    /// Settles what this frame's keys and clicks changed: an edit that refused to let
    /// the keyboard go keeps the cell cursor, a filled cell drops its outline, and
    /// `draft_row`, taken for the frame, comes back up to date with the draft.
    fn end_frame(&mut self, draft_row: Option<DraftRow>, draft_frame: DraftFrame<'_>) {
        if std::mem::take(&mut self.refused_leave)
            && let Some(edit) = &self.edit
        {
            self.cursor = Some(edit.cell());
        }
        let draft = &self.draft;
        self.missing = self.missing.filter(|&missing| draft.lacks(missing));
        self.draft_row = draft_row;
        self.refresh_draft_row(draft_frame, false);
    }

    /// Starts the Register over for another account: sorted by date, with no cell
    /// cursor and an empty draft row.
    fn start_over(&mut self) {
        self.sort = RegisterSort::default();
        self.centre_pending = false;
        self.cursor = None;
        self.edit = None;
        self.awaiting_category = None;
        self.lines_in_view = None;
        self.draft = Draft::default();
        self.missing = None;
        self.posted_at = None;
    }

    /// Formats the draft row again when the draft changed since it was formatted, or
    /// when `rebuilt` says the transactions' rows were just formatted again. A closed
    /// account has none.
    fn refresh_draft_row(&mut self, frame: DraftFrame<'_>, rebuilt: bool) {
        let DraftFrame {
            closed,
            currency,
            view,
            i18n,
        } = frame;
        if closed {
            self.draft_row = None;
            return;
        }
        let current = !rebuilt
            && self
                .draft_row
                .as_ref()
                .is_some_and(|draft_row| draft_row.draft == self.draft);
        if !current {
            self.draft_row = Some(DraftRow {
                draft: self.draft.clone(),
                row: RegisterRow::draft(&self.draft, currency, view, i18n),
            });
        }
    }

    /// Puts the cell cursor on the draft row's first cell, scrolling it into view on
    /// the next frame.
    pub(crate) fn go_to_draft_row(&mut self) {
        self.cursor = Some(CellAddress::draft(RegisterColumn::Date));
        self.reveal_draft = true;
    }

    /// Shows `columns`, left to right. A cell cursor in a column that is no longer shown
    /// moves to the Date column, and a sort by such a column becomes a sort by date.
    fn show_columns(&mut self, columns: &RegisterColumns) {
        self.shown = columns.shown();
        let sorted_shown = self
            .shown
            .iter()
            .any(|column| column.sort_key() == Some(self.sort.key()));
        if !sorted_shown {
            self.sort = RegisterSort::default();
            self.centre_pending = true;
        }
        if let Some(cursor) = self.cursor.as_mut()
            && !self.shown.contains(&cursor.column)
        {
            cursor.column = RegisterColumn::Date;
        }
    }

    /// Space between `column`'s cell edges and its text: [`Theme::EDGE_INSET`] on the
    /// Register's outer edges, which are the first column shown and the Actions
    /// column, and none elsewhere.
    fn insets(&self, column: RegisterColumn) -> Insets {
        let edge = Theme::EDGE_INSET;
        let left = self.shown.first() == Some(&column);
        let right = column == RegisterColumn::Actions;
        Insets {
            left: if left { edge } else { 0.0 },
            right: if right { edge } else { 0.0 },
        }
    }

    /// The columns the cell cursor can rest in, left to right.
    fn cursor_columns(&self) -> Vec<RegisterColumn> {
        self.shown
            .iter()
            .copied()
            .filter(|column| column.holds_cursor())
            .collect()
    }

    /// Carries out `action`, picked from transaction `id`'s Actions menu.
    fn act(&mut self, id: TransactionId, action: RowAction, commands: &CommandSender) {
        match action {
            RowAction::Delete => {
                // The confirm asks about the selected row.
                self.place_cursor(RowId::Transaction(id), RegisterColumn::Actions);
                commands.send(AppCommand::OpenDialog(DialogKind::DeleteTransaction));
            }
        }
    }

    /// Saves the cell being edited, if any, and closes its editor, as a Shortcut does
    /// before it acts.
    pub(crate) fn commit_edit(
        &mut self,
        ctx: &egui::Context,
        i18n: &Localizer,
        commands: &CommandSender,
    ) {
        let Some(edit) = self.edit.take() else {
            return;
        };
        if let (Some(model), Some(choices)) = (&self.model, &self.choices) {
            let draft_row = self.draft_row.as_ref().map(|draft| &draft.row);
            let rows = Rows::new(model, draft_row);
            let draft = &mut self.draft;
            // A refused edit is dropped: the Shortcut acts on the saved value.
            let _refused = match edit.clone() {
                Edit::Text(edit) => commit(&edit, rows, choices, commands, draft),
                Edit::Date(mut edit) => commit_date(i18n, &mut edit, rows, commands, draft),
            };
        }
        ctx.memory_mut(|memory| memory.surrender_focus(edit.editor_id()));
    }

    /// Queues a Tab press, with `modifiers` held, for the next frame.
    pub(crate) fn press_tab(&mut self, modifiers: Modifiers) {
        self.tabs.push(modifiers);
    }

    /// Takes the cached rows, rebuilt for `summary`, an account in `view`, when they
    /// are out of date, and whether they were rebuilt.
    fn take_model(
        &mut self,
        summary: &AccountSummary,
        view: &LedgerView,
        today_divider: TodayDivider,
        i18n: &Localizer,
    ) -> (RegisterModel, bool) {
        let account = summary.account().id();
        match self.model.take() {
            Some(model) if model.is_current(account, view, self.sort, today_divider, i18n) => {
                (model, false)
            }
            _ => (
                RegisterModel::new(summary, view, self.sort, today_divider, i18n),
                true,
            ),
        }
    }

    /// Draws the Register of `summary`, an account in `view`, filling the space left in
    /// `ui`, as `preferences` say. Only the rows in view are laid out. `today` is the
    /// date field's today, and the Today divider's.
    #[expect(
        clippy::too_many_arguments,
        reason = "the column widths and the Preferences are shared by every Register, so the Main window lends them here beside the account, its snapshot and today's date"
    )]
    pub(crate) fn show(
        &mut self,
        ui: &mut egui::Ui,
        i18n: &Localizer,
        theme: &Theme,
        commands: &CommandSender,
        summary: &AccountSummary,
        view: &LedgerView,
        widths: &mut ColumnWidths,
        preferences: &Preferences,
        today: Date<Iso>,
    ) {
        let columns = &preferences.register_columns;
        self.show_columns(columns);
        let account = summary.account().id();
        let closed = summary.account().is_closed();
        self.catch_up(account, closed, view);
        let today_divider = today_divider(preferences, today);
        let (model, rebuilt) = self.take_model(summary, view, today_divider, i18n);
        let choices = match self.choices.take() {
            Some(choices) if choices.is_current(view, i18n) => choices,
            _ => FieldChoices::new(view, i18n),
        };
        let currency = summary.account().currency();
        let draft_frame = DraftFrame {
            closed,
            currency,
            view,
            i18n,
        };
        self.refresh_draft_row(draft_frame, rebuilt);
        let draft_row = self.draft_row.take();
        let rows = Rows::new(&model, draft_row.as_ref().map(|draft| &draft.row));
        let keys = KeyContext {
            i18n,
            account,
            generation: view.generation(),
            closed,
            rows,
            choices: &choices,
            commands,
        };
        let key_scroll_offset = self.handle_keys(ui.ctx(), &keys);
        let row_height = theme.row_height();
        let pitch = row_height + ui.spacing().item_spacing.y;
        let centre_on = self.line_to_centre(rows);
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
                    // The rows stripe themselves, so the Today divider takes no stripe.
                    .striped(false)
                    .resizable(false)
                    .cell_layout(Layout::left_to_right(Align::Center))
                    .auto_shrink(false)
                    .max_scroll_height(f32::INFINITY);
                for &column in &self.shown {
                    table = table.column(column.width(theme, widths));
                }
                if let Some(offset) = scroll_offset {
                    table = table.vertical_scroll_offset(offset);
                }
                table
                    .header(row_height, |mut header| {
                        let frame = HeaderFrame {
                            i18n,
                            theme,
                            commands,
                            columns,
                        };
                        let shown = self.shown.clone();
                        for column in shown {
                            header.col(|ui| {
                                let heading = Heading {
                                    register_id,
                                    column,
                                    insets: self.insets(column),
                                    sort: self.sort,
                                    viewport,
                                };
                                self.heading_cell(ui, &heading, &frame, widths);
                            });
                        }
                    })
                    .body(|body| {
                        let frame = BodyFrame {
                            i18n,
                            today,
                            theme,
                            commands,
                            rows,
                            choices: &choices,
                            register_id,
                            access: Access::of(summary.account()),
                            viewport: Rect::NOTHING,
                        };
                        self.body(body, frame, row_height, pitch);
                    });
            });
        self.end_frame(draft_row, draft_frame);
        self.model = Some(model);
        self.choices = Some(choices);
    }

    /// Draws `heading` in its cell `ui`, with the column's resize handle, and the
    /// column chooser on the Actions column. Clicking a sortable heading sorts by it.
    fn heading_cell(
        &mut self,
        ui: &egui::Ui,
        heading: &Heading,
        frame: &HeaderFrame<'_>,
        widths: &mut ColumnWidths,
    ) {
        let HeaderFrame {
            i18n,
            theme,
            commands,
            columns,
        } = *frame;
        if let Some(key) = heading.show(ui, i18n, theme) {
            self.sort = self.sort.clicked(key);
            self.centre_pending = true;
            // The rows sort and scroll on the next frame.
            ui.ctx().request_repaint();
        }
        widths.resize_handle(ui, i18n, heading.column);
        if heading.column == RegisterColumn::Actions
            && let Some(columns) = column_chooser(ui, i18n, heading.register_id, columns)
        {
            commands.send(AppCommand::SetRegisterColumns(columns));
        }
    }

    /// Lays out the rows in view of the Register's body, and notes which of them are
    /// fully in view for keyboard scrolling. `frame`'s viewport is filled in here.
    fn body(&mut self, mut body: TableBody<'_>, frame: BodyFrame<'_>, row_height: f32, pitch: f32) {
        let viewport = body.ui_mut().clip_rect();
        let frame = BodyFrame { viewport, ..frame };
        let rows = frame.rows;
        let mut in_view: Option<(usize, usize)> = None;
        body.rows(row_height, rows.lines(), |mut table_row| {
            let index = table_row.index();
            let line = rows
                .line(index)
                .expect("`rows` lays out indices below `rows.lines()` only");
            let shown = self.shown.clone();
            let Line::Row {
                index: row_index,
                row,
            } = line
            else {
                let label = fl!(
                    frame.i18n,
                    "register-today-divider",
                    date = frame.i18n.format_date(frame.today)
                );
                let mut rule_start = f32::NEG_INFINITY;
                for (column_index, column) in shown.into_iter().enumerate() {
                    table_row.col(|ui| {
                        let divider = DividerCell {
                            register_id: frame.register_id,
                            label: (column_index == 0).then_some(label.as_str()),
                            insets: self.insets(column),
                            viewport,
                        };
                        rule_start = divider.show(ui, frame.theme, rule_start);
                    });
                }
                return;
            };
            let selected = self.cursor.is_some_and(|cursor| cursor.row == row.id());
            table_row.set_selected(selected);
            let stripe = frame.theme.stripe(row_index).filter(|_| !selected);
            for (column_index, column) in shown.into_iter().enumerate() {
                table_row.col(|ui| {
                    if let Some(stripe) = stripe {
                        paint_stripe(ui, frame.theme, viewport, stripe);
                    }
                    if column_index == 0 && fully_in(viewport, ui.max_rect()) {
                        in_view = Some(in_view.map_or((index, index), |(first, _)| (first, index)));
                    }
                    self.body_cell(ui, &frame, row, column_index, column);
                });
            }
        });
        self.lines_in_view = in_view.map(|(first, last)| LinesInView {
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
            register_id,
            access,
            viewport,
            ..
        } = *frame;
        let address = CellAddress {
            row: row.id(),
            column,
        };
        let cell = Cell {
            register_id,
            row,
            column,
            insets: self.insets(column),
            selected: self.cursor.is_some_and(|cursor| cursor.row == row.id()),
            cursor: self.cursor == Some(address),
            access,
            viewport,
        };
        let (response, action) = cell.show(ui, i18n, theme);
        // A refused date keeps the cell cursor until it is fixed or cancelled.
        if response.clicked() && !self.edit.as_ref().is_some_and(Edit::is_rejected) {
            self.place_cursor(row.id(), column);
            open_split_view(row, column, commands);
        }
        if let (Some(action), Some(id)) = (action, row.transaction()) {
            self.act(id, action, commands);
        }
        let editing = self
            .edit
            .as_ref()
            .is_some_and(|edit| edit.cell() == address);
        if let Some(missing) = self.missing
            && address == CellAddress::draft(missing_column(missing))
            && !editing
        {
            let rect = ui.max_rect();
            ui.painter()
                .rect_stroke(rect, 0.0, theme.missing_cell_stroke(), StrokeKind::Inside);
            let id = register_id.with("register_missing");
            let colour = ui.visuals().error_fg_color;
            floating_bubble(ui, id, rect, missing.message(i18n), colour);
        }
        match &self.edit {
            Some(Edit::Text(edit)) if edit.cell == address => self.edit_cell(ui, frame, column),
            Some(Edit::Date(edit)) if edit.cell == address => {
                self.edit_date_cell(ui, frame, column);
            }
            Some(Edit::Text(_) | Edit::Date(_)) | None => {}
        }
        let rules = theme.cell_rules(ui.max_rect(), column_index, ui.ctx().pixels_per_point());
        ui.ctx()
            .layer_painter(ui.layer_id())
            .with_clip_rect(theme.rule_clip(ui.clip_rect(), viewport))
            .extend(rules);
    }

    /// Draws the editor over the cell being edited, in `column`, with its ⏷ button and
    /// its list when it has them, and acts on what they report: losing focus commits
    /// the edit, and pressing a list row takes it.
    fn edit_cell(&mut self, ui: &mut egui::Ui, frame: &BodyFrame<'_>, column: RegisterColumn) {
        let insets = self.insets(column);
        let Some(Edit::Text(edit)) = self.edit.as_mut() else {
            return;
        };
        let cell = ui.max_rect();
        let mut field = cell;
        let list_button = matches!(edit.field, EditField::Text(TextField::Category)).then(|| {
            let button =
                Rect::from_x_y_ranges(cell.right() - cell.height()..=cell.right(), cell.y_range());
            field.max.x = button.left();
            list_button(ui, frame.i18n, button, edit.editor_id())
        });
        let typed = edit.text.clone();
        let response = editor(
            ui,
            edit,
            field,
            insets,
            heading_id(frame.register_id, column),
        );
        if edit.text != typed {
            edit.open_list(ListScope::Matching, frame.choices);
            edit.error = None;
            edit.preview = preview(frame.i18n, edit);
        }
        if list_button.as_ref().is_some_and(Response::clicked) {
            // The click took focus from the editor, which keeps the edit.
            if edit.list.is_some_and(|list| list.scope == ListScope::All) {
                edit.list = None;
            } else {
                edit.open_list(ListScope::All, frame.choices);
            }
            ui.memory_mut(|memory| memory.request_focus(edit.editor_id()));
        } else if response.lost_focus() {
            let edit = self.edit.take().expect("the edit was just shown");
            let Edit::Text(edit) = edit else {
                return;
            };
            match commit(
                &edit,
                frame.rows,
                frame.choices,
                frame.commands,
                &mut self.draft,
            ) {
                // A refused category leaves nothing to save; the Status bar says why.
                Commit::Done | Commit::Refused => {}
                Commit::Unreadable(error) => {
                    // Leaving the cell is refused too: the edit keeps the keyboard, and
                    // the cell cursor comes back to it once the rows are laid out.
                    ui.memory_mut(|memory| memory.request_focus(edit.editor_id()));
                    self.refused_leave = true;
                    let edit = edit.refused(error);
                    bubble(ui, frame.i18n, &edit, cell);
                    self.edit = Some(Edit::Text(edit));
                }
            }
            return;
        }
        bubble(ui, frame.i18n, edit, cell);
        let pressed = list(ui, frame, edit, cell);
        if let Some(row) = pressed
            && let Some(Edit::Text(mut edit)) = self.edit.take()
        {
            ui.memory_mut(|memory| memory.surrender_focus(edit.editor_id()));
            match take_row(&mut edit, row, frame.commands) {
                Taken::Choice => {
                    let _refused = commit(
                        &edit,
                        frame.rows,
                        frame.choices,
                        frame.commands,
                        &mut self.draft,
                    );
                }
                Taken::Create => {
                    self.awaiting_category = Some(AwaitingCategory {
                        edit,
                        leave: Leave::Stay,
                        outcome: None,
                    });
                }
            }
        }
    }

    /// Draws the date field over the Date cell being edited, in `column`. Losing focus
    /// commits the date; a refused date keeps its edit and the cell cursor, unfocused,
    /// until it is fixed or cancelled. Focus stays where the click put it.
    fn edit_date_cell(&mut self, ui: &egui::Ui, frame: &BodyFrame<'_>, column: RegisterColumn) {
        let Insets { left, right } = self.insets(column);
        let Some(Edit::Date(edit)) = self.edit.as_mut() else {
            return;
        };
        let rect = ui.max_rect();
        let inner =
            Rect::from_x_y_ranges(rect.left() + left..=rect.right() - right, rect.y_range());
        let response = edit
            .date
            .show(
                ui,
                frame.i18n,
                frame.theme,
                frame.today,
                editor_id(edit.cell),
                inner,
            )
            .labelled_by(heading_id(frame.register_id, column));
        if response.lost_focus()
            && let Some(Edit::Date(mut edit)) = self.edit.take()
            && commit_date(
                frame.i18n,
                &mut edit,
                frame.rows,
                frame.commands,
                &mut self.draft,
            ) == Commit::Refused
        {
            self.cursor = Some(edit.cell);
            self.edit = Some(Edit::Date(edit));
        }
    }

    /// Puts the cell cursor on a click in `column` of `row`. A click in a column the
    /// cursor can't rest in keeps the cursor's column.
    fn place_cursor(&mut self, row: RowId, column: RegisterColumn) {
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
        if let Some(moved) = self.resume_category_edit(ctx, keys, cursor) {
            return moved.scroll_offset;
        }
        self.refuse_characters(ctx, keys, cursor);
        if cursor.row == RowId::Draft
            && ctx.input_mut(|input| input.consume_key(Modifiers::COMMAND, Key::Enter))
        {
            // Ctrl+Enter saves the cell being edited, then adds the row.
            if self.commit_open_edit(ctx, keys) {
                self.post(keys);
            }
            return None;
        }
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
        match self.edit.take() {
            Some(Edit::Text(edit)) => return self.handle_edit_keys(ctx, keys, edit, cursor, tab),
            Some(Edit::Date(edit)) => return self.handle_date_keys(ctx, keys, edit, cursor, tab),
            None => {}
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
            if let Some(row) = keys.rows.find(cursor.row)
                && open_split_view(row, cursor.column, keys.commands)
            {
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

    /// Acts on this frame's keys for `edit`, the cell being edited under `cursor`,
    /// with `tab` the move a Tab press asks for, and returns the scroll offset that
    /// brings the cell cursor into view when it has left it. The edit stays open
    /// unless a key ends it.
    fn handle_edit_keys(
        &mut self,
        ctx: &egui::Context,
        keys: &KeyContext<'_>,
        mut edit: CellEdit,
        cursor: CellAddress,
        tab: Option<Move>,
    ) -> Option<f32> {
        let rows = edit.list_rows(keys.choices).len();
        let list_open = rows > 0;
        let offers_list = edit.field.offers_list();
        let (alt_down, down, up, escape, enter) = ctx.input_mut(|input| {
            // Alt+↓ first, since a bare ↓ also matches it.
            let alt_down = offers_list && input.consume_key(Modifiers::ALT, Key::ArrowDown);
            (
                alt_down,
                offers_list && input.consume_key(Modifiers::NONE, Key::ArrowDown),
                offers_list && input.consume_key(Modifiers::NONE, Key::ArrowUp),
                input.consume_key(Modifiers::NONE, Key::Escape),
                input.consume_key(Modifiers::NONE, Key::Enter),
            )
        });
        if alt_down {
            edit.open_list(ListScope::All, keys.choices);
        } else if down && list_open {
            edit.highlight_next(rows);
        } else if down {
            edit.open_list(ListScope::Matching, keys.choices);
        } else if up {
            edit.highlight_previous();
        }
        if escape {
            // The first Esc closes the list; the next cancels the edit.
            if list_open {
                edit.list = None;
                self.edit = Some(Edit::Text(edit));
            } else {
                ctx.memory_mut(|memory| memory.surrender_focus(edit.editor_id()));
            }
            return None;
        }
        let leave = match tab {
            _ if enter => Some(Leave::Enter),
            Some(step) => Some(Leave::Tab(step)),
            None => None,
        };
        if let Some(leave) = leave {
            if let Some(row) = edit.highlighted(keys.choices)
                && take_row(&mut edit, row, keys.commands) == Taken::Create
            {
                ctx.memory_mut(|memory| memory.surrender_focus(edit.editor_id()));
                self.awaiting_category = Some(AwaitingCategory {
                    edit,
                    leave,
                    outcome: None,
                });
                return None;
            }
            return self.leave_cell(ctx, keys, edit, cursor, leave);
        }
        self.edit = Some(Edit::Text(edit));
        None
    }

    /// Saves `edit`, the cell being edited under `cursor`, and leaves the cell as
    /// `leave` says, returning the scroll offset that brings the cell cursor into view
    /// when it has left it. A refused commit keeps the edit open.
    fn leave_cell(
        &mut self,
        ctx: &egui::Context,
        keys: &KeyContext<'_>,
        mut edit: CellEdit,
        cursor: CellAddress,
        leave: Leave,
    ) -> Option<f32> {
        match commit(
            &edit,
            keys.rows,
            keys.choices,
            keys.commands,
            &mut self.draft,
        ) {
            Commit::Done => {}
            Commit::Refused => {
                edit.list = None;
                ctx.memory_mut(|memory| memory.request_focus(edit.editor_id()));
                self.edit = Some(Edit::Text(edit));
                return None;
            }
            Commit::Unreadable(error) => {
                self.edit = Some(Edit::Text(edit.refused(error)));
                return None;
            }
        }
        ctx.memory_mut(|memory| memory.surrender_focus(edit.editor_id()));
        if leave == Leave::Enter
            && cursor.row == RowId::Draft
            && self.cursor_columns().last() == Some(&cursor.column)
        {
            // Enter on the draft row's last cell adds the row.
            self.post(keys);
            return None;
        }
        let step = match leave {
            Leave::Enter => Move::Next,
            Leave::Tab(step) => step,
            Leave::Stay => return None,
        };
        let moved = self.move_cursor(cursor, step, keys.rows)?;
        // Tab opens the cell it moved to. With nowhere to move it opens nothing,
        // since this frame's rows don't hold the text just committed yet.
        if matches!(leave, Leave::Tab(_)) {
            self.start_edit(ctx, keys, EditStart::Saved);
        }
        moved.scroll_offset
    }

    /// Tells the Category cell edit waiting on the New Category Dialog that the
    /// category it asked for was created, with `path` its path. The edit saves it and
    /// carries on as the key that took the Create row says.
    pub(crate) fn category_created(&mut self, path: String) {
        if let Some(awaiting) = &mut self.awaiting_category {
            awaiting.outcome = Some(CategoryOutcome::Created(path));
        }
    }

    /// Tells the Category cell edit waiting on the New Category Dialog that the Dialog
    /// was cancelled. The edit opens again with its typed text.
    pub(crate) fn category_cancelled(&mut self) {
        if let Some(awaiting) = &mut self.awaiting_category {
            awaiting.outcome = Some(CategoryOutcome::Cancelled);
        }
    }

    /// Carries on with the Category cell edit that waited on the New Category Dialog,
    /// once the Dialog has closed. Returns `None` while there is none to carry on
    /// with, else where the cell cursor went.
    fn resume_category_edit(
        &mut self,
        ctx: &egui::Context,
        keys: &KeyContext<'_>,
        cursor: CellAddress,
    ) -> Option<Moved> {
        let AwaitingCategory {
            mut edit,
            leave,
            outcome,
        } = self
            .awaiting_category
            .take_if(|awaiting| awaiting.outcome.is_some())?;
        edit.list = None;
        match outcome? {
            CategoryOutcome::Created(path) => {
                edit.text = path;
                Some(Moved {
                    scroll_offset: self.leave_cell(ctx, keys, edit, cursor, leave),
                })
            }
            CategoryOutcome::Cancelled => {
                focus_at_end(ctx, &edit);
                self.edit = Some(Edit::Text(edit));
                Some(Moved {
                    scroll_offset: None,
                })
            }
        }
    }

    /// Acts on this frame's keys for `edit`, the Date cell being edited under `cursor`,
    /// with `tab` the move a Tab press asks for, and returns the scroll offset that
    /// brings the cell cursor into view when it has left it. The first Esc closes the
    /// calendar and the next cancels the edit; a refused date keeps the edit open.
    fn handle_date_keys(
        &mut self,
        ctx: &egui::Context,
        keys: &KeyContext<'_>,
        mut edit: DateCellEdit,
        cursor: CellAddress,
        tab: Option<Move>,
    ) -> Option<f32> {
        let (escape, enter) = ctx.input_mut(|input| {
            (
                input.consume_key(Modifiers::NONE, Key::Escape),
                input.consume_key(Modifiers::NONE, Key::Enter),
            )
        });
        if escape {
            if edit.date.close_calendar() {
                self.edit = Some(Edit::Date(edit));
            } else {
                ctx.memory_mut(|memory| memory.surrender_focus(editor_id(edit.cell)));
            }
            return None;
        }
        if enter || tab.is_some() {
            if commit_date(
                keys.i18n,
                &mut edit,
                keys.rows,
                keys.commands,
                &mut self.draft,
            ) == Commit::Refused
            {
                self.edit = Some(Edit::Date(edit));
                return None;
            }
            ctx.memory_mut(|memory| memory.surrender_focus(editor_id(edit.cell)));
            let step = if enter { Move::Next } else { tab? };
            let moved = self.move_cursor(cursor, step, keys.rows)?;
            // Tab opens the cell it moved to, as it does from a text cell.
            if !enter {
                self.start_edit(ctx, keys, EditStart::Saved);
            }
            return moved.scroll_offset;
        }
        self.edit = Some(Edit::Date(edit));
        None
    }

    /// Removes from this frame's typed and pasted text every character the cell at
    /// `cursor` refuses, before its editor, or the typing that opens it, sees the text.
    /// Only an Amount cell refuses characters.
    fn refuse_characters(&self, ctx: &egui::Context, keys: &KeyContext<'_>, cursor: CellAddress) {
        let has_text = ctx.input(|input| {
            input
                .events
                .iter()
                .any(|event| matches!(event, Event::Text(_) | Event::Paste(_)))
        });
        if !has_text {
            return;
        }
        let opened;
        let reader = match &self.edit {
            Some(Edit::Text(CellEdit {
                field: EditField::Amount { reader, .. },
                ..
            })) => reader,
            Some(Edit::Text(_) | Edit::Date(_)) => return,
            None => {
                let Some((row, CellField::Amount)) = field_at(keys.rows, cursor) else {
                    return;
                };
                opened = keys.i18n.amount_reader(i18n_currency(row.currency()));
                &opened
            }
        };
        amount_field::refuse_characters(ctx, reader);
    }

    /// Moves the cell cursor from `cursor` by `step`. Returns `None`, leaving the
    /// cursor where it is, when there is nowhere to go.
    fn move_cursor(&mut self, cursor: CellAddress, step: Move, rows: Rows<'_>) -> Option<Moved> {
        let columns = self.cursor_columns();
        let row = rows.position(cursor.row)?;
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
                    (row + 1, 0)
                }
            }
            Move::NextEditable | Move::PreviousEditable => {
                let width = columns.len();
                let start = row * width + column;
                let editable = |position: usize| {
                    let row = rows.get(position / width)?;
                    // The remainder of a division by the width is a column index.
                    columns[position % width].field(row)
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
                .lines_in_view
                .and_then(|lines_in_view| lines_in_view.offset_showing(rows.line_of(row))),
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
        let (saved, field) = match field {
            CellField::Date => {
                // Typing goes to the date field, which starts the date over or acts on
                // a shortcut, so it opens on the saved date however the edit starts.
                ctx.memory_mut(|memory| memory.request_focus(editor_id(cursor)));
                self.edit = Some(Edit::Date(DateCellEdit {
                    cell: cursor,
                    date: DateEdit::new(row.saved_date()),
                }));
                return;
            }
            CellField::Text(field) => (field.saved(row), EditField::Text(field)),
            CellField::Amount => {
                let currency = row.currency();
                let reader = keys.i18n.amount_reader(i18n_currency(currency));
                (row.amount_text(), EditField::Amount { reader, currency })
            }
        };
        let text = match start {
            EditStart::Saved => saved.to_owned(),
            EditStart::Empty => String::new(),
        };
        let edit = CellEdit {
            cell: cursor,
            field,
            text,
            list: None,
            error: None,
            preview: None,
        };
        focus_at_end(ctx, &edit);
        self.edit = Some(Edit::Text(edit));
    }

    /// Saves the cell being edited, if any, and closes its editor, as Ctrl+Enter does
    /// before it adds the draft row. A highlighted list entry is taken first. Returns
    /// whether the edit, if any, was saved; a refused one stays open, saying why.
    fn commit_open_edit(&mut self, ctx: &egui::Context, keys: &KeyContext<'_>) -> bool {
        let Some(edit) = self.edit.take() else {
            return true;
        };
        let editor = edit.editor_id();
        let refused = match edit {
            Edit::Text(mut edit) => {
                if let Some(ListRow::Choice(text)) = edit.highlighted(keys.choices) {
                    text.clone_into(&mut edit.text);
                }
                match commit(
                    &edit,
                    keys.rows,
                    keys.choices,
                    keys.commands,
                    &mut self.draft,
                ) {
                    Commit::Done => None,
                    Commit::Refused => {
                        edit.list = None;
                        Some(Edit::Text(edit))
                    }
                    Commit::Unreadable(error) => Some(Edit::Text(edit.refused(error))),
                }
            }
            Edit::Date(mut edit) => {
                let commit = commit_date(
                    keys.i18n,
                    &mut edit,
                    keys.rows,
                    keys.commands,
                    &mut self.draft,
                );
                (commit == Commit::Refused).then_some(Edit::Date(edit))
            }
        };
        if refused.is_none() {
            ctx.memory_mut(|memory| memory.surrender_focus(editor));
        }
        self.edit = refused;
        self.edit.is_none()
    }

    /// Adds the draft row to the account. Once the ledger applies it, the draft row
    /// empties and the cell cursor goes to its first cell; a rejected post keeps the
    /// draft, and the Status bar says why. A row missing a cell the transaction needs
    /// is refused: the cursor goes to the first such cell, outlined, with a bubble
    /// saying what is missing.
    fn post(&mut self, keys: &KeyContext<'_>) {
        match self.draft.posting(keys.account) {
            Ok(command) => {
                keys.commands.send(AppCommand::Domain(command));
                self.posted_at = Some(keys.generation);
            }
            Err(missing) => {
                self.missing = Some(missing);
                self.cursor = Some(CellAddress::draft(missing_column(missing)));
            }
        }
    }
}

/// The draft row cell that `missing` names.
fn missing_column(missing: Missing) -> RegisterColumn {
    match missing {
        Missing::Date => RegisterColumn::Date,
        Missing::Category => RegisterColumn::Category,
        Missing::Amount => RegisterColumn::Amount,
    }
}

/// Gives `edit`'s editor keyboard focus, with the text cursor at the end of its text
/// wherever an earlier edit left it.
fn focus_at_end(ctx: &egui::Context, edit: &CellEdit) {
    let id = edit.editor_id();
    let mut state = TextEditState::load(ctx, id).unwrap_or_default();
    state
        .cursor
        .set_char_range(Some(CCursorRange::one(CCursor::new(
            edit.text.chars().count(),
        ))));
    state.store(ctx, id);
    ctx.memory_mut(|memory| memory.request_focus(id));
}

/// How a cell edit leaves its cell once its text is saved.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Leave {
    /// As Enter does: the cell cursor moves to the next cell, which stays closed.
    Enter,
    /// As Tab and Shift+Tab do: the cell cursor makes this move and opens the cell.
    Tab(Move),
    /// As a click on a list row does: the cell cursor stays.
    Stay,
}

/// A Category cell edit whose Create row opened the New Category Dialog, waiting for
/// the Dialog to close.
#[derive(Debug, Clone)]
struct AwaitingCategory {
    edit: CellEdit,
    /// How the edit leaves its cell once the new category is saved in it.
    leave: Leave,
    /// How the Dialog closed, or `None` while it is open.
    outcome: Option<CategoryOutcome>,
}

/// How the New Category Dialog closed.
#[derive(Debug, Clone, PartialEq, Eq)]
enum CategoryOutcome {
    /// It created the category with this path.
    Created(String),
    Cancelled,
}

/// A move of the cell cursor that happened.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Moved {
    /// The scroll offset that brings the cursor into view, when it has left it.
    scroll_offset: Option<f32>,
}

/// The row of `cell` and the field it edits, or `None` when its row is gone or it
/// can't be edited.
fn field_at(rows: Rows<'_>, cell: CellAddress) -> Option<(&RegisterRow, CellField)> {
    let row = rows.find(cell.row)?;
    Some((row, cell.column.field(row)?))
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
    /// Today's date, for the date field and the Today divider.
    today: Date<Iso>,
    theme: &'a Theme,
    commands: &'a CommandSender,
    rows: Rows<'a>,
    choices: &'a FieldChoices,
    register_id: Id,
    access: Access,
    /// The visible part of the Register's body.
    viewport: Rect,
}

/// Whether a Register's rows can change: a closed account's are read-only.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Access {
    Editable,
    ReadOnly,
}

impl Access {
    /// How `account`'s Register rows can be changed.
    fn of(account: &Account) -> Self {
        if account.is_closed() {
            Self::ReadOnly
        } else {
            Self::Editable
        }
    }
}

/// A command picked from a row's Actions menu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RowAction {
    Delete,
}

/// Whether `rect` lies wholly within `viewport` from top to bottom, give or take half a
/// point.
fn fully_in(viewport: Rect, rect: Rect) -> bool {
    let view = viewport.y_range().expand(0.5);
    view.contains(rect.top()) && view.contains(rect.bottom())
}

/// What the draft row is formatted with in one frame.
#[derive(Clone, Copy)]
struct DraftFrame<'a> {
    /// Whether the account is closed, so has no draft row.
    closed: bool,
    currency: CurrencyCode,
    view: &'a LedgerView,
    i18n: &'a Localizer,
}

/// What the Register's keys act on.
struct KeyContext<'a> {
    i18n: &'a Localizer,
    account: AccountId,
    /// The generation of the snapshot the Register shows.
    generation: Generation,
    closed: bool,
    rows: Rows<'a>,
    choices: &'a FieldChoices,
    commands: &'a CommandSender,
}

/// Asks for the split view of `row` when `column` is the Category cell of a split
/// transaction, whose `[Split]` acts as a button. Returns whether it asked.
fn open_split_view(row: &RegisterRow, column: RegisterColumn, commands: &CommandSender) -> bool {
    let split = row
        .transaction()
        .filter(|_| column == RegisterColumn::Category && row.is_split());
    if let Some(transaction) = split {
        commands.send(AppCommand::OpenSplitView(transaction));
    }
    split.is_some()
}

/// Whether a commit went through.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Commit {
    /// The value was saved, or matched the saved one, or its row is gone.
    Done,
    /// The value can't be saved, so nothing was sent: a category no category is named
    /// (the Status bar says so), or a refused date (its bubble says so).
    Refused,
    /// The amount's text can't be read, for this reason, so nothing was sent.
    Unreadable(AmountParseError),
}

/// Saves `edit`'s text, trimmed, unless it matches the saved text or saved amount: in
/// `draft` on the draft row, else by sending one edit Command.
fn commit(
    edit: &CellEdit,
    rows: Rows<'_>,
    choices: &FieldChoices,
    commands: &CommandSender,
    draft: &mut Draft,
) -> Commit {
    let Some(row) = rows.find(edit.cell.row) else {
        return Commit::Done;
    };
    let text = edit.text.trim();
    let change = match &edit.field {
        EditField::Text(field) => {
            let Ok(change) = field.edit(row, text, choices) else {
                commands.send(AppCommand::ShowStatus(StatusMessage::NoSuchCategory));
                return Commit::Refused;
            };
            change
        }
        EditField::Amount { reader, currency } => match amount_edit(row, text, reader, *currency) {
            Ok(change) => change,
            Err(error) => return Commit::Unreadable(error),
        },
    };
    if let Some(change) = change {
        save(row.id(), change, commands, draft);
    }
    Commit::Done
}

/// Saves `edit` in a cell of `row`: in `draft` on the draft row, else by sending one
/// edit Command.
fn save(row: RowId, edit: TransactionEdit, commands: &CommandSender, draft: &mut Draft) {
    match row {
        RowId::Transaction(transaction) => {
            commands.send(AppCommand::Domain(DomainCommand::EditTransaction {
                transaction,
                edit,
            }));
        }
        RowId::Draft => draft.edit(edit),
    }
}

/// The edit that saves `text`, already trimmed, as the amount of `row`, read by
/// `reader` in `currency`, or `None` when it matches the saved amount.
fn amount_edit(
    row: &RegisterRow,
    text: &str,
    reader: &AmountReader,
    currency: CurrencyCode,
) -> Result<Option<TransactionEdit>, AmountParseError> {
    if text == row.amount_text() {
        return Ok(None);
    }
    let amount = reader.read(text)?.amount();
    let money = Money::new(amount, currency)
        .expect("the reader gives amounts at the currency's decimal places");
    let saved = row.amount().map(FormattedAmount::money);
    Ok((saved != Some(money)).then_some(TransactionEdit::Amount(money)))
}

/// Saves `edit`'s date, unless it matches the saved date: in `draft` on the draft
/// row, else by sending one edit Command. A refused date shows the format message
/// and saves nothing.
fn commit_date(
    i18n: &Localizer,
    edit: &mut DateCellEdit,
    rows: Rows<'_>,
    commands: &CommandSender,
    draft: &mut Draft,
) -> Commit {
    let Some(row) = rows.find(edit.cell.row) else {
        return Commit::Done;
    };
    match edit.date.commit(i18n) {
        date_field::Commit::Unchanged => Commit::Done,
        date_field::Commit::Changed(date) => {
            save(row.id(), TransactionEdit::Date(date), commands, draft);
            Commit::Done
        }
        date_field::Commit::Rejected => Commit::Refused,
    }
}

/// What taking a list row did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Taken {
    /// The row's text replaced the edit's, ready to commit.
    Choice,
    /// The New Category Dialog was asked for, prefilled with the typed name, and the
    /// edit waits for it to close.
    Create,
}

/// Takes `row` of `edit`'s list.
fn take_row(edit: &mut CellEdit, row: ListRow<'_>, commands: &CommandSender) -> Taken {
    match row {
        ListRow::Choice(text) => {
            text.clone_into(&mut edit.text);
            Taken::Choice
        }
        ListRow::Create(name) => {
            commands.send(AppCommand::OpenDialog(DialogKind::NewCategory { name }));
            Taken::Create
        }
    }
}

/// The id of `column`'s heading, which labels the cell editor in that column.
fn heading_id(register_id: Id, column: RegisterColumn) -> Id {
    register_id.with(("register_heading", column))
}

/// The `TextEdit` over the cell being edited, filling `rect`. It keeps Esc, so the
/// Register can cancel the edit before egui drops the field's focus. A field with a
/// list keeps ↑ and ↓ too, which would otherwise move focus out of it.
fn editor(
    ui: &mut egui::Ui,
    edit: &mut CellEdit,
    rect: Rect,
    Insets { left, right }: Insets,
    heading: Id,
) -> Response {
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
                vertical_arrows: edit.field.offers_list(),
                ..Default::default()
            }),
    )
    .labelled_by(heading)
}

/// The ⏷ button in `rect`, beside the Category cell being edited, which lists every
/// category.
fn list_button(ui: &egui::Ui, i18n: &Localizer, rect: Rect, editor: Id) -> Response {
    let response = ui.interact(rect, editor.with("list_button"), Sense::click());
    let label = fl!(i18n, "register-category-list");
    response.widget_info(|| WidgetInfo::labeled(WidgetType::Button, ui.is_enabled(), &label));
    let visuals = ui.style().interact(&response);
    if response.hovered() || response.is_pointer_button_down_on() {
        ui.painter().rect_filled(rect, 0.0, visuals.weak_bg_fill);
    }
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        "⏷",
        TextStyle::Body.resolve(ui.style()),
        visuals.text_color(),
    );
    response
}

/// Draws `edit`'s list, while it is open, in a popup below `cell`, scrolled to show
/// a highlight that moved, and returns the row pressed this frame. A row is taken on
/// the press, since the release would come after the press has moved focus off the
/// editor.
fn list<'a>(
    ui: &egui::Ui,
    frame: &BodyFrame<'a>,
    edit: &mut CellEdit,
    cell: Rect,
) -> Option<ListRow<'a>> {
    let rows = edit.list_rows(frame.choices);
    let state = edit.list.as_mut().filter(|_| !rows.is_empty())?;
    let row_height = frame.theme.row_height();
    let pitch = row_height + ui.spacing().item_spacing.y;
    #[expect(
        clippy::cast_precision_loss,
        reason = "at most LIST_MAX_ROWS rows show at once"
    )]
    let height = rows.len().min(LIST_MAX_ROWS) as f32 * pitch - ui.spacing().item_spacing.y;
    let width = cell.width().max(LIST_MIN_WIDTH);
    let mut scroll = ScrollArea::vertical()
        .max_height(height)
        .min_scrolled_height(height);
    if let Some(offset) = state.reveal_offset(pitch, height) {
        scroll = scroll.vertical_scroll_offset(offset);
    }
    state.reveal = false;
    let highlight = state.highlight;
    let mut pressed = None;
    let output = Area::new(edit.editor_id().with("list"))
        .order(Order::Foreground)
        .fixed_pos(cell.left_bottom())
        .show(ui.ctx(), |ui| {
            Frame::popup(ui.style())
                .show(ui, |ui| {
                    ui.set_width(width);
                    scroll.show_rows(ui, row_height, rows.len(), |ui, in_view| {
                        for index in in_view {
                            // `show_rows` lays out indices below `rows.len()` only.
                            let row = &rows[index];
                            let highlighted = highlight == Some(index);
                            if list_row(ui, frame.i18n, row, highlighted, row_height) {
                                pressed = Some(row.clone());
                            }
                        }
                    })
                })
                .inner
        })
        .inner;
    if let Some(state) = &mut edit.list {
        state.offset = output.state.offset.y;
    }
    pressed
}

/// Draws one row of a combobox list, `row_height` high, and returns whether the
/// pointer pressed it this frame.
fn list_row(
    ui: &mut egui::Ui,
    i18n: &Localizer,
    row: &ListRow<'_>,
    highlighted: bool,
    row_height: f32,
) -> bool {
    let (rect, response) =
        ui.allocate_exact_size(vec2(ui.available_width(), row_height), Sense::click());
    let label = match row {
        ListRow::Choice(text) => (*text).to_owned(),
        ListRow::Create(name) => fl!(i18n, "register-create-category", name = name.clone()),
    };
    response.widget_info(|| {
        WidgetInfo::selected(
            WidgetType::SelectableLabel,
            ui.is_enabled(),
            highlighted,
            &label,
        )
    });
    ui.ctx().accesskit_node_builder(response.id, |node| {
        node.set_role(Role::ListBoxOption);
        node.set_selected(highlighted);
    });
    let visuals = ui.visuals();
    let (fill, text_color) = if highlighted {
        (
            Some(visuals.selection.bg_fill),
            visuals.selection.stroke.color,
        )
    } else if response.hovered() {
        (
            Some(visuals.widgets.hovered.weak_bg_fill),
            visuals.text_color(),
        )
    } else {
        (None, visuals.text_color())
    };
    if let Some(fill) = fill {
        ui.painter().rect_filled(rect, 0.0, fill);
    }
    let galley = WidgetText::from(label).into_galley(
        ui,
        Some(TextWrapMode::Truncate),
        rect.width(),
        TextStyle::Body,
    );
    let top = rect.center().y - galley.size().y / 2.0;
    ui.painter()
        .galley(pos2(rect.left(), top), galley, text_color);
    response.is_pointer_button_down_on()
}

/// Shows why `edit`'s last commit was refused, or else the result of the arithmetic
/// typed so far, in a bubble floating below its cell at `cell`, so the rows below
/// never move.
fn bubble(ui: &egui::Ui, i18n: &Localizer, edit: &CellEdit, cell: Rect) {
    let EditField::Amount { reader, currency } = &edit.field else {
        return;
    };
    let (text, colour) = match (edit.error, &edit.preview) {
        (Some(error), _) => (
            amount_field::error_text(i18n, error, reader, *currency),
            ui.visuals().error_fg_color,
        ),
        (None, Some(preview)) => (preview.text(i18n), ui.visuals().text_color()),
        (None, None) => return,
    };
    floating_bubble(ui, edit.editor_id().with("bubble"), cell, text, colour);
}

/// Shows `text` in `colour`, in a bubble with `id` floating below the cell at `cell`,
/// so the rows below never move.
fn floating_bubble(ui: &egui::Ui, id: Id, cell: Rect, text: String, colour: egui::Color32) {
    Area::new(id)
        .order(Order::Tooltip)
        .fixed_pos(cell.left_bottom() + vec2(0.0, ui.spacing().item_spacing.y))
        .interactable(false)
        .show(ui.ctx(), |ui| {
            Frame::popup(ui.style()).show(ui, |ui| {
                // `extend` keeps the bubble on one line near the window's edge.
                ui.add(Label::new(egui::RichText::new(text).color(colour)).extend());
            });
        });
}

/// The result of the arithmetic in `edit`'s text, or `None` when the text is a plain
/// amount or can't be read.
fn preview(i18n: &Localizer, edit: &CellEdit) -> Option<Preview> {
    let EditField::Amount { reader, currency } = &edit.field else {
        return None;
    };
    Preview::of(i18n, reader, *currency, &edit.text)
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

/// What drawing the header needs beside the heading itself.
#[derive(Clone, Copy)]
struct HeaderFrame<'a> {
    i18n: &'a Localizer,
    theme: &'a Theme,
    commands: &'a CommandSender,
    /// The columns every Register shows, which the column chooser changes.
    columns: &'a RegisterColumns,
}

/// A column heading on the header band.
struct Heading {
    /// The Register's id, which the heading's id derives from.
    register_id: Id,
    column: RegisterColumn,
    /// Space between the cell's edges and its text.
    insets: Insets,
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
            insets,
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
            insets,
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
    /// Space between the cell's edges and its text.
    insets: Insets,
    selected: bool,
    /// Whether the cell cursor is on this cell.
    cursor: bool,
    access: Access,
    /// The visible part of the Register's body.
    viewport: Rect,
}

impl Cell<'_> {
    /// Draws the cell, the whole of which is one click target, and returns the command
    /// picked from its Actions menu, if it has one.
    fn show(
        &self,
        ui: &egui::Ui,
        i18n: &Localizer,
        theme: &Theme,
    ) -> (Response, Option<RowAction>) {
        let Self {
            register_id,
            row,
            column,
            insets,
            selected,
            cursor,
            access,
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
            RegisterColumn::Amount => row
                .amount()
                .map_or_else(WidgetText::default, |amount| amount.styled(theme).into()),
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
                let mut action = None;
                // The draft row is no transaction yet, so it has no commands.
                let draft = row.id() == RowId::Draft;
                if (selected || hovered || open) && !draft {
                    action = actions_menu(ui, i18n, &response, access);
                } else {
                    // Hidden, the menu leaves an empty cell that selects the row.
                    cell_info(ui, &response, selected, "");
                }
                return (response, action);
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
        paint_text(ui, rect, column, insets, text);
        if cursor {
            ui.painter()
                .rect_stroke(rect, 0.0, theme.cell_cursor_stroke(), StrokeKind::Inside);
        }
        (response, None)
    }
}

/// The Today divider as `preferences` say, on `today`.
fn today_divider(preferences: &Preferences, today: Date<Iso>) -> TodayDivider {
    if preferences.today_divider {
        TodayDivider::Shown { today }
    } else {
        TodayDivider::Hidden
    }
}

/// Fills cell `ui` and half the gap around it with `stripe`, as `egui_extras` fills a
/// selected row, inside `viewport`, the visible part of the Register's body.
fn paint_stripe(ui: &egui::Ui, theme: &Theme, viewport: Rect, stripe: egui::Color32) {
    let rect = ui
        .max_rect()
        .expand2(0.5 * ui.spacing().item_spacing)
        .round_ui();
    ui.ctx()
        .layer_painter(ui.layer_id())
        .with_clip_rect(theme.rule_clip(ui.clip_rect(), viewport))
        .rect_filled(rect, 0.0, stripe);
}

/// One cell of the Today divider's line: a rule across the cell, beginning with the
/// divider's label in the first cell.
struct DividerCell<'a> {
    /// The Register's id, which the label's id derives from.
    register_id: Id,
    /// The label, in the first cell only.
    label: Option<&'a str>,
    /// Space between the cell's edges and the label.
    insets: Insets,
    /// The visible part of the Register's body, which the label may run across.
    viewport: Rect,
}

impl DividerCell<'_> {
    /// Draws the cell, its rule starting no further left than `rule_start`, and returns
    /// where the next cell's rule may start: past the label, which runs on over the
    /// cells beside it.
    fn show(&self, ui: &egui::Ui, theme: &Theme, rule_start: f32) -> f32 {
        let rect = ui.max_rect();
        let half_gap = ui.spacing().item_spacing.x / 2.0;
        let painter = ui
            .ctx()
            .layer_painter(ui.layer_id())
            .with_clip_rect(self.viewport);
        let mut rule_start = rule_start.max(rect.left() - half_gap);
        if let Some(label) = self.label {
            let galley = painter.layout_no_wrap(
                label.to_owned(),
                TextStyle::Body.resolve(ui.style()),
                theme.today_divider(),
            );
            let top_left = pos2(
                rect.left() + self.insets.left,
                rect.center().y - galley.size().y / 2.0,
            );
            let text = Rect::from_min_size(top_left, galley.size());
            let response = ui.interact(
                text,
                self.register_id.with("register_today_divider"),
                Sense::hover(),
            );
            response.widget_info(|| WidgetInfo::labeled(WidgetType::Label, true, label));
            painter.galley(top_left, galley, theme.today_divider());
            rule_start = text.right() + ui.spacing().item_spacing.x;
        }
        let right = rect.right() + half_gap;
        if rule_start < right {
            let rule = theme.today_divider_rule(
                Rangef::new(rule_start, right),
                rect.center().y,
                ui.ctx().pixels_per_point(),
            );
            painter.add(rule);
        }
        rule_start
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

/// Paints `text` in `rect`, inset by `insets` and aligned as `column` says, truncated
/// to fit.
/// Returns where the text went, unless `rect` is out of view.
fn paint_text(
    ui: &egui::Ui,
    rect: Rect,
    column: RegisterColumn,
    Insets { left, right }: Insets,
    text: WidgetText,
) -> Option<Rect> {
    if !ui.is_rect_visible(rect) {
        return None;
    }
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
/// the menu below it. The default fonts lack ⋯, so its dots are painted. Returns the
/// command picked from the menu. A read-only row's commands are disabled.
fn actions_menu(
    ui: &egui::Ui,
    i18n: &Localizer,
    response: &Response,
    access: Access,
) -> Option<RowAction> {
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
    Popup::menu(response)
        .show(|ui| {
            let delete = Button::new(fl!(i18n, "register-actions-delete"));
            ui.add_enabled(access == Access::Editable, delete)
                .clicked()
                .then_some(RowAction::Delete)
        })
        .and_then(|menu| menu.inner)
}

/// The column chooser: a ⚙ button on the right of the Actions heading cell `ui`, which
/// opens a list of the columns, each with buttons that move it and a checkbox that shows
/// or hides it. The list stays open until a click lands outside it. Returns `columns`
/// as the user changed them, in the frame they change. The button's id, and so the
/// list's, derives from `register_id`, so the list stays open when the cell moves.
fn column_chooser(
    ui: &egui::Ui,
    i18n: &Localizer,
    register_id: Id,
    columns: &RegisterColumns,
) -> Option<RegisterColumns> {
    let cell = ui.max_rect();
    let size = cell.height();
    let button = Rect::from_center_size(
        pos2(
            cell.right() - Theme::EDGE_INSET - size / 2.0,
            cell.center().y,
        ),
        vec2(size, size),
    );
    let id = register_id.with("register_column_chooser");
    let response = ui.interact(button, id, Sense::click());
    let label = fl!(i18n, "register-column-chooser");
    response.widget_info(|| WidgetInfo::labeled(WidgetType::Button, ui.is_enabled(), &label));
    if ui.is_rect_visible(button) {
        let visuals = ui.style().interact(&response);
        if response.hovered() || response.is_pointer_button_down_on() {
            ui.painter().rect_filled(button, 0.0, visuals.weak_bg_fill);
        }
        ui.painter().text(
            button.center(),
            Align2::CENTER_CENTER,
            "⚙",
            TextStyle::Body.resolve(ui.style()),
            visuals.text_color(),
        );
    }
    Popup::menu(&response)
        .close_behavior(PopupCloseBehavior::CloseOnClickOutside)
        .show(|ui| {
            let mut changed = columns.clone();
            let entries = columns.entries();
            let last = entries.len().saturating_sub(1);
            for (index, (column, shown)) in entries.into_iter().enumerate() {
                ui.horizontal(|ui| {
                    let title = column.title(i18n);
                    let up = fl!(i18n, "register-column-move-up", column = title.clone());
                    if icon_button(ui, "⏶", &up, index > 0).clicked() {
                        changed.move_up(column);
                    }
                    let down = fl!(i18n, "register-column-move-down", column = title.clone());
                    if icon_button(ui, "⏷", &down, index < last).clicked() {
                        changed.move_down(column);
                    }
                    let mut ticked = shown;
                    let checkbox = Checkbox::new(&mut ticked, title);
                    if ui.add_enabled(column.can_hide(), checkbox).changed() {
                        changed.set_shown(column, ticked);
                    }
                });
            }
            (changed != *columns).then_some(changed)
        })
        .and_then(|popup| popup.inner)
}

/// A button showing `glyph`, which assistive technology reads as `label`.
fn icon_button(ui: &mut egui::Ui, glyph: &str, label: &str, enabled: bool) -> Response {
    let response = ui.add_enabled(enabled, Button::new(glyph));
    response.widget_info(|| WidgetInfo::labeled(WidgetType::Button, enabled, label));
    response
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
                    register.show(
                        ui,
                        i18n,
                        theme,
                        commands,
                        summary,
                        view,
                        widths,
                        &Preferences::default(),
                        Date::try_new_iso(2026, 10, 14).expect("14 October 2026 is a date"),
                    );
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

    #[test]
    fn enter_on_the_create_row_asks_for_the_new_category_dialog_with_the_typed_name() {
        let (mut harness, receiver) = register();
        click_cell(&mut harness, "Auto & Transport:Fuel");
        harness.event(Event::Text("Pets".to_owned()));
        harness.run();

        press(&mut harness, Key::Enter);

        assert_eq!(
            receiver.try_iter().collect::<Vec<_>>(),
            [AppCommand::OpenDialog(DialogKind::NewCategory {
                name: "Pets".to_owned()
            })]
        );
    }

    #[test]
    fn a_refused_amount_sends_nothing() {
        let (mut harness, receiver) = register();
        click_cell(&mut harness, "-86.42");
        harness.event(Event::Text("12.345".to_owned()));
        harness.run();

        press(&mut harness, Key::Enter);

        assert_eq!(receiver.try_iter().collect::<Vec<_>>(), []);
    }

    #[test]
    fn an_amount_typed_as_the_saved_one_sends_nothing() {
        let (mut harness, receiver) = register();
        click_cell(&mut harness, "-86.42");
        harness.event(Event::Text("-86.420".to_owned()));
        harness.run();

        press(&mut harness, Key::Enter);

        assert_eq!(receiver.try_iter().collect::<Vec<_>>(), []);
    }

    #[test]
    fn a_post_the_ledger_never_applies_keeps_the_draft() {
        let (mut harness, receiver) = register();
        // The Register opens with the cell cursor on the draft row's Date cell.
        for c in "10/15/2026".chars() {
            harness.event(Event::Text(c.to_string()));
            harness.run();
        }
        press(&mut harness, Key::Enter);
        press(&mut harness, Key::ArrowRight);
        harness.event(Event::Text("Corner Cafe".to_owned()));
        harness.run();
        press(&mut harness, Key::Enter);
        harness.event(Event::Text("Food & Dining:Groceries".to_owned()));
        harness.run();
        press(&mut harness, Key::Enter);
        press(&mut harness, Key::ArrowRight);
        harness.event(Event::Text("-12".to_owned()));
        harness.run();

        harness.key_press_modifiers(Modifiers::COMMAND, Key::Enter);
        harness.run();

        let posted = receiver.try_iter().any(|command| {
            matches!(
                command,
                AppCommand::Domain(DomainCommand::PostTransaction { .. })
            )
        });
        assert!(posted);
        // The snapshot never changes, as when the ledger rejects the post.
        harness.get_by_role_and_label(Role::Cell, "Corner Cafe");
    }
}
