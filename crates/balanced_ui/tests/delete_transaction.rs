//! Deleting a transaction through its confirm Dialog, and the Dialog keys every light
//! Dialog shares, driven through the AccessKit tree the way a user drives it.

use balanced_core::Ledger;
use balanced_i18n::Localizer;
use balanced_ui::App;
use eframe::egui::accesskit::Role;
use eframe::egui::{Event, Key, Modifiers, Vec2, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable as _};

/// The App on stub data, in the test locale, in a window of `size`.
fn app(size: Vec2) -> Harness<'static, App> {
    Harness::builder()
        .with_size(size)
        .build_eframe(|cc| App::new(cc, Localizer::for_tests(), Ledger::stub()))
}

/// The App with Everyday Checking open and its Costco transaction selected.
fn costco_selected() -> Harness<'static, App> {
    let mut harness = app(vec2(1280.0, 720.0));
    harness
        .get_by_role_and_label(Role::Button, "Everyday Checking")
        .click();
    harness.run();
    harness.get_by_role_and_label(Role::Cell, "Costco").click();
    harness.run();
    harness
}

/// Presses `key` with `modifiers` held, then runs the App until it settles.
fn press(harness: &mut Harness<'static, App>, modifiers: Modifiers, key: Key) {
    harness.event(Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers,
    });
    harness.run();
}

/// Whether the Register lists a transaction with `payee`.
fn listed(harness: &Harness<'static, App>, payee: &str) -> bool {
    harness.query_by_role_and_label(Role::Cell, payee).is_some()
}

/// Whether the delete confirm is open.
fn confirm_open(harness: &Harness<'static, App>) -> bool {
    harness
        .query_by_label("Delete this transaction? Costco, 9/12/2026, -206.27")
        .is_some()
}

#[test]
fn ctrl_d_then_enter_deletes_the_selected_transaction() {
    let mut harness = costco_selected();

    press(&mut harness, Modifiers::COMMAND, Key::D);
    press(&mut harness, Modifiers::NONE, Key::Enter);

    assert!(!listed(&harness, "Costco"));
    assert!(!confirm_open(&harness));
}

#[test]
fn enter_deletes_after_focus_leaves_delete() {
    let mut harness = costco_selected();
    press(&mut harness, Modifiers::COMMAND, Key::D);
    // A click on the question takes focus off Delete.
    harness
        .get_by_label("Delete this transaction? Costco, 9/12/2026, -206.27")
        .click();
    harness.run();
    assert!(
        !harness
            .get_by_role_and_label(Role::Button, "Delete")
            .is_focused()
    );

    press(&mut harness, Modifiers::NONE, Key::Enter);

    assert!(!listed(&harness, "Costco"));
}

#[test]
fn the_confirm_asks_about_the_selected_transaction() {
    let mut harness = costco_selected();

    press(&mut harness, Modifiers::COMMAND, Key::D);

    assert!(confirm_open(&harness));
    assert!(listed(&harness, "Costco"));
}

#[test]
fn esc_cancels_and_gives_focus_back_to_the_widget_that_had_it() {
    let mut harness = costco_selected();
    harness.get_by_role_and_label(Role::Cell, "Costco").focus();
    harness.run();

    press(&mut harness, Modifiers::COMMAND, Key::D);
    press(&mut harness, Modifiers::NONE, Key::Escape);

    assert!(!confirm_open(&harness));
    assert!(listed(&harness, "Costco"));
    assert!(
        harness
            .get_by_role_and_label(Role::Cell, "Costco")
            .is_focused()
    );
}

#[test]
fn cancel_closes_the_confirm_without_deleting() {
    let mut harness = costco_selected();
    press(&mut harness, Modifiers::COMMAND, Key::D);

    harness
        .get_by_role_and_label(Role::Button, "Cancel")
        .click();
    harness.run();

    assert!(!confirm_open(&harness));
    assert!(listed(&harness, "Costco"));
}

#[test]
fn clicking_the_dimmed_window_cancels() {
    let mut harness = costco_selected();
    press(&mut harness, Modifiers::COMMAND, Key::D);

    // The dimmed backdrop has no AccessKit node, so the click goes to a position: the
    // top-left corner, outside the Dialog, which sits in the middle.
    harness.event(Event::PointerButton {
        pos: eframe::egui::pos2(5.0, 5.0),
        button: eframe::egui::PointerButton::Primary,
        pressed: true,
        modifiers: Modifiers::NONE,
    });
    harness.event(Event::PointerButton {
        pos: eframe::egui::pos2(5.0, 5.0),
        button: eframe::egui::PointerButton::Primary,
        pressed: false,
        modifiers: Modifiers::NONE,
    });
    harness.run();

    assert!(!confirm_open(&harness));
    assert!(listed(&harness, "Costco"));
}

