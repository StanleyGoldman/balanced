//! The Register, driven through the AccessKit tree the way a user drives it.

use balanced_core::Ledger;
use balanced_i18n::Localizer;
use balanced_ui::{App, Clock};
use eframe::App as _;
use eframe::egui::accesskit::{Action, Role, SortDirection, Toggled};
use eframe::egui::{CursorIcon, Event, Key, Modifiers, PointerButton, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable as _};
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

/// The label of the cell under the cell cursor.
fn cursor(harness: &Harness<'static, App>) -> String {
    let mut cursors = harness
        .query_all_by_role(Role::Cell)
        .filter(|cell| cell.accesskit_node().is_selected() == Some(true));
    let cursor = cursors.next().expect("a cell holds the cell cursor");
    assert!(cursors.next().is_none(), "one cell holds the cell cursor");
    cursor.accesskit_node().label().unwrap_or_default()
}

/// The labels of every Register cell, top row first, each row left to right.
fn cells(harness: &Harness<'static, App>) -> Vec<String> {
    harness
        .query_all_by_role(Role::Cell)
        .map(|cell| cell.accesskit_node().label().unwrap_or_default())
        .collect()
}

/// The labels of the Register cells under the header `title`, top row first.
fn column(harness: &Harness<'static, App>, title: &str) -> Vec<String> {
    let header = harness
        .get_by_role_and_label(Role::ColumnHeader, title)
        .rect()
        .x_range();
    harness
        .query_all_by_role(Role::Cell)
        .filter(|cell| header.contains(cell.rect().center().x))
        .map(|cell| cell.accesskit_node().label().unwrap_or_default())
        .collect()
}

/// Clicks the Register header `title`.
fn sort_by(harness: &mut Harness<'static, App>, title: &str) {
    harness
        .get_by_role_and_label(Role::ColumnHeader, title)
        .click();
    harness.run();
}

#[test]
fn a_transaction_shows_each_column_with_its_running_balance() {
    let mut harness = app();

    open(&mut harness, "Everyday Checking");

    let cells = cells(&harness);
    let payee = cells
        .iter()
        .position(|cell| cell == "Fresh Market")
        .expect("the Register lists Fresh Market");
    // Date, Num, Payee, Category, Memo, Amount, Balance.
    assert_eq!(
        cells[payee - 2..payee + 5],
        [
            "9/5/2026",
            "",
            "Fresh Market",
            "Food & Dining:Groceries",
            "",
            "-86.42",
            "2,863.58",
        ]
    );
}

#[test]
fn a_split_transaction_shows_split_in_its_category() {
    let mut harness = app();

    open(&mut harness, "Everyday Checking");

    let cells = cells(&harness);
    let payee = cells
        .iter()
        .position(|cell| cell == "Costco")
        .expect("the Register lists Costco");
    assert_eq!(
        cells[payee..payee + 4],
        ["Costco", "[Split]", "", "-206.27"]
    );
}

#[test]
fn the_opening_balance_is_the_first_transaction() {
    let mut harness = app();

    open(&mut harness, "Everyday Checking");

    let cells = cells(&harness);
    assert_eq!(
        cells[..7],
        [
            "1/1/2026",
            "",
            "Opening Balance",
            "",
            "",
            "1,200.00",
            "1,200.00",
        ]
    );
}

#[test]
fn clicking_a_transaction_selects_its_row() {
    let mut harness = app();
    open(&mut harness, "Everyday Checking");

    harness.get_by_role_and_label(Role::Cell, "Costco").click();
    harness.run();

    let costco = harness.get_by_role_and_label(Role::Cell, "Costco");
    let fresh_market = harness.get_by_role_and_label(Role::Cell, "Fresh Market");
    assert_eq!(costco.accesskit_node().toggled(), Some(Toggled::True));
    assert_eq!(
        fresh_market.accesskit_node().toggled(),
        Some(Toggled::False)
    );
}

#[test]
fn the_selected_row_shows_its_actions_menu() {
    let mut harness = app();
    open(&mut harness, "Everyday Checking");
    let menus_before = harness
        .query_all_by_role_and_label(Role::Button, "Actions")
        .count();

    harness.get_by_role_and_label(Role::Cell, "Costco").click();
    harness.run();

    let menus_after = harness
        .query_all_by_role_and_label(Role::Button, "Actions")
        .count();
    assert_eq!((menus_before, menus_after), (0, 1));
}

#[test]
fn an_open_account_offers_a_new_transaction() {
    let mut harness = app();

    open(&mut harness, "Everyday Checking");

    let add = harness.get_by_role_and_label(Role::Button, "+ Transaction");
    assert!(!add.accesskit_node().is_disabled());
    assert!(harness.query_by_label("Closed — read-only").is_none());
}

#[test]
fn a_closed_account_is_read_only() {
    let mut harness = app();
    open(&mut harness, "Closed");

    open(&mut harness, "Old Checking");

    let add = harness.get_by_role_and_label(Role::Button, "+ Transaction");
    assert!(add.accesskit_node().is_disabled());
    harness.get_by_label("Closed — read-only");
}

