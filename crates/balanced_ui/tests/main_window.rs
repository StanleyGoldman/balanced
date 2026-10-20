// SPDX-License-Identifier: GPL-3.0-or-later WITH AdditionRef-Balanced-Plugin-Exception

//! The Main window, driven through the AccessKit tree the way a user drives it.

use balanced_core::Ledger;
use balanced_i18n::Localizer;
use balanced_ui::{App, Clock, PreferencesFile};
use eframe::egui::accesskit::{Role, Toggled};
use eframe::egui::{Color32, Event, Key, Modifiers, Shape, ViewportCommand, ViewportId, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable as _};
use icu_calendar::Date;

/// The App on stub data, in the test locale.
fn app() -> Harness<'static, App> {
    let mut harness = Harness::new_eframe(|cc| {
        App::on_ledger(
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

/// Queues one key press with `modifiers` held, as a single event. `step()` plays
/// each queued event in a frame of its own, so `key_press_modifiers`, which queues
/// four, runs past the frame whose output a test reads.
fn press(harness: &Harness<'static, App>, modifiers: Modifiers, key: Key) {
    harness.event(Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers,
    });
}

#[test]
fn home_is_active_at_start() {
    let harness = app();

    let home = harness.get_by_role_and_label(Role::Button, "Home");

    assert_eq!(home.accesskit_node().toggled(), Some(Toggled::True));
    harness.get_by_label("Your overview of accounts, bills and spending will appear here.");
}

#[test]
fn clicking_a_tab_shows_its_placeholder() {
    let mut harness = app();

    harness.get_by_role_and_label(Role::Button, "Bills").click();
    harness.run();

    let bills = harness.get_by_role_and_label(Role::Button, "Bills");
    assert_eq!(bills.accesskit_node().toggled(), Some(Toggled::True));
    harness.get_by_label("Upcoming bills and deposits will appear here.");
}

#[test]
fn help_about_shows_the_version_in_the_status_bar() {
    let mut harness = app();

    harness.get_by_role_and_label(Role::Button, "Help").click();
    harness.run();
    harness
        .get_by_role_and_label(Role::Button, "About Balanced")
        .click();
    harness.run();

    harness.get_by_label("Balanced 0.0.0");
}

#[test]
fn file_quit_closes_the_main_window() {
    let mut harness = app();

    harness.get_by_role_and_label(Role::Button, "File").click();
    harness.run();
    harness
        .get_by_role_and_label(Role::Button, "Quit Ctrl+Q")
        .click();
    // One step plays the click, which sends Quit; the next applies it. Only that
    // frame's output carries the viewport command.
    harness.step();
    harness.step();

    let root = &harness.output().viewport_output[&ViewportId::ROOT];
    assert_eq!(root.commands, [ViewportCommand::Close]);
}

#[test]
fn ctrl_q_closes_the_main_window() {
    let mut harness = app();

    press(&harness, Modifiers::COMMAND, Key::Q);
    // One step plays the key press, which sends Quit; the next applies it.
    harness.step();
    harness.step();

    let root = &harness.output().viewport_output[&ViewportId::ROOT];
    assert_eq!(root.commands, [ViewportCommand::Close]);
}

#[test]
fn file_quit_shows_its_shortcut() {
    let mut harness = app();

    harness.get_by_role_and_label(Role::Button, "File").click();
    harness.run();

    // egui reads a menu item's label and its Shortcut text as one label.
    harness.get_by_role_and_label(Role::Button, "Quit Ctrl+Q");
}

#[test]
fn selecting_an_account_opens_it_under_the_accounts_tab() {
    let mut harness = app();

    harness
        .get_by_role_and_label(Role::Button, "Everyday Checking")
        .click();
    harness.run();

    let accounts = harness.get_by_role_and_label(Role::Button, "Accounts");
    assert_eq!(accounts.accesskit_node().toggled(), Some(Toggled::True));
    harness.get_by_role_and_label(Role::Label, "Everyday Checking");
    harness.get_by_role_and_label(Role::Label, "Ending $4,071.81");
}

#[test]
fn selecting_another_account_opens_it_in_place_of_the_open_one() {
    let mut harness = app();
    harness
        .get_by_role_and_label(Role::Button, "Everyday Checking")
        .click();
    harness.run();

    harness
        .get_by_role_and_label(Role::Button, "Emergency Fund")
        .click();
    harness.run();

    harness.get_by_role_and_label(Role::Label, "Emergency Fund");
    assert!(
        harness
            .query_by_role_and_label(Role::Label, "Everyday Checking")
            .is_none()
    );
}

#[test]
fn folding_a_group_hides_its_accounts() {
    let mut harness = app();

    harness
        .get_by_role_and_label(Role::Button, "Banking")
        .click();
    harness.run();

    let banking = harness.get_by_role_and_label(Role::Button, "Banking");
    assert_eq!(banking.accesskit_node().toggled(), Some(Toggled::False));
    assert!(
        harness
            .query_by_role_and_label(Role::Button, "Everyday Checking")
            .is_none()
    );
}

#[test]
fn closed_accounts_start_folded_and_unfold() {
    let mut harness = app();
    assert!(
        harness
            .query_by_role_and_label(Role::Button, "Old Checking")
            .is_none()
    );

    harness
        .get_by_role_and_label(Role::Button, "Closed")
        .click();
    harness.run();

    harness.get_by_role_and_label(Role::Button, "Old Checking");
}

#[test]
fn hidden_accounts_are_left_out_but_count_in_net_worth() {
    let harness = app();

    let holiday_fund = harness.query_by_role_and_label(Role::Button, "Holiday Fund");

    assert!(holiday_fund.is_none());
    // The stub's Net Worth with the hidden Holiday Fund's $800.00 in it.
    harness.get_by_role_and_label(Role::Label, "$286,922.06");
}

/// The text of every `Label` on screen, which holds a tooltip's text as its value.
/// A tooltip shows among the window's other Labels, so tests check it is among them.
fn label_values(harness: &Harness<'static, App>) -> Vec<String> {
    harness
        .query_all_by_role(Role::Label)
        .filter_map(|label| label.accesskit_node().value())
        .collect()
}

#[test]
fn hovering_an_account_whose_name_is_cut_short_shows_the_whole_name() {
    let mut harness = app();

    // "Household Checking" doesn't fit beside its balance in the default width.
    harness
        .get_by_role_and_label(Role::Button, "Household Checking")
        .hover();
    harness.run();

    assert!(
        label_values(&harness).contains(&"Household Checking".to_owned()),
        "{:?}",
        label_values(&harness)
    );
}

#[test]
fn hovering_an_account_whose_name_fits_shows_no_tooltip() {
    let mut harness = app();

    harness.get_by_role_and_label(Role::Button, "Car").hover();
    harness.run();

    assert!(!label_values(&harness).contains(&"Car".to_owned()));
}

#[test]
fn add_account_sits_below_net_worth() {
    let harness = app();

    let add_account = harness.get_by_role_and_label(Role::Button, "+ Add Account");
    let net_worth = harness.get_by_role_and_label(Role::Label, "$286,922.06");

    assert!(add_account.rect().top() >= net_worth.rect().bottom());
}

#[test]
fn the_account_bar_header_holds_only_the_fold_button() {
    let harness = app();

    let banking = harness
        .get_by_role_and_label(Role::Button, "Banking")
        .rect();

    // The header is the strip of the Account bar above its first heading, under the tabs.
    let tabs_bottom = harness
        .get_by_role_and_label(Role::Button, "Home")
        .rect()
        .bottom();
    let header: Vec<_> = harness
        .query_all_by_role(Role::Button)
        .filter(|button| {
            let rect = button.rect();
            rect.top() >= tabs_bottom
                && rect.bottom() <= banking.top()
                && rect.right() <= banking.right()
        })
        .map(|button| button.accesskit_node().label().unwrap_or_default())
        .collect();
    assert_eq!(header, ["Fold the Account bar"]);
}

/// Opens `account` from the Account bar.
fn open_account(harness: &mut Harness<'static, App>, account: &str) {
    harness.get_by_role_and_label(Role::Button, account).click();
    harness.run();
}

#[test]
fn the_context_header_shows_the_current_balance_without_future_rows_and_the_ending_with_them() {
    let mut harness = app();

    open_account(&mut harness, "Everyday Checking");

    // Today is 14 October 2026; the payments of 20 October and 1 November count only
    // in Ending.
    let values = label_values(&harness);
    assert!(
        values.contains(&"Current $5,642.21".to_owned()),
        "{values:?}"
    );
    assert!(
        values.contains(&"Ending $4,071.81".to_owned()),
        "{values:?}"
    );
}

#[test]
fn an_account_with_no_future_rows_shows_both_balances_equal() {
    let mut harness = app();

    open_account(&mut harness, "Emergency Fund");

    let values = label_values(&harness);
    assert!(
        values.contains(&"Current $12,000.00".to_owned()),
        "{values:?}"
    );
    assert!(
        values.contains(&"Ending $12,000.00".to_owned()),
        "{values:?}"
    );
}

#[test]
fn the_context_header_shows_no_balance_beside_the_account_name() {
    let mut harness = app();

    open_account(&mut harness, "Emergency Fund");

    // The Account bar's balances are part of its account buttons, so no Label shows
    // the bare amount.
    let shown = label_values(&harness)
        .iter()
        .filter(|value| *value == "$12,000.00")
        .count();
    assert_eq!(shown, 0);
}

/// The colours `amount` is drawn in, wherever it is drawn alone or as one section of
/// a longer text, from the last frame's shapes.
fn amount_colours(harness: &Harness<'static, App>, amount: &str) -> Vec<Color32> {
    harness
        .output()
        .shapes
        .iter()
        .filter_map(
            #[expect(clippy::wildcard_enum_match_arm, reason = "foreign enum")]
            |clipped| match &clipped.shape {
                Shape::Text(shape) => Some(shape),
                _ => None,
            },
        )
        .flat_map(|shape| {
            let job = &shape.galley.job;
            job.sections
                .iter()
                .filter(|section| {
                    job.text[section.byte_range.start.0..section.byte_range.end.0].trim() == amount
                })
                .map(|section| shape.override_text_color.unwrap_or(section.format.color))
                .collect::<Vec<_>>()
        })
        .collect()
}

#[test]
fn a_negative_balance_in_the_context_header_takes_the_negative_colour() {
    let mut harness = app();

    open_account(&mut harness, "Visa Rewards");

    let negative = harness.ctx.global_style().visuals.error_fg_color;
    // In the Account bar, then as Ending and as Current.
    assert_eq!(
        amount_colours(&harness, "-$412.30"),
        [negative, negative, negative]
    );
}

/// The `Label` whose text is `value`.
fn label<'a>(harness: &'a Harness<'static, App>, value: &str) -> egui_kittest::Node<'a> {
    harness
        .query_all_by_role(Role::Label)
        .find(|label| label.accesskit_node().value().as_deref() == Some(value))
        .unwrap_or_else(|| panic!("no Label reads {value:?}"))
}

#[test]
fn a_narrow_window_cuts_the_account_name_short_and_keeps_the_balances_and_button_whole() {
    let mut harness = app();
    open_account(&mut harness, "Everyday Checking");
    let header_widths = |harness: &Harness<'static, App>| {
        [
            label(harness, "Current $5,642.21").rect(),
            label(harness, "Ending $4,071.81").rect(),
            harness
                .get_by_role_and_label(Role::Button, "+ Transaction")
                .rect(),
        ]
        .map(|rect| rect.width())
    };
    let roomy = header_widths(&harness);

    harness.set_size(vec2(560.0, 600.0));
    harness.run();

    assert_eq!(header_widths(&harness), roomy);
    // Only a name cut short shows itself whole when hovered.
    label(&harness, "Everyday Checking").hover();
    harness.run();
    let named = label_values(&harness)
        .iter()
        .filter(|value| *value == "Everyday Checking")
        .count();
    assert_eq!(named, 2);
}
