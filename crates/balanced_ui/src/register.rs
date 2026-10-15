//! The Register: one account's transactions in a table, one row each, with the running
//! balance after each.

use balanced_core::{AccountSummary, LedgerView, TransactionId};
use balanced_i18n::Localizer;
use eframe::egui::accesskit::{self, Role};
use eframe::egui::{
    self, Align, CursorIcon, Id, Layout, Popup, Rect, Response, ScrollArea, Sense, Shape, Stroke,
    TextStyle, TextWrapMode, WidgetInfo, WidgetText, WidgetType, pos2, vec2,
};
use egui_extras::{Column, TableBuilder};

use crate::i18n::fl;
use crate::register_model::{RegisterModel, RegisterRow, RegisterSort, SortDirection, SortKey};
use crate::theme::Theme;

/// The narrowest Payee and Category get before the Register scrolls sideways.
const MIN_FLEX_WIDTH: f32 = 120.0;

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

    /// How wide the column is. Payee and Category share the width the others leave,
    /// down to [`MIN_FLEX_WIDTH`]; below that the Register scrolls sideways.
    // TODO(#101): take the widths from the column chooser's saved layout.
    fn width(self, theme: &Theme) -> Column {
        let column = match self {
            Self::Date => Column::exact(80.0),
            Self::Number => Column::exact(48.0),
            Self::Payee | Self::Category => Column::remainder().at_least(MIN_FLEX_WIDTH),
            Self::Memo => Column::exact(140.0),
            Self::Amount => Column::exact(90.0),
            Self::Balance => Column::exact(100.0),
            Self::Actions => Column::exact(theme.row_height() + Theme::EDGE_INSET),
        };
        column.clip(true)
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

/// Space between a cell's left and right edges and its text, in points.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Insets {
    left: f32,
    right: f32,
}

/// The open account's Register: its cached rows, its sort and the selected
/// transaction. Opening another account sorts it by date and clears the selection. None
/// of it outlives the run.
#[derive(Debug, Default)]
pub(crate) struct Register {
    /// The rows of the last account shown, rebuilt when they are out of date.
    model: Option<RegisterModel>,
    sort: RegisterSort,
    /// Whether the sort changed since the rows were last shown, so the selected row
    /// scrolls to the middle of the Register.
    sorted: bool,
    selected: Option<TransactionId>,
}