#[test]
fn the_hovered_row_shows_its_actions_menu() {
    let mut harness = app();
    open(&mut harness, "Everyday Checking");

    harness.get_by_role_and_label(Role::Cell, "Costco").hover();
    harness.run();

    let menus = harness
        .query_all_by_role_and_label(Role::Button, "Actions")
        .count();
    assert_eq!(menus, 1);
}

#[test]
fn only_the_rows_in_view_of_a_large_register_are_laid_out() {
    let mut harness = app();

    open(&mut harness, "Household Checking");

    // The account has 20,001 transactions, of eight cells each; the window shows a few
    // dozen rows.
    let rows = harness.query_all_by_role(Role::Cell).count() / 8;
    assert!((1..=50).contains(&rows), "{rows} rows laid out");
}

#[test]
fn opening_another_account_clears_the_selection() {
    let mut harness = app();
    open(&mut harness, "Everyday Checking");
    harness.get_by_role_and_label(Role::Cell, "Costco").click();
    harness.run();

    open(&mut harness, "Joint Checking");
    open(&mut harness, "Everyday Checking");

    let costco = harness.get_by_role_and_label(Role::Cell, "Costco");
    assert_eq!(costco.accesskit_node().toggled(), Some(Toggled::False));
}

#[test]
fn clicking_payee_sorts_by_payee() {
    let mut harness = app();
    open(&mut harness, "Everyday Checking");

    sort_by(&mut harness, "Payee");

    assert_eq!(
        column(&harness, "Payee"),
        [
            "Acme Corp",
            "Acme Corp",
            "Bean There",
            "City Power & Light",
            "Costco",
            "Fresh Market",
            "Maple Property Management",
            "Opening Balance",
            "Shell",
            "Trattoria Roma",
            // The draft row stays at the bottom under any sort.
            "",
        ]
    );
}

#[test]
fn sorting_by_payee_blanks_the_balance() {
    let mut harness = app();
    open(&mut harness, "Everyday Checking");

    sort_by(&mut harness, "Payee");

    // Ten transactions, then the draft row, which has no balance.
    assert_eq!(column(&harness, "Balance"), [""; 11]);
}

#[test]
fn opening_another_account_sorts_it_by_date() {
    let mut harness = app();
    open(&mut harness, "Everyday Checking");
    sort_by(&mut harness, "Payee");
    open(&mut harness, "Closed");

    open(&mut harness, "Old Checking");

    assert_eq!(
        column(&harness, "Payee"),
        ["Opening Balance", "First Savings Bank", "Cash withdrawal"]
    );
}

#[test]
fn clicking_payee_again_reverses_the_sort_but_not_its_ties() {
    let mut harness = app();
    open(&mut harness, "Everyday Checking");
    sort_by(&mut harness, "Payee");

    sort_by(&mut harness, "Payee");

    let payees = column(&harness, "Payee");
    let dates = column(&harness, "Date");
    assert_eq!(
        payees,
        [
            "Trattoria Roma",
            "Shell",
            "Opening Balance",
            "Maple Property Management",
            "Fresh Market",
            "Costco",
            "City Power & Light",
            "Bean There",
            "Acme Corp",
            "Acme Corp",
            // The draft row stays at the bottom whichever way the sort runs.
            "",
        ]
    );
    // The two Acme Corp salaries tie, so they stay oldest first.
    assert_eq!(dates[8..10], ["9/1/2026", "9/18/2026"]);
}

#[test]
fn sorting_by_date_again_brings_the_balances_back() {
    let mut harness = app();
    open(&mut harness, "Everyday Checking");
    sort_by(&mut harness, "Payee");

    sort_by(&mut harness, "Date");

    assert_eq!(
        column(&harness, "Balance"),
        [
            "1,200.00", "4,400.00", "2,950.00", "2,863.58", "2,858.83", "2,766.73", "2,560.46",
            "2,512.26", "5,712.26", "5,647.46", // The draft row, which has no balance.
            "",
        ]
    );
}

#[test]
fn a_register_opens_sorted_by_date_with_every_column_but_number_and_balance_sortable() {
    let mut harness = app();

    open(&mut harness, "Everyday Checking");

    let header = |title| harness.get_by_role_and_label(Role::ColumnHeader, title);
    let sorts = |title| {
        let node = header(title).accesskit_node();
        (
            node.data().supports_action(Action::Click),
            node.sort_direction(),
        )
    };
    assert_eq!(sorts("Date"), (true, Some(SortDirection::Ascending)));
    for title in ["Payee", "Category", "Memo", "Amount"] {
        assert_eq!(sorts(title), (true, None), "{title}");
    }
    for title in ["Num", "Balance"] {
        assert_eq!(sorts(title), (false, None), "{title}");
    }
}

#[test]
fn a_sortable_header_shows_a_pointing_hand() {
    let mut harness = app();
    open(&mut harness, "Everyday Checking");

    harness
        .get_by_role_and_label(Role::ColumnHeader, "Payee")
        .hover();
    harness.run();

    assert_eq!(
        harness.output().platform_output.cursor_icon,
        CursorIcon::PointingHand
    );
}

