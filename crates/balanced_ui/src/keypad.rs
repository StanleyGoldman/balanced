// SPDX-License-Identifier: GPL-3.0-or-later WITH AdditionRef-Balanced-Plugin-Exception

//! The keypad: a calculator that opens under an amount field, from the 🖩 button
//! beside the field or with Alt+↓, and that every amount field draws the same way.
//!
//! It starts from the field's text, so an empty field gives an empty keypad. Its keys
//! and the keyboard, while its display has focus, build a sum, and the line under the
//! display shows the result as the field's preview does. Enter or OK hands the result
//! back to the field; Esc, or a press outside the keypad, closes it and hands back
//! nothing. Its arithmetic is the field's own `AmountReader`.

use balanced_core::CurrencyCode;
use balanced_i18n::{AmountReader, AmountStyle, Localizer};
use eframe::egui::text::{CCursor, CCursorRange};
use eframe::egui::text_edit::TextEditState;
use eframe::egui::{
    self, Align, Align2, Area, EventFilter, Frame, Id, Key, Label, Layout, Modifiers, Order, Rect,
    Response, RichText, Sense, StrokeKind, TextEdit, TextStyle, WidgetInfo, WidgetType, vec2,
};

use crate::amount_field::{self, Preview};
use crate::formatted_amount::i18n_currency;
use crate::i18n::fl;
use crate::theme::Theme;

/// How many keys wide the keypad is.
const COLUMNS: f32 = 4.0;

/// How many keys tall the keypad's keys are.
const ROWS: f32 = 5.0;

/// A key's width, in row heights.
const KEY_WIDTH: f32 = 1.8;

/// A key's height, in row heights.
const KEY_HEIGHT: f32 = 1.4;

/// The keypad's state while it is open.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Keypad {
    /// The sum typed so far.
    text: String,
    /// Whether the display takes keyboard focus, with its text cursor at the end, in
    /// the next frame it is drawn.
    focus: bool,
    /// The line under the display, and the sum it was worked out for. It is worked
    /// out again only when the sum changes.
    result: Option<(String, ResultLine)>,
}

/// The line under the keypad's display.
#[derive(Debug, Clone, PartialEq, Eq)]
enum ResultLine {
    /// Nothing is typed.
    Blank,
    /// The sum's result, as the field's preview shows it.
    Result(Preview),
    /// The sum ends in an operator, waiting for its next amount.
    Waiting,
    NotAnAmount,
}

impl ResultLine {
    /// The line for `text`, read by `reader` in `currency`.
    fn of(i18n: &Localizer, reader: &AmountReader, currency: CurrencyCode, text: &str) -> Self {
        if text.trim().is_empty() {
            return Self::Blank;
        }
        match Preview::of_any(i18n, reader, currency, text) {
            Ok(preview) => Self::Result(preview),
            Err(_) if AmountReader::awaits_operand(text) => Self::Waiting,
            Err(_) => Self::NotAnAmount,
        }
    }
}

/// What the keypad did this frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Outcome {
    /// It stays open.
    Open,
    /// It closed, handing nothing back.
    Cancelled,
    /// It closed, handing back this amount, formatted as the field shows it.
    Taken(String),
}

/// An arithmetic operator on the keypad.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum Operator {
    Add,
    Subtract,
    Multiply,
    Divide,
}

impl Operator {
    /// The character the operator is typed as, which the reader reads.
    fn typed(self) -> char {
        match self {
            Self::Add => '+',
            Self::Subtract => '-',
            Self::Multiply => '*',
            Self::Divide => '/',
        }
    }
}

/// What the keypad's tall key does: OK under a field, = in the Calculator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Finish {
    /// Hands the result back to the field.
    Ok,
    /// Replaces the sum with its result.
    Equals,
}

/// One key of the keypad.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum PadKey {
    Digit(char),
    Operator(Operator),
    Decimal,
    Clear,
    Backspace,
    /// The tall key, OK or = as its host's [`Finish`] says.
    Finish,
}

