//! Spotlight: dims the window except the tracked widgets and regions its targets name.
//!
//! This module holds the window-free parts (command parsing, target matching and the overlay
//! geometry); [`crate::Harness`] records rects, paints and logs.

use egui::{Rect, pos2};

/// A `spotlight` control command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SpotlightCommand {
    /// `spotlight on`, or a bare `spotlight`: turn it on, keeping the targets.
    On,
    /// `spotlight off`: turn it off, keeping the targets.
    Off,
    /// `spotlight <target>[; <target>…]`: set the targets and turn it on.
    Targets(Vec<String>),
}

impl SpotlightCommand {
    /// Parse one control line, or return `None` when it isn't a `spotlight` command. Targets are
    /// separated by `;`, because widget names contain spaces and commas.
    pub(crate) fn parse(line: &str) -> Option<Self> {
        let rest = line.trim().strip_prefix("spotlight")?;
        if !(rest.is_empty() || rest.starts_with(char::is_whitespace)) {
            return None;
        }
        let rest = rest.trim();
        if rest.eq_ignore_ascii_case("off") {
            return Some(Self::Off);
        }
        let targets: Vec<String> = rest
            .split(';')
            .map(str::trim)
            .filter(|t| !t.is_empty())
            .map(str::to_owned)
            .collect();
        if targets.is_empty() || rest.eq_ignore_ascii_case("on") {
            Some(Self::On)
        } else {
            Some(Self::Targets(targets))
        }
    }
}

/// Whether the spotlight is on, and the targets it lights.
#[derive(Debug, Default)]
pub(crate) struct Spotlight {
    on: bool,
    targets: Vec<String>,
    /// `targets` in lower case, for matching.
    lowered: Vec<String>,
}

impl Spotlight {
    pub(crate) fn is_on(&self) -> bool {
        self.on
    }

    pub(crate) fn targets(&self) -> &[String] {
        &self.targets
    }

    pub(crate) fn set_on(&mut self, on: bool) {
        self.on = on;
    }

    pub(crate) fn apply(&mut self, command: SpotlightCommand) {
        match command {
            SpotlightCommand::On => self.on = true,
            SpotlightCommand::Off => self.on = false,
            SpotlightCommand::Targets(targets) => {
                self.lowered = targets.iter().map(|t| t.to_lowercase()).collect();
                self.targets = targets;
                self.on = true;
            }
        }
    }

    /// Whether some target is a case-insensitive substring of `name`.
    pub(crate) fn matches(&self, name: &str) -> bool {
        if self.lowered.is_empty() {
            return false;
        }
        let name = name.to_lowercase();
        self.lowered.iter().any(|t| name.contains(t.as_str()))
    }
}

/// The `rects` that lie inside no other rect of `rects`, in their original order.
///
/// A region and the widgets inside it often match the same target; only the region is kept.
/// Of several equal rects, the first is kept. Rects that only partly overlap are all kept.
pub(crate) fn outermost(rects: &[Rect]) -> Vec<Rect> {
    rects
        .iter()
        .enumerate()
        .filter(|&(i, rect)| {
            !rects
                .iter()
                .enumerate()
                .any(|(j, other)| j != i && other.contains_rect(*rect) && (other != rect || j < i))
        })
        .map(|(_, rect)| *rect)
        .collect()
}

