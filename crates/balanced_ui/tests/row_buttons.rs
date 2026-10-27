// SPDX-License-Identifier: GPL-3.0-or-later WITH AdditionRef-Balanced-Plugin-Exception

//! The Save, Cancel, Split and More actions buttons in the editing row's Actions cell.

use balanced_core::Ledger;
use balanced_i18n::Localizer;
use balanced_ui::{App, Clock, PreferencesFile};
use eframe::App as _;
use eframe::egui::accesskit::Role;
use eframe::egui::{Event, Key, Modifiers, Rect, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable as _};
use icu_calendar::Date;
use icu_locale::{langid, locale};

/// The App on stub data, in the test locale, wide enough to show every Register column.
fn app() -> Harness<'static, App> {
    app_in(Localizer::for_tests(), 1440.0)
}

/// The App on stub data, in `i18n`'s locale, `width` points wide.
fn app_in(i18n: Localizer, width: f32) -> Harness<'static, App> {
    let mut harness = Harness::builder()
        .with_size(vec2(width, 720.0))
        .build_eframe(|cc| {
            App::on_ledger(
                cc,
                i18n,
                Ledger::stub(),
                Clock::fixed(Date::try_new_iso(2026, 10, 14).expect("14 October 2026 is a date")),
                PreferencesFile::for_tests(),
            )
        });
    harness.run();
    harness
}