impl PadKey {
    /// The keys, each with its place in the grid: its column and row, then how many
    /// columns and rows it spans.
    const LAYOUT: [(Self, [f32; 4]); 18] = [
        (Self::Clear, [0.0, 0.0, 1.0, 1.0]),
        (Self::Backspace, [1.0, 0.0, 1.0, 1.0]),
        (Self::Operator(Operator::Divide), [2.0, 0.0, 1.0, 1.0]),
        (Self::Operator(Operator::Multiply), [3.0, 0.0, 1.0, 1.0]),
        (Self::Digit('7'), [0.0, 1.0, 1.0, 1.0]),
        (Self::Digit('8'), [1.0, 1.0, 1.0, 1.0]),
        (Self::Digit('9'), [2.0, 1.0, 1.0, 1.0]),
        (Self::Operator(Operator::Subtract), [3.0, 1.0, 1.0, 1.0]),
        (Self::Digit('4'), [0.0, 2.0, 1.0, 1.0]),
        (Self::Digit('5'), [1.0, 2.0, 1.0, 1.0]),
        (Self::Digit('6'), [2.0, 2.0, 1.0, 1.0]),
        (Self::Operator(Operator::Add), [3.0, 2.0, 1.0, 1.0]),
        (Self::Digit('1'), [0.0, 3.0, 1.0, 1.0]),
        (Self::Digit('2'), [1.0, 3.0, 1.0, 1.0]),
        (Self::Digit('3'), [2.0, 3.0, 1.0, 1.0]),
        (Self::Finish, [3.0, 3.0, 1.0, 2.0]),
        (Self::Digit('0'), [0.0, 4.0, 2.0, 1.0]),
        (Self::Decimal, [2.0, 4.0, 1.0, 1.0]),
    ];

    /// What the key shows, and what a screen reader reads for it, with the tall key
    /// doing what `finish` says.
    fn face(self, i18n: &Localizer, reader: &AmountReader, finish: Finish) -> (String, String) {
        match self {
            Self::Digit(digit) => (digit.to_string(), digit.to_string()),
            Self::Operator(Operator::Add) => ("+".to_owned(), fl!(i18n, "keypad-add")),
            Self::Operator(Operator::Subtract) => ("−".to_owned(), fl!(i18n, "keypad-subtract")),
            Self::Operator(Operator::Multiply) => ("×".to_owned(), fl!(i18n, "keypad-multiply")),
            Self::Operator(Operator::Divide) => ("÷".to_owned(), fl!(i18n, "keypad-divide")),
            Self::Decimal => (
                reader.decimal_mark().to_owned(),
                fl!(i18n, "keypad-decimal"),
            ),
            Self::Clear => (fl!(i18n, "keypad-clear", "key"), fl!(i18n, "keypad-clear")),
            Self::Backspace => ("⏴".to_owned(), fl!(i18n, "keypad-backspace")),
            Self::Finish => match finish {
                Finish::Ok => (fl!(i18n, "keypad-ok"), fl!(i18n, "keypad-ok")),
                Finish::Equals => ("=".to_owned(), fl!(i18n, "keypad-equals")),
            },
        }
    }
}

impl Keypad {
    /// The keypad over a field holding `text`.
    pub(crate) fn new(text: &str) -> Self {
        Self {
            text: text.trim().to_owned(),
            focus: true,
            result: None,
        }
    }

