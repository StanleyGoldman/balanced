//! PROTOTYPE (#483): Register editing-row edge cases (keys, prompts, confirms). Throwaway,
//! never merged; see `docs/agents/prototypes.md`.
//!
//! This is the real Balanced App, compiled from `crates/balanced_ui/src` with
//! `#[path]`, on `Ledger::stub()` in memory. The modules a setting changes are patched
//! copies beside this file (`app.rs`, `register.rs`, `main_window.rs`, `split_dialog.rs`,
//! `dialog.rs`); `diff -u ../../src/<module>.rs <module>.rs` shows each patch. With every
//! setting on its first option the App behaves as it ships.
//!
//! Run: `cargo run -p balanced_ui --example register_edge_prototype -- [--variant <A-D>]
//! [--log <dir>/events.jsonl] [--control <dir>/control.txt]`.
//!
//! Keys (Alt, since every plain key types into the Register): Alt+A–D variant (step 12),
//! Alt+←/→ variant, Alt+1–3 cycle the current step's settings, Alt+PgUp/PgDn step.
//! F2 note, F3 marker, F4 dock, F5 spotlight, F6 prompt panel (the app's own F6, Switch
//! pane, is off while the prototype runs).
//!
//! Control verbs: `variant <A-D>`, `toggle <id> [<option 1-based>]` (no option cycles),
//! `step <n>`, `screenshot [label]`, `dock on|off`, `reload` (fresh stub data, every
//! change thrown away), `quit`, `account <name>`, `key <Ctrl+Shift+Name>` (a key press,
//! e.g. `key Tab`, `key Ctrl+Enter`, `key Esc`), `text <chars>` (typed text),
//! `click <tracked widget name>` (a left click on its centre), `resetsettings`.

#![allow(
    dead_code,
    unreachable_pub,
    missing_debug_implementations,
    clippy::allow_attributes,
    clippy::allow_attributes_without_reason,
    clippy::pedantic,
    clippy::too_many_lines,
    clippy::wildcard_enum_match_arm,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::str_to_string,
    clippy::let_underscore_must_use,
    clippy::needless_pass_by_ref_mut,
    clippy::use_self,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::as_conversions,
    clippy::cast_possible_truncation,
    clippy::print_stdout,
    clippy::print_stderr,
    clippy::disallowed_methods,
    clippy::collapsible_if,
    clippy::type_complexity,
    clippy::string_slice,
    clippy::missing_docs_in_private_items
)]

#[path = "../../src/account_bar.rs"]
mod account_bar;
#[path = "../../src/accounts_model.rs"]
mod accounts_model;
#[path = "../../src/amount_field.rs"]
mod amount_field;
mod app;
#[path = "../../src/calculator.rs"]
mod calculator;
#[path = "../../src/clock.rs"]
mod clock;
#[path = "../../src/combo_field.rs"]
mod combo_field;
#[path = "../../src/combobox.rs"]
mod combobox;
#[path = "../../src/command.rs"]
mod command;
#[path = "../../src/data_file.rs"]
mod data_file;
#[path = "../../src/date_field.rs"]
mod date_field;
mod dialog;
#[path = "../../src/draft_row.rs"]
mod draft_row;
#[path = "../../src/formatted_amount.rs"]
mod formatted_amount;
#[path = "../../src/go_to_date.rs"]
mod go_to_date;
#[path = "../../src/i18n.rs"]
mod i18n;
#[path = "../../src/keymap.rs"]
mod keymap;
#[path = "../../src/keypad.rs"]
mod keypad;
#[path = "../../src/layout.rs"]
mod layout;
mod main_window;
#[path = "../../src/manage_accounts.rs"]
mod manage_accounts;
#[path = "../../src/menu_nav.rs"]
mod menu_nav;
#[path = "../../src/new_account.rs"]
mod new_account;
#[path = "../../src/new_category.rs"]
mod new_category;
#[path = "../../src/preferences.rs"]
mod preferences;
#[path = "../../src/preferences_dialog.rs"]
mod preferences_dialog;
mod proto;
#[path = "../../src/recent_files.rs"]
mod recent_files;
mod register;
#[path = "../../src/register_columns.rs"]
mod register_columns;
#[path = "../../src/register_filter.rs"]
mod register_filter;
#[path = "../../src/register_model.rs"]
mod register_model;
#[path = "../../src/route.rs"]
mod route;
#[path = "../../src/row_changes.rs"]
mod row_changes;
#[path = "../../src/save_refusal.rs"]
mod save_refusal;
mod split_dialog;
#[path = "../../src/theme.rs"]
mod theme;
#[path = "../../src/triangle_button.rs"]
mod triangle_button;
#[path = "../../src/user_action.rs"]
mod user_action;
#[path = "../../src/welcome.rs"]
mod welcome;