#[test]
fn the_selected_row_stays_centred_after_a_sort() {
    let mut harness = app();
    open(&mut harness, "Household Checking");
    // The account opens on its draft row, below the last of its 20,001 rows.
    for _ in 0..20 {
        press(&mut harness, Modifiers::NONE, Key::ArrowUp);
    }
    let payee = harness
        .get_by_role_and_label(Role::ColumnHeader, "Payee")
        .rect()
        .x_range();
    let selected_payee = harness
        .query_all_by_role(Role::Cell)
        .find(|cell| {
            payee.contains(cell.rect().center().x)
                && cell.accesskit_node().toggled() == Some(Toggled::True)
        })
        .and_then(|cell| cell.accesskit_node().label())
        .expect("a row is selected");

    // Thousands of rows share each payee, so the selected row sorts far from the end.
    sort_by(&mut harness, "Payee");

    let header = harness
        .get_by_role_and_label(Role::ColumnHeader, "Payee")
        .rect();
    let bottom = harness
        .query_all_by_role(Role::Cell)
        .map(|cell| cell.rect().bottom())
        .fold(header.bottom(), f32::max);
    let middle = header.bottom().midpoint(bottom);
    let selected = harness
        .query_all_by_role(Role::Cell)
        .find(|cell| {
            cell.accesskit_node().toggled() == Some(Toggled::True)
                && cell.accesskit_node().label().as_deref() == Some(selected_payee.as_str())
        })
        .map(|cell| cell.rect().center().y);
    assert!(
        selected.is_some_and(|y| (y - middle).abs() <= header.height()),
        "the selected row is at {selected:?}, the middle at {middle}"
    );
}

#[test]
fn selecting_a_row_after_a_sort_leaves_the_rows_still() {
    let mut harness = app();
    open(&mut harness, "Household Checking");
    sort_by(&mut harness, "Payee");
    // A row in the lower half of the Register, which centring would move up.
    let header = harness
        .get_by_role_and_label(Role::ColumnHeader, "Payee")
        .rect()
        .x_range();
    let payees: Vec<_> = harness
        .query_all_by_role(Role::Cell)
        .filter(|cell| header.contains(cell.rect().center().x))
        .collect();
    let payee = &payees[payees.len() * 3 / 4];
    let clicked = payee.rect();

    payee.click();
    harness.run();

    let selected = harness
        .query_all_by_role(Role::Cell)
        .find(|cell| cell.accesskit_node().toggled() == Some(Toggled::True))
        .map(|cell| cell.rect().center().y);
    assert_eq!(selected, Some(clicked.center().y));
}

#[test]
fn enter_opens_the_cell_under_the_cursor_with_its_saved_text() {
    let mut harness = app();
    open(&mut harness, "Everyday Checking");
    click_cell(&mut harness, "Fresh Market");

    press(&mut harness, Modifiers::NONE, Key::Enter);

    assert_eq!(editor(&harness, "Payee").as_deref(), Some("Fresh Market"));
}

#[test]
fn typing_on_a_cell_starts_editing_it_with_the_typed_text() {
    let mut harness = app();
    open(&mut harness, "Everyday Checking");
    click_cell(&mut harness, "Fresh Market");

    type_text(&mut harness, "C");

    assert_eq!(editor(&harness, "Payee").as_deref(), Some("C"));
}

#[test]
fn enter_commits_and_moves_right_without_opening_the_next_cell() {
    let mut harness = app();
    open(&mut harness, "Everyday Checking");
    click_cell(&mut harness, "Fresh Market");
    press(&mut harness, Modifiers::NONE, Key::Enter);

    type_text(&mut harness, " Downtown");
    press(&mut harness, Modifiers::NONE, Key::Enter);

    harness.get_by_role_and_label(Role::Cell, "Fresh Market Downtown");
    assert!(harness.query_by_role(Role::TextInput).is_none());
    assert_eq!(cursor(&harness), "Food & Dining:Groceries");
}

#[test]
fn enter_on_the_last_cell_of_a_row_wraps_to_the_next_row() {
    let mut harness = app();
    open(&mut harness, "Everyday Checking");
    click_cell(&mut harness, "-86.42");
    press(&mut harness, Modifiers::NONE, Key::Enter);

    press(&mut harness, Modifiers::NONE, Key::Enter);

    // Bean There's date, the row after Fresh Market's.
    assert_eq!(cursor(&harness), "9/6/2026");
}

#[test]
fn tab_commits_and_edits_the_next_editable_cell() {
    let mut harness = app();
    open(&mut harness, "Everyday Checking");
    click_cell(&mut harness, "Fresh Market");
    type_text(&mut harness, "Corner Shop");

    press(&mut harness, Modifiers::NONE, Key::Tab);

    harness.get_by_role_and_label(Role::Cell, "Corner Shop");
    assert_eq!(
        editor(&harness, "Category").as_deref(),
        Some("Food & Dining:Groceries")
    );
    assert_eq!(editor(&harness, "Payee"), None);
}

