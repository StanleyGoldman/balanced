// SPDX-License-Identifier: GPL-3.0-or-later WITH AdditionRef-Balanced-Plugin-Exception

//! Reaching and pressing the Register's Clr and Flag cells from the keyboard, driven
//! through the AccessKit tree the way a keyboard user drives it.

use balanced_core::Ledger;
use balanced_i18n::Localizer;
use balanced_ui::{App, Clock, PreferencesFile};
use eframe::App as _;
use eframe::egui::accesskit::Role;
use eframe::egui::{Event, Key, Modifiers, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{By, NodeT as _, Queryable as _};
use icu_calendar::Date;

/// Fresh Market's Clr cell in Everyday Checking, which starts Uncleared.
const FRESH_MARKET_CLR: &str = "Clr Fresh Market, 9/5/2026, -86.42";

/// Fresh Market's Flag cell in Everyday Checking, which starts unflagged.
const FRESH_MARKET_FLAG: &str = "Flag Fresh Market, 9/5/2026, -86.42";

/// The App on stub data, in the test locale, wide enough to show every Register column,
/// with Everyday Checking open.
fn everyday_checking() -> Harness<'static, App> {
    everyday_checking_with(PreferencesFile::for_tests())
}

/// The App as [`everyday_checking`] makes it, with "Enter moves to the next row"
/// turned off in `preferences.toml`, which lives in the returned directory.
fn everyday_checking_with_enter_moving_off() -> (tempfile::TempDir, Harness<'static, App>) {
    let dir = tempfile::tempdir().expect("the test machine can make a temporary directory");
    std::fs::write(
        dir.path().join("preferences.toml"),
        "version = 2\nenter_moves_to_next_row = false\n",
    )
    .expect("the test can write the preferences file");
    let harness = everyday_checking_with(PreferencesFile::in_dir(dir.path()));
    (dir, harness)
}

/// The App on stub data with `preferences`, in the test locale, wide enough to show
/// every Register column, with Everyday Checking open.
fn everyday_checking_with(preferences: PreferencesFile) -> Harness<'static, App> {
    let mut harness = Harness::builder()
        .with_size(vec2(1440.0, 720.0))
        .build_eframe(|cc| {
            App::on_ledger(
                cc,
                Localizer::for_tests(),
                Ledger::stub(),
                Clock::fixed(Date::try_new_iso(2026, 10, 14).expect("14 October 2026 is a date")),
                preferences,
            )
        });
    harness.run();
    harness
        .get_by_role_and_label(Role::Button, "Everyday Checking")
        .click();
    harness.run();
    harness
}

/// Plays `events` in one frame, first passing them through the App's `raw_input_hook`
/// as eframe does. `egui_kittest` skips the hook.
fn send(harness: &mut Harness<'static, App>, events: impl IntoIterator<Item = Event>) {
    let ctx = harness.ctx.clone();
    let mut input = std::mem::take(harness.input_mut());
    input.events.extend(events);
    harness.state_mut().raw_input_hook(&ctx, &mut input);
    *harness.input_mut() = input;
    harness.step();
    harness.run();
}

/// The event of pressing `key` with `modifiers` held.
fn key(modifiers: Modifiers, key: Key) -> Event {
    Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers,
    }
}

/// Presses `key` alone, then lets the App settle.
fn press(harness: &mut Harness<'static, App>, pressed: Key) {
    send(harness, [key(Modifiers::NONE, pressed)]);
}

/// Presses Space, which also types a space, as eframe reports it.
fn press_space(harness: &mut Harness<'static, App>) {
    send(
        harness,
        [
            key(Modifiers::NONE, Key::Space),
            Event::Text(" ".to_owned()),
        ],
    );
}

/// Puts the cell cursor on the cell labelled `label` by clicking it.
fn click_cell(harness: &mut Harness<'static, App>, label: &str) {
    harness.get_by_role_and_label(Role::Cell, label).click();
    harness.run();
}

/// The label of the cell under the cell cursor, whatever its role: a Clr cell is a
/// button and a Flag cell a checkbox.
fn cursor(harness: &Harness<'static, App>) -> String {
    let mut cursors = harness.query_all(By::default()).filter(|node| {
        let node = node.accesskit_node();
        matches!(node.role(), Role::Cell | Role::Button | Role::CheckBox)
            && node.is_selected() == Some(true)
    });
    let cursor = cursors.next().expect("a cell holds the cell cursor");
    assert!(cursors.next().is_none(), "one cell holds the cell cursor");
    cursor.accesskit_node().label().unwrap_or_default()
}