use std::collections::VecDeque;
use std::path::PathBuf;

use balanced_core::Ledger;
use balanced_i18n::Localizer;
use balanced_prototype_harness::{Args, Harness};
use eframe::egui::{
    self, Event, Key, Modifiers, Panel, PointerButton, RichText, Stroke, Ui, ViewportBuilder,
    ViewportCommand,
};
use icu_locale::{Locale, locale};
use serde_json::json;

pub use app::App;
pub use clock::Clock;
pub use preferences::PreferencesFile;
pub use recent_files::RecentFilesFile;

use crate::command::AppCommand;
use crate::proto::{SETTINGS, STEPS, VARIANTS};
use crate::route::Route;

struct Proto {
    app: App,
    step: u8,
    dock_open: bool,
    maximize_sent: bool,
    /// Input to put into the next frames' raw input, one batch a frame.
    inject: VecDeque<Vec<Event>>,
    /// Where the pointer is, as the injected clicks last left it.
    pointer: Option<egui::Pos2>,
}

impl Proto {
    fn set_step(&mut self, step: u8, cause: &str) {
        let step = step.clamp(1, STEPS);
        if step == self.step {
            return;
        }
        self.step = step;
        proto::log("step", json!({ "step": step, "cause": cause }));
    }

    fn set_variant(&mut self, v: u8, cause: &str) {
        proto::set_variant(v);
        let (key, label) = VARIANTS[proto::variant() as usize];
        proto::log(
            "variant",
            json!({ "variant": key, "label": label, "cause": cause }),
        );
        proto::with(|h| h.screenshot(format!("variant-{key}")));
    }

    fn set_setting(&mut self, ctx: &egui::Context, index: usize, option: Option<u8>, cause: &str) {
        let def = &SETTINGS[index];
        let next = option.unwrap_or_else(|| proto::get(def.setting) + 1);
        proto::set(def.setting, next);
        let now = proto::get(def.setting);
        proto::log(
            "toggle",
            json!({
                "setting": def.id,
                "ticket": def.ticket,
                "option": now + 1,
                "label": def.options[now as usize],
                "cause": cause,
            }),
        );
        if def.setting == proto::Setting::Locale {
            self.app.set_i18n(localizer(now));
        }
        ctx.request_repaint();
        proto::with(|h| h.screenshot(format!("{}-{}", def.id, now + 1)));
    }