#[test]
fn shift_tab_commits_and_edits_the_previous_editable_cell() {
    let mut harness = app();
    open(&mut harness, "Everyday Checking");
    click_cell(&mut harness, "September rent");
    press(&mut harness, Modifiers::NONE, Key::Enter);

    press(&mut harness, Modifiers::SHIFT, Key::Tab);

    assert_eq!(editor(&harness, "Category").as_deref(), Some("Home"));
    assert_eq!(editor(&harness, "Memo"), None);
}

#[test]
fn escape_cancels_the_edit_and_restores_the_saved_text() {
    let mut harness = app();
    open(&mut harness, "Everyday Checking");
    click_cell(&mut harness, "Fresh Market");
    press(&mut harness, Modifiers::NONE, Key::Enter);
    type_text(&mut harness, " Downtown");

    press(&mut harness, Modifiers::NONE, Key::Escape);

    assert!(harness.query_by_role(Role::TextInput).is_none());
    assert_eq!(cursor(&harness), "Fresh Market");
}

#[test]
fn clicking_another_cell_commits_the_edit() {
    let mut harness = app();
    open(&mut harness, "Everyday Checking");
    click_cell(&mut harness, "Fresh Market");
    type_text(&mut harness, "Corner Shop");

    click_cell(&mut harness, "Costco");

    harness.get_by_role_and_label(Role::Cell, "Corner Shop");
    assert!(harness.query_by_role(Role::TextInput).is_none());
}

#[test]
fn the_arrow_keys_move_the_cell_cursor() {
    let mut harness = app();
    open(&mut harness, "Everyday Checking");
    click_cell(&mut harness, "Fresh Market");

    press(&mut harness, Modifiers::NONE, Key::ArrowDown);
    let down = cursor(&harness);
    press(&mut harness, Modifiers::NONE, Key::ArrowRight);
    let right = cursor(&harness);
    press(&mut harness, Modifiers::NONE, Key::ArrowUp);
    let up = cursor(&harness);
    press(&mut harness, Modifiers::NONE, Key::ArrowLeft);
    let left = cursor(&harness);

    assert_eq!(
        [down, right, up, left],
        [
            "Bean There",
            "Food & Dining:Restaurants:Coffee Shops",
            "Food & Dining:Groceries",
            "Fresh Market",
        ]
    );
}

#[test]
fn the_cursor_row_is_the_selected_row() {
    let mut harness = app();
    open(&mut harness, "Everyday Checking");
    click_cell(&mut harness, "Fresh Market");

    press(&mut harness, Modifiers::NONE, Key::ArrowDown);

    let bean_there = harness.get_by_role_and_label(Role::Cell, "Bean There");
    let fresh_market = harness.get_by_role_and_label(Role::Cell, "Fresh Market");
    assert_eq!(bean_there.accesskit_node().toggled(), Some(Toggled::True));
    assert_eq!(
        fresh_market.accesskit_node().toggled(),
        Some(Toggled::False)
    );
}

#[test]
fn a_closed_accounts_cells_do_not_open_for_editing() {
    let mut harness = app();
    open(&mut harness, "Closed");
    open(&mut harness, "Old Checking");
    click_cell(&mut harness, "Closing the account");

    type_text(&mut harness, "x");

    assert!(harness.query_by_role(Role::TextInput).is_none());
    harness.get_by_label("This account is closed, so its transactions can't be changed.");
}

#[test]
fn moving_the_cursor_above_the_view_scrolls_to_it_in_the_same_frame() {
    let mut harness = app();
    // The account opens on its draft row, below the last of its 20,001 rows.
    open(&mut harness, "Household Checking");
    let header = harness.get_by_role_and_label(Role::ColumnHeader, "Payee");
    let top = header.accesskit_node().bounding_box().unwrap().y1;

    for _ in 0..60 {
        send(
            &mut harness,
            Event::Key {
                key: Key::ArrowUp,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::NONE,
            },
        );
        let cell = harness
            .query_all_by_role(Role::Cell)
            .find(|cell| cell.accesskit_node().is_selected() == Some(true))
            .expect("the cursor's row is laid out in the frame the cursor moves");
        let cell_top = cell.accesskit_node().bounding_box().unwrap().y0;
        assert!(cell_top >= top - 8.0, "{cell_top} is above {top}");
    }
}

#[test]
fn an_edit_starts_with_the_text_cursor_at_the_end() {
    let mut harness = app();
    open(&mut harness, "Everyday Checking");
    click_cell(&mut harness, "Fresh Market");
    press(&mut harness, Modifiers::NONE, Key::Enter);
    press(&mut harness, Modifiers::NONE, Key::Home);
    press(&mut harness, Modifiers::NONE, Key::Escape);

    press(&mut harness, Modifiers::NONE, Key::Enter);
    type_text(&mut harness, "!");

    assert_eq!(editor(&harness, "Payee").as_deref(), Some("Fresh Market!"));
}

