//! Creating a category from a Register Category cell through the New Category Dialog,
//! and the keys and placement every heavy Dialog shares, driven through the AccessKit
//! tree the way a user drives it.

use balanced_core::Ledger;
use balanced_i18n::Localizer;
use balanced_ui::{App, Clock, PreferencesFile};
use eframe::App as _;
use eframe::egui::accesskit::Role;
use eframe::egui::{Event, Key, Modifiers, Vec2, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable as _};
use icu_calendar::Date;

/// The App on stub data, in the test locale, in a window of `size`.
fn app(size: Vec2) -> Harness<'static, App> {
    let mut harness = Harness::builder().with_size(size).build_eframe(|cc| {
        App::new(
            cc,
            Localizer::for_tests(),
            Ledger::stub(),
            Clock::fixed(Date::try_new_iso(2026, 10, 14).expect("14 October 2026 is a date")),
            PreferencesFile::for_tests(),
        )
    });
    harness.run();
    harness
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

/// Types `text` into whatever has the keyboard, then lets the App settle.
fn type_text(harness: &mut Harness<'static, App>, text: &str) {
    send(harness, Event::Text(text.to_owned()));
    harness.run();
}

/// The App with Everyday Checking open and `typed` typed over the Shell row's
/// category, with the combobox list open.
fn typed_in_category(typed: &str) -> Harness<'static, App> {
    let mut harness = app(vec2(1280.0, 720.0));
    harness
        .get_by_role_and_label(Role::Button, "Everyday Checking")
        .click();
    harness.run();
    harness
        .get_by_role_and_label(Role::Cell, "Auto & Transport:Fuel")
        .click();
    harness.run();
    type_text(&mut harness, typed);
    harness
}

/// Moves the open list's highlight down to its last row, the Create row.
fn highlight_create_row(harness: &mut Harness<'static, App>) {
    let rows = harness.query_all_by_role(Role::ListBoxOption).count();
    for _ in 1..rows {
        press(harness, Modifiers::NONE, Key::ArrowDown);
    }
}

/// The App with New Category open from the Create row for `typed`, taken with Enter.
fn new_category_for(typed: &str) -> Harness<'static, App> {
    let mut harness = typed_in_category(typed);
    highlight_create_row(&mut harness);
    press(&mut harness, Modifiers::NONE, Key::Enter);
    harness
}

/// The text in the text field labelled `label`, or `None` when none is shown.
fn field(harness: &Harness<'static, App>, label: &str) -> Option<String> {
    harness
        .query_by_role_and_label(Role::TextInput, label)
        .map(|node| node.accesskit_node().value().unwrap_or_default())
}

#[test]
fn the_create_row_opens_new_category_prefilled_with_the_typed_name() {
    let harness = new_category_for("Pets");

    assert_eq!(field(&harness, "Name").as_deref(), Some("Pets"));
}

/// The label of the cell under the cell cursor.
fn cursor(harness: &Harness<'static, App>) -> String {
    let mut cursors = harness
        .query_all_by_role(Role::Cell)
        .filter(|cell| cell.accesskit_node().is_selected() == Some(true));
    let cursor = cursors.next().expect("a cell holds the cell cursor");
    assert!(cursors.next().is_none(), "one cell holds the cell cursor");
    cursor.accesskit_node().label().unwrap_or_default()
}

/// Whether a label shows `text`. A `Label`'s text is its AccessKit value.
fn shows(harness: &Harness<'static, App>, text: &str) -> bool {
    harness
        .query_all_by_role(Role::Label)
        .any(|label| label.accesskit_node().value().as_deref() == Some(text))
}

/// Whether a Register cell shows `label`.
fn cell_shows(harness: &Harness<'static, App>, label: &str) -> bool {
    harness.query_by_role_and_label(Role::Cell, label).is_some()
}

/// Whether the New Category Dialog is open.
fn dialog_open(harness: &Harness<'static, App>) -> bool {
    field(harness, "Name").is_some()
}

#[test]
fn ok_creates_the_category_fills_the_cell_and_enter_moves_on() {
    let mut harness = new_category_for("Pets");

    harness.get_by_role_and_label(Role::Button, "OK").click();
    harness.run();

    assert!(!dialog_open(&harness));
    assert_eq!(field(&harness, "Category"), None);
    assert!(cell_shows(&harness, "Pets"));
    // Enter took the Create row, so the cell cursor moves right as Enter does.
    assert_eq!(cursor(&harness), "");
}

#[test]
fn enter_in_the_name_field_presses_ok() {
    let mut harness = new_category_for("Pets");

    press(&mut harness, Modifiers::NONE, Key::Enter);

    assert!(!dialog_open(&harness));
    assert!(cell_shows(&harness, "Pets"));
}

