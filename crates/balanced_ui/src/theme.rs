//! The theme: Balanced's one look, in light and dark, shaped by the Preferences that
//! affect it. This is the one module that builds colours.

#![expect(
    clippy::disallowed_methods,
    reason = "the theme module defines the palette"
)]

use balanced_core::Money;
use eframe::egui;
use egui::{
    Color32, CornerRadius, FontFamily, FontId, Frame, Margin, Rect, RichText, Shape, Stroke,
    TextStyle, Vec2, vec2,
};

/// Light or dark colours.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum Scheme {
    #[default]
    Light,
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "the Preferences dialog (#99) is the first to choose it"
        )
    )]
    Dark,
}

/// How tightly rows and controls are packed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum Density {
    #[default]
    Normal,
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "the Preferences dialog (#99) is the first to choose it"
        )
    )]
    Dense,
}

/// Which rules the Register draws between its cells.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum Gridlines {
    #[default]
    Off,
    /// A rule between rows.
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "the Register Preferences (#100) are the first to choose them"
        )
    )]
    Horizontal,
    /// A rule between columns.
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "the Register Preferences (#100) are the first to choose them"
        )
    )]
    Vertical,
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "the Register Preferences (#100) are the first to choose them"
        )
    )]
    Both,
}

/// The Preferences that shape the look.
// TODO(#48): read these from the stored Preferences, and add the font face once the
// faces on offer are decided.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ThemeSettings {
    pub(crate) scheme: Scheme,
    pub(crate) density: Density,
    pub(crate) gridlines: Gridlines,
    /// Fill every other Register row with the stripe colour.
    pub(crate) alternating_rows: bool,
    /// Draw amounts in the monospace font, so their digits line up in a column.
    pub(crate) monospace_money: bool,
    /// Draw negative amounts in the negative colour.
    pub(crate) coloured_negatives: bool,
}

impl Default for ThemeSettings {
    fn default() -> Self {
        Self {
            scheme: Scheme::default(),
            density: Density::default(),
            gridlines: Gridlines::default(),
            alternating_rows: true,
            monospace_money: false,
            coloured_negatives: true,
        }
    }
}

/// Body text size, in points.
const BODY_TEXT: f32 = 13.0;

/// Horizontal space between widgets, and between Register columns.
const COLUMN_GAP: f32 = 8.0;

/// The most vertical padding a button gets, so it fits inside a row.
const MAX_BUTTON_PADDING_Y: f32 = 3.0;

/// Space around the text of the Net Worth footer.
#[expect(dead_code, reason = "the Account bar (#84) draws the Net Worth footer")]
const FOOTER_MARGIN: Margin = Margin::symmetric(8, 6);

/// The colours of one [`Scheme`].
#[derive(Debug, Clone, Copy, PartialEq)]
struct Palette {
    /// Tinted panels around the content: the Menu bar, Account bar and Status bar.
    chrome: Color32,
    /// The content background, such as the Register's rows.
    body: Color32,
    /// Column headers and buttons at rest.
    header: Color32,
    /// The fill of every other row when alternating rows are on.
    stripe: Color32,
    /// Gridlines and widget outlines.
    grid: Color32,
    /// Links, focus and the outline of a hovered or pressed widget.
    accent: Color32,
    /// The soft tint behind selected text and rows.
    selection: Color32,
    text: Color32,
    /// Negative amounts, when coloured negatives are on.
    negative: Color32,
}

impl Palette {
    fn of(scheme: Scheme) -> Self {
        let rgb = Color32::from_rgb;
        match scheme {
            Scheme::Light => Self {
                chrome: rgb(220, 228, 238),
                body: rgb(255, 255, 255),
                header: rgb(205, 215, 230),
                stripe: rgb(238, 243, 250),
                grid: rgb(200, 208, 220),
                accent: rgb(31, 94, 168),
                selection: rgb(200, 218, 240),
                text: rgb(28, 32, 38),
                negative: rgb(190, 30, 30),
            },
            Scheme::Dark => Self {
                chrome: rgb(30, 36, 46),
                body: rgb(22, 26, 34),
                header: rgb(40, 50, 66),
                stripe: rgb(28, 34, 44),
                grid: rgb(52, 60, 74),
                accent: rgb(96, 160, 235),
                selection: rgb(40, 70, 110),
                text: rgb(222, 228, 236),
                negative: rgb(255, 110, 110),
            },
        }
    }
}

