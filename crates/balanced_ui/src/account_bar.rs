//! The Account bar: the accounts in their fixed groups with their balances, group
//! headings that fold, and Net Worth in a footer.

use std::collections::HashSet;
use std::sync::Arc;

use balanced_core::AccountId;
use balanced_i18n::Localizer;
use eframe::egui::{
    self, CentralPanel, Color32, Frame, Galley, Panel, Rect, Response, RichText, ScrollArea, Sense,
    Shape, Stroke, TextStyle, TextWrapMode, WidgetInfo, WidgetText, WidgetType, pos2, vec2,
};

use crate::accounts_model::{AccountRow, AccountsModel, FormattedAmount, Section};
use crate::command::{AppCommand, CommandSender};
use crate::i18n::fl;
use crate::route::Route;
use crate::theme::Theme;

/// Which Account bar sections are folded. None of it outlives the run yet: every run
/// starts with only Closed folded.
// TODO(#102): remember the folded sections between runs.
#[derive(Debug)]
pub(crate) struct AccountBar {
    folded: HashSet<Section>,
}

impl Default for AccountBar {
    fn default() -> Self {
        Self {
            folded: HashSet::from([Section::Closed]),
        }
    }
}

impl AccountBar {
    /// Draws the Account bar from `model`, highlighting `selected`. Clicking an
    /// account opens it; clicking a heading folds or unfolds its section.
    pub(crate) fn show(
        &mut self,
        ui: &mut egui::Ui,
        i18n: &Localizer,
        theme: &Theme,
        commands: &CommandSender,
        model: &AccountsModel,
        selected: Option<AccountId>,
    ) {
        Panel::left("account_bar")
            .frame(theme.chrome_frame())
            .show(ui, |ui| {
                Panel::bottom("net_worth")
                    .frame(theme.footer_frame())
                    .show(ui, |ui| Self::net_worth(ui, i18n, theme, model.net_worth()));
                CentralPanel::default().frame(Frame::NONE).show(ui, |ui| {
                    ScrollArea::vertical().show(ui, |ui| {
                        self.sections(ui, i18n, theme, commands, model, selected);
                    });
                });
            });
    }

    fn sections(
        &mut self,
        ui: &mut egui::Ui,
        i18n: &Localizer,
        theme: &Theme,
        commands: &CommandSender,
        model: &AccountsModel,
        selected: Option<AccountId>,
    ) {
        for section_rows in model.sections() {
            let section = section_rows.section();
            let fold = if self.folded.contains(&section) {
                Fold::Folded
            } else {
                Fold::Open
            };
            if heading(ui, section, section.label(i18n), fold).clicked() {
                match fold {
                    Fold::Open => self.folded.insert(section),
                    Fold::Folded => self.folded.remove(&section),
                };
            }
            match fold {
                Fold::Folded => continue,
                Fold::Open => {}
            }
            for row in section_rows.rows() {
                let balance = model
                    .balance(row.id())
                    .expect("the model formats a balance for every account it lists");
                if account_row(ui, theme, row, balance, selected == Some(row.id())).clicked() {
                    commands.send(AppCommand::Navigate(Route::Account(row.id())));
                }
            }
        }
    }

    fn net_worth(ui: &mut egui::Ui, i18n: &Localizer, theme: &Theme, net_worth: &FormattedAmount) {
        ui.horizontal(|ui| {
            ui.label(fl!(i18n, "account-bar-net-worth"));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(net_worth.styled(theme));
            });
        });
    }
}

/// Whether a section shows its accounts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Fold {
    Open,
    Folded,
}

/// The width of the fold triangle's column, which also indents the account rows.
fn icon_column(ui: &egui::Ui) -> f32 {
    ui.spacing().icon_width + ui.spacing().item_spacing.x
}

/// Takes the full width of the Account bar, `height` tall, as one click target whose
/// id comes from `id_salt`, so its state stays with it when sections fold above it.
fn full_width_target(ui: &mut egui::Ui, id_salt: impl egui::AsIdSalt, height: f32) -> Response {
    let (_, rect) = ui.allocate_space(vec2(ui.available_width(), height));
    let id = ui.make_persistent_id(id_salt);
    ui.interact(rect, id, Sense::click())
}