#[test]
fn tab_on_the_last_transactions_last_cell_commits_and_edits_the_draft_rows_date() {
    let mut harness = app();
    open(&mut harness, "Everyday Checking");
    // Trattoria Roma's amount, the last transaction's last cell.
    click_cell(&mut harness, "-64.80");
    type_text(&mut harness, "-70");

    press(&mut harness, Modifiers::NONE, Key::Tab);

    assert_eq!(amount_of(&harness, "Trattoria Roma"), "-70.00");
    assert_eq!(editor(&harness, "Date").as_deref(), Some(""));
}

#[test]
fn tab_on_the_draft_rows_last_cell_commits_and_closes_it() {
    let mut harness = app();
    open(&mut harness, "Everyday Checking");
    for _ in 0..5 {
        press(&mut harness, Modifiers::NONE, Key::ArrowRight);
    }
    type_text(&mut harness, "-70");

    press(&mut harness, Modifiers::NONE, Key::Tab);

    assert_eq!(cells(&harness).last().map(String::as_str), Some(""));
    assert!(
        cells(&harness)
            .iter()
            .rev()
            .take(8)
            .any(|cell| cell == "-70.00")
    );
    assert!(harness.query_by_role(Role::TextInput).is_none());
}

#[test]
fn the_register_leaves_ctrl_tab_to_the_rest_of_the_app() {
    let mut harness = app();
    open(&mut harness, "Everyday Checking");
    click_cell(&mut harness, "Fresh Market");
    let ctrl_tab = Event::Key {
        key: Key::Tab,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Modifiers::COMMAND,
    };
    let mut input = eframe::egui::RawInput {
        events: vec![ctrl_tab.clone()],
        ..Default::default()
    };

    let ctx = harness.ctx.clone();
    harness.state_mut().raw_input_hook(&ctx, &mut input);

    assert_eq!(input.events, [ctrl_tab]);
}

/// Renames Fresh Market to Corner Shop through its Payee cell.
fn rename_fresh_market(harness: &mut Harness<'static, App>) {
    click_cell(harness, "Fresh Market");
    type_text(harness, "Corner Shop");
    press(harness, Modifiers::NONE, Key::Enter);
}

#[test]
fn ctrl_z_undoes_a_cell_edit() {
    let mut harness = app();
    open(&mut harness, "Everyday Checking");
    rename_fresh_market(&mut harness);

    press(&mut harness, Modifiers::COMMAND, Key::Z);

    harness.get_by_role_and_label(Role::Cell, "Fresh Market");
}

#[test]
fn ctrl_y_redoes_an_undone_cell_edit() {
    let mut harness = app();
    open(&mut harness, "Everyday Checking");
    rename_fresh_market(&mut harness);
    press(&mut harness, Modifiers::COMMAND, Key::Z);

    press(&mut harness, Modifiers::COMMAND, Key::Y);

    harness.get_by_role_and_label(Role::Cell, "Corner Shop");
}

#[test]
fn ctrl_shift_z_redoes_an_undone_cell_edit() {
    let mut harness = app();
    open(&mut harness, "Everyday Checking");
    rename_fresh_market(&mut harness);
    press(&mut harness, Modifiers::COMMAND, Key::Z);

    press(&mut harness, Modifiers::COMMAND | Modifiers::SHIFT, Key::Z);

    harness.get_by_role_and_label(Role::Cell, "Corner Shop");
}

#[test]
fn ctrl_z_during_a_cell_edit_undoes_typing_not_the_ledger() {
    let mut harness = app();
    open(&mut harness, "Everyday Checking");
    rename_fresh_market(&mut harness);
    click_cell(&mut harness, "Corner Shop");
    press(&mut harness, Modifiers::NONE, Key::Enter);
    type_text(&mut harness, " Downtown");

    press(&mut harness, Modifiers::COMMAND, Key::Z);

    assert_eq!(editor(&harness, "Payee").as_deref(), Some("Corner Shop"));
    press(&mut harness, Modifiers::NONE, Key::Escape);
    harness.get_by_role_and_label(Role::Cell, "Corner Shop");
}

/// Whether the Edit menu's Undo and Redo items are disabled, opening the menu to
/// look and closing it again.
fn undo_redo_disabled(harness: &mut Harness<'static, App>) -> (bool, bool) {
    harness.get_by_role_and_label(Role::Button, "Edit").click();
    harness.run();
    let disabled = |label| {
        harness
            .get_by_role_and_label(Role::Button, label)
            .accesskit_node()
            .is_disabled()
    };
    let items = (disabled("Undo Ctrl+Z"), disabled("Redo Ctrl+Y"));
    harness.get_by_role_and_label(Role::Button, "Edit").click();
    harness.run();
    items
}

#[test]
fn the_edit_menu_enables_undo_and_redo_only_when_they_can_act() {
    let mut harness = app();
    open(&mut harness, "Everyday Checking");
    assert_eq!(undo_redo_disabled(&mut harness), (true, true));

    rename_fresh_market(&mut harness);
    assert_eq!(undo_redo_disabled(&mut harness), (false, true));

    harness.get_by_role_and_label(Role::Button, "Edit").click();
    harness.run();
    harness
        .get_by_role_and_label(Role::Button, "Undo Ctrl+Z")
        .click();
    harness.run();
    harness.get_by_role_and_label(Role::Cell, "Fresh Market");
    assert_eq!(undo_redo_disabled(&mut harness), (true, false));
}