/// The sizes that change with [`Density`].
#[derive(Debug, Clone, Copy, PartialEq)]
struct Sizes {
    /// Register row height, which is also the height of a button or text field.
    row: f32,
    /// Vertical space between widgets.
    gap: f32,
    /// Horizontal padding inside a button.
    button_padding: f32,
    /// Space inside a menu.
    menu_margin: i8,
}

impl Sizes {
    fn of(density: Density) -> Self {
        match density {
            Density::Normal => Self {
                row: 20.0,
                gap: 4.0,
                button_padding: 8.0,
                menu_margin: 6,
            },
            Density::Dense => Self {
                row: 16.0,
                gap: 2.0,
                button_padding: 6.0,
                menu_margin: 4,
            },
        }
    }
}

/// The look built from a [`ThemeSettings`].
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Theme {
    settings: ThemeSettings,
    palette: Palette,
    sizes: Sizes,
}

impl Theme {
    /// Space between the Register's edge and the text in its first and last columns.
    #[expect(dead_code, reason = "the Register (#85) is the first caller")]
    pub(crate) const EDGE_INSET: f32 = 6.0;

    pub(crate) fn new(settings: ThemeSettings) -> Self {
        Self {
            settings,
            palette: Palette::of(settings.scheme),
            sizes: Sizes::of(settings.density),
        }
    }

    /// The egui style for every window.
    pub(crate) fn style(&self) -> egui::Style {
        let Self {
            settings,
            palette,
            sizes,
        } = self;
        let mut style = egui::Style {
            visuals: match settings.scheme {
                Scheme::Light => egui::Visuals::light(),
                Scheme::Dark => egui::Visuals::dark(),
            },
            ..egui::Style::default()
        };
        style.interaction.selectable_labels = false;
        style.text_styles = [
            (
                TextStyle::Small,
                FontId::new(BODY_TEXT - 2.0, FontFamily::Proportional),
            ),
            (
                TextStyle::Body,
                FontId::new(BODY_TEXT, FontFamily::Proportional),
            ),
            (
                TextStyle::Button,
                FontId::new(BODY_TEXT, FontFamily::Proportional),
            ),
            (
                TextStyle::Heading,
                FontId::new(BODY_TEXT + 5.0, FontFamily::Proportional),
            ),
            (
                TextStyle::Monospace,
                FontId::new(BODY_TEXT, FontFamily::Monospace),
            ),
        ]
        .into();

        let spacing = &mut style.spacing;
        spacing.interact_size.y = sizes.row;
        spacing.item_spacing = self.item_spacing();
        spacing.button_padding = vec2(sizes.button_padding, sizes.gap.min(MAX_BUTTON_PADDING_Y));
        spacing.menu_margin = Margin::same(sizes.menu_margin);

        let visuals = &mut style.visuals;
        visuals.panel_fill = palette.body;
        visuals.window_fill = palette.body;
        visuals.extreme_bg_color = palette.body;
        visuals.faint_bg_color = palette.stripe;
        visuals.error_fg_color = palette.negative;
        visuals.hyperlink_color = palette.accent;
        visuals.window_corner_radius = CornerRadius::ZERO;
        visuals.menu_corner_radius = CornerRadius::ZERO;
        visuals.window_stroke = Stroke::new(1.0, palette.grid);
        visuals.selection.bg_fill = palette.selection;
        visuals.selection.stroke = Stroke::new(1.0, palette.text);

        let hover = palette.selection.gamma_multiply(0.6);
        let widgets = &mut visuals.widgets;
        for state in [
            &mut widgets.noninteractive,
            &mut widgets.inactive,
            &mut widgets.hovered,
            &mut widgets.active,
            &mut widgets.open,
        ] {
            state.corner_radius = CornerRadius::ZERO;
            state.fg_stroke.color = palette.text;
        }
        widgets.noninteractive.bg_stroke = Stroke::new(1.0, palette.grid);
        widgets.inactive.weak_bg_fill = palette.header;
        widgets.inactive.bg_fill = palette.header;
        widgets.inactive.bg_stroke = Stroke::new(1.0, palette.grid);
        widgets.hovered.weak_bg_fill = hover;
        widgets.hovered.bg_fill = hover;
        widgets.hovered.bg_stroke = Stroke::new(1.0, palette.accent);
        widgets.active.weak_bg_fill = palette.selection;
        widgets.active.bg_fill = palette.selection;
        widgets.active.bg_stroke = Stroke::new(1.0, palette.accent);
        widgets.open.weak_bg_fill = palette.selection;
        style
    }