    fn apply_control(&mut self, ctx: &egui::Context, cmd: &str) {
        let (verb, rest) = cmd
            .split_once(' ')
            .map_or((cmd, ""), |(v, r)| (v, r.trim()));
        let bad = || proto::log("control_unknown", json!({ "command": cmd }));
        match verb {
            "variant" => match VARIANTS
                .iter()
                .position(|(k, _)| k.eq_ignore_ascii_case(rest))
            {
                Some(v) => self.set_variant(v as u8, "control"),
                None => bad(),
            },
            "toggle" => {
                let (id, value) = rest
                    .split_once(' ')
                    .map_or((rest, None), |(k, v)| (k, Some(v.trim())));
                let Some(index) = SETTINGS.iter().position(|d| d.id == id) else {
                    bad();
                    return;
                };
                let option = value.and_then(|v| v.parse::<u8>().ok()).map(|n| n.saturating_sub(1));
                self.set_setting(ctx, index, option, "control");
            }
            "resetsettings" => {
                for (i, d) in SETTINGS.iter().enumerate() {
                    if proto::get(d.setting) != 0 {
                        self.set_setting(ctx, i, Some(0), "control");
                    }
                }
            }
            "step" => match rest.parse::<u8>() {
                Ok(n) => self.set_step(n, "control"),
                Err(_) => bad(),
            },
            "screenshot" => {
                let label = if rest.is_empty() { "shot" } else { rest };
                proto::with(|h| h.screenshot(label));
            }
            "dock" => self.dock_open = rest != "off",
            "reload" => {
                self.app.replace_ledger(Ledger::stub());
                proto::log("reload", json!({}));
            }
            "quit" => ctx.send_viewport_cmd(ViewportCommand::Close),
            "account" => {
                let id = self.app.view().and_then(|view| {
                    view.accounts()
                        .iter()
                        .find(|a| a.account().name().eq_ignore_ascii_case(rest))
                        .map(|a| a.account().id())
                });
                match id {
                    Some(id) => self.app.send(AppCommand::Navigate(Route::Account(id))),
                    None => bad(),
                }
            }
            "key" => match parse_key(rest) {
                Some((key, modifiers)) => {
                    let event = |pressed| Event::Key {
                        key,
                        physical_key: None,
                        pressed,
                        repeat: false,
                        modifiers,
                    };
                    let mut down = vec![event(true)];
                    // A real Space press types a space too.
                    if key == Key::Space && modifiers.is_none() {
                        down.push(Event::Text(" ".to_owned()));
                    }
                    self.inject.push_back(down);
                    self.inject.push_back(vec![event(false)]);
                }
                None => bad(),
            },
            "text" => self.inject.push_back(vec![Event::Text(rest.to_owned())]),
            "click" => match proto::find(rest) {
                Some((name, rect)) => {
                    let pos = rect.center();
                    proto::log("control_click", json!({ "widget": name }));
                    let button = |pressed| Event::PointerButton {
                        pos,
                        button: PointerButton::Primary,
                        pressed,
                        modifiers: Modifiers::NONE,
                    };
                    self.inject
                        .push_back(vec![Event::PointerMoved(pos), button(true)]);
                    self.inject.push_back(vec![button(false)]);
                    self.pointer = Some(pos);
                }
                None => bad(),
            },
            _ => bad(),
        }
    }