#[test]
fn typing_in_a_category_lists_matches_anywhere_and_enter_takes_the_top_one() {
    let mut harness = app();
    open(&mut harness, "Everyday Checking");
    click_cell(&mut harness, "Auto & Transport:Fuel");
    type_text(&mut harness, "roc");

    press(&mut harness, Modifiers::NONE, Key::Enter);

    let payee = column(&harness, "Payee")
        .iter()
        .position(|payee| payee == "Shell")
        .expect("the Register lists Shell");
    assert_eq!(
        column(&harness, "Category")[payee],
        "Food & Dining:Groceries"
    );
}

#[test]
fn up_past_the_top_match_then_enter_keeps_the_typed_payee() {
    let mut harness = app();
    open(&mut harness, "Everyday Checking");
    click_cell(&mut harness, "Shell");
    type_text(&mut harness, "Fresh");
    press(&mut harness, Modifiers::NONE, Key::ArrowUp);

    press(&mut harness, Modifiers::NONE, Key::Enter);

    let payees = column(&harness, "Payee");
    assert_eq!(payees.iter().filter(|payee| *payee == "Fresh").count(), 1);
    assert_eq!(
        payees
            .iter()
            .filter(|payee| *payee == "Fresh Market")
            .count(),
        1
    );
}

/// The labels of the rows of the open combobox list, top first.
fn list_rows(harness: &Harness<'static, App>) -> Vec<String> {
    harness
        .query_all_by_role(Role::ListBoxOption)
        .map(|row| row.accesskit_node().label().unwrap_or_default())
        .collect()
}

#[test]
fn the_first_esc_closes_the_list_and_the_second_ends_the_edit() {
    let mut harness = app();
    open(&mut harness, "Everyday Checking");
    click_cell(&mut harness, "Auto & Transport:Fuel");
    type_text(&mut harness, "roc");

    press(&mut harness, Modifiers::NONE, Key::Escape);
    let after_first = (editor(&harness, "Category"), list_rows(&harness));
    press(&mut harness, Modifiers::NONE, Key::Escape);

    assert_eq!(after_first, (Some("roc".to_owned()), Vec::new()));
    assert_eq!(editor(&harness, "Category"), None);
    harness.get_by_role_and_label(Role::Cell, "Auto & Transport:Fuel");
}

/// The label of the highlighted row of the open combobox list, if any.
fn highlighted(harness: &Harness<'static, App>) -> Option<String> {
    harness
        .query_all_by_role(Role::ListBoxOption)
        .find(|row| row.accesskit_node().is_selected() == Some(true))
        .map(|row| row.accesskit_node().label().unwrap_or_default())
}

#[test]
fn typing_an_unknown_category_lists_its_matches_then_a_create_row() {
    let mut harness = app();
    open(&mut harness, "Everyday Checking");
    click_cell(&mut harness, "Auto & Transport:Fuel");

    type_text(&mut harness, "insur");

    assert_eq!(
        list_rows(&harness),
        [
            "Auto & Transport:Insurance",
            "Home:Insurance",
            "Create \"insur\"…",
        ]
    );
    assert_eq!(
        highlighted(&harness).as_deref(),
        Some("Auto & Transport:Insurance")
    );
}

#[test]
fn a_list_row_is_taken_on_the_press_before_any_release() {
    let mut harness = app();
    open(&mut harness, "Everyday Checking");
    click_cell(&mut harness, "Auto & Transport:Fuel");
    type_text(&mut harness, "insur");
    let home = harness
        .get_by_role_and_label(Role::ListBoxOption, "Home:Insurance")
        .rect()
        .center();

    harness.event(Event::PointerMoved(home));
    harness.event(Event::PointerButton {
        pos: home,
        button: PointerButton::Primary,
        pressed: true,
        modifiers: Modifiers::NONE,
    });
    harness.run();

    harness.get_by_role_and_label(Role::Cell, "Home:Insurance");
    assert_eq!(editor(&harness, "Category"), None);
}

#[test]
fn the_list_button_shows_only_while_a_category_is_edited() {
    let mut harness = app();
    open(&mut harness, "Everyday Checking");
    click_cell(&mut harness, "Auto & Transport:Fuel");
    let before = harness
        .query_by_role_and_label(Role::Button, "Show all categories")
        .is_some();

    press(&mut harness, Modifiers::NONE, Key::Enter);

    harness.get_by_role_and_label(Role::Button, "Show all categories");
    assert!(!before);
}