    /// The height of a Register row, and of a button or text field.
    #[expect(dead_code, reason = "the Register (#85) is the first caller")]
    pub(crate) fn row_height(&self) -> f32 {
        self.sizes.row
    }

    /// The tinted frame of the panels around the content: the Menu bar, Account bar and
    /// Status bar.
    pub(crate) fn chrome_frame(&self) -> Frame {
        Frame::new().fill(self.palette.chrome)
    }

    /// The Net Worth footer under the Account bar: chrome, with room around its text.
    #[expect(dead_code, reason = "the Account bar (#84) draws the Net Worth footer")]
    pub(crate) fn footer_frame(&self) -> Frame {
        self.chrome_frame().inner_margin(FOOTER_MARGIN)
    }

    /// The fill of the Register's column header band.
    #[expect(dead_code, reason = "the Register (#85) is the first caller")]
    pub(crate) fn header_fill(&self) -> Color32 {
        self.palette.header
    }

    /// Whether the Register stripes alternate rows: pass it to `TableBuilder::striped`.
    /// Only the Register stripes, so the style leaves `Visuals::striped` off.
    #[expect(dead_code, reason = "the Register (#85) is the first caller")]
    pub(crate) fn alternating_rows(&self) -> bool {
        self.settings.alternating_rows
    }

    /// The fill behind row `row` of a list laid out by hand, counting from 0 at the top:
    /// the even rows, as `egui_extras` stripes them. Paint it before the row's gridlines.
    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "the Register (#85) is the first caller")
    )]
    pub(crate) fn stripe(&self, row: usize) -> Option<Color32> {
        (self.settings.alternating_rows && row.is_multiple_of(2)).then_some(self.palette.stripe)
    }

    /// The gridlines for one Register cell, given its rect (`ui.max_rect()` inside the
    /// cell) and the display's pixels per point. Paint them through [`Theme::rule_clip`].
    ///
    /// Each cell rules the gaps on its own left and top. `egui_extras` paints a stripe or
    /// selection across half the gap on every side of its cell, so a rule on the right or
    /// bottom would be covered by the next cell's fill. The first column (`column` 0) has
    /// no gap to its left. Rules sit on a pixel centre and are one pixel wide: at a
    /// fractional scale, an unsnapped line straddles two pixels and fades to almost nothing.
    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "the Register (#85) is the first caller")
    )]
    pub(crate) fn cell_rules(
        &self,
        cell: Rect,
        column: usize,
        pixels_per_point: f32,
    ) -> Vec<Shape> {
        let snap = |v: f32| ((v * pixels_per_point).floor() + 0.5) / pixels_per_point;
        let stroke = Stroke::new(1.0 / pixels_per_point, self.palette.grid);
        let half_gap = self.item_spacing() / 2.0;
        let (vertical, horizontal) = match self.settings.gridlines {
            Gridlines::Off => (false, false),
            Gridlines::Horizontal => (false, true),
            Gridlines::Vertical => (true, false),
            Gridlines::Both => (true, true),
        };
        let mut rules = Vec::new();
        if vertical && column > 0 {
            rules.push(Shape::vline(
                snap(cell.left() - half_gap.x),
                cell.y_range().expand(half_gap.y),
                stroke,
            ));
        }
        if horizontal {
            rules.push(Shape::hline(
                cell.x_range().expand(half_gap.x),
                snap(cell.top() - half_gap.y),
                stroke,
            ));
        }
        rules
    }

    /// The clip for painting a cell's rules, given the cell's clip rect and the Register's
    /// scroll viewport.
    ///
    /// A cell's own painter is clipped to the cell, so the rules in the gaps would never
    /// show. The clip widens by half the gap plus one point, without which a rule on the
    /// clip's edge is cut off, and stays inside the viewport so rules never paint over
    /// the panels around the Register.
    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "the Register (#85) is the first caller")
    )]
    pub(crate) fn rule_clip(&self, cell_clip: Rect, viewport: Rect) -> Rect {
        cell_clip
            .expand2(self.item_spacing() / 2.0 + Vec2::splat(1.0))
            .intersect(viewport)
    }

    /// The space between widgets, which is also the gap between Register cells.
    fn item_spacing(&self) -> Vec2 {
        vec2(COLUMN_GAP, self.sizes.gap)
    }

    /// `text`, the formatted `amount`, styled for display.
    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "the Register (#85) is the first caller")
    )]
    pub(crate) fn amount(&self, amount: &Money, text: impl Into<String>) -> RichText {
        let text = RichText::new(text);
        let text = if self.settings.monospace_money {
            text.monospace()
        } else {
            text
        };
        if self.settings.coloured_negatives && amount.is_negative() {
            text.color(self.palette.negative)
        } else {
            text
        }
    }
}