#[test]
fn delete_has_first_focus() {
    let mut harness = costco_selected();

    press(&mut harness, Modifiers::COMMAND, Key::D);

    assert!(
        harness
            .get_by_role_and_label(Role::Button, "Delete")
            .is_focused()
    );
}

#[test]
fn ctrl_n_does_nothing_while_the_confirm_is_open() {
    let mut harness = costco_selected();
    press(&mut harness, Modifiers::COMMAND, Key::D);

    press(&mut harness, Modifiers::COMMAND, Key::N);

    // Outside a Dialog, Ctrl+N answers in the Status bar until the draft row exists.
    assert!(harness.query_by_label("Not available yet").is_none());
    assert!(confirm_open(&harness));
}

#[test]
fn ctrl_n_acts_once_the_confirm_closes() {
    let mut harness = costco_selected();
    press(&mut harness, Modifiers::COMMAND, Key::D);
    press(&mut harness, Modifiers::NONE, Key::Escape);

    press(&mut harness, Modifiers::COMMAND, Key::N);

    assert!(harness.query_by_label("Not available yet").is_some());
}

#[test]
fn the_actions_menu_opens_the_confirm_and_leaves_focus_nowhere_after() {
    let mut harness = costco_selected();
    harness
        .get_by_role_and_label(Role::Button, "Actions")
        .click();
    harness.run();

    harness
        .get_by_role_and_label(Role::Button, "Delete…")
        .click();
    harness.run();
    assert!(confirm_open(&harness));
    press(&mut harness, Modifiers::NONE, Key::Escape);

    assert!(!confirm_open(&harness));
    assert!(
        !harness
            .root()
            .children_recursive()
            .any(|node| node.is_focused())
    );
}

#[test]
fn the_actions_menu_deletes_once_confirmed() {
    let mut harness = costco_selected();
    harness
        .get_by_role_and_label(Role::Button, "Actions")
        .click();
    harness.run();
    harness
        .get_by_role_and_label(Role::Button, "Delete…")
        .click();
    harness.run();

    harness
        .get_by_role_and_label(Role::Button, "Delete")
        .click();
    harness.run();

    assert!(!listed(&harness, "Costco"));
}

#[test]
fn the_delete_key_alone_opens_nothing() {
    let mut harness = costco_selected();

    press(&mut harness, Modifiers::NONE, Key::Delete);

    assert!(!confirm_open(&harness));
}

#[test]
fn ctrl_d_with_nothing_selected_opens_nothing() {
    let mut harness = app(vec2(1280.0, 720.0));
    harness
        .get_by_role_and_label(Role::Button, "Everyday Checking")
        .click();
    harness.run();

    press(&mut harness, Modifiers::COMMAND, Key::D);

    assert!(
        harness
            .query_by_role_and_label(Role::Button, "Delete")
            .is_none()
    );
}

#[test]
fn ctrl_d_in_a_closed_account_opens_nothing() {
    let mut harness = app(vec2(1280.0, 720.0));
    harness
        .get_by_role_and_label(Role::Button, "Closed")
        .click();
    harness.run();
    harness
        .get_by_role_and_label(Role::Button, "Old Checking")
        .click();
    harness.run();
    harness
        .get_by_role_and_label(Role::Cell, "Opening Balance")
        .click();
    harness.run();

    press(&mut harness, Modifiers::COMMAND, Key::D);

    assert!(
        harness
            .query_by_role_and_label(Role::Button, "Delete")
            .is_none()
    );
}

#[test]
fn the_confirm_keeps_inside_a_small_window() {
    let mut harness = costco_selected();
    let small = vec2(260.0, 90.0);
    harness.set_size(small);
    harness.run();

    press(&mut harness, Modifiers::COMMAND, Key::D);

    // The Dialog keeps a 24-point margin to each edge of the window, its question
    // scrolling so that its buttons stay in view.
    let inside = eframe::egui::Rect::from_min_max(
        eframe::egui::pos2(24.0, 24.0),
        (small - vec2(24.0, 24.0)).to_pos2(),
    );
    for button in ["Delete", "Cancel"] {
        let rect = harness.get_by_role_and_label(Role::Button, button).rect();
        assert!(inside.contains_rect(rect), "{button} at {rect:?}");
    }
}