#[test]
fn the_list_button_lists_every_category_and_keeps_the_edit() {
    let mut harness = app();
    open(&mut harness, "Everyday Checking");
    click_cell(&mut harness, "Auto & Transport:Fuel");
    press(&mut harness, Modifiers::NONE, Key::Enter);

    harness
        .get_by_role_and_label(Role::Button, "Show all categories")
        .click();
    harness.run();

    // The list scrolls, so only its top rows are laid out.
    assert_eq!(
        list_rows(&harness)[..4],
        [
            "Auto & Transport",
            "Auto & Transport:Fuel",
            "Auto & Transport:Insurance",
            "Food & Dining",
        ]
    );
    assert_eq!(
        editor(&harness, "Category").as_deref(),
        Some("Auto & Transport:Fuel")
    );
}

#[test]
fn alt_down_lists_every_category() {
    let mut harness = app();
    open(&mut harness, "Everyday Checking");
    click_cell(&mut harness, "Auto & Transport:Fuel");
    type_text(&mut harness, "roc");

    press(&mut harness, Modifiers::ALT, Key::ArrowDown);

    // The list scrolls, so only its top rows are laid out.
    assert_eq!(
        list_rows(&harness)[..4],
        [
            "Auto & Transport",
            "Auto & Transport:Fuel",
            "Auto & Transport:Insurance",
            "Food & Dining",
        ]
    );
}

#[test]
fn clicking_split_says_the_split_view_is_not_available_yet() {
    let mut harness = app();
    open(&mut harness, "Everyday Checking");

    click_cell(&mut harness, "[Split]");

    harness.get_by_label("Not available yet");
}

#[test]
fn enter_on_split_says_the_split_view_is_not_available_yet() {
    let mut harness = app();
    open(&mut harness, "Everyday Checking");
    click_cell(&mut harness, "Costco");
    press(&mut harness, Modifiers::NONE, Key::ArrowRight);

    press(&mut harness, Modifiers::NONE, Key::Enter);

    harness.get_by_label("Not available yet");
}

#[test]
fn enter_on_an_unknown_category_without_a_highlight_keeps_editing_and_says_why() {
    let mut harness = app();
    open(&mut harness, "Everyday Checking");
    click_cell(&mut harness, "Auto & Transport:Fuel");
    type_text(&mut harness, "Pets");
    press(&mut harness, Modifiers::NONE, Key::ArrowUp);

    press(&mut harness, Modifiers::NONE, Key::Enter);

    assert_eq!(editor(&harness, "Category").as_deref(), Some("Pets"));
    harness.get_by_label("No category has that name. Pick one from the list, or create it.");
}

#[test]
fn typing_a_payee_lists_every_accounts_matching_payees_alphabetically() {
    let mut harness = app();
    open(&mut harness, "Everyday Checking");
    click_cell(&mut harness, "Shell");

    type_text(&mut harness, "ar");

    assert_eq!(list_rows(&harness), ["Fresh Market", "Hardware Barn"]);
}

#[test]
fn listing_every_category_highlights_the_cells_own() {
    let mut harness = app();
    open(&mut harness, "Everyday Checking");
    click_cell(&mut harness, "Auto & Transport:Fuel");
    press(&mut harness, Modifiers::NONE, Key::Enter);

    press(&mut harness, Modifiers::ALT, Key::ArrowDown);

    assert_eq!(
        highlighted(&harness).as_deref(),
        Some("Auto & Transport:Fuel")
    );
}

#[test]
fn moving_the_highlight_past_the_visible_rows_scrolls_the_list() {
    let mut harness = app();
    open(&mut harness, "Everyday Checking");
    click_cell(&mut harness, "Auto & Transport:Fuel");
    press(&mut harness, Modifiers::NONE, Key::Enter);
    press(&mut harness, Modifiers::ALT, Key::ArrowDown);

    for _ in 0..14 {
        press(&mut harness, Modifiers::NONE, Key::ArrowDown);
    }

    assert_eq!(highlighted(&harness).as_deref(), Some("Shopping:Household"));
}

/// The Amount cell of the row whose Payee is `payee`.
fn amount_of(harness: &Harness<'static, App>, payee: &str) -> String {
    let row = column(harness, "Payee")
        .iter()
        .position(|cell| cell == payee)
        .expect("the Register lists the payee");
    column(harness, "Amount").swap_remove(row)
}

#[test]
fn typed_arithmetic_commits_its_result_rounded_to_the_currency_decimal_places() {
    let mut harness = app();
    open(&mut harness, "Everyday Checking");
    click_cell(&mut harness, "-86.42");
    type_text(&mut harness, "100/3");

    press(&mut harness, Modifiers::NONE, Key::Enter);

    assert_eq!(amount_of(&harness, "Fresh Market"), "33.33");
}

#[test]
fn an_amount_with_excess_decimal_places_stays_in_edit_mode() {
    let mut harness = app();
    open(&mut harness, "Everyday Checking");
    click_cell(&mut harness, "-86.42");
    type_text(&mut harness, "12.345");

    press(&mut harness, Modifiers::NONE, Key::Enter);

    assert_eq!(editor(&harness, "Amount").as_deref(), Some("12.345"));
    assert_eq!(amount_of(&harness, "Fresh Market"), "-86.42");
}

