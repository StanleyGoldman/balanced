// SPDX-License-Identifier: GPL-3.0-or-later WITH AdditionRef-Balanced-Plugin-Exception

//! A square button showing a painted triangle, for arrows egui's fonts lack.

use eframe::egui::{
    self, Color32, Rect, Response, Sense, Shape, Stroke, WidgetInfo, WidgetType, vec2,
};

/// Which way a triangle button's triangle points.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Direction {
    Left,
    Right,
    Up,
    Down,
}

/// Whether a heading or a tree row shows what is under it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Fold {
    Open,
    Folded,
}

/// ▾ when what is under it shows and ▸ when it is folded, filling `icon`. The default
/// fonts lack both glyphs, so the triangle is painted.
pub(crate) fn fold_triangle(icon: Rect, fold: Fold, color: Color32) -> Shape {
    let points = match fold {
        Fold::Open => vec![icon.left_top(), icon.right_top(), icon.center_bottom()],
        Fold::Folded => vec![icon.left_top(), icon.right_center(), icon.left_bottom()],
    };
    Shape::convex_polygon(points, color, Stroke::NONE)
}

/// A square button labelled `label` for AccessKit and in its tooltip, showing a
/// triangle pointing `direction`. The triangle is painted, as the fold triangles are,
/// because egui's default fonts have no arrows.
pub(crate) fn triangle_button(ui: &mut egui::Ui, label: String, direction: Direction) -> Response {
    let size = ui.spacing().interact_size.y;
    let (rect, response) = ui.allocate_exact_size(vec2(size, size), Sense::click());
    let enabled = ui.is_enabled();
    response.widget_info(|| WidgetInfo::labeled(WidgetType::Button, enabled, &label));
    if ui.is_rect_visible(rect) {
        let visuals = ui.style().interact(&response);
        if response.hovered() || response.is_pointer_button_down_on() {
            ui.painter().rect_filled(rect, 0.0, visuals.weak_bg_fill);
        }
        // Each triangle's base is half the button wide, and its tip a third of the button
        // from the base.
        let sideways = Rect::from_center_size(rect.center(), vec2(size / 3.0, size / 2.0));
        let upright = Rect::from_center_size(rect.center(), vec2(size / 2.0, size / 3.0));
        let points = match direction {
            Direction::Left => vec![
                sideways.right_top(),
                sideways.left_center(),
                sideways.right_bottom(),
            ],
            Direction::Right => vec![
                sideways.left_top(),
                sideways.right_center(),
                sideways.left_bottom(),
            ],
            Direction::Up => vec![
                upright.left_bottom(),
                upright.center_top(),
                upright.right_bottom(),
            ],
            Direction::Down => vec![
                upright.left_top(),
                upright.center_bottom(),
                upright.right_top(),
            ],
        };
        ui.painter().add(Shape::convex_polygon(
            points,
            visuals.text_color(),
            Stroke::NONE,
        ));
    }
    response.on_hover_text(label)
}