impl Default for Theme {
    fn default() -> Self {
        Self::new(ThemeSettings::default())
    }
}

#[cfg(test)]
mod tests {
    use balanced_core::CurrencyCode;
    use egui::pos2;
    use rust_decimal::Decimal;
    use rust_decimal_macros::dec;

    use super::*;

    /// The light palette's grid colour, `width` points wide.
    fn grid_rule(width: f32) -> Stroke {
        Stroke::new(width, Color32::from_rgb(200, 208, 220))
    }

    fn gridlines(gridlines: Gridlines) -> Theme {
        Theme::new(ThemeSettings {
            gridlines,
            ..ThemeSettings::default()
        })
    }

    /// A cell 80 × 20 points, with a 4 × 2 point half-gap around it at normal density.
    fn cell() -> Rect {
        Rect::from_min_max(pos2(100.0, 40.0), pos2(180.0, 60.0))
    }

    fn usd(amount: Decimal) -> Money {
        Money::new(amount, CurrencyCode::new("USD").unwrap()).unwrap()
    }

    #[test]
    fn body_text_is_13_px() {
        let style = Theme::default().style();

        let body = &style.text_styles[&egui::TextStyle::Body];
        assert_eq!(body.size, 13.0);
    }

    #[test]
    fn rows_are_20_px_normal_and_16_px_dense() {
        let normal = Theme::default().style();
        let dense = Theme::new(ThemeSettings {
            density: Density::Dense,
            ..ThemeSettings::default()
        })
        .style();

        assert_eq!(normal.spacing.interact_size.y, 20.0);
        assert_eq!(dense.spacing.interact_size.y, 16.0);
    }

    #[test]
    fn dense_tightens_the_gap_between_widgets() {
        let normal = Theme::default().style();
        let dense = Theme::new(ThemeSettings {
            density: Density::Dense,
            ..ThemeSettings::default()
        })
        .style();

        assert_eq!(normal.spacing.item_spacing, vec2(8.0, 4.0));
        assert_eq!(dense.spacing.item_spacing, vec2(8.0, 2.0));
    }

    #[test]
    fn corners_are_square() {
        let style = Theme::default().style();

        let widgets = &style.visuals.widgets;
        assert_eq!(widgets.inactive.corner_radius, CornerRadius::ZERO);
        assert_eq!(widgets.hovered.corner_radius, CornerRadius::ZERO);
        assert_eq!(style.visuals.window_corner_radius, CornerRadius::ZERO);
        assert_eq!(style.visuals.menu_corner_radius, CornerRadius::ZERO);
    }

    #[test]
    fn scheme_picks_the_light_or_dark_palette() {
        let light = Theme::default().style();
        let dark = Theme::new(ThemeSettings {
            scheme: Scheme::Dark,
            ..ThemeSettings::default()
        })
        .style();

        assert!(!light.visuals.dark_mode);
        assert_eq!(light.visuals.panel_fill, Color32::from_rgb(255, 255, 255));
        assert_eq!(
            light.visuals.widgets.inactive.fg_stroke.color,
            Color32::from_rgb(28, 32, 38)
        );
        assert!(dark.visuals.dark_mode);
        assert_eq!(dark.visuals.panel_fill, Color32::from_rgb(22, 26, 34));
        assert_eq!(
            dark.visuals.widgets.inactive.fg_stroke.color,
            Color32::from_rgb(222, 228, 236)
        );
    }

    #[test]
    fn even_rows_are_striped() {
        let theme = Theme::default();

        let fills = [0, 1, 2, 3].map(|row| theme.stripe(row));

        let stripe = Some(Color32::from_rgb(238, 243, 250));
        assert_eq!(fills, [stripe, None, stripe, None]);
    }

    #[test]
    fn no_row_is_striped_when_alternating_rows_are_off() {
        let theme = Theme::new(ThemeSettings {
            alternating_rows: false,
            ..ThemeSettings::default()
        });

        let fills = [0, 1].map(|row| theme.stripe(row));

        assert_eq!(fills, [None, None]);
    }