#[test]
fn an_amount_with_excess_decimal_places_shows_why_in_a_bubble() {
    let mut harness = app();
    open(&mut harness, "Everyday Checking");
    click_cell(&mut harness, "-86.42");
    type_text(&mut harness, "12.345");

    press(&mut harness, Modifiers::NONE, Key::Enter);

    let bubble = harness.query_by_label("USD amounts have 2 decimal places");
    assert!(bubble.is_some(), "no error bubble");
}

#[test]
fn an_amount_written_with_the_wrong_marks_names_the_locale_marks() {
    let mut harness = app();
    open(&mut harness, "Everyday Checking");
    click_cell(&mut harness, "-86.42");
    type_text(&mut harness, "1,5");

    press(&mut harness, Modifiers::NONE, Key::Enter);

    let message = "Write amounts with “.” as the decimal mark and “,” between thousands.";
    assert!(harness.query_by_label(message).is_some(), "no error bubble");
}

#[test]
fn leaving_a_refused_amount_keeps_the_cell_in_edit_mode() {
    let mut harness = app();
    open(&mut harness, "Everyday Checking");
    click_cell(&mut harness, "-86.42");
    type_text(&mut harness, "12.345");

    click_cell(&mut harness, "Bean There");
    type_text(&mut harness, "6");

    assert_eq!(editor(&harness, "Amount").as_deref(), Some("12.3456"));
    assert_eq!(cursor(&harness), "-86.42");
}

#[test]
fn an_amount_field_refuses_letters() {
    let mut harness = app();
    open(&mut harness, "Everyday Checking");
    click_cell(&mut harness, "-86.42");
    press(&mut harness, Modifiers::NONE, Key::Enter);

    type_text(&mut harness, "1a+b2");

    assert_eq!(editor(&harness, "Amount").as_deref(), Some("-86.421+2"));
}

#[test]
fn typing_a_letter_on_an_amount_cell_opens_nothing() {
    let mut harness = app();
    open(&mut harness, "Everyday Checking");
    click_cell(&mut harness, "-86.42");

    type_text(&mut harness, "x");

    assert_eq!(editor(&harness, "Amount"), None);
}

#[test]
fn a_pasted_amount_loses_its_letters() {
    let mut harness = app();
    open(&mut harness, "Everyday Checking");
    click_cell(&mut harness, "-86.42");
    press(&mut harness, Modifiers::NONE, Key::Enter);
    for _ in "-86.42".chars() {
        press(&mut harness, Modifiers::NONE, Key::Backspace);
    }

    send(&mut harness, Event::Paste("USD 12.50".to_owned()));
    harness.run();

    assert_eq!(editor(&harness, "Amount").as_deref(), Some(" 12.50"));
}

#[test]
fn typed_arithmetic_previews_its_rounded_result() {
    let mut harness = app();
    open(&mut harness, "Everyday Checking");
    click_cell(&mut harness, "-86.42");

    type_text(&mut harness, "100/3");

    assert!(harness.query_by_label("~ $33.33").is_some(), "no preview");
}

#[test]
fn typed_arithmetic_with_an_exact_result_previews_it_unmarked() {
    let mut harness = app();
    open(&mut harness, "Everyday Checking");
    click_cell(&mut harness, "-86.42");

    type_text(&mut harness, "10+5");

    assert!(harness.query_by_label("= $15.00").is_some(), "no preview");
}

#[test]
fn a_plain_amount_has_no_preview() {
    let mut harness = app();
    open(&mut harness, "Everyday Checking");
    click_cell(&mut harness, "-86.42");

    type_text(&mut harness, "12");

    assert!(harness.query_by_label("= $12.00").is_none(), "a preview");
}

#[test]
fn changing_a_refused_amount_clears_its_error() {
    let mut harness = app();
    open(&mut harness, "Everyday Checking");
    click_cell(&mut harness, "-86.42");
    type_text(&mut harness, "12.345");
    press(&mut harness, Modifiers::NONE, Key::Enter);

    press(&mut harness, Modifiers::NONE, Key::Backspace);

    let bubble = harness.query_by_label("USD amounts have 2 decimal places");
    assert!(bubble.is_none(), "the error bubble stayed");
}

#[test]
fn a_reformatted_amount_reopens_with_the_text_cursor_at_the_end() {
    let mut harness = app();
    open(&mut harness, "Everyday Checking");
    click_cell(&mut harness, "-86.42");
    type_text(&mut harness, "100/3");
    press(&mut harness, Modifiers::NONE, Key::Enter);
    click_cell(&mut harness, "33.33");

    press(&mut harness, Modifiers::NONE, Key::Enter);
    type_text(&mut harness, "1");

    assert_eq!(editor(&harness, "Amount").as_deref(), Some("33.331"));
}

#[test]
fn a_split_transactions_amount_does_not_open_for_editing() {
    let mut harness = app();
    open(&mut harness, "Everyday Checking");
    // Costco's amount, the sum of its split lines.
    click_cell(&mut harness, "-206.27");

    press(&mut harness, Modifiers::NONE, Key::Enter);

    assert_eq!(editor(&harness, "Amount"), None);
}