    /// Draws the keypad under `field`, right-aligned with it, leaving room for the
    /// field's preview under it, and acts on this frame's keys and presses. `reader`
    /// and `currency` are the field's; `button` is the field's 🖩 button, which a
    /// press outside the keypad may land on without closing it.
    #[expect(
        clippy::too_many_arguments,
        reason = "the keypad borrows the field's reader, currency and place for the frame"
    )]
    pub(crate) fn show(
        &mut self,
        ctx: &egui::Context,
        i18n: &Localizer,
        theme: &Theme,
        reader: &AmountReader,
        currency: CurrencyCode,
        id: Id,
        field: Rect,
        button: Rect,
    ) -> Outcome {
        let display = display_id(id);
        let mut outcome = Outcome::Open;
        if ctx.memory(|memory| memory.has_focus(display)) {
            amount_field::refuse_characters(ctx, reader);
            let (enter, escape, alt_down) = ctx.input_mut(|input| {
                input.consume_key(Modifiers::NONE, Key::Tab);
                (
                    input.consume_key(Modifiers::NONE, Key::Enter),
                    input.consume_key(Modifiers::NONE, Key::Escape),
                    input.consume_key(Modifiers::ALT, Key::ArrowDown),
                )
            });
            if enter {
                outcome = self.take(i18n, reader, currency);
            }
            if escape || alt_down {
                return Outcome::Cancelled;
            }
        }
        let row_height = theme.row_height();
        let gap = ctx.global_style().spacing.item_spacing;
        // The field's preview floats one row under the field.
        let anchor = field.right_bottom() + vec2(0.0, row_height + 2.0 * gap.y);
        let area = Area::new(id.with("keypad"))
            .order(Order::Foreground)
            .pivot(Align2::RIGHT_TOP)
            .fixed_pos(anchor)
            .show(ctx, |ui| {
                Frame::popup(ui.style())
                    .show(ui, |ui| {
                        let caption = fl!(i18n, "field-amount-keypad");
                        self.body(ui, i18n, theme, reader, currency, id, &caption, Finish::Ok)
                    })
                    .inner
            });
        if self.press(area.inner, reader) {
            outcome = self.take(i18n, reader, currency);
        }
        if outcome == Outcome::Open {
            let outside = ctx.input(|input| {
                input.pointer.any_pressed()
                    && input.pointer.interact_pos().is_some_and(|pos| {
                        !area.response.rect.contains(pos) && !button.contains(pos)
                    })
            });
            if outside {
                return Outcome::Cancelled;
            }
        }
        self.end_frame(ctx, id);
        outcome
    }

    /// Draws the keypad's caption, its display labelled `caption`, the line under the
    /// display and its keys, the last of them `finish`, into `ui`, and returns the key
    /// pressed this frame. `id` is the keypad's own.
    #[expect(
        clippy::too_many_arguments,
        reason = "the body borrows its host's reader, currency and labels for the frame"
    )]
    pub(crate) fn body(
        &mut self,
        ui: &mut egui::Ui,
        i18n: &Localizer,
        theme: &Theme,
        reader: &AmountReader,
        currency: CurrencyCode,
        id: Id,
        caption: &str,
        finish: Finish,
    ) -> Option<PadKey> {
        let row_height = theme.row_height();
        let gap = ui.spacing().item_spacing;
        let key = vec2(row_height * KEY_WIDTH, row_height * KEY_HEIGHT);
        let width = width(ui, theme);
        ui.set_width(width);
        let caption = ui.label(RichText::new(caption).small().weak());
        ui.add(
            TextEdit::singleline(&mut self.text)
                .id(display_id(id))
                .horizontal_align(Align::RIGHT)
                .desired_width(width)
                .event_filter(EventFilter {
                    escape: true,
                    ..Default::default()
                }),
        )
        .labelled_by(caption.id);
        self.result_line(ui, i18n, reader, currency, width);
        let (grid, _) = ui.allocate_exact_size(
            vec2(width, key.y * ROWS + gap.y * (ROWS - 1.0)),
            Sense::hover(),
        );
        let mut pressed = None;
        for (pad_key, [column, row, columns, rows]) in PadKey::LAYOUT {
            let min = grid.min + vec2(column * (key.x + gap.x), row * (key.y + gap.y));
            let size = vec2(
                key.x * columns + gap.x * (columns - 1.0),
                key.y * rows + gap.y * (rows - 1.0),
            );
            let (face, label) = pad_key.face(i18n, reader, finish);
            let rect = Rect::from_min_size(min, size);
            if key_button(ui, id.with(("keypad_key", pad_key)), rect, &face, &label).clicked() {
                pressed = Some(pad_key);
            }
        }
        pressed
    }

    /// Adds `pressed`, the key the body reported, if any, to the sum, and returns
    /// whether it was the finishing key, OK or =, which the host acts on.
    pub(crate) fn press(&mut self, pressed: Option<PadKey>, reader: &AmountReader) -> bool {
        let Some(pad_key) = pressed else {
            return false;
        };
        self.focus = true;
        match pad_key {
            PadKey::Digit(digit) => self.text.push(digit),
            PadKey::Operator(operator) => self.text.push(operator.typed()),
            PadKey::Decimal => self.text.push_str(reader.decimal_mark()),
            PadKey::Clear => self.text.clear(),
            PadKey::Backspace => {
                self.text.pop();
            }
            PadKey::Finish => return true,
        }
        false
    }

    /// Gives the display of the keypad with `id` the keyboard, with its text cursor at
    /// the end, when a key or a refused result asked for it this frame.
    pub(crate) fn end_frame(&mut self, ctx: &egui::Context, id: Id) {
        if std::mem::take(&mut self.focus) {
            focus_at_end(ctx, display_id(id), &self.text);
        }
    }

    /// The sum's result, formatted as an amount field shows it, or `None` when the
    /// sum isn't an amount. A `None` gives the display the keyboard back.
    pub(crate) fn result(
        &mut self,
        i18n: &Localizer,
        reader: &AmountReader,
        currency: CurrencyCode,
    ) -> Option<String> {
        if let Ok(calculation) = reader.read(self.text.trim()) {
            Some(i18n.format_amount(
                calculation.amount(),
                i18n_currency(currency),
                AmountStyle::Bare,
            ))
        } else {
            self.focus = true;
            None
        }
    }

    /// Replaces the sum with its result, as = does, or leaves it, focused, when it
    /// isn't an amount.
    pub(crate) fn equals(
        &mut self,
        i18n: &Localizer,
        reader: &AmountReader,
        currency: CurrencyCode,
    ) {
        if let Some(result) = self.result(i18n, reader, currency) {
            self.text = result;
            self.focus = true;
        }
    }

    /// Hands the sum's result back, formatted as the field shows amounts, or keeps the
    /// keypad open with its display focused when the sum isn't an amount.
    fn take(&mut self, i18n: &Localizer, reader: &AmountReader, currency: CurrencyCode) -> Outcome {
        self.result(i18n, reader, currency)
            .map_or(Outcome::Open, Outcome::Taken)
    }

    /// The line under the display: the sum's result as the field's preview shows it,
    /// "…" while the sum ends in an operator, or "Not an amount".
    fn result_line(
        &mut self,
        ui: &mut egui::Ui,
        i18n: &Localizer,
        reader: &AmountReader,
        currency: CurrencyCode,
        width: f32,
    ) {
        let text = &self.text;
        let line = match &self.result {
            Some((worked_out, line)) if worked_out == text => line,
            Some(_) | None => {
                let line = ResultLine::of(i18n, reader, currency, text);
                &self.result.insert((text.clone(), line)).1
            }
        };
        let text = match line {
            ResultLine::Blank => RichText::new(""),
            ResultLine::Result(preview) => RichText::new(preview.text(i18n)),
            ResultLine::Waiting => RichText::new(fl!(i18n, "keypad-waiting")),
            ResultLine::NotAnAmount => {
                RichText::new(fl!(i18n, "keypad-not-an-amount")).color(ui.visuals().error_fg_color)
            }
        };
        let height = TextStyle::Body.resolve(ui.style()).size;
        ui.allocate_ui_with_layout(
            vec2(width, height),
            Layout::right_to_left(Align::Center),
            |ui| ui.add(Label::new(text).extend()),
        );
    }
}

