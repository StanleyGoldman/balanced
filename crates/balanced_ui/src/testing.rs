// SPDX-License-Identifier: GPL-3.0-or-later WITH AdditionRef-Balanced-Plugin-Exception

//! Support for tests: the App drawn by the lavapipe software rasterizer, the snapshot
//! check with the project thresholds (see `docs/practices/testing.md`), and the check of
//! the Preferences the App saved.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use balanced_core::{AccountKind, CurrencyCode, DomainCommand, Ledger, Money};
use balanced_i18n::Localizer;
use eframe::egui::{self, vec2};
use egui_kittest::{Harness, SnapshotOptions};
use egui_wgpu::{WgpuSetup, WgpuSetupCreateNew, wgpu};

use icu_calendar::Date;

use crate::command::AppCommand;
use crate::preferences::{Preferences, Scheme};
use crate::{App, Clock, PreferencesFile};

/// How much one pixel may differ before it counts as failing: kittest's weighted squared
/// YIQ distance.
const THRESHOLD: f32 = 0.6;

/// How many pixels may fail before the snapshot fails: none.
const MAX_FAILED_PIXELS: usize = 0;

/// The App on stub data in the test locale, with a preferences file choosing `scheme`,
/// wide enough to show every Register column, drawn by lavapipe.
///
/// # Panics
///
/// When the first snapshot renders, if lavapipe isn't installed.
pub fn app(scheme: egui::Theme) -> Harness<'static, App> {
    let preferences_file = PreferencesFile::for_tests();
    let preferences = Preferences {
        scheme: match scheme {
            egui::Theme::Light => Scheme::Light,
            egui::Theme::Dark => Scheme::Dark,
        },
        ..Preferences::default()
    };
    preferences_file
        .save(&preferences)
        .expect("the test machine can write to a temporary directory");
    Harness::builder()
        .with_size(vec2(1280.0, 720.0))
        // kittest switches egui's theme after the App is built, as an OS theme change
        // does. The Scheme alone must decide the look.
        .with_theme(scheme)
        .wgpu_setup(lavapipe())
        .build_eframe(move |cc| {
            App::on_ledger(cc, Localizer::for_tests(), Ledger::stub(), clock(), preferences_file)
        })
}

/// Sends the Command that creates a data file at `path` and switches to it, as File →
/// New Data File… does once the native picker returns `path`.
pub fn new_data_file(harness: &Harness<'_, App>, path: &Path) {
    harness
        .state()
        .send(AppCommand::NewDataFile(path.to_owned()));
}

/// Sends the Command that opens the data file at `path` and switches to it, as File →
/// Open Data File… does once the native picker returns `path`.
pub fn open_data_file(harness: &Harness<'_, App>, path: &Path) {
    harness
        .state()
        .send(AppCommand::OpenDataFile(path.to_owned()));
}

/// Sends `command` to the open ledger, as a widget does, so it is applied in the same
/// frame as the Commands the App sent before it.
pub fn send_domain(harness: &Harness<'_, App>, command: DomainCommand) {
    harness.state().send(AppCommand::Domain(command));
}

/// A closed data file named `name` in `dir`, in US dollars, holding checking accounts
/// named `accounts`, created in that order, so the last one is the change Undo
/// reverses first.
///
/// # Panics
///
/// If `dir` can't take a new data file.
pub fn data_file(dir: &Path, name: &str, accounts: &[&str]) -> PathBuf {
    let path = dir.join(name);
    let usd = CurrencyCode::new("USD").expect("USD is an ISO 4217 currency");
    let mut ledger = Ledger::create(&path, usd).expect("a new path takes a data file");
    for account in accounts {
        ledger
            .apply(DomainCommand::CreateAccount {
                name: (*account).to_owned(),
                kind: AccountKind::Checking,
                opened: Date::try_new_iso(2026, 9, 1).expect("1 September 2026 is a date"),
                opening_balance: Money::new("10.00".parse().expect("10.00 is a number"), usd)
                    .expect("10.00 has two decimal places"),
            })
            .expect("a new file takes a new account");
    }
    path
}

/// A clock stopped on 14 October 2026.
fn clock() -> Clock {
    Clock::fixed(Date::try_new_iso(2026, 10, 14).expect("14 October 2026 is a date"))
}

/// Renders `harness` and compares it with `tests/snapshots/{name}.png`, using the project
/// thresholds. `UPDATE_SNAPSHOTS=1` rewrites a snapshot that doesn't match.
///
/// # Panics
///
/// If the image differs from the snapshot, or the snapshot is missing.
#[track_caller]
pub fn check_snapshot(harness: &mut Harness<'_, App>, name: &str) {
    let options = SnapshotOptions::new()
        .threshold(THRESHOLD)
        .max_failed_pixels(MAX_FAILED_PIXELS);
    #[expect(
        clippy::disallowed_methods,
        reason = "the one call that applies the project thresholds"
    )]
    let result = harness.try_snapshot_options(name, &options);
    if let Err(err) = result {
        panic!("{err}");
    }
}

/// Checks that the preferences file in `config_dir` holds each key of `expected`, a
/// `preferences.toml` fragment, with its value. Keys the fragment leaves out aren't
/// checked.
///
/// # Panics
///
/// If the file is missing or isn't TOML, if `expected` isn't TOML, or if a key of
/// `expected` is missing from the file or holds another value.
#[track_caller]
pub fn check_saved_preferences(config_dir: &Path, expected: &str) {
    let text = std::fs::read_to_string(config_dir.join("preferences.toml"))
        .expect("the App wrote the preferences file");
    let saved: toml::Table = toml::from_str(&text).expect("the preferences file is TOML");
    let expected: toml::Table = toml::from_str(expected).expect("the expected keys are TOML");
    let checked: toml::Table = saved
        .into_iter()
        .filter(|(key, _)| expected.contains_key(key))
        .collect();
    assert_eq!(checked, expected);
}

/// A wgpu setup that only accepts lavapipe, Mesa's Vulkan software rasterizer, so every
/// machine draws the same pixels.
fn lavapipe() -> WgpuSetup {
    let mut setup = WgpuSetupCreateNew::without_display_handle();
    setup.instance_descriptor.backends = wgpu::Backends::VULKAN;
    setup.native_adapter_selector = Some(Arc::new(|adapters, _surface| {
        adapters
            .iter()
            .find(|adapter| {
                let info = adapter.get_info();
                info.device_type == wgpu::DeviceType::Cpu && info.name.contains("llvmpipe")
            })
            .cloned()
            .ok_or_else(|| {
                "no lavapipe Vulkan adapter: install Mesa's Vulkan drivers \
                 (`mesa-vulkan-drivers` on Debian and Ubuntu)"
                    .to_owned()
            })
    }));
    WgpuSetup::CreateNew(setup)
}