/// Paints `galley` at `left`, centred vertically in `row`.
fn paint_centred(ui: &egui::Ui, galley: Arc<Galley>, left: f32, row: Rect, color: Color32) {
    let top = row.center().y - galley.size().y / 2.0;
    ui.painter().galley(pos2(left, top), galley, color);
}

/// A section heading: a fold triangle and the section's name, the full width of the
/// Account bar.
fn heading(ui: &mut egui::Ui, section: Section, label: String, fold: Fold) -> Response {
    let response = full_width_target(
        ui,
        ("account_bar_heading", section),
        ui.spacing().interact_size.y,
    );
    let enabled = ui.is_enabled();
    let open = fold == Fold::Open;
    response
        .widget_info(|| WidgetInfo::selected(WidgetType::CollapsingHeader, enabled, open, &label));
    let rect = response.rect;
    if ui.is_rect_visible(rect) {
        let visuals = ui.style().interact(&response);
        if response.hovered() {
            ui.painter().rect_filled(rect, 0.0, visuals.weak_bg_fill);
        }
        let icon_size = ui.spacing().icon_width / 2.0;
        let icon = Rect::from_center_size(
            pos2(rect.left() + ui.spacing().icon_width / 2.0, rect.center().y),
            vec2(icon_size, icon_size),
        );
        ui.painter()
            .add(fold_triangle(icon, fold, visuals.text_color()));
        let text_left = rect.left() + icon_column(ui);
        let galley = WidgetText::from(RichText::new(label).strong()).into_galley(
            ui,
            Some(TextWrapMode::Truncate),
            rect.right() - text_left,
            TextStyle::Body,
        );
        paint_centred(ui, galley, text_left, rect, visuals.text_color());
    }
    response
}

/// ▾ when the section is open and ▸ when it is folded, filling `icon`. The default
/// fonts lack both glyphs, so the triangle is painted.
fn fold_triangle(icon: Rect, fold: Fold, color: Color32) -> Shape {
    let points = match fold {
        Fold::Open => vec![icon.left_top(), icon.right_top(), icon.center_bottom()],
        Fold::Folded => vec![icon.left_top(), icon.right_center(), icon.left_bottom()],
    };
    Shape::convex_polygon(points, color, Stroke::NONE)
}

/// An account row: the name on the left and the balance on the right, indented under
/// its heading. The whole row is one click target, and its text can't be selected.
fn account_row(
    ui: &mut egui::Ui,
    theme: &Theme,
    row: &AccountRow,
    balance: &FormattedAmount,
    selected: bool,
) -> Response {
    let response = full_width_target(ui, ("account_bar_row", row.id()), theme.row_height());
    let enabled = ui.is_enabled();
    response.widget_info(|| {
        WidgetInfo::selected(WidgetType::SelectableLabel, enabled, selected, row.name())
    });
    let rect = response.rect;
    if !ui.is_rect_visible(rect) {
        return response;
    }
    let indent = rect.left() + icon_column(ui);
    let gap = ui.spacing().item_spacing;
    if row.starts_sub_group() {
        let rule_y = rect.top() - gap.y / 2.0;
        ui.painter().add(theme.sub_group_rule(
            egui::Rangef::new(indent, rect.right()),
            rule_y,
            ui.pixels_per_point(),
        ));
    }
    let visuals = ui.style().interact_selectable(&response, selected);
    if selected || response.hovered() || response.is_pointer_button_down_on() {
        ui.painter().rect_filled(rect, 0.0, visuals.weak_bg_fill);
    }
    let text_color = visuals.text_color();
    let balance_galley = WidgetText::from(balance.styled(theme)).into_galley(
        ui,
        Some(TextWrapMode::Extend),
        f32::INFINITY,
        TextStyle::Body,
    );
    let balance_left = rect.right() - gap.x - balance_galley.size().x;
    let name_galley = WidgetText::from(row.name()).into_galley(
        ui,
        Some(TextWrapMode::Truncate),
        (balance_left - gap.x - indent).max(0.0),
        TextStyle::Body,
    );
    paint_centred(ui, name_galley, indent, rect, text_color);
    paint_centred(ui, balance_galley, balance_left, rect, text_color);
    response
}