#[test]
fn the_new_category_is_offered_by_every_category_cell_after() {
    let mut harness = new_category_for("Pets");
    press(&mut harness, Modifiers::NONE, Key::Enter);
    harness
        .get_by_role_and_label(Role::Cell, "Food & Dining:Groceries")
        .click();
    harness.run();

    type_text(&mut harness, "pet");

    let rows: Vec<String> = harness
        .query_all_by_role(Role::ListBoxOption)
        .map(|row| row.accesskit_node().label().unwrap_or_default())
        .collect();
    assert_eq!(rows, ["Pets", "Create \"pet\"…"]);
}

#[test]
fn tab_on_the_create_row_opens_the_next_cell_once_created() {
    let mut harness = typed_in_category("Pets");
    highlight_create_row(&mut harness);
    press(&mut harness, Modifiers::NONE, Key::Tab);

    press(&mut harness, Modifiers::NONE, Key::Enter);

    assert!(cell_shows(&harness, "Pets"));
    assert!(field(&harness, "Memo").is_some());
}

#[test]
fn a_path_creates_the_category_under_the_one_it_names() {
    let mut harness = new_category_for("home:Pets");

    press(&mut harness, Modifiers::NONE, Key::Enter);

    assert!(cell_shows(&harness, "Home:Pets"));
}

#[test]
fn a_duplicate_name_shows_inline_and_keeps_the_dialog_open() {
    let mut harness = new_category_for("Home : insurance");

    press(&mut harness, Modifiers::NONE, Key::Enter);

    assert!(dialog_open(&harness));
    assert!(shows(
        &harness,
        "A category named “insurance” is already there."
    ));
    assert_eq!(field(&harness, "Name").as_deref(), Some("Home : insurance"));
}

#[test]
fn an_unknown_parent_shows_inline_and_keeps_the_dialog_open() {
    let mut harness = new_category_for("Nowhere:Pets");

    press(&mut harness, Modifiers::NONE, Key::Enter);

    assert!(dialog_open(&harness));
    assert!(shows(&harness, "No category is named “Nowhere”."));
}

#[test]
fn cancel_goes_back_to_the_cell_edit_with_the_typed_text() {
    let mut harness = new_category_for("Pets");

    harness
        .get_by_role_and_label(Role::Button, "Cancel")
        .click();
    harness.run();

    assert!(!dialog_open(&harness));
    assert_eq!(field(&harness, "Category").as_deref(), Some("Pets"));
    assert!(cell_shows(&harness, "Auto & Transport:Fuel"));
}

#[test]
fn esc_cancels() {
    let mut harness = new_category_for("Pets");

    press(&mut harness, Modifiers::NONE, Key::Escape);

    assert!(!dialog_open(&harness));
    assert_eq!(field(&harness, "Category").as_deref(), Some("Pets"));
}

#[test]
fn the_close_button_cancels() {
    let mut harness = new_category_for("Pets");

    harness
        .get_by_role_and_label(Role::Button, "Close dialog")
        .click();
    harness.run();

    assert!(!dialog_open(&harness));
    assert_eq!(field(&harness, "Category").as_deref(), Some("Pets"));
}

#[test]
fn clicking_the_dimmed_window_cancels() {
    let mut harness = new_category_for("Pets");

    // The dimmed backdrop has no AccessKit node, so the click goes to a position: the
    // top-left corner, outside the Dialog, which sits in the middle.
    for pressed in [true, false] {
        harness.event(Event::PointerButton {
            pos: eframe::egui::pos2(5.0, 5.0),
            button: eframe::egui::PointerButton::Primary,
            pressed,
            modifiers: Modifiers::NONE,
        });
    }
    harness.run();

    assert!(!dialog_open(&harness));
    assert_eq!(field(&harness, "Category").as_deref(), Some("Pets"));
}

#[test]
fn undo_takes_the_category_out_of_the_cell_then_deletes_it() {
    let mut harness = new_category_for("Pets");
    press(&mut harness, Modifiers::NONE, Key::Enter);

    press(&mut harness, Modifiers::COMMAND, Key::Z);
    let after_first = harness
        .query_by_role_and_label(Role::Cell, "Auto & Transport:Fuel")
        .is_some();
    press(&mut harness, Modifiers::COMMAND, Key::Z);
    harness
        .get_by_role_and_label(Role::Cell, "Food & Dining:Groceries")
        .click();
    harness.run();
    type_text(&mut harness, "pet");

    assert!(after_first);
    let rows: Vec<String> = harness
        .query_all_by_role(Role::ListBoxOption)
        .map(|row| row.accesskit_node().label().unwrap_or_default())
        .collect();
    assert_eq!(rows, ["Create \"pet\"…"]);
}