    /// The prototype's own keys, all with Alt so none types into the Register.
    fn keys(&mut self, ctx: &egui::Context) {
        let alt = |key| ctx.input_mut(|i| i.consume_key(Modifiers::ALT, key));
        for (v, key) in [Key::A, Key::B, Key::C, Key::D].into_iter().enumerate() {
            if alt(key) {
                self.set_variant(v as u8, "key");
            }
        }
        if alt(Key::ArrowLeft) {
            self.set_variant(proto::variant() + 3, "key");
        }
        if alt(Key::ArrowRight) {
            self.set_variant(proto::variant() + 1, "key");
        }
        if alt(Key::PageUp) {
            self.set_step(self.step.saturating_sub(1), "key");
        }
        if alt(Key::PageDown) {
            self.set_step(self.step + 1, "key");
        }
        let in_step: Vec<usize> = SETTINGS
            .iter()
            .enumerate()
            .filter(|(_, d)| d.step == self.step)
            .map(|(i, _)| i)
            .collect();
        for (n, key) in [Key::Num1, Key::Num2, Key::Num3].into_iter().enumerate() {
            if alt(key)
                && let Some(&index) = in_step.get(n)
            {
                self.set_setting(ctx, index, None, "key");
            }
        }
        if ctx.input(|i| i.key_pressed(Key::F4)) {
            self.dock_open = !self.dock_open;
            proto::log("dock", json!({ "open": self.dock_open, "cause": "key" }));
        }
        // F6 is the harness's prompt panel here, not the app's Switch pane.
        ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::F6));
    }

    fn dock(&mut self, ui: &mut Ui) {
        let frame = {
            let v = ui.visuals();
            egui::Frame::side_top_panel(ui.style())
                .fill(v.warn_fg_color.gamma_multiply(0.10).blend(v.panel_fill))
                .stroke(Stroke::new(1.0, v.warn_fg_color))
        };
        let ctx = ui.ctx().clone();
        if !self.dock_open {
            Panel::right("prototype_dock_edge")
                .exact_size(22.0)
                .resizable(false)
                .frame(frame)
                .show(ui, |ui| {
                    let r = ui
                        .small_button("<")
                        .on_hover_text("Show prototype controls (F4)");
                    proto::track(&r, "dock: expand");
                    if r.clicked() {
                        self.dock_open = true;
                    }
                });
            return;
        }
        Panel::right("prototype_dock")
            .exact_size(340.0)
            .resizable(false)
            .frame(frame)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("PROTOTYPE #483").strong());
                    let r = ui.small_button(">").on_hover_text("Collapse (F4)");
                    proto::track(&r, "dock: collapse");
                    if r.clicked() {
                        self.dock_open = false;
                    }
                });
                ui.separator();
                ui.horizontal(|ui| {
                    let r = ui.small_button("<");
                    proto::track(&r, "dock: previous step");
                    if r.clicked() {
                        self.set_step(self.step.saturating_sub(1), "click");
                    }
                    ui.label(RichText::new(format!("Step {}/{STEPS}", self.step)).strong());
                    let r = ui.small_button(">");
                    proto::track(&r, "dock: next step");
                    if r.clicked() {
                        self.set_step(self.step + 1, "click");
                    }
                    ui.label(RichText::new("Alt+PgUp/PgDn").weak());
                });
                let mut n = 0;
                for (index, def) in SETTINGS.iter().enumerate() {
                    if def.step != self.step {
                        continue;
                    }
                    n += 1;
                    ui.add_space(4.0);
                    ui.label(
                        RichText::new(format!("Alt+{n}  {} (#{})", def.title, def.ticket)).strong(),
                    );
                    let now = proto::get(def.setting);
                    for (o, option) in def.options.iter().enumerate() {
                        let r = ui.selectable_label(now as usize == o, format!("   {option}"));
                        proto::track(&r, format!("dock: {} {}", def.id, o + 1));
                        if r.clicked() && now as usize != o {
                            self.set_setting(&ctx, index, Some(o as u8), "click");
                        }
                    }
                }
                if self.step == 12 {
                    ui.add_space(4.0);
                    ui.label(RichText::new("Variant (Alt+A–D): the editing row's buttons").strong());
                    for (v, (key, label)) in VARIANTS.iter().enumerate() {
                        let r = ui.selectable_label(
                            proto::variant() as usize == v,
                            format!("   {key}  {label}"),
                        );
                        proto::track(&r, format!("dock: variant {key}"));
                        if r.clicked() {
                            self.set_variant(v as u8, "click");
                        }
                    }
                    ui.label(
                        RichText::new("Hide Balance, Clr or Flag with the Register's column chooser.")
                            .weak(),
                    );
                }
                ui.separator();
                ui.collapsing("Settings changed in other steps", |ui| {
                    let mut any = false;
                    for def in SETTINGS.iter().filter(|d| d.step != self.step) {
                        let now = proto::get(def.setting);
                        if now != 0 {
                            any = true;
                            ui.label(format!(
                                "Step {}: {}: {}",
                                def.step, def.title, def.options[now as usize]
                            ));
                        }
                    }
                    if !any {
                        ui.label(RichText::new("none: today's behaviour").weak());
                    }
                });
                ui.separator();
                proto::with(|h| {
                    h.dock_spotlight(ui);
                    h.dock_prompt(ui);
                });
                ui.separator();
                let r = ui.button("Reset the data");
                proto::track(&r, "dock: reset data");
                if r.clicked() {
                    self.apply_control(&ctx, "reload");
                }
                ui.label(
                    RichText::new("F2 note · F3 marker · F4 dock · F5 spotlight · F6 prompt").weak(),
                );
            });
    }
}

impl eframe::App for Proto {
    fn logic(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        self.app.logic(ctx, frame);
    }

    fn raw_input_hook(&mut self, ctx: &egui::Context, raw_input: &mut egui::RawInput) {
        if let Some(batch) = self.inject.pop_front() {
            raw_input.events.extend(batch);
            ctx.request_repaint();
        }
        let tabs = |input: &egui::RawInput| {
            input
                .events
                .iter()
                .filter(|e| matches!(e, Event::Key { key: Key::Tab, pressed: true, .. }))
                .count()
        };
        let before = tabs(raw_input);
        self.app.raw_input_hook(ctx, raw_input);
        // The Register takes Tab before egui (and the harness) sees it, so log it here.
        if tabs(raw_input) < before {
            proto::log("key", json!({ "key": "Tab", "field": "register" }));
        }
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        self.app.save(storage);
    }

