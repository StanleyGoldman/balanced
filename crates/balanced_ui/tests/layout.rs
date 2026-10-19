//! The Main window's layout across a restart: saved by one App, restored by the next.

use std::collections::HashMap;

use balanced_core::Ledger;
use balanced_i18n::Localizer;
use balanced_ui::{App, Clock, PreferencesFile};
use eframe::App as _;
use eframe::egui::accesskit::{Role, Toggled};
use eframe::egui::{Event, Modifiers, PointerButton, Pos2, pos2, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable as _};
use icu_calendar::Date;

/// eframe storage in memory.
#[derive(Default)]
struct MemoryStorage(HashMap<String, String>);

impl eframe::Storage for MemoryStorage {
    fn get_string(&self, key: &str) -> Option<String> {
        self.0.get(key).cloned()
    }

    fn set_string(&mut self, key: &str, value: String) {
        self.0.insert(key.to_owned(), value);
    }

    fn remove_string(&mut self, key: &str) {
        self.0.remove(key);
    }

    fn flush(&mut self) {}
}

/// The App on stub data, in the test locale, restored from `storage`, wide enough to
/// show every Register column, as a run starts: on Home.
fn app(storage: &MemoryStorage) -> Harness<'_, App> {
    let mut harness = Harness::builder()
        .with_size(vec2(1280.0, 720.0))
        .build_eframe(|cc| {
            cc.storage = Some(storage);
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

/// Saves `harness`'s App into fresh storage, as eframe does on exit.
fn save(harness: &mut Harness<'_, App>) -> MemoryStorage {
    let mut storage = MemoryStorage::default();
    harness.state_mut().save(&mut storage);
    storage
}

#[test]
fn folded_groups_survive_a_restart() {
    let empty = MemoryStorage::default();
    let mut first = app(&empty);
    first.get_by_role_and_label(Role::Button, "Banking").click();
    first.run();
    first.get_by_role_and_label(Role::Button, "Closed").click();
    first.run();
    let storage = save(&mut first);

    let second = app(&storage);

    let banking = second.get_by_role_and_label(Role::Button, "Banking");
    assert_eq!(banking.accesskit_node().toggled(), Some(Toggled::False));
    second.get_by_role_and_label(Role::Button, "Old Checking");
}

#[test]
fn a_restart_starts_on_home() {
    let empty = MemoryStorage::default();
    let mut first = app(&empty);
    open(&mut first, "Everyday Checking");
    let storage = save(&mut first);

    let second = app(&storage);

    let home = second.get_by_role_and_label(Role::Button, "Home");
    assert_eq!(home.accesskit_node().toggled(), Some(Toggled::True));
}

#[test]
fn only_the_layout_is_saved() {
    let empty = MemoryStorage::default();
    let mut harness = app(&empty);
    open(&mut harness, "Everyday Checking");

    let storage = save(&mut harness);

    let keys: Vec<&str> = storage.0.keys().map(String::as_str).collect();
    assert_eq!(keys, ["layout"]);
    // The layout names no account, so it holds neither navigation nor financial data.
    assert!(!storage.0["layout"].contains("Everyday Checking"));
}

/// Storage holding `entry` as the saved layout.
fn storage_with(entry: &str) -> MemoryStorage {
    MemoryStorage(HashMap::from([("layout".to_owned(), entry.to_owned())]))
}

/// The right edge of the Account bar: where its account rows end.
fn account_bar_right(harness: &Harness<'_, App>) -> f32 {
    harness
        .get_by_role_and_label(Role::Button, "Everyday Checking")
        .rect()
        .right()
}

/// Drags with the primary button from `from` to `to`.
fn drag(harness: &mut Harness<'_, App>, from: Pos2, to: Pos2) {
    harness.event(Event::PointerMoved(from));
    harness.event(Event::PointerButton {
        pos: from,
        button: PointerButton::Primary,
        pressed: true,
        modifiers: Modifiers::NONE,
    });
    harness.event(Event::PointerMoved(to));
    harness.event(Event::PointerButton {
        pos: to,
        button: PointerButton::Primary,
        pressed: false,
        modifiers: Modifiers::NONE,
    });
    harness.run();
}

#[test]
fn the_account_bar_opens_at_its_saved_width() {
    let empty = MemoryStorage::default();
    let storage = storage_with("(version: 1, account_bar_width: 300.0)");

    let default = app(&empty);
    let restored = app(&storage);

    // A first run's Account bar is 200 points wide.
    assert_eq!(
        account_bar_right(&restored) - account_bar_right(&default),
        100.0
    );
}

#[test]
fn a_dragged_account_bar_width_survives_a_restart() {
    let empty = MemoryStorage::default();
    let mut first = app(&empty);
    let edge = account_bar_right(&first);
    let y = first
        .get_by_role_and_label(Role::Button, "Everyday Checking")
        .rect()
        .center()
        .y;
    drag(&mut first, pos2(edge, y), pos2(edge + 80.0, y));
    let dragged = account_bar_right(&first);
    let storage = save(&mut first);

    let second = app(&storage);

    assert!(
        dragged > edge + 70.0,
        "the drag widened the Account bar to {dragged}"
    );
    assert_eq!(account_bar_right(&second), dragged);
}

#[test]
fn folding_the_account_bar_to_a_rail_hides_its_accounts() {
    let empty = MemoryStorage::default();
    let mut harness = app(&empty);

    harness
        .get_by_role_and_label(Role::Button, "Fold the Account bar")
        .click();
    harness.run();

    assert!(
        harness
            .query_by_role_and_label(Role::Button, "Everyday Checking")
            .is_none()
    );
    harness
        .get_by_role_and_label(Role::Button, "Unfold the Account bar")
        .click();
    harness.run();
    harness.get_by_role_and_label(Role::Button, "Everyday Checking");
}

#[test]
fn a_folded_account_bar_survives_a_restart() {
    let empty = MemoryStorage::default();
    let mut first = app(&empty);
    first
        .get_by_role_and_label(Role::Button, "Fold the Account bar")
        .click();
    first.run();
    let storage = save(&mut first);

    let second = app(&storage);

    assert!(
        second
            .query_by_role_and_label(Role::Button, "Everyday Checking")
            .is_none()
    );
    second.get_by_role_and_label(Role::Button, "Unfold the Account bar");
}

#[test]
fn the_folded_rail_has_no_add_account() {
    let empty = MemoryStorage::default();
    let mut harness = app(&empty);

    click(&mut harness, "Fold the Account bar");

    assert!(
        harness
            .query_by_role_and_label(Role::Button, "+ Add Account")
            .is_none()
    );
}

#[test]
fn file_new_account_opens_the_wizard_with_the_account_bar_folded() {
    let empty = MemoryStorage::default();
    let mut harness = app(&empty);
    click(&mut harness, "Fold the Account bar");

    click(&mut harness, "File");
    click(&mut harness, "New Account…");

    harness.get_by_role_and_label(Role::Label, "Step 1 of 4: Type");
}

/// Opens the account named `name` from the Account bar.
fn open(harness: &mut Harness<'_, App>, name: &str) {
    harness.get_by_role_and_label(Role::Button, name).click();
    harness.run();
}

/// The width of the Register column headed `title`.
fn column_width(harness: &Harness<'_, App>, title: &str) -> f32 {
    harness
        .get_by_role_and_label(Role::ColumnHeader, title)
        .rect()
        .width()
}

/// Drags the edge after the Register column headed `title` by `by` points.
fn resize_column(harness: &mut Harness<'_, App>, title: &str, by: f32) {
    let handle = harness
        .get_by_role_and_label(Role::Splitter, &format!("Resize {title}"))
        .rect()
        .center();
    drag(harness, handle, handle + vec2(by, 0.0));
}

#[test]
fn a_resized_column_is_shared_by_every_register() {
    let empty = MemoryStorage::default();
    let mut harness = app(&empty);
    open(&mut harness, "Everyday Checking");
    let before = column_width(&harness, "Memo");

    resize_column(&mut harness, "Memo", 60.0);
    open(&mut harness, "Emergency Fund");

    assert_eq!(column_width(&harness, "Memo"), before + 60.0);
}

#[test]
fn a_resized_column_survives_a_restart() {
    let empty = MemoryStorage::default();
    let mut first = app(&empty);
    open(&mut first, "Everyday Checking");
    let before = column_width(&first, "Memo");
    resize_column(&mut first, "Memo", 60.0);
    let storage = save(&mut first);

    let mut second = app(&storage);
    open(&mut second, "Everyday Checking");

    assert_eq!(column_width(&second, "Memo"), before + 60.0);
}

/// Clicks the button labelled `label`, then lets the App settle.
fn click(harness: &mut Harness<'_, App>, label: &str) {
    harness.get_by_role_and_label(Role::Button, label).click();
    harness.run();
}

/// Whether the Account bar is folded to its rail.
fn is_rail(harness: &Harness<'_, App>) -> bool {
    harness
        .query_by_role_and_label(Role::Button, "Unfold the Account bar")
        .is_some()
}

#[test]
fn switching_tabs_leaves_the_account_bar_open() {
    let empty = MemoryStorage::default();
    let mut harness = app(&empty);

    click(&mut harness, "Accounts");
    click(&mut harness, "Home");

    assert!(!is_rail(&harness));
}

#[test]
fn switching_tabs_leaves_a_folded_account_bar_folded() {
    let empty = MemoryStorage::default();
    let mut harness = app(&empty);
    click(&mut harness, "Fold the Account bar");

    click(&mut harness, "Accounts");
    click(&mut harness, "Home");

    assert!(is_rail(&harness));
}