#[test]
fn the_arrow_keys_move_the_cell_cursor_onto_clr_then_flag() {
    let mut harness = everyday_checking();
    // Fresh Market's Payment cell, then its Deposit cell.
    click_cell(&mut harness, "86.42");
    press(&mut harness, Key::ArrowRight);

    press(&mut harness, Key::ArrowRight);
    let on_clr = cursor(&harness);
    press(&mut harness, Key::ArrowRight);

    assert_eq!(on_clr, FRESH_MARKET_CLR);
    assert_eq!(cursor(&harness), FRESH_MARKET_FLAG);
}

/// The status the Clr cell labelled `label` reads out.
fn status(harness: &Harness<'static, App>, label: &str) -> String {
    harness
        .get_by_role_and_label(Role::Button, label)
        .accesskit_node()
        .value()
        .expect("a Clr cell reads out its status")
}

/// Moves the cell cursor onto Fresh Market's Clr cell with the arrow keys, from its
/// Payment cell.
fn arrow_to_fresh_market_clr(harness: &mut Harness<'static, App>) {
    click_cell(harness, "86.42");
    press(harness, Key::ArrowRight);
    press(harness, Key::ArrowRight);
}

#[test]
fn space_on_an_uncleared_rows_clr_cell_marks_it_cleared() {
    let mut harness = everyday_checking();
    arrow_to_fresh_market_clr(&mut harness);

    press_space(&mut harness);

    assert_eq!(status(&harness, FRESH_MARKET_CLR), "Cleared");
}

/// The question the confirm asks before un-reconciling the first salary.
const UNRECONCILE_ACME_CORP: &str = "Un-reconcile this transaction? It has been matched against a statement, and will be marked Cleared. Acme Corp, 9/1/2026, 3,200.00";

#[test]
fn space_on_a_reconciled_rows_clr_cell_asks_before_un_reconciling_it() {
    let mut harness = everyday_checking();
    // The first salary's Date cell, then right past Number, Payee, Category, Memo,
    // Payment and Deposit.
    click_cell(&mut harness, "9/1/2026");
    for _ in 0..7 {
        press(&mut harness, Key::ArrowRight);
    }

    press_space(&mut harness);

    assert!(harness.query_by_label(UNRECONCILE_ACME_CORP).is_some());
}

/// Whether the Flag cell labelled `label` shows its transaction flagged.
fn flagged(harness: &Harness<'static, App>, label: &str) -> bool {
    harness
        .get_by_role_and_label(Role::CheckBox, label)
        .accesskit_node()
        .toggled()
        == Some(eframe::egui::accesskit::Toggled::True)
}

#[test]
fn space_on_a_flag_cell_toggles_the_flag() {
    let mut harness = everyday_checking();
    arrow_to_fresh_market_clr(&mut harness);
    press(&mut harness, Key::ArrowRight);

    press_space(&mut harness);
    let after_one = flagged(&harness, FRESH_MARKET_FLAG);
    press_space(&mut harness);

    assert!(after_one);
    assert!(!flagged(&harness, FRESH_MARKET_FLAG));
}

/// Presses Shift+Tab, then lets the App settle.
fn shift_tab(harness: &mut Harness<'static, App>) {
    send(harness, [key(Modifiers::SHIFT, Key::Tab)]);
}

/// Types `text` into whatever has the keyboard, then lets the App settle.
fn type_text(harness: &mut Harness<'static, App>, text: &str) {
    send(harness, [Event::Text(text.to_owned())]);
}

/// The label of the button with keyboard focus, if a button has it.
fn focused_button(harness: &Harness<'static, App>) -> Option<String> {
    harness
        .query_all_by_role(Role::Button)
        .find(|node| node.accesskit_node().is_focused())
        .map(|node| node.accesskit_node().label().unwrap_or_default())
}

/// Fresh Market's Clr and Flag cells while its row shows it renamed to Corner Shop.
const CORNER_SHOP_CLR: &str = "Clr Corner Shop, 9/5/2026, -86.42";
const CORNER_SHOP_FLAG: &str = "Flag Corner Shop, 9/5/2026, -86.42";

/// Renames Fresh Market to Corner Shop in its Payee cell, then Tabs on to its Deposit
/// cell, the last cell of its row that can be edited.
fn change_fresh_market_and_tab_to_deposit(harness: &mut Harness<'static, App>) {
    click_cell(harness, "Fresh Market");
    type_text(harness, "Corner Shop");
    // Category, Memo, Payment, then Deposit.
    for _ in 0..4 {
        press(harness, Key::Tab);
    }
}

