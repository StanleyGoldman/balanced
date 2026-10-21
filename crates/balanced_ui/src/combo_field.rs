// SPDX-License-Identifier: GPL-3.0-or-later WITH AdditionRef-Balanced-Plugin-Exception

//! [`ComboText`]: the text of a field that lists entries as you type, as the Register's
//! Payee and Category cells and a split line's Category field do, with its Inline
//! completion and its open list, and the list's popup.

use balanced_i18n::Localizer;
use eframe::egui::accesskit::Role;
use eframe::egui::text::{CCursor, CCursorRange};
use eframe::egui::text_edit::TextEditState;
use eframe::egui::{
    self, Area, Event, Frame, Id, Order, Rect, Response, ScrollArea, Sense, TextStyle,
    TextWrapMode, WidgetInfo, WidgetText, WidgetType, pos2, vec2,
};

use crate::combobox::{ListRow, rest_after};
use crate::i18n::fl;

/// The glyph of a button that drops a list or menu down.
pub(crate) const DROP_DOWN_GLYPH: &str = "⏷";

/// The narrowest a combobox list gets, so a narrow field's list still shows entries.
const LIST_MIN_WIDTH: f32 = 240.0;

/// The most rows a combobox list shows at once before it scrolls.
const LIST_MAX_ROWS: usize = 10;

/// Which entries a combobox list shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ListScope {
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

/// A combobox field's text, the Inline completion ending it, if any, and its list while
/// that is open. The rows of the list come from the field's owner, as a function of the
/// text they match: `list` in the methods below.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ComboText {
    pub(crate) text: String,
    /// While the text ends in an Inline completion, the length in bytes of the typed
    /// text before it.
    completion: Option<usize>,
    list: Option<ListState>,
}

impl ComboText {
    /// `text`, with no completion and the list closed.
    pub(crate) fn new(text: String) -> Self {
        Self {
            text,
            completion: None,
            list: None,
        }
    }

    /// The text typed so far, without the Inline completion after it.
    pub(crate) fn typed(&self) -> &str {
        &self.text[..self.completion.unwrap_or(self.text.len())]
    }

    /// Whether the text ends in an Inline completion.
    pub(crate) fn is_completed(&self) -> bool {
        self.completion.is_some()
    }

    /// Forgets the Inline completion, keeping its text as typed. Returns whether there
    /// was one.
    pub(crate) fn forget_completion(&mut self) -> bool {
        self.completion.take().is_some()
    }

    /// Drops the Inline completion, if one shows, leaving the typed text.
    pub(crate) fn drop_completion(&mut self) {
        if let Some(typed) = self.completion.take() {
            self.text.truncate(typed);
        }
    }

    /// Whether the list is open on every entry.
    pub(crate) fn lists_all(&self) -> bool {
        self.list.is_some_and(|list| list.scope == ListScope::All)
    }

    pub(crate) fn close_list(&mut self) {
        self.list = None;
    }