    #[test]
    fn negative_amounts_are_coloured() {
        let theme = Theme::default();

        let text = theme.amount(&usd(dec!(-12.50)), "-12.50");

        assert_eq!(
            text,
            RichText::new("-12.50").color(Color32::from_rgb(190, 30, 30))
        );
    }

    #[test]
    fn positive_amounts_keep_the_text_colour() {
        let theme = Theme::default();

        let text = theme.amount(&usd(dec!(12.50)), "12.50");

        assert_eq!(text, RichText::new("12.50"));
    }

    #[test]
    fn negative_amounts_keep_the_text_colour_when_coloured_negatives_are_off() {
        let theme = Theme::new(ThemeSettings {
            coloured_negatives: false,
            ..ThemeSettings::default()
        });

        let text = theme.amount(&usd(dec!(-12.50)), "-12.50");

        assert_eq!(text, RichText::new("-12.50"));
    }

    #[test]
    fn amounts_are_monospace_when_monospace_money_is_on() {
        let theme = Theme::new(ThemeSettings {
            monospace_money: true,
            ..ThemeSettings::default()
        });

        let text = theme.amount(&usd(dec!(12.50)), "12.50");

        assert_eq!(text, RichText::new("12.50").monospace());
    }

    #[test]
    fn no_rules_when_gridlines_are_off() {
        let theme = gridlines(Gridlines::Off);

        let rules = theme.cell_rules(cell(), 1, 1.0);

        assert_eq!(rules, []);
    }

    #[test]
    fn horizontal_rules_run_along_the_top_of_the_gap_above_the_cell() {
        let theme = gridlines(Gridlines::Horizontal);

        let rules = theme.cell_rules(cell(), 1, 1.0);

        let grid = grid_rule(1.0);
        assert_eq!(rules, [Shape::hline(96.0..=184.0, 38.5, grid)]);
    }

    #[test]
    fn vertical_rules_run_down_the_gap_left_of_the_cell() {
        let theme = gridlines(Gridlines::Vertical);

        let rules = theme.cell_rules(cell(), 1, 1.0);

        let grid = grid_rule(1.0);
        assert_eq!(rules, [Shape::vline(96.5, 38.0..=62.0, grid)]);
    }

    #[test]
    fn the_first_column_has_no_vertical_rule() {
        let theme = gridlines(Gridlines::Both);

        let rules = theme.cell_rules(cell(), 0, 1.0);

        let grid = grid_rule(1.0);
        assert_eq!(rules, [Shape::hline(96.0..=184.0, 38.5, grid)]);
    }

    #[test]
    fn both_draws_the_left_and_top_rules() {
        let theme = gridlines(Gridlines::Both);

        let rules = theme.cell_rules(cell(), 1, 1.0);

        let grid = grid_rule(1.0);
        assert_eq!(
            rules,
            [
                Shape::vline(96.5, 38.0..=62.0, grid),
                Shape::hline(96.0..=184.0, 38.5, grid),
            ]
        );
    }

    #[test]
    fn rules_are_one_pixel_wide_on_a_pixel_centre_at_fractional_scales() {
        let theme = gridlines(Gridlines::Horizontal);

        let rules = theme.cell_rules(cell(), 1, 1.25);

        // 38 points is pixel 47.5; the rule sits on the centre of pixel 47, at 47.5 / 1.25.
        let grid = grid_rule(0.8);
        assert_eq!(rules, [Shape::hline(96.0..=184.0, 38.0, grid)]);
    }

    #[test]
    fn dense_rows_have_a_narrower_gap_to_rule() {
        let theme = Theme::new(ThemeSettings {
            gridlines: Gridlines::Horizontal,
            density: Density::Dense,
            ..ThemeSettings::default()
        });

        let rules = theme.cell_rules(cell(), 1, 1.0);

        let grid = grid_rule(1.0);
        assert_eq!(rules, [Shape::hline(96.0..=184.0, 39.5, grid)]);
    }

    #[test]
    fn the_rule_clip_covers_the_gap_and_stays_in_the_viewport() {
        let theme = Theme::default();
        let viewport = Rect::from_min_max(pos2(0.0, 39.0), pos2(500.0, 500.0));

        let clip = theme.rule_clip(cell(), viewport);

        // Half the gap (4 × 2) plus one point, cut at the viewport's top.
        assert_eq!(
            clip,
            Rect::from_min_max(pos2(95.0, 39.0), pos2(185.0, 63.0))
        );
    }
}
