//! Image snapshots of the layout-critical views, in light and dark. They are checked on
//! Linux only, by `cargo xtask snapshots` (see `docs/practices/testing.md`).

#![cfg(target_os = "linux")]

use balanced_ui::App;
use balanced_ui::testing::{app, check_snapshot};
use eframe::egui::accesskit::Role;
use eframe::egui::{Event, Theme};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable as _;

/// Clicks the widget with `role` and `label`, then lets the App settle.
fn click(harness: &mut Harness<'static, App>, role: Role, label: &str) {
    harness.get_by_role_and_label(role, label).click();
    harness.run();
}

/// Moves the pointer out of the window so no hover or cursor shows in the snapshot.
fn pointer_gone(harness: &mut Harness<'static, App>) {
    harness.event(Event::PointerGone);
    harness.run();
}

/// Unfolds the Account bar, which auto-fold shows as a rail on Home, where every run
/// starts.
fn unfold_account_bar(harness: &mut Harness<'static, App>) {
    harness.run();
    click(harness, Role::Button, "Unfold the Account bar");
}

/// Opens the account named `name` from the Account bar.
fn open_account(harness: &mut Harness<'static, App>, name: &str) {
    unfold_account_bar(harness);
    click(harness, Role::Button, name);
    pointer_gone(harness);
}

/// Chooses gridlines between both rows and columns in Tools → Preferences.
fn choose_both_gridlines(harness: &mut Harness<'static, App>) {
    click(harness, Role::Button, "Tools");
    click(harness, Role::Button, "Preferences…");
    click(harness, Role::Tab, "Register");
    click(harness, Role::RadioButton, "Both");
    click(harness, Role::Button, "OK");
}

#[test]
#[ignore = "image snapshot: run with `cargo xtask snapshots`"]
fn account_bar_light() {
    let mut harness = app(Theme::Light);

    unfold_account_bar(&mut harness);
    pointer_gone(&mut harness);

    check_snapshot(&mut harness, "account_bar_light");
}

#[test]
#[ignore = "image snapshot: run with `cargo xtask snapshots`"]
fn account_bar_dark() {
    let mut harness = app(Theme::Dark);

    unfold_account_bar(&mut harness);
    pointer_gone(&mut harness);

    check_snapshot(&mut harness, "account_bar_dark");
}

#[test]
#[ignore = "image snapshot: run with `cargo xtask snapshots`"]
fn register_light() {
    let mut harness = app(Theme::Light);

    open_account(&mut harness, "Everyday Checking");

    check_snapshot(&mut harness, "register_light");
}

#[test]
#[ignore = "image snapshot: run with `cargo xtask snapshots`"]
fn register_dark() {
    let mut harness = app(Theme::Dark);

    open_account(&mut harness, "Everyday Checking");

    check_snapshot(&mut harness, "register_dark");
}

#[test]
#[ignore = "image snapshot: run with `cargo xtask snapshots`"]
fn register_gridlines_light() {
    let mut harness = app(Theme::Light);
    choose_both_gridlines(&mut harness);

    open_account(&mut harness, "Everyday Checking");

    check_snapshot(&mut harness, "register_gridlines_light");
}

#[test]
#[ignore = "image snapshot: run with `cargo xtask snapshots`"]
fn register_gridlines_dark() {
    let mut harness = app(Theme::Dark);
    choose_both_gridlines(&mut harness);

    open_account(&mut harness, "Everyday Checking");

    check_snapshot(&mut harness, "register_gridlines_dark");
}
