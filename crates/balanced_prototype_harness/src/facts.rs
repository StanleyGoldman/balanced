// SPDX-License-Identifier: GPL-3.0-or-later WITH AdditionRef-Balanced-Plugin-Exception

//! Window facts: one line about the window, shown at the foot of the bottom panel.
//!
//! [`crate::Harness`] samples them each frame; this module formats them.

use egui::{Pos2, Vec2};

/// What the bottom panel's facts line reports about the window.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub(crate) struct WindowFacts {
    /// Frames per second over the last full second, once one has passed.
    pub(crate) fps: Option<f64>,
    /// Frames drawn since launch.
    pub(crate) frames: u64,
    /// The outer window's top-left corner on the desktop; native Wayland never reports it.
    pub(crate) pos: Option<Pos2>,
    /// The drawable area's size in points.
    pub(crate) size: Option<Vec2>,
    pub(crate) focused: Option<bool>,
    pub(crate) occluded: Option<bool>,
}

impl WindowFacts {
    /// `4 fps · frame 473 · pos 320,158 · 1280×800 · focused`, with `?` for what isn't known yet.
    pub(crate) fn line(&self) -> String {
        let fps = self
            .fps
            .map_or_else(|| "? fps".to_owned(), |f| format!("{f:.0} fps"));
        let pos = self.pos.map_or_else(
            || "pos ?".to_owned(),
            |p| format!("pos {:.0},{:.0}", p.x, p.y),
        );
        let size = self
            .size
            .map_or_else(|| "size ?".to_owned(), |s| format!("{:.0}×{:.0}", s.x, s.y));
        let focus = match self.focused {
            Some(true) => "focused",
            Some(false) => "unfocused",
            None => "focus ?",
        };
        let occluded = if self.occluded == Some(true) {
            " · OCCLUDED"
        } else {
            ""
        };
        format!(
            "{fps} · frame {} · {pos} · {size} · {focus}{occluded}",
            self.frames
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_facts_read_as_one_line() {
        let facts = WindowFacts {
            fps: Some(59.6),
            frames: 473,
            pos: Some(Pos2::new(320.0, 158.0)),
            size: Some(Vec2::new(1280.0, 800.0)),
            focused: Some(true),
            occluded: Some(false),
        };
        assert_eq!(
            facts.line(),
            "60 fps · frame 473 · pos 320,158 · 1280×800 · focused"
        );
    }

    #[test]
    fn unknown_facts_show_a_question_mark() {
        assert_eq!(
            WindowFacts::default().line(),
            "? fps · frame 0 · pos ? · size ? · focus ?"
        );
    }

    #[test]
    fn an_occluded_window_says_so() {
        let facts = WindowFacts {
            focused: Some(false),
            occluded: Some(true),
            ..WindowFacts::default()
        };
        assert!(facts.line().ends_with("unfocused · OCCLUDED"));
    }
}
