// SPDX-License-Identifier: GPL-3.0-or-later WITH AdditionRef-Balanced-Plugin-Exception

//! The Calculator: a Tool window, opened from Tools ▸ Calculator, that stays open over
//! the Main view while the user works in the Register.
//!
//! Its body is the keypad's, with = in place of OK: = and Enter replace the sum with its
//! result. Its one Paste button hands the result to the Main window, which puts it into
//! the amount cell last edited. Esc in its display, or its ×, closes it.

use balanced_core::CurrencyCode;
use balanced_i18n::{AmountReader, Localizer};
use eframe::egui::{self, Align2, Button, Id, Key, LayerId, Modifiers, Order, Pos2, Window};

use crate::amount_field;
use crate::dialog::{self, TitleRow};
use crate::formatted_amount::i18n_currency;
use crate::i18n::fl;
use crate::keypad::{self, Finish, Keypad};
use crate::theme::Theme;

/// The Calculator's state while it is open.
#[derive(Debug, Clone)]
pub(crate) struct Calculator {
    keypad: Keypad,
    /// The currency it reads and shows amounts in.
    currency: CurrencyCode,
    /// What reads its sum, with the marks of the locale it opened in, so a locale
    /// change never rereads the sum under other marks.
    reader: AmountReader,
    /// Whether it has been placed yet. It opens at its default spot.
    placed: bool,
}

/// What the Calculator did this frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Outcome {
    /// It stays open.
    Open,
    /// It closed.
    Closed,
    /// Paste was pressed, with this result, formatted as an amount field shows it.
    Paste(String),
    /// Paste was pressed while the sum isn't an amount.
    PasteRefused,
}

impl Calculator {
    /// The Calculator, empty, with its display taking the keyboard, reading and
    /// showing amounts in `currency` as `i18n` formats them.
    pub(crate) fn new(i18n: &Localizer, currency: CurrencyCode) -> Self {
        Self {
            keypad: Keypad::new(""),
            currency,
            reader: i18n.amount_reader(i18n_currency(currency)),
            placed: false,
        }
    }

    /// Draws the Calculator, opening with its top right corner at `spot`, and acts on
    /// this frame's keys and presses.
    pub(crate) fn show(
        &mut self,
        ctx: &egui::Context,
        i18n: &Localizer,
        theme: &Theme,
        spot: Pos2,
    ) -> Outcome {
        let id = Id::new("calculator");
        let Self {
            keypad,
            currency,
            reader,
            placed,
        } = self;
        let currency = *currency;
        if ctx.memory(|memory| memory.has_focus(keypad::display_id(id))) {
            amount_field::refuse_characters(ctx, reader);
            let (enter, escape) = ctx.input_mut(|input| {
                (
                    input.consume_key(Modifiers::NONE, Key::Enter),
                    input.consume_key(Modifiers::NONE, Key::Escape),
                )
            });
            if escape {
                return Outcome::Closed;
            }
            if enter {
                keypad.equals(i18n, reader, currency);
            }
        }
        let title = fl!(i18n, "calculator");
        let TitleRow {
            gap, close_slot, ..
        } = TitleRow::of(ctx, &title);
        let room = ctx.content_rect().shrink(Theme::DIALOG_MARGIN);
        let mut window =
            Window::new((title.clone(), egui::Atom::custom(id.with("close_gap"), gap)))
                .id(id)
                .collapsible(false)
                .resizable(false)
                .pivot(Align2::RIGHT_TOP)
                .constrain_to(room);
        if !std::mem::replace(placed, true) {
            window = window.current_pos(spot);
        }
        let shown = window.show(ctx, |ui| {
            let pressed = keypad.body(
                ui,
                i18n,
                theme,
                reader,
                currency,
                id,
                &title,
                Finish::Equals,
            );
            let paste = ui.add_sized(
                [ui.available_width(), theme.row_height() * 1.4],
                Button::new(fl!(i18n, "calculator-paste")),
            );
            (pressed, paste.clicked())
        });
        let Some(shown) = shown else {
            return Outcome::Open;
        };
        let closed = dialog::close_button(
            ctx,
            id,
            LayerId::new(Order::Middle, id),
            shown.response.rect,
            close_slot,
            &fl!(i18n, "calculator-close"),
        )
        .clicked();
        if closed {
            return Outcome::Closed;
        }
        let (pressed, paste) = shown.inner.unwrap_or_default();
        if keypad.press(pressed, reader) {
            keypad.equals(i18n, reader, currency);
        }
        let outcome = if paste {
            match keypad.result(i18n, reader, currency) {
                Some(result) => Outcome::Paste(result),
                None => Outcome::PasteRefused,
            }
        } else {
            Outcome::Open
        };
        keypad.end_frame(ctx, id);
        outcome
    }
}