/// The id of the display of the keypad with `id`.
pub(crate) fn display_id(id: Id) -> Id {
    id.with("keypad_display")
}

/// How wide the keypad's body is in `ui`, drawn with `theme`.
fn width(ui: &egui::Ui, theme: &Theme) -> f32 {
    let key_width = theme.row_height() * KEY_WIDTH;
    key_width * COLUMNS + ui.spacing().item_spacing.x * (COLUMNS - 1.0)
}

/// The 🖩 button in `rect`, beside the amount field with `field`, which opens and
/// closes its keypad. It never takes focus.
pub(crate) fn button(ui: &egui::Ui, i18n: &Localizer, field: Id, rect: Rect) -> Response {
    let response = ui
        .interact(rect, field.with("keypad_button"), Sense::CLICK)
        .on_hover_text(fl!(i18n, "field-amount-keypad", "tooltip"));
    let label = fl!(i18n, "field-amount-keypad");
    response.widget_info(|| WidgetInfo::labeled(WidgetType::Button, ui.is_enabled(), &label));
    if ui.is_rect_visible(rect) {
        let visuals = ui.style().interact(&response);
        if response.hovered() || response.is_pointer_button_down_on() {
            ui.painter().rect_filled(rect, 0.0, visuals.weak_bg_fill);
        }
        ui.painter().text(
            rect.center(),
            Align2::CENTER_CENTER,
            "🖩",
            TextStyle::Body.resolve(ui.style()),
            visuals.text_color(),
        );
    }
    response
}

/// One key in `rect`, showing `face` and read as `label`. It never takes focus, so the
/// display keeps it.
fn key_button(ui: &egui::Ui, id: Id, rect: Rect, face: &str, label: &str) -> Response {
    let response = ui.interact(rect, id, Sense::CLICK);
    response.widget_info(|| WidgetInfo::labeled(WidgetType::Button, ui.is_enabled(), label));
    let visuals = ui.style().interact(&response);
    ui.painter().rect(
        rect,
        visuals.corner_radius,
        visuals.weak_bg_fill,
        visuals.bg_stroke,
        StrokeKind::Inside,
    );
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        face,
        TextStyle::Body.resolve(ui.style()),
        visuals.text_color(),
    );
    response
}

/// Gives the text field with `id`, holding `text`, keyboard focus, with its text
/// cursor at the end: the display, where a key just added to the sum, or the field
/// the keypad closed over.
pub(crate) fn focus_at_end(ctx: &egui::Context, id: Id, text: &str) {
    let mut state = TextEditState::load(ctx, id).unwrap_or_default();
    state
        .cursor
        .set_char_range(Some(CCursorRange::one(CCursor::new(text.chars().count()))));
    state.store(ctx, id);
    ctx.memory_mut(|memory| memory.request_focus(id));
}
