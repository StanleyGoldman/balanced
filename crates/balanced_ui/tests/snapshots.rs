//! Image snapshots of the layout-critical views, in light and dark. They are checked on
//! Linux only, by `cargo xtask snapshots` (see `docs/practices/testing.md`).

#![cfg(target_os = "linux")]

use balanced_ui::App;
use balanced_ui::testing::{app, check_snapshot};
use eframe::egui::accesskit::Role;
use eframe::egui::{Event, Theme};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable as _;

/// Opens the account named `name` from the Account bar, then moves the pointer out of the
/// window so no hover or cursor shows in the snapshot.
fn open(harness: &mut Harness<'static, App>, name: &str) {
    harness.get_by_role_and_label(Role::Button, name).click();
    harness.run();
    harness.event(Event::PointerGone);
    harness.run();
}

#[test]
#[ignore = "image snapshot: run with `cargo xtask snapshots`"]
fn account_bar_light() {
    let mut harness = app(Theme::Light);

    harness.run();

    check_snapshot(&mut harness, "account_bar_light");
}

#[test]
#[ignore = "image snapshot: run with `cargo xtask snapshots`"]
fn account_bar_dark() {
    let mut harness = app(Theme::Dark);

    harness.run();

    check_snapshot(&mut harness, "account_bar_dark");
}

#[test]
#[ignore = "image snapshot: run with `cargo xtask snapshots`"]
fn register_light() {
    let mut harness = app(Theme::Light);

    open(&mut harness, "Everyday Checking");

    check_snapshot(&mut harness, "register_light");
}

#[test]
#[ignore = "image snapshot: run with `cargo xtask snapshots`"]
fn register_dark() {
    let mut harness = app(Theme::Dark);

    open(&mut harness, "Everyday Checking");

    check_snapshot(&mut harness, "register_dark");
}
