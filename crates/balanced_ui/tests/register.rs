//! The Register, driven through the AccessKit tree the way a user drives it.

use balanced_core::Ledger;
use balanced_i18n::Localizer;
use balanced_ui::{App, Clock};
use eframe::App as _;
use eframe::egui::accesskit::{Action, Role, SortDirection, Toggled};
use eframe::egui::{CursorIcon, Event, Key, Modifiers, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable as _};

/// The App on stub data, in the test locale, wide enough to show every Register column.
fn app() -> Harness<'static, App> {
    Harness::builder()
        .with_size(vec2(1280.0, 720.0))
        .build_eframe(|cc| App::new(cc, Localizer::for_tests(), Ledger::stub(), Clock::default()))
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
        ]
    );
}

#[test]
fn sorting_by_payee_blanks_the_balance() {
    let mut harness = app();
    open(&mut harness, "Everyday Checking");

    sort_by(&mut harness, "Payee");

    assert_eq!(column(&harness, "Balance"), [""; 10]);
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
        ]
    );
    // The two Acme Corp salaries tie, so they stay oldest first.
    assert_eq!(dates[8..], ["9/1/2026", "9/18/2026"]);
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
            "2,512.26", "5,712.26", "5,647.46",
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
    harness
        .get_by_role_and_label(Role::Cell, "Opening Balance")
        .click();
    harness.run();

    // Of 20,001 rows, the opening balance sorts thousands of rows down by payee.
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
        .query_by_role_and_label(Role::Cell, "Opening Balance")
        .map(|cell| cell.rect().center().y);
    assert!(
        selected.is_some_and(|y| (y - middle).abs() <= header.height()),
        "the selected row is at {selected:?}, the middle at {middle}"
    );
}

#[test]
fn selecting_a_row_after_a_sort_with_nothing_selected_leaves_the_rows_still() {
    let mut harness = app();
    open(&mut harness, "Household Checking");
    sort_by(&mut harness, "Payee");
    // A row in the lower half of the Register, which centring would move up.
    let payees: Vec<_> = harness
        .query_all_by_role(Role::Cell)
        .filter(|cell| cell.accesskit_node().label().as_deref() == Some("Acme Corp"))
        .collect();
    let acme = &payees[payees.len() * 3 / 4];
    let clicked = acme.rect();

    acme.click();
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
    // Category isn't edited as text, so Tab passes it.
    assert_eq!(editor(&harness, "Memo").as_deref(), Some(""));
    assert_eq!(editor(&harness, "Payee"), None);
}

#[test]
fn shift_tab_commits_and_edits_the_previous_editable_cell() {
    let mut harness = app();
    open(&mut harness, "Everyday Checking");
    click_cell(&mut harness, "September rent");
    press(&mut harness, Modifiers::NONE, Key::Enter);

    press(&mut harness, Modifiers::SHIFT, Key::Tab);

    assert_eq!(
        editor(&harness, "Payee").as_deref(),
        Some("Maple Property Management")
    );
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
fn moving_the_cursor_below_the_view_scrolls_to_it_in_the_same_frame() {
    let mut harness = app();
    open(&mut harness, "Household Checking");
    click_cell(&mut harness, "Opening Balance");
    let net_worth = harness.get_by_role_and_label(Role::Label, "Net Worth");
    // The Account bar's footer ends where the Register's body does.
    let bottom = net_worth.accesskit_node().bounding_box().unwrap().y1;

    for _ in 0..60 {
        send(
            &mut harness,
            Event::Key {
                key: Key::ArrowDown,
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
        let cell_bottom = cell.accesskit_node().bounding_box().unwrap().y1;
        assert!(
            cell_bottom <= bottom + 8.0,
            "{cell_bottom} is below {bottom}"
        );
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
fn tab_on_the_last_editable_cell_commits_and_closes_it() {
    let mut harness = app();
    open(&mut harness, "Everyday Checking");
    click_cell(&mut harness, "Anniversary dinner");
    press(&mut harness, Modifiers::NONE, Key::Enter);
    type_text(&mut harness, " out");

    press(&mut harness, Modifiers::NONE, Key::Tab);

    harness.get_by_role_and_label(Role::Cell, "Anniversary dinner out");
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