/// Rectangles that cover `screen` except the `holes`, without overlapping each other.
///
/// The screen is cut into horizontal bands at every hole's top and bottom edge; within a band,
/// the gaps between the holes that span it become rectangles.
pub(crate) fn overlay_rects(screen: Rect, holes: &[Rect]) -> Vec<Rect> {
    let holes: Vec<Rect> = holes
        .iter()
        .map(|h| h.intersect(screen))
        .filter(Rect::is_positive)
        .collect();
    let mut edges: Vec<f32> = holes
        .iter()
        .flat_map(|h| [h.min.y, h.max.y])
        .chain([screen.min.y, screen.max.y])
        .collect();
    edges.sort_by(f32::total_cmp);
    edges.dedup();

    let mut out = Vec::new();
    for band in edges.windows(2) {
        let &[top, bottom] = band else {
            continue;
        };
        let mut spans: Vec<(f32, f32)> = holes
            .iter()
            .filter(|h| h.min.y <= top && h.max.y >= bottom)
            .map(|h| (h.min.x, h.max.x))
            .collect();
        spans.sort_by(|a, b| a.0.total_cmp(&b.0));
        let mut x = screen.min.x;
        for (left, right) in spans {
            if left > x {
                out.push(Rect::from_min_max(pos2(x, top), pos2(left, bottom)));
            }
            x = x.max(right);
        }
        if x < screen.max.x {
            out.push(Rect::from_min_max(pos2(x, top), pos2(screen.max.x, bottom)));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn targets(list: &[&str]) -> SpotlightCommand {
        SpotlightCommand::Targets(list.iter().map(|s| (*s).to_owned()).collect())
    }

    fn rect(x0: f32, y0: f32, x1: f32, y1: f32) -> Rect {
        Rect::from_min_max(pos2(x0, y0), pos2(x1, y1))
    }

    #[test]
    fn targets_are_separated_by_semicolons_and_keep_commas() {
        let parsed =
            SpotlightCommand::parse("spotlight payment cell, row 1204;  context header: Filter ;");
        assert_eq!(
            parsed,
            Some(targets(&[
                "payment cell, row 1204",
                "context header: Filter"
            ]))
        );
    }

    #[test]
    fn on_off_and_bare_keep_the_targets() {
        assert_eq!(
            SpotlightCommand::parse("spotlight off"),
            Some(SpotlightCommand::Off)
        );
        assert_eq!(
            SpotlightCommand::parse("spotlight on"),
            Some(SpotlightCommand::On)
        );
        assert_eq!(
            SpotlightCommand::parse("spotlight"),
            Some(SpotlightCommand::On)
        );
    }

    #[test]
    fn other_commands_are_not_spotlight() {
        assert_eq!(SpotlightCommand::parse("spotlights register"), None);
        assert_eq!(SpotlightCommand::parse("variant b"), None);
    }

    #[test]
    fn off_then_on_keeps_targets() {
        let mut spotlight = Spotlight::default();
        spotlight.apply(targets(&["register"]));
        spotlight.apply(SpotlightCommand::Off);
        assert!(!spotlight.is_on());
        spotlight.apply(SpotlightCommand::On);
        assert!(spotlight.is_on());
        assert_eq!(spotlight.targets(), ["register".to_owned()]);
    }

    #[test]
    fn matches_case_insensitive_substrings_of_any_target() {
        let mut spotlight = Spotlight::default();
        spotlight.apply(targets(&["FILTER", "row 12"]));
        assert!(spotlight.matches("context header: Filter"));
        assert!(spotlight.matches("payee cell, row 1204"));
        assert!(!spotlight.matches("account bar"));
    }

    #[test]
    fn no_targets_match_nothing() {
        let spotlight = Spotlight::default();
        assert!(!spotlight.matches("register"));
    }

    #[test]
    fn a_region_swallows_the_widgets_inside_it() {
        let region = rect(0.0, 0.0, 100.0, 300.0);
        let rows: Vec<Rect> = (0..26_u8)
            .map(|i| {
                let top = f32::from(i) * 10.0;
                rect(5.0, top, 95.0, top + 10.0)
            })
            .collect();
        let rects: Vec<Rect> = rows.iter().copied().chain([region]).collect();

        assert_eq!(outermost(&rects), vec![region]);
    }

    #[test]
    fn partly_overlapping_rects_are_both_kept() {
        let rects = [rect(0.0, 0.0, 50.0, 50.0), rect(25.0, 25.0, 75.0, 75.0)];

        assert_eq!(outermost(&rects), rects.to_vec());
    }

    #[test]
    fn equal_rects_are_kept_once() {
        let r = rect(10.0, 10.0, 20.0, 20.0);

        assert_eq!(outermost(&[r, r, r]), vec![r]);
    }

    #[test]
    fn nested_rects_drop_and_separate_rects_stay() {
        let outer = rect(0.0, 0.0, 50.0, 50.0);
        let inner = rect(10.0, 10.0, 20.0, 20.0);
        let innermost = rect(12.0, 12.0, 18.0, 18.0);
        let separate = rect(60.0, 60.0, 90.0, 90.0);
        let inside_separate = rect(70.0, 70.0, 80.0, 80.0);
        let rects = [inner, separate, innermost, outer, inside_separate];

        assert_eq!(outermost(&rects), vec![separate, outer]);
    }

    /// Each sample point inside the screen lies in exactly one overlay rect, or in a hole and
    /// none.
    fn assert_covers_all_but_holes(screen: Rect, holes: &[Rect]) {
        let overlay = overlay_rects(screen, holes);
        for i in 0..100_u8 {
            for j in 0..100_u8 {
                let p = pos2(f32::from(i) + 0.5, f32::from(j) + 0.5);
                let covering = overlay.iter().filter(|r| r.contains(p)).count();
                let in_hole = holes.iter().any(|h| h.contains(p));
                assert_eq!(covering, usize::from(!in_hole), "at {p:?}");
            }
        }
    }

    #[test]
    fn no_holes_cover_the_screen_with_one_rect() {
        let screen = rect(0.0, 0.0, 100.0, 100.0);
        assert_eq!(overlay_rects(screen, &[]), vec![screen]);
    }

    #[test]
    fn overlay_leaves_separate_holes_open() {
        let holes = [rect(10.0, 10.0, 30.0, 20.0), rect(50.0, 60.0, 90.0, 80.0)];
        assert_covers_all_but_holes(rect(0.0, 0.0, 100.0, 100.0), &holes);
    }

    #[test]
    fn overlay_leaves_overlapping_holes_open() {
        let holes = [
            rect(10.0, 10.0, 40.0, 40.0),
            rect(30.0, 30.0, 60.0, 70.0),
            rect(35.0, 5.0, 45.0, 90.0),
        ];
        assert_covers_all_but_holes(rect(0.0, 0.0, 100.0, 100.0), &holes);
    }

    #[test]
    fn holes_past_the_screen_edge_are_clipped() {
        let holes = [
            rect(-20.0, 90.0, 20.0, 130.0),
            rect(200.0, 0.0, 300.0, 50.0),
        ];
        assert_covers_all_but_holes(rect(0.0, 0.0, 100.0, 100.0), &holes);
    }
}
