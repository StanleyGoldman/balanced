//! The Register: one account's transactions in a table, one row each, with the running
//! balance after each.

use balanced_core::{AccountSummary, LedgerView, TransactionId};
use balanced_i18n::Localizer;
use eframe::egui::accesskit::Role;
use eframe::egui::{
    self, Align, Id, Layout, Popup, Rect, Response, ScrollArea, Sense, TextStyle, TextWrapMode,
    WidgetInfo, WidgetText, WidgetType, pos2, vec2,
};
use egui_extras::{Column, TableBuilder};

use crate::i18n::fl;
use crate::register_model::{RegisterModel, RegisterRow};
use crate::theme::Theme;

/// The narrowest Payee and Category get before the Register scrolls sideways.
const MIN_FLEX_WIDTH: f32 = 120.0;

/// The radius of each dot of the Actions menu's ⋯, and the space between their centres.
const ACTION_DOT_RADIUS: f32 = 1.5;
const ACTION_DOT_SPACING: f32 = 4.5;

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
    fn insets(self) -> (f32, f32) {
        match self {
            Self::Date => (Theme::EDGE_INSET, 0.0),
            Self::Actions => (0.0, Theme::EDGE_INSET),
            Self::Number
            | Self::Payee
            | Self::Category
            | Self::Memo
            | Self::Amount
            | Self::Balance => (0.0, 0.0),
        }
    }
}

/// The open account's Register: its cached rows and the selected transaction. None of
/// it outlives the run.
#[derive(Debug, Default)]
pub(crate) struct Register {
    /// The rows of the last account shown, rebuilt when they are out of date.
    model: Option<RegisterModel>,
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
        let model = match self.model.take() {
            Some(model) if model.is_current(account, view, i18n) => model,
            _ => RegisterModel::new(summary, view, i18n),
        };
        let row_height = theme.row_height();
        ScrollArea::horizontal()
            .id_salt(("register_sideways", account))
            .auto_shrink(false)
            .show(ui, |ui| {
                let viewport = ui.clip_rect();
                let register_id = ui.id();
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
                table
                    .header(row_height, |mut header| {
                        for column in RegisterColumn::ALL {
                            header.col(|ui| header_cell(ui, theme, viewport, column, i18n));
                        }
                    })
                    .body(|mut body| {
                        let body_viewport = body.ui_mut().clip_rect();
                        let rows = model.rows();
                        body.rows(row_height, rows.len(), |mut table_row| {
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

/// A column heading on the header band. The band's fill covers the cell and half the
/// gap around it, clipped to the Register's viewport so it never paints past it.
fn header_cell(
    ui: &egui::Ui,
    theme: &Theme,
    viewport: Rect,
    column: RegisterColumn,
    i18n: &Localizer,
) {
    let rect = ui.max_rect();
    let band = rect.expand2(ui.spacing().item_spacing / 2.0);
    ui.ctx()
        .layer_painter(ui.layer_id())
        .with_clip_rect(band.intersect(viewport))
        .rect_filled(band, 0.0, theme.header_fill());
    let title = column.title(i18n);
    let response = ui.interact(rect, ui.id().with("register_heading"), Sense::hover());
    response.widget_info(|| WidgetInfo::labeled(WidgetType::Other, ui.is_enabled(), &title));
    ui.ctx()
        .accesskit_node_builder(response.id, |node| node.set_role(Role::ColumnHeader));
    paint_text(
        ui,
        rect,
        column,
        WidgetText::from(egui::RichText::new(title).strong()),
    );
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
            RegisterColumn::Balance => row.balance().styled(theme).into(),
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

/// Paints `text` in `rect`, inset and aligned as `column` says, truncated to fit.
fn paint_text(ui: &egui::Ui, rect: Rect, column: RegisterColumn, text: WidgetText) {
    if !ui.is_rect_visible(rect) {
        return;
    }
    let (left, right) = column.insets();
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
    ui.painter()
        .galley(pos2(x, top), galley, ui.visuals().text_color());
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