impl Register {
    /// Draws the Register of `summary`, an account in `view`, filling the space left in
    /// `ui`. Only the rows in view are laid out.
    pub(crate) fn show(
        &mut self,
        ui: &mut egui::Ui,
        i18n: &Localizer,
        theme: &Theme,
        summary: &AccountSummary,
        view: &LedgerView,
    ) {
        let account = summary.account().id();
        if self
            .model
            .as_ref()
            .is_some_and(|model| model.account() != account)
        {
            self.sort = RegisterSort::default();
            self.selected = None;
        }
        let model = match self.model.take() {
            Some(model) if model.is_current(account, view, self.sort, i18n) => model,
            _ => RegisterModel::new(summary, view, self.sort, i18n),
        };
        let row_height = theme.row_height();
        let centre_on = self
            .selected
            .filter(|_| std::mem::take(&mut self.sorted))
            .and_then(|id| model.rows().iter().position(|row| row.id() == id));
        ScrollArea::horizontal()
            .id_salt(("register_sideways", account))
            .auto_shrink(false)
            .show(ui, |ui| {
                let viewport = ui.clip_rect();
                let register_id = ui.id();
                let scroll_offset = centre_on.map(|index| {
                    // The body is what the header row leaves, and each row takes its
                    // height and one gap.
                    let gap = ui.spacing().item_spacing.y;
                    let body = ui.available_height() - row_height - gap;
                    #[expect(
                        clippy::cast_precision_loss,
                        reason = "row indices stay far below 2^24, where f32 is exact"
                    )]
                    let top = index as f32 * (row_height + gap);
                    (top - (body - row_height) / 2.0).max(0.0)
                });
                let mut table = TableBuilder::new(ui)
                    .id_salt(("register", account))
                    .striped(theme.alternating_rows())
                    .resizable(false)
                    .cell_layout(Layout::left_to_right(Align::Center))
                    .auto_shrink(false)
                    .max_scroll_height(f32::INFINITY);
                for column in RegisterColumn::ALL {
                    table = table.column(column.width(theme));
                }
                if let Some(offset) = scroll_offset {
                    table = table.vertical_scroll_offset(offset);
                }
                table
                    .header(row_height, |mut header| {
                        for column in RegisterColumn::ALL {
                            header.col(|ui| {
                                let heading = Heading {
                                    column,
                                    sort: self.sort,
                                    viewport,
                                };
                                if let Some(key) = heading.show(ui, i18n, theme) {
                                    self.sort = self.sort.clicked(key);
                                    self.sorted = true;
                                    // The rows sort and scroll on the next frame.
                                    ui.ctx().request_repaint();
                                }
                            });
                        }
                    })
                    .body(|mut body| {
                        let body_viewport = body.ui_mut().clip_rect();
                        let rows = model.rows();
                        body.rows(row_height, rows.len(), |mut table_row| {
                            // `rows` lays out indices below `rows.len()` only.
                            let row = &rows[table_row.index()];
                            let selected = self.selected == Some(row.id());
                            table_row.set_selected(selected);
                            for (index, column) in RegisterColumn::ALL.into_iter().enumerate() {
                                table_row.col(|ui| {
                                    let cell = Cell {
                                        register_id,
                                        row,
                                        column,
                                        selected,
                                        viewport: body_viewport,
                                    };
                                    if cell.show(ui, i18n, theme).clicked() {
                                        self.selected = Some(row.id());
                                    }
                                    let rules = theme.cell_rules(
                                        ui.max_rect(),
                                        index,
                                        ui.ctx().pixels_per_point(),
                                    );
                                    ui.ctx()
                                        .layer_painter(ui.layer_id())
                                        .with_clip_rect(
                                            theme.rule_clip(ui.clip_rect(), body_viewport),
                                        )
                                        .extend(rules);
                                });
                            }
                        });
                    });
            });
        self.model = Some(model);
    }
}

/// A column heading on the header band.
struct Heading {
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
            column,
            sort,
            viewport,
        } = *self;
        let rect = ui.max_rect();
        let key = column.sort_key();
        let sense = if key.is_some() {
            Sense::click()
        } else {
            Sense::hover()
        };
        let mut response = ui.interact(rect, ui.id().with("register_heading"), sense);
        let fill = if key.is_some() && response.hovered() {
            ui.visuals().widgets.hovered.weak_bg_fill
        } else {
            theme.header_fill()
        };
        let band = rect.expand2(ui.spacing().item_spacing / 2.0);
        ui.ctx()
            .layer_painter(ui.layer_id())
            .with_clip_rect(band.intersect(viewport))
            .rect_filled(band, 0.0, fill);
        if key.is_some() {
            response = response.on_hover_cursor(CursorIcon::PointingHand);
        }
        let direction = key
            .filter(|&key| key == sort.key())
            .map(|_| sort.direction());
        let title = column.title(i18n);
        response.widget_info(|| WidgetInfo::labeled(WidgetType::Other, ui.is_enabled(), &title));
        ui.ctx().accesskit_node_builder(response.id, |node| {
            node.set_role(Role::ColumnHeader);
            if let Some(direction) = direction {
                node.set_sort_direction(match direction {
                    SortDirection::Ascending => accesskit::SortDirection::Ascending,
                    SortDirection::Descending => accesskit::SortDirection::Descending,
                });
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
        ui.ctx()
            .accesskit_node_builder(response.id, |node| node.set_role(Role::Cell));
        paint_text(ui, rect, column, text);
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
