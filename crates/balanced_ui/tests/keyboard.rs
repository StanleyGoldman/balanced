//! Keyboard focus and Menu bar navigation, driven through the AccessKit tree the way a
//! keyboard user drives it.

use balanced_core::Ledger;
use balanced_i18n::Localizer;
use balanced_ui::{App, Clock};
use eframe::App as _;
use eframe::egui::accesskit::Role;
use eframe::egui::{Event, Key, Modifiers, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{By, NodeT as _, Queryable as _};
use icu_calendar::Date;

/// The App on stub data, in the test locale, wide enough to show every Register column.
fn app() -> Harness<'static, App> {
    Harness::builder()
        .with_size(vec2(1280.0, 720.0))
        .build_eframe(|cc| {
            App::new(
                cc,
                Localizer::for_tests(),
                Ledger::stub(),
                Clock::fixed(Date::try_new_iso(2026, 10, 14).expect("14 October 2026 is a date")),
            )
        })
}

/// Plays `event` in one frame, first passing it through the App's `raw_input_hook`
/// as eframe does. `egui_kittest` skips the hook.
fn send(harness: &mut Harness<'static, App>, event: Event) {
    let ctx = harness.ctx.clone();
    let mut input = std::mem::take(harness.input_mut());
    input.events.push(event);
    harness.state_mut().raw_input_hook(&ctx, &mut input);
    *harness.input_mut() = input;
    harness.step();
}

/// Presses `key` with `modifiers` held, then lets the App settle.
fn press(harness: &mut Harness<'static, App>, modifiers: Modifiers, key: Key) {
    send(
        harness,
        Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers,
        },
    );
    harness.run();
}

/// Opens the account named `name` from the Account bar.
fn open(harness: &mut Harness<'static, App>, name: &str) {
    harness.get_by_role_and_label(Role::Button, name).click();
    harness.run();
}

/// Puts the cell cursor on the cell labelled `label` by clicking it.
fn click_cell(harness: &mut Harness<'static, App>, label: &str) {
    harness.get_by_role_and_label(Role::Cell, label).click();
    harness.run();
}

/// The label of the cell under the cell cursor.
fn cursor(harness: &Harness<'static, App>) -> String {
    let cursor = harness
        .query_all_by_role(Role::Cell)
        .find(|cell| cell.accesskit_node().is_selected() == Some(true))
        .expect("a cell holds the cell cursor");
    cursor.accesskit_node().label().unwrap_or_default()
}

/// The label of the widget with keyboard focus, or `None` when nothing has it.
fn focused(harness: &Harness<'static, App>) -> Option<String> {
    harness
        .query_all(By::default())
        .find(|node| node.accesskit_node().is_focused())
        .map(|node| node.accesskit_node().label().unwrap_or_default())
}

#[test]
fn f6_moves_focus_to_the_account_bar() {
    let mut harness = app();

    press(&mut harness, Modifiers::NONE, Key::F6);

    assert_eq!(focused(&harness).as_deref(), Some("Everyday Checking"));
}

#[test]
fn f6_focuses_the_open_account_in_the_account_bar() {
    let mut harness = app();
    open(&mut harness, "Joint Checking");

    press(&mut harness, Modifiers::NONE, Key::F6);

    assert_eq!(focused(&harness).as_deref(), Some("Joint Checking"));
}

#[test]
fn shift_f6_hands_the_keyboard_back_to_the_register() {
    let mut harness = app();
    open(&mut harness, "Everyday Checking");
    click_cell(&mut harness, "Fresh Market");
    press(&mut harness, Modifiers::NONE, Key::F6);

    press(&mut harness, Modifiers::SHIFT, Key::F6);
    press(&mut harness, Modifiers::NONE, Key::ArrowDown);

    assert_eq!(focused(&harness), None);
    assert_eq!(cursor(&harness), "Bean There");
}

/// Types `text` into whatever has the keyboard, then lets the App settle.
fn type_text(harness: &mut Harness<'static, App>, text: &str) {
    send(harness, Event::Text(text.to_owned()));
    harness.run();
}

#[test]
fn typing_a_letter_in_the_account_bar_jumps_to_the_next_account_with_it() {
    let mut harness = app();
    press(&mut harness, Modifiers::NONE, Key::F6);

    type_text(&mut harness, "e");

    assert_eq!(focused(&harness).as_deref(), Some("Emergency Fund"));
}

#[test]
fn the_letter_jump_wraps_to_the_first_account_with_the_letter() {
    let mut harness = app();
    press(&mut harness, Modifiers::NONE, Key::F6);
    type_text(&mut harness, "e");

    type_text(&mut harness, "E");

    assert_eq!(focused(&harness).as_deref(), Some("Everyday Checking"));
}

#[test]
fn f10_opens_the_first_menu_on_its_first_enabled_item() {
    let mut harness = app();

    press(&mut harness, Modifiers::NONE, Key::F10);

    assert_eq!(focused(&harness).as_deref(), Some("New Account…"));
}