    fn on_exit(&mut self, gl: Option<&eframe::glow::Context>) {
        self.app.on_exit(gl);
        proto::log("session_end", json!({}));
    }

    fn persist_egui_memory(&self) -> bool {
        false
    }

    fn ui(&mut self, ui: &mut Ui, frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        if !self.maximize_sent {
            ctx.send_viewport_cmd(ViewportCommand::Maximized(true));
            self.maximize_sent = true;
        }
        proto::clear_rects();
        if let Some(cmd) = proto::with(|h| h.begin_frame(&ctx)) {
            self.apply_control(&ctx, &cmd);
        }
        self.keys(&ctx);
        self.dock(ui);
        proto::with(|h| h.prompt_panel(ui));
        self.app.ui(ui, frame);
        if let Some(text) = proto::current_notice() {
            egui::Area::new(egui::Id::new("prototype_notice"))
                .order(egui::Order::Tooltip)
                .anchor(egui::Align2::CENTER_TOP, [0.0, 48.0])
                .show(&ctx, |ui| {
                    egui::Frame::popup(ui.style())
                        .fill(ui.visuals().warn_fg_color.gamma_multiply(0.25).blend(ui.visuals().panel_fill))
                        .show(ui, |ui| ui.label(RichText::new(text).strong()));
                });
            ctx.request_repaint_after(std::time::Duration::from_millis(500));
        }
        if !self.inject.is_empty() {
            ctx.request_repaint();
        }
        proto::with(|h| h.end_frame(&ctx));
    }
}

/// `Ctrl+Shift+Enter` and the like, with egui's key names plus Esc, Up, Down, Left, Right.
fn parse_key(text: &str) -> Option<(Key, Modifiers)> {
    let mut modifiers = Modifiers::NONE;
    let mut key = None;
    for part in text.split('+') {
        match part.trim().to_lowercase().as_str() {
            "ctrl" | "cmd" => modifiers = modifiers | Modifiers::COMMAND,
            "shift" => modifiers = modifiers | Modifiers::SHIFT,
            "alt" => modifiers = modifiers | Modifiers::ALT,
            "esc" => key = Some(Key::Escape),
            "up" => key = Some(Key::ArrowUp),
            "down" => key = Some(Key::ArrowDown),
            "left" => key = Some(Key::ArrowLeft),
            "right" => key = Some(Key::ArrowRight),
            _ => key = Key::from_name(part.trim()),
        }
    }
    Some((key?, modifiers))
}

fn localizer(option: u8) -> Localizer {
    let tag: Locale = if option == 1 {
        locale!("de-DE")
    } else {
        locale!("en-US")
    };
    Localizer::new(&tag.id, &tag).expect("load the messages")
}

fn main() -> eframe::Result {
    let args = Args::from_env();
    let harness = Harness::new(&args);
    proto::install(harness);
    if let Some(v) = args
        .variant
        .as_deref()
        .and_then(|k| VARIANTS.iter().position(|(key, _)| key.eq_ignore_ascii_case(k)))
    {
        proto::set_variant(v as u8);
    }
    // PROTOTYPE, wipe me: preferences and Recent files live here, never in the user's config.
    let config_dir: PathBuf = std::env::temp_dir().join("balanced-register-edge-prototype");
    let _ = std::fs::remove_dir_all(&config_dir);
    std::fs::create_dir_all(&config_dir).expect("create the prototype's config dir");
    let options = eframe::NativeOptions {
        viewport: ViewportBuilder::default()
            .with_title("Balanced — PROTOTYPE register edge cases")
            .with_maximized(true)
            .with_inner_size([1600.0, 1000.0]),
        ..Default::default()
    };
    eframe::run_native(
        "balanced_register_edge_prototype",
        options,
        Box::new(move |cc| {
            let app = App::on_ledger(cc, localizer(0), Ledger::stub(), Clock::default(), &config_dir);
            proto::log("start", json!({}));
            Ok(Box::new(Proto {
                app,
                step: 1,
                dock_open: true,
                maximize_sent: false,
                inject: VecDeque::new(),
                pointer: None,
            }))
        }),
    )
}
