//! Image snapshot support for tests: the App drawn by the lavapipe software rasterizer,
//! and the snapshot check with the project thresholds (see `docs/practices/testing.md`).

use std::sync::Arc;

use balanced_core::Ledger;
use balanced_i18n::Localizer;
use eframe::egui::{self, vec2};
use egui_kittest::{Harness, SnapshotOptions};
use egui_wgpu::{WgpuSetup, WgpuSetupCreateNew, wgpu};

use icu_calendar::Date;

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
            App::new(cc, Localizer::for_tests(), Ledger::stub(), clock(), preferences_file)
        })
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