#[test]
fn down_skips_a_disabled_menu_item() {
    let mut harness = app();
    press(&mut harness, Modifiers::NONE, Key::F10);

    press(&mut harness, Modifiers::NONE, Key::ArrowDown);

    // Close Register Tab, between New Account and Quit, is disabled while no Register
    // is open.
    assert_eq!(focused(&harness).as_deref(), Some("Quit Ctrl+Q"));
}

#[test]
fn up_and_down_move_within_a_menu_and_wrap() {
    let mut harness = app();
    open(&mut harness, "Everyday Checking");
    press(&mut harness, Modifiers::NONE, Key::F10);

    let mut seen = Vec::new();
    for key in [Key::ArrowDown, Key::ArrowDown, Key::ArrowUp] {
        press(&mut harness, Modifiers::NONE, key);
        seen.push(focused(&harness).unwrap_or_default());
    }

    assert_eq!(
        seen,
        [
            "Close Register Tab Ctrl+W",
            "Quit Ctrl+Q",
            "Close Register Tab Ctrl+W"
        ]
    );
}

#[test]
fn left_and_right_move_between_menus_and_wrap() {
    let mut harness = app();
    press(&mut harness, Modifiers::NONE, Key::F10);

    let mut seen = Vec::new();
    for key in [Key::ArrowLeft, Key::ArrowRight] {
        press(&mut harness, Modifiers::NONE, key);
        seen.push(focused(&harness).unwrap_or_default());
    }

    // Left from File wraps to Help, and Right from Help wraps back to File.
    assert_eq!(seen, ["About Balanced", "New Account…"]);
}

#[test]
fn enter_chooses_the_focused_menu_item() {
    let mut harness = app();
    press(&mut harness, Modifiers::NONE, Key::F10);
    press(&mut harness, Modifiers::NONE, Key::ArrowLeft);

    press(&mut harness, Modifiers::NONE, Key::Enter);

    harness.get_by_label("Balanced 0.0.0");
}

#[test]
fn enter_closes_the_menu() {
    let mut harness = app();
    press(&mut harness, Modifiers::NONE, Key::F10);
    press(&mut harness, Modifiers::NONE, Key::ArrowLeft);

    press(&mut harness, Modifiers::NONE, Key::Enter);

    assert!(harness.query_by_label("About Balanced").is_none());
}

#[test]
fn esc_closes_the_menu() {
    let mut harness = app();
    press(&mut harness, Modifiers::NONE, Key::F10);

    press(&mut harness, Modifiers::NONE, Key::Escape);

    assert!(harness.query_by_label("Quit Ctrl+Q").is_none());
}

/// Holds `modifiers` down, or releases them all with `Modifiers::NONE`, then lets the
/// App settle.
fn hold(harness: &mut Harness<'static, App>, modifiers: Modifiers) {
    send(harness, Event::ModifiersChanged(modifiers));
    harness.run();
}

#[test]
fn a_lone_alt_opens_the_first_menu() {
    let mut harness = app();

    hold(&mut harness, Modifiers::ALT);
    hold(&mut harness, Modifiers::NONE);

    assert_eq!(focused(&harness).as_deref(), Some("New Account…"));
}

#[test]
fn alt_released_after_a_chord_opens_no_menu() {
    let mut harness = app();

    hold(&mut harness, Modifiers::ALT);
    press(&mut harness, Modifiers::ALT, Key::F);
    hold(&mut harness, Modifiers::NONE);

    assert!(harness.query_by_label("Quit Ctrl+Q").is_none());
}

#[test]
fn a_shortcut_pressed_mid_edit_commits_the_edit_first() {
    let mut harness = app();
    open(&mut harness, "Everyday Checking");
    click_cell(&mut harness, "Fresh Market");
    type_text(&mut harness, "Corner Shop");

    press(&mut harness, Modifiers::COMMAND, Key::W);
    open(&mut harness, "Everyday Checking");

    let payees: Vec<String> = harness
        .query_all_by_role(Role::Cell)
        .map(|cell| cell.accesskit_node().label().unwrap_or_default())
        .filter(|label| ["Fresh Market", "Corner Shop"].contains(&label.as_str()))
        .collect();
    assert_eq!(payees, ["Corner Shop"]);
}

#[test]
fn alt_released_after_leaving_the_window_opens_no_menu() {
    let mut harness = app();

    // Alt+Tab: the OS takes the Tab, and the window loses focus before Alt is up.
    hold(&mut harness, Modifiers::ALT);
    send(&mut harness, Event::WindowFocused(false));
    hold(&mut harness, Modifiers::NONE);

    assert!(harness.query_by_label("Quit Ctrl+Q").is_none());
}

#[test]
fn a_shortcut_closes_the_open_menu() {
    let mut harness = app();
    press(&mut harness, Modifiers::NONE, Key::F10);

    press(&mut harness, Modifiers::NONE, Key::F6);

    assert!(harness.query_by_label("Quit Ctrl+Q").is_none());
}

#[test]
fn f10_opens_no_menu_behind_a_dialog() {
    let mut harness = app();
    open(&mut harness, "Everyday Checking");
    click_cell(&mut harness, "Fresh Market");
    press(&mut harness, Modifiers::COMMAND, Key::D);

    press(&mut harness, Modifiers::NONE, Key::F10);

    assert!(harness.query_by_label("Quit Ctrl+Q").is_none());
}