    /// The rows of the list, from `list`, or none while it is closed.
    pub(crate) fn rows<'a>(&self, list: impl FnOnce(&str) -> Vec<ListRow<'a>>) -> Vec<ListRow<'a>> {
        let Some(state) = self.list else {
            return Vec::new();
        };
        match state.scope {
            ListScope::Matching => list(self.typed()),
            ListScope::All => list(""),
        }
    }

    /// Opens the list of `scope`. A list of matches starts with its top row
    /// highlighted, and a list of every entry with the entry the text names, else its
    /// top row.
    pub(crate) fn open_list<'a>(
        &mut self,
        scope: ListScope,
        list: impl FnOnce(&str) -> Vec<ListRow<'a>>,
    ) {
        self.list = Some(ListState {
            scope,
            highlight: Some(0),
            reveal: true,
            offset: 0.0,
        });
        let named = match scope {
            ListScope::Matching => None,
            ListScope::All => self.rows(list).iter().position(
                |row| matches!(row, ListRow::Choice(text) if text.eq_ignore_ascii_case(self.text.trim())),
            ),
        };
        if let (Some(state), Some(named)) = (&mut self.list, named) {
            state.highlight = Some(named);
        }
    }

    /// Completes the typed text with the rest of the first entry of the open list that
    /// starts with it, ignoring case, and highlights that entry. Returns the range of
    /// the completion, or `None` when no entry starts with the text. An entry that is
    /// the text, ignoring case, is highlighted and leaves nothing to complete.
    pub(crate) fn complete<'a>(
        &mut self,
        list: impl FnOnce(&str) -> Vec<ListRow<'a>>,
    ) -> Option<CCursorRange> {
        let (index, rest) = self
            .rows(list)
            .into_iter()
            .enumerate()
            .find_map(|(index, row)| match row {
                ListRow::Choice(text) => Some((index, rest_after(text, &self.text)?.to_owned())),
                ListRow::Create(_) => None,
            })?;
        if let Some(state) = &mut self.list {
            state.highlight = Some(index);
        }
        if rest.is_empty() {
            return None;
        }
        let typed = CCursor::new(self.text.chars().count());
        self.completion = Some(self.text.len());
        self.text.push_str(&rest);
        Some(CCursorRange::two(
            typed,
            CCursor::new(self.text.chars().count()),
        ))
    }

    /// Completes the text, just typed into in the field with id `editor`, when the
    /// caret is at its end. The completion is selected, so the next character typed
    /// replaces it.
    pub(crate) fn complete_typing<'a>(
        &mut self,
        ctx: &egui::Context,
        editor: Id,
        list: impl FnOnce(&str) -> Vec<ListRow<'a>>,
    ) {
        let Some(mut state) = TextEditState::load(ctx, editor) else {
            return;
        };
        let caret_at_end = state.cursor.char_range().is_some_and(|range| {
            range.is_empty() && range.primary.index == CCursor::new(self.text.chars().count()).index
        });
        if !caret_at_end {
            return;
        }
        let Some(completion) = self.complete(list) else {
            return;
        };
        state.cursor.set_char_range(Some(completion));
        state.store(ctx, editor);
        ctx.request_repaint();
    }

    /// The highlighted row of the list, if it is open with one highlighted.
    pub(crate) fn highlighted<'a>(
        &self,
        list: impl FnOnce(&str) -> Vec<ListRow<'a>>,
    ) -> Option<ListRow<'a>> {
        let highlight = self.list?.highlight?;
        self.rows(list).into_iter().nth(highlight)
    }

    /// Moves the highlight one row down the list of `rows` rows, stopping at the last.
    pub(crate) fn highlight_next(&mut self, rows: usize) {
        if let Some(state) = &mut self.list {
            let last = rows.saturating_sub(1);
            state.highlight = Some(state.highlight.map_or(0, |row| (row + 1).min(last)));
            state.reveal = true;
        }
    }

    /// Clears the list's highlight, so Tab and Enter keep the text.
    pub(crate) fn highlight_none(&mut self) {
        if let Some(state) = &mut self.list {
            state.highlight = None;
        }
    }

    /// Moves the highlight one row up the list. Up from the top row clears it, so
    /// Enter keeps the typed text.
    pub(crate) fn highlight_previous(&mut self) {
        if let Some(state) = &mut self.list {
            state.highlight = state.highlight.and_then(|row| row.checked_sub(1));
            state.reveal = true;
        }
    }

    /// Draws the list of `rows`, while it is open, in a popup with `id` below `field`,
    /// each row `row_height` high, scrolled to show a highlight that moved, and returns
    /// the row pressed this frame. A row is taken on the press, since the release would
    /// come after the press has moved focus off the field.
    pub(crate) fn show_list<'a>(
        &mut self,
        ui: &egui::Ui,
        i18n: &Localizer,
        id: Id,
        rows: &[ListRow<'a>],
        field: Rect,
        row_height: f32,
    ) -> Option<ListRow<'a>> {
        let state = self.list.as_mut().filter(|_| !rows.is_empty())?;
        let pitch = row_height + ui.spacing().item_spacing.y;
        #[expect(
            clippy::cast_precision_loss,
            reason = "at most LIST_MAX_ROWS rows show at once"
        )]
        let height = rows.len().min(LIST_MAX_ROWS) as f32 * pitch - ui.spacing().item_spacing.y;
        let width = field.width().max(LIST_MIN_WIDTH);
        let mut scroll = ScrollArea::vertical()
            .max_height(height)
            .min_scrolled_height(height);
        if let Some(offset) = state.reveal_offset(pitch, height) {
            scroll = scroll.vertical_scroll_offset(offset);
        }
        state.reveal = false;
        let highlight = state.highlight;
        let mut pressed = None;
        let output = Area::new(id)
            .order(Order::Foreground)
            .fixed_pos(field.left_bottom())
            .show(ui.ctx(), |ui| {
                Frame::popup(ui.style())
                    .show(ui, |ui| {
                        ui.set_width(width);
                        scroll.show_rows(ui, row_height, rows.len(), |ui, in_view| {
                            for index in in_view {
                                // `show_rows` lays out indices below `rows.len()` only.
                                let row = &rows[index];
                                let highlighted = highlight == Some(index);
                                if list_row(ui, i18n, row, highlighted, row_height) {
                                    pressed = Some(row.clone());
                                }
                            }
                        })
                    })
                    .inner
            })
            .inner;
        if let Some(state) = &mut self.list {
            state.offset = output.state.offset.y;
        }
        pressed
    }
}

/// The ⏷ button in `rect`, beside the Category field with id `editor`, which lists
/// every category.
pub(crate) fn list_button(ui: &egui::Ui, i18n: &Localizer, rect: Rect, editor: Id) -> Response {
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
        DROP_DOWN_GLYPH,
        TextStyle::Body.resolve(ui.style()),
        visuals.text_color(),
    );
    response
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

/// Whether text was typed or pasted this frame.
pub(crate) fn text_typed(ctx: &egui::Context) -> bool {
    ctx.input(|input| {
        input
            .events
            .iter()
            .any(|event| matches!(event, Event::Text(_) | Event::Paste(_)))
    })
}