#[test]
fn the_dialog_opens_centred() {
    let harness = new_category_for("Pets");

    let dialog = harness.get_by_role_and_label(Role::Button, "OK").rect();
    let name = harness
        .get_by_role_and_label(Role::TextInput, "Name")
        .rect();
    let centre = name.union(dialog).center();
    assert!(
        (centre.x - 640.0).abs() < 40.0 && (centre.y - 360.0).abs() < 60.0,
        "centred at {centre:?}"
    );
}

#[test]
fn the_dialog_keeps_inside_a_small_window() {
    let mut harness = typed_in_category("Pets");
    let small = vec2(300.0, 160.0);
    harness.set_size(small);
    harness.run();
    highlight_create_row(&mut harness);
    press(&mut harness, Modifiers::NONE, Key::Enter);

    // The Dialog keeps a 24-point margin to each edge of the window.
    let inside = eframe::egui::Rect::from_min_max(
        eframe::egui::pos2(24.0, 24.0),
        (small - vec2(24.0, 24.0)).to_pos2(),
    );
    for button in ["OK", "Cancel", "Close dialog"] {
        let rect = harness.get_by_role_and_label(Role::Button, button).rect();
        assert!(inside.contains_rect(rect), "{button} at {rect:?}");
    }
}

#[test]
fn dragging_the_title_bar_moves_the_dialog() {
    let mut harness = new_category_for("Pets");
    let before = harness.get_by_role_and_label(Role::Button, "OK").rect();
    let title = harness
        .get_by_role_and_label(Role::Button, "Close dialog")
        .rect()
        .left_center()
        - vec2(60.0, 0.0);

    drag(&mut harness, title, title + vec2(-100.0, -50.0));

    let after = harness.get_by_role_and_label(Role::Button, "OK").rect();
    assert_eq!(after.min - before.min, vec2(-100.0, -50.0));
}

#[test]
fn dragging_inside_the_body_leaves_the_dialog_in_place() {
    let mut harness = new_category_for("Pets");
    let before = harness.get_by_role_and_label(Role::Button, "OK").rect();
    let name = harness
        .get_by_role_and_label(Role::TextInput, "Name")
        .rect()
        .center();
    // Empty space in the button row, left of OK.
    let blank = before.left_center() - vec2(100.0, 0.0);

    drag(&mut harness, name, name + vec2(-100.0, -50.0));
    drag(&mut harness, blank, blank + vec2(-100.0, -50.0));

    let after = harness.get_by_role_and_label(Role::Button, "OK").rect();
    assert_eq!(after.min, before.min);
}

#[test]
fn dragging_from_the_close_button_leaves_the_dialog_in_place() {
    let mut harness = new_category_for("Pets");
    let before = harness.get_by_role_and_label(Role::Button, "OK").rect();
    let close = harness
        .get_by_role_and_label(Role::Button, "Close dialog")
        .rect()
        .center();

    drag(&mut harness, close, close + vec2(-100.0, -50.0));

    let after = harness.get_by_role_and_label(Role::Button, "OK").rect();
    assert_eq!(after.min, before.min);
}

/// Drags the pointer from `from` to `to` with the primary button held.
fn drag(harness: &mut Harness<'static, App>, from: eframe::egui::Pos2, to: eframe::egui::Pos2) {
    harness.event(Event::PointerMoved(from));
    harness.event(Event::PointerButton {
        pos: from,
        button: eframe::egui::PointerButton::Primary,
        pressed: true,
        modifiers: Modifiers::NONE,
    });
    harness.step();
    harness.event(Event::PointerMoved(to));
    harness.step();
    harness.event(Event::PointerButton {
        pos: to,
        button: eframe::egui::PointerButton::Primary,
        pressed: false,
        modifiers: Modifiers::NONE,
    });
    harness.run();
}

#[test]
fn dragging_the_bottom_right_corner_resizes_the_dialog() {
    let mut harness = new_category_for("Pets");
    let name = harness
        .get_by_role_and_label(Role::TextInput, "Name")
        .rect();
    let ok = harness.get_by_role_and_label(Role::Button, "OK").rect();
    // The resize grip sits just inside the Dialog's frame, below and right of OK.
    let corner = eframe::egui::pos2(name.right() + 4.0, ok.bottom() + 4.0);

    drag(&mut harness, corner, corner + vec2(80.0, 0.0));

    let wider = harness
        .get_by_role_and_label(Role::TextInput, "Name")
        .rect();
    assert_eq!(wider.width() - name.width(), 80.0);
}

#[test]
fn no_shortcut_acts_behind_the_dialog() {
    let mut harness = new_category_for("Pets");

    press(&mut harness, Modifiers::COMMAND, Key::W);

    assert!(dialog_open(&harness));
    assert!(cell_shows(&harness, "Auto & Transport:Fuel"));
}