/// Opens the account named `name` from the Account bar.
fn open(harness: &mut Harness<'static, App>, name: &str) {
    harness.get_by_role_and_label(Role::Button, name).click();
    harness.run();
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

/// Puts the cell cursor on the cell labelled `label` by clicking it.
fn click_cell(harness: &mut Harness<'static, App>, label: &str) {
    harness.get_by_role_and_label(Role::Cell, label).click();
    harness.run();
}

/// The text in the open cell editor labelled `column`, or `None` when no such editor
/// is open.
fn editor(harness: &Harness<'static, App>, column: &str) -> Option<String> {
    harness
        .query_by_role_and_label(Role::TextInput, column)
        .map(|node| node.accesskit_node().value().unwrap_or_default())
}

/// The label of the button with keyboard focus, if a button has it.
fn focused_button(harness: &Harness<'static, App>) -> Option<String> {
    harness
        .query_all_by_role(Role::Button)
        .find(|node| node.accesskit_node().is_focused())
        .map(|node| node.accesskit_node().label().unwrap_or_default())
}

/// Opens Fresh Market's Deposit cell, the last cell of its row that can be edited.
fn open_last_cell(harness: &mut Harness<'static, App>) {
    click_cell(harness, "86.42");
    press(harness, Modifiers::NONE, Key::ArrowRight);
    press(harness, Modifiers::NONE, Key::Enter);
}

/// Whether a cell editor is open: a text field other than the filter bar's search box.
fn editing(harness: &Harness<'static, App>) -> bool {
    harness
        .query_all_by_role(Role::TextInput)
        .any(|node| node.accesskit_node().label().as_deref() != Some("Search"))
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

/// Whether the cell cursor is on the row of the cell labelled `label`.
fn cursor_on_row_of(harness: &Harness<'static, App>, label: &str) -> bool {
    let row = harness
        .get_by_role_and_label(Role::Cell, label)
        .rect()
        .center()
        .y;
    harness
        .query_all_by_role(Role::Cell)
        .filter(|cell| cell.accesskit_node().is_selected() == Some(true))
        .any(|cell| cell.rect().y_range().contains(row))
}

/// Whether a label shows `text`, as the Status bar and Dialogs show messages.
fn shows_text(harness: &Harness<'static, App>, text: &str) -> bool {
    harness
        .query_all_by_role(Role::Label)
        .any(|label| label.accesskit_node().value().as_deref() == Some(text))
}

/// Whether a cell is labelled `label`.
fn shows(harness: &Harness<'static, App>, label: &str) -> bool {
    harness.query_by_role_and_label(Role::Cell, label).is_some()
}

/// Whether the Edit menu's Undo item is enabled, opening the menu to look and closing
/// it again.
fn can_undo(harness: &mut Harness<'static, App>) -> bool {
    harness.get_by_role_and_label(Role::Button, "Edit").click();
    harness.run();
    let enabled = !harness
        .get_by_role_and_label(Role::Button, "Undo Ctrl+Z")
        .accesskit_node()
        .is_disabled();
    harness.get_by_role_and_label(Role::Button, "Edit").click();
    harness.run();
    enabled
}

/// Clicks the button labelled `label`.
fn click_button(harness: &mut Harness<'static, App>, label: &str) {
    harness.get_by_role_and_label(Role::Button, label).click();
    harness.run();
}

/// Renames Fresh Market to Corner Shop in its Payee cell, leaving the cell open.
fn rename_fresh_market(harness: &mut Harness<'static, App>) {
    click_cell(harness, "Fresh Market");
    type_text(harness, "Corner Shop");
}

/// How many buttons are labelled each of `labels`.
fn button_counts<const N: usize>(harness: &Harness<'static, App>, labels: [&str; N]) -> [usize; N] {
    labels.map(|label| {
        harness
            .query_all_by_role_and_label(Role::Button, label)
            .count()
    })
}

/// The editing row's buttons, the buttons in its toolbar.
fn row_buttons(harness: &Harness<'static, App>) -> Vec<(String, Rect)> {
    let buttons: Vec<_> = harness
        .query_all_by_role(Role::Button)
        .filter(|button| {
            button
                .accesskit_node()
                .parent()
                .is_some_and(|parent| parent.role() == Role::Toolbar)
        })
        .map(|button| {
            let label = button.accesskit_node().label().unwrap_or_default();
            (label, button.rect())
        })
        .collect();
    assert_eq!(buttons.len(), 4, "the editing row shows its four buttons");
    buttons
}

/// The labels of the editing row's buttons that reach outside the Actions column, the
/// rightmost column, whose heading is blank.
fn buttons_outside_actions(harness: &Harness<'static, App>) -> Vec<String> {
    let actions = harness
        .query_all_by_role(Role::ColumnHeader)
        .max_by(|a, b| a.rect().center().x.total_cmp(&b.rect().center().x))
        .expect("the Register has headings")
        .rect()
        .x_range();
    row_buttons(harness)
        .into_iter()
        .filter(|(_, rect)| !(actions.contains(rect.left()) && actions.contains(rect.right())))
        .map(|(label, _)| label)
        .collect()
}

/// The labels of the cells, and of the editors over them, that any of the editing
/// row's buttons overlaps.
fn cells_under_buttons(harness: &Harness<'static, App>) -> Vec<String> {
    let buttons = row_buttons(harness);
    let under = |rect: Rect| {
        buttons
            .iter()
            .any(|(_, button)| button.intersect(rect).is_positive())
    };
    [Role::Cell, Role::CheckBox, Role::TextInput]
        .into_iter()
        .flat_map(|role| harness.query_all_by_role(role).collect::<Vec<_>>())
        .chain(harness.query_all_by_role(Role::Button).filter(|button| {
            button
                .accesskit_node()
                .parent()
                .is_none_or(|parent| parent.role() != Role::Toolbar)
        }))
        // The Actions cell, which holds the buttons, is blank.
        .filter(|node| !node.accesskit_node().label().unwrap_or_default().is_empty())
        .filter(|node| under(node.rect()))
        .map(|node| node.accesskit_node().label().unwrap_or_default())
        .collect()
}

/// Opens Fresh Market's Payee cell in Everyday Checking, which makes its row the
/// editing row.
fn edit_fresh_market(harness: &mut Harness<'static, App>) {
    open(harness, "Everyday Checking");
    click_cell(harness, "Fresh Market");
    press(harness, Modifiers::NONE, Key::Enter);
}

#[test]
fn the_editing_rows_buttons_lie_in_the_actions_column_over_no_cell() {
    let mut harness = app();

    edit_fresh_market(&mut harness);
    assert_eq!(cells_under_buttons(&harness), Vec::<String>::new());

    assert_eq!(buttons_outside_actions(&harness), Vec::<String>::new());
}

#[test]
fn in_german_the_editing_rows_buttons_lie_in_the_actions_column_over_no_cell() {
    let german = Localizer::for_tests_in(&langid!("de"), &locale!("de-DE"))
        .expect("the embedded de messages and de-DE formatting data load");
    let mut harness = app_in(german, 1440.0);

    edit_fresh_market(&mut harness);
    assert_eq!(cells_under_buttons(&harness), Vec::<String>::new());

    assert_eq!(buttons_outside_actions(&harness), Vec::<String>::new());
}

/// The Register's column headings, left to right. The Actions column's is empty.
fn headings(harness: &Harness<'static, App>) -> Vec<String> {
    let mut headers: Vec<_> = harness
        .query_all_by_role(Role::ColumnHeader)
        .map(|header| {
            let label = header.accesskit_node().label().unwrap_or_default();
            (header.rect().left(), label)
        })
        .collect();
    headers.sort_by(|(left, _), (right, _)| left.total_cmp(right));
    headers.into_iter().map(|(_, label)| label).collect()
}

/// Opens Everyday Checking, makes `change` in its column chooser, then opens Fresh
/// Market's Payee cell, which makes its row the editing row.
fn edit_fresh_market_after(
    harness: &mut Harness<'static, App>,
    change: impl FnOnce(&mut Harness<'static, App>),
) {
    open(harness, "Everyday Checking");
    click_button(harness, "Choose columns");
    change(harness);
    press(harness, Modifiers::NONE, Key::Escape);
    click_cell(harness, "Fresh Market");
    press(harness, Modifiers::NONE, Key::Enter);
}

#[test]
fn with_balance_clr_or_flag_hidden_the_editing_rows_buttons_cover_no_cell() {
    for column in ["Balance", "Clr", "Flag"] {
        let mut harness = app();

        edit_fresh_market_after(&mut harness, |harness| {
            harness
                .get_by_role_and_label(Role::CheckBox, column)
                .click();
            harness.run();
        });

        assert!(!headings(&harness).iter().any(|heading| heading == column));
        assert_eq!(
            cells_under_buttons(&harness),
            Vec::<String>::new(),
            "{column} hidden"
        );
        assert_eq!(
            buttons_outside_actions(&harness),
            Vec::<String>::new(),
            "{column} hidden"
        );
    }
}

#[test]
fn with_memo_payment_or_deposit_beside_actions_the_editing_rows_buttons_cover_no_cell() {
    for column in ["Memo", "Payment", "Deposit"] {
        let mut harness = app();

        edit_fresh_market_after(&mut harness, |harness| {
            // Memo has the furthest to go: past Payment, Deposit, Balance, Clr and Flag.
            for _ in 0..5 {
                click_button(harness, &format!("Move {column} down"));
            }
        });

        let headings = headings(&harness);
        assert_eq!(
            headings[headings.len() - 2],
            column,
            "{column} sits beside Actions"
        );
        assert_eq!(
            cells_under_buttons(&harness),
            Vec::<String>::new(),
            "{column} beside"
        );
        assert_eq!(
            buttons_outside_actions(&harness),
            Vec::<String>::new(),
            "{column} beside"
        );
    }
}

#[test]
fn in_a_narrow_window_the_register_scrolls_sideways_and_the_buttons_cover_no_cell() {
    let mut harness = app_in(Localizer::for_tests(), 900.0);

    edit_fresh_market(&mut harness);

    let right = harness
        .query_all_by_role(Role::ColumnHeader)
        .map(|header| header.rect().right())
        .fold(f32::NEG_INFINITY, f32::max);
    assert!(
        right > 900.0,
        "the Register runs past the window's right edge"
    );
    assert_eq!(cells_under_buttons(&harness), Vec::<String>::new());
    assert_eq!(buttons_outside_actions(&harness), Vec::<String>::new());
}

#[test]
fn the_editing_row_shows_save_cancel_split_and_more_in_place_of_its_actions_menu() {
    let mut harness = app();
    open(&mut harness, "Everyday Checking");
    click_cell(&mut harness, "Fresh Market");

    press(&mut harness, Modifiers::NONE, Key::Enter);

    let shown = button_counts(
        &harness,
        ["Save", "Add", "Cancel", "Split", "More actions", "Actions"],
    );
    assert_eq!(shown, [1, 0, 1, 1, 1, 0]);
}

#[test]
fn the_draft_row_shows_add_in_place_of_save() {
    let mut harness = app();
    open(&mut harness, "Everyday Checking");

    // The cell cursor starts on the draft row's Date cell, and typing opens it.
    type_text(&mut harness, "1");

    let shown = button_counts(&harness, ["Save", "Add", "Cancel", "Split", "More actions"]);
    assert_eq!(shown, [0, 1, 1, 1, 1]);
}

#[test]
fn a_highlighted_row_keeps_its_actions_menu() {
    let mut harness = app();
    open(&mut harness, "Everyday Checking");

    click_cell(&mut harness, "Fresh Market");

    let shown = button_counts(
        &harness,
        ["Save", "Add", "Cancel", "More actions", "Actions"],
    );
    assert_eq!(shown, [0, 0, 0, 0, 1]);
}

#[test]
fn the_actions_column_heading_is_blank() {
    let mut harness = app();
    open(&mut harness, "Everyday Checking");

    let rightmost = harness
        .query_all_by_role(Role::ColumnHeader)
        .max_by(|a, b| a.rect().center().x.total_cmp(&b.rect().center().x))
        .expect("the Register has headings");

    // The Actions column is the last column.
    assert_eq!(rightmost.accesskit_node().label().unwrap_or_default(), "");
}

#[test]
fn tab_from_the_last_editable_cell_of_a_changed_row_moves_through_save_cancel_split_and_more() {
    let mut harness = app();
    open(&mut harness, "Everyday Checking");
    rename_fresh_market(&mut harness);
    // Payee, Category, Memo, Payment, then Deposit.
    for _ in 0..4 {
        press(&mut harness, Modifiers::NONE, Key::Tab);
    }

    let mut reached = Vec::new();
    for _ in 0..4 {
        press(&mut harness, Modifiers::NONE, Key::Tab);
        reached.push(focused_button(&harness).unwrap_or_default());
    }

    assert_eq!(reached, ["Save", "Cancel", "Split", "More actions"]);
    assert!(!editing(&harness));
}

#[test]
fn tab_from_the_last_editable_cell_of_an_unchanged_row_moves_through_split_and_more() {
    let mut harness = app();
    open(&mut harness, "Everyday Checking");
    open_last_cell(&mut harness);

    let mut reached = Vec::new();
    for _ in 0..2 {
        press(&mut harness, Modifiers::NONE, Key::Tab);
        reached.push(focused_button(&harness).unwrap_or_default());
    }

    assert_eq!(reached, ["Split", "More actions"]);
    assert!(!editing(&harness));
}

#[test]
fn shift_tab_from_save_goes_back_to_the_last_editable_cell() {
    let mut harness = app();
    open(&mut harness, "Everyday Checking");
    rename_fresh_market(&mut harness);
    // Payee, Category, Memo, Payment, Deposit, then Save and Cancel.
    for _ in 0..6 {
        press(&mut harness, Modifiers::NONE, Key::Tab);
    }

    press(&mut harness, Modifiers::SHIFT, Key::Tab);
    let back_one = focused_button(&harness);
    press(&mut harness, Modifiers::SHIFT, Key::Tab);

    assert_eq!(back_one.as_deref(), Some("Save"));
    assert_eq!(editor(&harness, "Deposit").as_deref(), Some(""));
}

#[test]
fn shift_tab_from_split_of_an_unchanged_row_goes_back_to_the_last_editable_cell() {
    let mut harness = app();
    open(&mut harness, "Everyday Checking");
    open_last_cell(&mut harness);
    press(&mut harness, Modifiers::NONE, Key::Tab);

    press(&mut harness, Modifiers::SHIFT, Key::Tab);

    assert_eq!(editor(&harness, "Deposit").as_deref(), Some(""));
}

#[test]
fn tab_from_more_goes_round_to_the_rows_first_cell() {
    let mut harness = app();
    open(&mut harness, "Everyday Checking");
    open_last_cell(&mut harness);
    // Split, then More.
    for _ in 0..2 {
        press(&mut harness, Modifiers::NONE, Key::Tab);
    }

    press(&mut harness, Modifiers::NONE, Key::Tab);

    assert_eq!(editor(&harness, "Date").as_deref(), Some("9/5/2026"));
}

#[test]
fn clicking_save_saves_the_row_and_keeps_the_cell_cursor_on_it() {
    let mut harness = app();
    open(&mut harness, "Everyday Checking");
    rename_fresh_market(&mut harness);

    click_button(&mut harness, "Save");

    assert!(!editing(&harness));
    assert_eq!(cursor(&harness), "Corner Shop");
    assert!(can_undo(&mut harness));
}

/// Renames Fresh Market, Tabs from its Payee cell to Save, and presses `key` there.
fn save_from_the_keyboard(key: Key) -> Harness<'static, App> {
    let mut harness = app();
    open(&mut harness, "Everyday Checking");
    rename_fresh_market(&mut harness);
    // Payee, Category, Memo, Payment, Deposit, then Save.
    for _ in 0..5 {
        press(&mut harness, Modifiers::NONE, Key::Tab);
    }

    press(&mut harness, Modifiers::NONE, key);
    harness
}

#[test]
fn enter_on_save_saves_the_row_and_keeps_the_cell_cursor_on_it() {
    let mut harness = save_from_the_keyboard(Key::Enter);

    assert!(!editing(&harness));
    assert!(cursor_on_row_of(&harness, "Corner Shop"));
    assert!(can_undo(&mut harness));
}

#[test]
fn space_on_save_saves_the_row_and_keeps_the_cell_cursor_on_it() {
    let mut harness = save_from_the_keyboard(Key::Space);

    assert!(!editing(&harness));
    assert!(cursor_on_row_of(&harness, "Corner Shop"));
    assert!(can_undo(&mut harness));
}

#[test]
fn clicking_add_adds_the_draft_row_and_opens_a_fresh_one() {
    let mut harness = app();
    open(&mut harness, "Everyday Checking");
    for c in "10/15/2026".chars() {
        type_text(&mut harness, &c.to_string());
    }
    // Date, Num, Payee.
    press(&mut harness, Modifiers::NONE, Key::Tab);
    press(&mut harness, Modifiers::NONE, Key::Tab);
    type_text(&mut harness, "Corner Cafe");
    press(&mut harness, Modifiers::NONE, Key::Tab);
    type_text(&mut harness, "Food & Dining:Groceries");

    click_button(&mut harness, "Add");

    assert!(shows(&harness, "Corner Cafe"));
    assert_eq!(cursor(&harness), "");
    assert_eq!(button_counts(&harness, ["Add"]), [0]);
}

#[test]
fn clicking_cancel_throws_away_the_rows_changes() {
    let mut harness = app();
    open(&mut harness, "Everyday Checking");
    rename_fresh_market(&mut harness);
    press(&mut harness, Modifiers::NONE, Key::Tab);

    click_button(&mut harness, "Cancel");

    assert!(!editing(&harness));
    assert!(!shows(&harness, "Corner Shop"));
    assert_eq!(cursor(&harness), "Food & Dining:Groceries");
    assert!(!can_undo(&mut harness));
}

#[test]
fn clicking_split_opens_the_split_dialog() {
    let mut harness = app();
    open(&mut harness, "Everyday Checking");
    rename_fresh_market(&mut harness);

    click_button(&mut harness, "Split");

    harness.get_by_role_and_label(Role::TextInput, "Line 1 category");
}

#[test]
fn more_holds_the_rows_actions_menu() {
    let mut harness = app();
    open(&mut harness, "Everyday Checking");
    rename_fresh_market(&mut harness);

    click_button(&mut harness, "More actions");

    assert_eq!(button_counts(&harness, ["Flag", "Delete…"]), [1, 1]);
}

#[test]
fn mores_menu_leaves_out_split_which_has_its_own_button() {
    let mut harness = app();
    open(&mut harness, "Everyday Checking");
    rename_fresh_market(&mut harness);

    click_button(&mut harness, "More actions");

    assert_eq!(button_counts(&harness, ["Split"]), [1]);
}

#[test]
fn mores_menu_stays_open_on_an_unchanged_row() {
    let mut harness = app();
    open(&mut harness, "Everyday Checking");
    click_cell(&mut harness, "Fresh Market");
    // Opens the Payee cell, unchanged.
    press(&mut harness, Modifiers::NONE, Key::Enter);

    click_button(&mut harness, "More actions");

    assert_eq!(button_counts(&harness, ["Flag", "Delete…"]), [1, 1]);
}

#[test]
fn tab_on_the_draft_row_skips_its_disabled_more_and_goes_round_to_its_date() {
    let mut harness = app();
    open(&mut harness, "Everyday Checking");
    // From the draft row's Date: Num, Payee, Category, Memo, Payment, Deposit, then
    // Split, as the empty row's Add and Cancel are disabled.
    for _ in 0..7 {
        press(&mut harness, Modifiers::NONE, Key::Tab);
    }
    let on_split = focused_button(&harness);

    press(&mut harness, Modifiers::NONE, Key::Tab);

    assert_eq!(on_split.as_deref(), Some("Split"));
    assert!(editor(&harness, "Date").is_some());
}

#[test]
fn escape_on_a_button_throws_away_the_rows_changes() {
    let mut harness = app();
    open(&mut harness, "Everyday Checking");
    rename_fresh_market(&mut harness);
    // Payee, Category, Memo, Payment, Deposit, then Save.
    for _ in 0..5 {
        press(&mut harness, Modifiers::NONE, Key::Tab);
    }

    press(&mut harness, Modifiers::NONE, Key::Escape);

    assert!(!shows(&harness, "Corner Shop"));
    assert_eq!(button_counts(&harness, ["Save"]), [0]);
}

#[test]
fn enter_on_an_item_of_mores_menu_closes_the_menu() {
    let mut harness = app();
    open(&mut harness, "Everyday Checking");
    rename_fresh_market(&mut harness);
    click_button(&mut harness, "More actions");
    harness.get_by_role_and_label(Role::Button, "Flag").focus();
    harness.run();

    press(&mut harness, Modifiers::NONE, Key::Enter);

    assert_eq!(button_counts(&harness, ["Flag", "Unflag"]), [0, 0]);
}

#[test]
fn clicking_save_on_an_amount_that_cant_be_read_says_the_row_cant_be_saved() {
    let mut harness = app();
    open(&mut harness, "Everyday Checking");
    rename_fresh_market(&mut harness);
    click_cell(&mut harness, "86.42");
    type_text(&mut harness, "12.345");

    click_button(&mut harness, "Save");

    assert!(shows_text(&harness, "Can't save this transaction"));
}

#[test]
fn cancel_on_the_draft_row_empties_it() {
    let mut harness = app();
    open(&mut harness, "Everyday Checking");
    // From the draft row's Date: Num, then Payee.
    press(&mut harness, Modifiers::NONE, Key::Tab);
    press(&mut harness, Modifiers::NONE, Key::Tab);
    type_text(&mut harness, "Corner Cafe");
    press(&mut harness, Modifiers::NONE, Key::Tab);

    click_button(&mut harness, "Cancel");

    assert!(!shows(&harness, "Corner Cafe"));
    assert!(!editing(&harness));
}

/// Whether the button labelled `label` is enabled.
fn enabled(harness: &Harness<'static, App>, label: &str) -> bool {
    !harness
        .get_by_role_and_label(Role::Button, label)
        .accesskit_node()
        .is_disabled()
}

/// Whether each of the editing row's Save, Cancel, Split and More actions buttons is
/// enabled.
fn enabled_buttons(harness: &Harness<'static, App>) -> [bool; 4] {
    ["Save", "Cancel", "Split", "More actions"].map(|label| enabled(harness, label))
}

#[test]
fn an_unchanged_editing_row_disables_save_and_cancel() {
    let mut harness = app();
    open(&mut harness, "Everyday Checking");
    click_cell(&mut harness, "Fresh Market");

    press(&mut harness, Modifiers::NONE, Key::Enter);

    assert_eq!(enabled_buttons(&harness), [false, false, true, true]);
}

#[test]
fn changing_a_value_enables_save_and_cancel() {
    let mut harness = app();
    open(&mut harness, "Everyday Checking");

    rename_fresh_market(&mut harness);

    assert_eq!(enabled_buttons(&harness), [true, true, true, true]);
}

#[test]
fn moving_through_cells_without_changing_one_leaves_save_disabled() {
    let mut harness = app();
    open(&mut harness, "Everyday Checking");
    click_cell(&mut harness, "Fresh Market");
    press(&mut harness, Modifiers::NONE, Key::Enter);

    // Payee, Category, then Memo.
    press(&mut harness, Modifiers::NONE, Key::Tab);
    press(&mut harness, Modifiers::NONE, Key::Tab);

    assert!(!enabled(&harness, "Save"));
}

#[test]
fn typing_back_the_saved_value_disables_save_again() {
    let mut harness = app();
    open(&mut harness, "Everyday Checking");
    click_cell(&mut harness, "Fresh Market");

    // Typing opens the cell holding just the typed text, the saved payee.
    type_text(&mut harness, "Fresh Market");

    assert!(!enabled(&harness, "Save"));
}

#[test]
fn an_empty_draft_row_disables_add_and_cancel() {
    let mut harness = app();
    open(&mut harness, "Everyday Checking");

    // From the draft row's Date, Tab opens Num, leaving the row empty.
    press(&mut harness, Modifiers::NONE, Key::Tab);

    assert_eq!(
        ["Add", "Cancel"].map(|label| enabled(&harness, label)),
        [false, false]
    );
}

/// Whether a cell holds the cell cursor.
fn shows_cursor(harness: &Harness<'static, App>) -> bool {
    harness
        .query_all_by_role(Role::Cell)
        .any(|cell| cell.accesskit_node().is_selected() == Some(true))
}

#[test]
fn a_focused_row_button_hides_the_cell_cursor() {
    let mut harness = app();
    open(&mut harness, "Everyday Checking");
    open_last_cell(&mut harness);

    // Tab from the unchanged row's last cell focuses Split.
    press(&mut harness, Modifiers::NONE, Key::Tab);

    assert_eq!(focused_button(&harness).as_deref(), Some("Split"));
    assert!(!shows_cursor(&harness));
}

#[test]
fn tabbing_from_the_row_buttons_back_into_a_cell_shows_the_cell_cursor_again() {
    let mut harness = app();
    open(&mut harness, "Everyday Checking");
    open_last_cell(&mut harness);
    press(&mut harness, Modifiers::NONE, Key::Tab);

    press(&mut harness, Modifiers::SHIFT, Key::Tab);

    assert_eq!(cursor(&harness), "");
}
