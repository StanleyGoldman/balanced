//! The Account bar: the accounts in their fixed groups with their balances, group
//! headings that fold, and Net Worth in a footer.

use std::collections::HashSet;

use balanced_core::AccountId;
use balanced_i18n::Localizer;
use eframe::egui::{
    self, CentralPanel, Frame, Panel, Rect, Response, RichText, ScrollArea, Sense, Shape, Stroke,
    TextStyle, TextWrapMode, WidgetInfo, WidgetText, WidgetType, pos2, vec2,
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
        for section in model.sections() {
            let open = !self.folded.contains(&section.section);
            if heading(ui, section.section.label(i18n), open).clicked() {
                if open {
                    self.folded.insert(section.section);
                } else {
                    self.folded.remove(&section.section);
                }
            }
            if !open {
                continue;
            }
            for row in &section.rows {
                let balance = model
                    .balance(row.id)
                    .expect("the model formats a balance for every account it lists");
                if account_row(ui, theme, row, balance, selected == Some(row.id)).clicked() {
                    commands.send(AppCommand::Navigate(Route::Account(row.id)));
                }
            }
        }
    }

    fn net_worth(ui: &mut egui::Ui, i18n: &Localizer, theme: &Theme, net_worth: &FormattedAmount) {
        ui.horizontal(|ui| {
            ui.label(fl!(i18n, "account-bar-net-worth"));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(theme.amount(&net_worth.money, net_worth.text.clone()));
            });
        });
    }
}

/// The width of the fold triangle's column, which also indents the account rows.
fn icon_column(ui: &egui::Ui) -> f32 {
    ui.spacing().icon_width + ui.spacing().item_spacing.x
}

/// A section heading: a fold triangle and the section's name, the full width of the
/// Account bar.
fn heading(ui: &mut egui::Ui, label: String, open: bool) -> Response {
    let height = ui.spacing().interact_size.y;
    let (rect, response) =
        ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::click());
    let enabled = ui.is_enabled();
    response
        .widget_info(|| WidgetInfo::selected(WidgetType::CollapsingHeader, enabled, open, &label));
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
            .add(fold_triangle(icon, open, visuals.text_color()));
        let text_left = rect.left() + icon_column(ui);
        let galley = WidgetText::from(RichText::new(label).strong()).into_galley(
            ui,
            Some(TextWrapMode::Truncate),
            rect.right() - text_left,
            TextStyle::Body,
        );
        let top = rect.center().y - galley.size().y / 2.0;
        ui.painter()
            .galley(pos2(text_left, top), galley, visuals.text_color());
    }
    response
}

/// ▾ when the section is open and ▸ when it is folded, filling `icon`. The default
/// fonts lack both glyphs, so the triangle is painted.
fn fold_triangle(icon: Rect, open: bool, color: egui::Color32) -> Shape {
    let points = if open {
        vec![icon.left_top(), icon.right_top(), icon.center_bottom()]
    } else {
        vec![icon.left_top(), icon.right_center(), icon.left_bottom()]
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
    let (rect, response) = ui.allocate_exact_size(
        vec2(ui.available_width(), theme.row_height()),
        Sense::click(),
    );
    let enabled = ui.is_enabled();
    response.widget_info(|| {
        WidgetInfo::selected(WidgetType::SelectableLabel, enabled, selected, &row.name)
    });
    if !ui.is_rect_visible(rect) {
        return response;
    }
    let indent = rect.left() + icon_column(ui);
    let gap = ui.spacing().item_spacing;
    if row.starts_sub_group {
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
    let balance_galley = WidgetText::from(theme.amount(&balance.money, balance.text.clone()))
        .into_galley(
            ui,
            Some(TextWrapMode::Extend),
            f32::INFINITY,
            TextStyle::Body,
        );
    let balance_left = rect.right() - gap.x - balance_galley.size().x;
    let name_galley = WidgetText::from(row.name.as_str()).into_galley(
        ui,
        Some(TextWrapMode::Truncate),
        (balance_left - gap.x - indent).max(0.0),
        TextStyle::Body,
    );
    let centred = |height: f32| rect.center().y - height / 2.0;
    let name_top = centred(name_galley.size().y);
    let balance_top = centred(balance_galley.size().y);
    ui.painter()
        .galley(pos2(indent, name_top), name_galley, text_color);
    ui.painter()
        .galley(pos2(balance_left, balance_top), balance_galley, text_color);
    response
}
