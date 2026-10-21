// SPDX-License-Identifier: GPL-3.0-or-later WITH AdditionRef-Balanced-Plugin-Exception

//! Reading the category tree and its usage in the Category List, driven through the
//! AccessKit tree the way a user drives it.

use balanced_core::Ledger;
use balanced_i18n::Localizer;
use balanced_ui::{App, Clock, PreferencesFile};
use eframe::egui::accesskit::Role;
use eframe::egui::{Event, Key, Modifiers};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable as _;
use icu_calendar::Date;

/// The App on `ledger`, in the test locale.
fn app_on(ledger: Ledger) -> Harness<'static, App> {
    let mut harness = Harness::builder()
        .with_size(eframe::egui::vec2(1280.0, 800.0))
        .build_eframe(|cc| {
            App::on_ledger(
                cc,
                Localizer::for_tests(),
                ledger,
                Clock::fixed(Date::try_new_iso(2026, 10, 14).expect("14 October 2026 is a date")),
                PreferencesFile::for_tests(),
            )
        });
    harness.run();
    harness
}

/// Opens the Category List from the Tools menu.
fn open_category_list(harness: &mut Harness<'static, App>) {
    harness.get_by_role_and_label(Role::Button, "Tools").click();
    harness.run();
    harness
        .get_by_role_and_label(Role::Button, "Category List…")
        .click();
    harness.run();
}

/// The App on stub data, with the Category List open.
fn category_list() -> Harness<'static, App> {
    let mut harness = app_on(Ledger::stub());
    open_category_list(&mut harness);
    harness
}

/// Whether the Category List is open.
fn is_open(harness: &Harness<'static, App>) -> bool {
    harness
        .query_by_role_and_label(Role::Window, "Category List")
        .is_some()
}

/// Presses `key` with no modifiers, then runs the App until it settles.
fn press(harness: &mut Harness<'static, App>, key: Key) {
    harness.event(Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Modifiers::NONE,
    });
    harness.run();
}

#[test]
fn tools_category_list_opens_the_dialog_and_close_dismisses_it() {
    let mut harness = category_list();
    let opened = is_open(&harness);

    harness.get_by_role_and_label(Role::Button, "Close").click();
    harness.run();

    assert!(opened);
    assert!(!is_open(&harness));
}

#[test]
fn escape_dismisses_the_category_list() {
    let mut harness = category_list();

    press(&mut harness, Key::Escape);

    assert!(!is_open(&harness));
}