#[test]
fn tab_from_the_editing_rows_deposit_goes_to_clr_then_flag_then_save() {
    let mut harness = everyday_checking();
    change_fresh_market_and_tab_to_deposit(&mut harness);

    let mut reached = Vec::new();
    for _ in 0..2 {
        press(&mut harness, Key::Tab);
        reached.push(cursor(&harness));
    }
    press(&mut harness, Key::Tab);
    reached.push(focused_button(&harness).unwrap_or_default());

    assert_eq!(reached, [CORNER_SHOP_CLR, CORNER_SHOP_FLAG, "Save"]);
}

#[test]
fn shift_tab_from_save_goes_back_through_flag_and_clr_to_deposit() {
    let mut harness = everyday_checking();
    change_fresh_market_and_tab_to_deposit(&mut harness);
    // Clr, Flag, then Save.
    for _ in 0..3 {
        press(&mut harness, Key::Tab);
    }

    let mut reached = Vec::new();
    for _ in 0..2 {
        shift_tab(&mut harness);
        reached.push(cursor(&harness));
    }
    shift_tab(&mut harness);

    assert_eq!(reached, [CORNER_SHOP_FLAG, CORNER_SHOP_CLR]);
    assert!(
        harness
            .query_by_role_and_label(Role::TextInput, "Deposit")
            .is_some()
    );
}

#[test]
fn tab_from_an_unchanged_rows_flag_goes_to_split_with_the_buttons_shown_all_the_way() {
    let mut harness = everyday_checking();
    // Fresh Market's Deposit cell, opened.
    click_cell(&mut harness, "86.42");
    press(&mut harness, Key::ArrowRight);
    press(&mut harness, Key::Enter);

    let mut buttons_shown = Vec::new();
    for _ in 0..2 {
        press(&mut harness, Key::Tab);
        buttons_shown.push(
            harness
                .query_by_role_and_label(Role::Button, "Split")
                .is_some(),
        );
    }
    press(&mut harness, Key::Tab);

    assert_eq!(buttons_shown, [true, true]);
    assert_eq!(focused_button(&harness).as_deref(), Some("Split"));
}

#[test]
fn space_on_a_changed_rows_clr_and_flag_acts_at_once_and_keeps_its_changes() {
    let mut harness = everyday_checking();
    change_fresh_market_and_tab_to_deposit(&mut harness);
    press(&mut harness, Key::Tab);

    press_space(&mut harness);
    press(&mut harness, Key::Tab);
    press_space(&mut harness);

    assert!(
        harness
            .query_by_role_and_label(Role::Button, "Continue editing")
            .is_none()
    );
    assert!(
        harness
            .query_by_role_and_label(Role::Cell, "Corner Shop")
            .is_some()
    );
    assert_eq!(status(&harness, CORNER_SHOP_CLR), "Cleared");
    assert!(flagged(&harness, CORNER_SHOP_FLAG));
    assert_eq!(cursor(&harness), CORNER_SHOP_FLAG);
}

#[test]
fn escape_on_a_clr_cell_tab_reached_ends_the_unchanged_rows_editing() {
    let mut harness = everyday_checking();
    // Fresh Market's Deposit cell, opened, then Tab to its Clr cell.
    click_cell(&mut harness, "86.42");
    press(&mut harness, Key::ArrowRight);
    press(&mut harness, Key::Enter);
    press(&mut harness, Key::Tab);

    press(&mut harness, Key::Escape);

    assert!(
        harness
            .query_by_role_and_label(Role::Button, "Split")
            .is_none()
    );
    assert_eq!(cursor(&harness), FRESH_MARKET_CLR);
}

#[test]
fn ctrl_enter_on_a_clr_cell_tab_reached_saves_the_row_and_ends_its_editing() {
    let (_dir, mut harness) = everyday_checking_with_enter_moving_off();
    change_fresh_market_and_tab_to_deposit(&mut harness);
    press(&mut harness, Key::Tab);

    send(&mut harness, [key(Modifiers::COMMAND, Key::Enter)]);

    assert_eq!(cursor(&harness), CORNER_SHOP_CLR);
    assert!(
        harness
            .query_by_role_and_label(Role::Button, "Split")
            .is_none()
    );
}

#[test]
fn save_pressed_after_tabbing_past_flag_ends_the_rows_editing() {
    let mut harness = everyday_checking();
    change_fresh_market_and_tab_to_deposit(&mut harness);
    // Clr, Flag, then Save.
    for _ in 0..3 {
        press(&mut harness, Key::Tab);
    }

    press(&mut harness, Key::Enter);

    assert!(
        harness
            .query_by_role_and_label(Role::Button, "Split")
            .is_none()
    );
}

#[test]
fn shift_tab_from_a_changed_row_onto_the_previous_rows_flag_asks_then_rests_there() {
    let mut harness = everyday_checking();
    click_cell(&mut harness, "Fresh Market");
    type_text(&mut harness, "Corner Shop");
    // Payee, Num, Date, then the row before's Flag cell.
    for _ in 0..3 {
        shift_tab(&mut harness);
    }
    let asked = harness
        .query_by_role_and_label(Role::Button, "Continue editing")
        .is_some();
    let buttons_while_asking = harness
        .query_by_role_and_label(Role::Button, "Split")
        .is_some();

    harness
        .get_by_role_and_label(Role::Button, "Discard")
        .click();
    harness.run();

    assert!(asked);
    assert!(!buttons_while_asking);
    assert!(cursor(&harness).starts_with("Flag "));
    assert!(cursor(&harness) != FRESH_MARKET_FLAG);
    assert!(
        harness
            .query_by_role_and_label(Role::Button, "Split")
            .is_some()
    );
}

/// The title of the column heading over the cell under the cell cursor.
fn cursor_column(harness: &Harness<'static, App>) -> String {
    let mut cursors = harness.query_all(By::default()).filter(|node| {
        let node = node.accesskit_node();
        matches!(node.role(), Role::Cell | Role::Button | Role::CheckBox)
            && node.is_selected() == Some(true)
    });
    let x = cursors
        .next()
        .expect("a cell holds the cell cursor")
        .rect()
        .center()
        .x;
    harness
        .query_all_by_role(Role::ColumnHeader)
        .find(|heading| heading.rect().x_range().contains(x))
        .expect("a heading sits over every cell")
        .accesskit_node()
        .label()
        .unwrap_or_default()
}

#[test]
fn the_right_arrow_from_the_draft_rows_deposit_passes_over_its_clr_and_flag() {
    // The cell cursor starts on the draft row's Date cell.
    let mut harness = everyday_checking();
    // Number, Payee, Category, Memo, Payment, Deposit.
    for _ in 0..6 {
        press(&mut harness, Key::ArrowRight);
    }

    press(&mut harness, Key::ArrowRight);

    assert_eq!(cursor_column(&harness), "Deposit");
}

#[test]
fn the_down_arrow_from_the_last_rows_clr_lands_on_the_draft_rows_deposit() {
    let mut harness = everyday_checking();
    // The November rent's Payment, the last transaction's, then its Deposit and Clr.
    click_cell(&mut harness, "1,475.00");
    press(&mut harness, Key::ArrowRight);
    press(&mut harness, Key::ArrowRight);
    let on_clr = cursor(&harness);

    press(&mut harness, Key::ArrowDown);

    assert!(on_clr.starts_with("Clr "));
    assert_eq!(cursor(&harness), "");
    assert_eq!(cursor_column(&harness), "Deposit");
}

#[test]
fn a_click_on_the_draft_rows_clr_cell_from_a_clr_cell_rests_the_cursor_on_its_deposit() {
    let mut harness = everyday_checking();
    arrow_to_fresh_market_clr(&mut harness);
    let clr = harness
        .get_by_role_and_label(Role::ColumnHeader, "Clr")
        .rect()
        .x_range();
    // The draft row's Clr cell, the lowest under the heading.
    let draft_clr = harness
        .query_all_by_role(Role::Cell)
        .filter(|cell| clr.contains(cell.rect().center().x))
        .max_by(|a, b| a.rect().top().total_cmp(&b.rect().top()))
        .expect("the draft row has a Clr cell");
    draft_clr.click();
    harness.run();

    assert_eq!(cursor(&harness), "");
    assert_eq!(cursor_column(&harness), "Deposit");
}

#[test]
fn the_down_arrow_from_a_leftmost_clr_lands_on_the_draft_rows_nearest_cell_to_the_right() {
    let dir = tempfile::tempdir().expect("the test machine can make a temporary directory");
    std::fs::write(
        dir.path().join("preferences.toml"),
        "version = 2\n\
         register_columns = [\"clr\", \"flag\", \"num\", \"date\", \"payee\", \"category\", \"memo\", \"payment\", \"deposit\", \"balance\"]\n",
    )
    .expect("the test can write the preferences file");
    let mut harness = everyday_checking_with(PreferencesFile::in_dir(dir.path()));
    // The November rent's Payment, the last transaction's, then left past Memo,
    // Category, Payee, Date, Num and Flag to Clr.
    click_cell(&mut harness, "1,475.00");
    for _ in 0..7 {
        press(&mut harness, Key::ArrowLeft);
    }
    let on_clr = cursor(&harness);

    press(&mut harness, Key::ArrowDown);

    assert!(on_clr.starts_with("Clr "));
    assert_eq!(cursor_column(&harness), "Num");
}
