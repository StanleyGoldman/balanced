//! PROTOTYPE (#374): the Reconcile window. Throwaway, never merged; see
//! `docs/agents/prototypes.md`.
//!
//! Question: what shape should the Reconcile window take? #216 settled the behaviour;
//! this settles the layout.
//!
//! This is the real Balanced App, compiled from `crates/balanced_ui/src` with `#[path]`,
//! on a fixed-seed reconcile scenario written into a scratch data file. The modules the
//! prototype hooks into are patched copies beside this file (`app.rs`, `main_window.rs`,
//! `register.rs`, `register_filter.rs`, `account_bar.rs`); `diff -u ../../src/<module>.rs
//! <module>.rs` shows each patch, all marked `PROTOTYPE`. `reconcile.rs` draws the three
//! variants; `data.rs` builds the scenario.
//!
//! Run: `cargo run -p balanced_ui --example reconcile_prototype -- [--variant a|b|c]
//! [--log <dir>/events.jsonl] [--control <dir>/control.txt]`.
//!
//! Keys (Alt, since every plain key types into the Register): Alt+A/B/C variant,
//! Alt+←/→ variant, Alt+1–6 cycle a toggle. F2 note, F3 marker, F4 dock, F5 spotlight,
//! F6 prompt panel (the app's own F6, Switch pane, is off while the prototype runs).
//!
//! Control verbs: `variant <a|b|c>`, `toggle <key|1-6> [on|off|<option 1-based>]`,
//! `screenshot [label]`, `dock on|off`, `reload` (fresh data, every change thrown away),
//! `quit`, `account <name>`, `reconcile` (the Start Dialog for the open account),
//! `date <m/d/yyyy>`, `ending <text>` (fill the Start Dialog), `ok`, `cancel` (the top
//! prototype Dialog's buttons), `tick <stmt|all|num|payee>`, `untick <all|num|payee>`,
//! `select <num|payee>`, `edit`, `new`, `amount <text>`, `category <name>`, `finish`,
//! `later`, `editstatement`, `key <Ctrl+Shift+Name>`, `text <chars>`,
//! `click <tracked widget name>`, `wait <ms>`, `width <points>|max`.

#![allow(
    dead_code,
    unreachable_pub,
    unused_imports,
    private_interfaces,
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
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    clippy::integer_division,
    clippy::integer_division_remainder_used,
    clippy::print_stdout,
    clippy::print_stderr,
    clippy::disallowed_methods,
    clippy::collapsible_if,
    clippy::type_complexity,
    clippy::too_many_arguments,
    clippy::string_slice,
    clippy::shadow_unrelated,
    clippy::missing_docs_in_private_items,
    clippy::float_arithmetic,
    clippy::modulo_arithmetic,
    clippy::min_ident_chars,
    clippy::single_char_lifetime_names,
    clippy::absolute_paths,
    clippy::needless_range_loop,
    clippy::if_not_else,
    clippy::option_if_let_else,
    clippy::manual_let_else,
    clippy::unnecessary_wraps
)]

mod account_bar;
#[path = "../../src/accounts_model.rs"]
mod accounts_model;
#[path = "../../src/amount_field.rs"]
mod amount_field;
mod app;
#[path = "../../src/calculator.rs"]
mod calculator;
#[path = "../../src/category_list.rs"]
mod category_list;
#[path = "../../src/clock.rs"]
mod clock;
#[path = "../../src/combo_field.rs"]
mod combo_field;
#[path = "../../src/combobox.rs"]
mod combobox;
#[path = "../../src/command.rs"]
mod command;
mod data;
#[path = "../../src/data_file.rs"]
mod data_file;
#[path = "../../src/date_field.rs"]
mod date_field;
#[path = "../../src/dialog.rs"]
mod dialog;
#[path = "../../src/draft_row.rs"]
mod draft_row;
#[path = "../../src/edit_category.rs"]
mod edit_category;
#[path = "../../src/field_keys.rs"]
mod field_keys;
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
mod reconcile;
mod register;
#[path = "../../src/register_columns.rs"]
mod register_columns;
mod register_filter;
#[path = "../../src/register_model.rs"]
mod register_model;
#[path = "../../src/route.rs"]
mod route;
#[path = "../../src/row_changes.rs"]
mod row_changes;
#[path = "../../src/save_refusal.rs"]
mod save_refusal;
#[path = "../../src/split_dialog.rs"]
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
use crate::reconcile::{TOGGLES, Variant};
use crate::route::Route;

struct Proto {
    app: App,
    scenario: data::Scenario,
    data_path: PathBuf,
    dock_open: bool,
    maximize_sent: bool,
    /// Input to put into the next frames' raw input, one batch a frame.
    inject: VecDeque<Vec<Event>>,
    /// Control commands waiting for the injected input before them to land.
    pending: VecDeque<String>,
    /// `wait <ms>` holds the commands after it until then.
    hold_until: Option<std::time::Instant>,
    injected_last_frame: bool,
}

const TOGGLE_KEYS: [Key; 6] = [
    Key::Num1,
    Key::Num2,
    Key::Num3,
    Key::Num4,
    Key::Num5,
    Key::Num6,
];

impl Proto {
    fn set_variant(&mut self, to: Variant, cause: &str) {
        let recon = self.app.recon();
        let from = recon.variant();
        recon.variant = Some(to);
        proto::log(
            "variant",
            json!({ "from": from.key(), "to": to.key(), "label": to.label(), "cause": cause, "reconciling": recon.active.is_some() }),
        );
        proto::screenshot(format!("variant-{}", to.key()));
    }

    fn set_toggle(&mut self, key: &str, value: Option<&str>, cause: &str) {
        let index = key
            .parse::<usize>()
            .ok()
            .and_then(|n| n.checked_sub(1))
            .filter(|i| *i < TOGGLES.len())
            .or_else(|| TOGGLES.iter().position(|t| t.0 == key));
        let Some(i) = index else {
            proto::log("control_unknown", json!({ "toggle": key }));
            return;
        };
        let recon = self.app.recon();
        let n = TOGGLES[i].3.len();
        let new = match value {
            None => (recon.tog[i] + 1) % n,
            Some("on") => 1.min(n - 1),
            Some("off") => 0,
            Some(v) => v
                .parse::<usize>()
                .map_or(recon.tog[i], |v| v.saturating_sub(1).min(n - 1)),
        };
        recon.tog[i] = new;
        proto::log(
            "toggle",
            json!({ "key": TOGGLES[i].0, "option": TOGGLES[i].3[new], "index": new + 1, "cause": cause, "applies_to": TOGGLES[i].2 }),
        );
        proto::screenshot(format!("toggle-{}-{}", TOGGLES[i].0, new + 1));
    }

    fn reload(&mut self) {
        let open = self.app.open_account().and_then(|id| {
            self.app
                .view()
                .and_then(|v| v.account(id).map(|s| s.account().name().to_owned()))
        });
        // The old ledger holds the file; build into a fresh one.
        let path = fresh_path();
        let (ledger, scenario) = data::build(&path);
        self.app.replace_ledger(ledger, open.as_deref());
        self.app.recon().reset(&scenario);
        let _ = std::fs::remove_file(&self.data_path);
        self.data_path = path;
        self.scenario = scenario;
        proto::log("reload", json!({ "reopened": open }));
    }

    fn apply_control(&mut self, ctx: &egui::Context, cmd: &str) {
        let (verb, rest) = cmd
            .split_once(' ')
            .map_or((cmd, ""), |(v, r)| (v, r.trim()));
        let bad = || proto::log("control_unknown", json!({ "command": cmd }));
        match verb {
            "variant" => match Variant::from_key(rest) {
                Some(v) => self.set_variant(v, "control"),
                None => bad(),
            },
            "toggle" => {
                let (key, value) = rest
                    .split_once(' ')
                    .map_or((rest, None), |(k, v)| (k, Some(v.trim())));
                self.set_toggle(key, value, "control");
            }
            "screenshot" => proto::screenshot(if rest.is_empty() { "shot" } else { rest }),
            "dock" => self.dock_open = rest != "off",
            "reload" => self.reload(),
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
            "ok" | "cancel" => {
                let Some(view) = self.app.view() else {
                    return bad();
                };
                let today = self.app.today();
                let commands = self.app.commands().clone();
                if !self
                    .app
                    .recon()
                    .answer_top(ctx, &view, &commands, verb == "ok", today)
                {
                    bad();
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
            "width" => {
                if rest == "max" {
                    ctx.send_viewport_cmd(ViewportCommand::Maximized(true));
                } else if let Ok(width) = rest.parse::<f32>() {
                    let height = ctx.content_rect().height();
                    ctx.send_viewport_cmd(ViewportCommand::Maximized(false));
                    ctx.send_viewport_cmd(ViewportCommand::InnerSize(egui::vec2(width, height)));
                } else {
                    bad();
                }
                proto::log("width", json!({ "width": rest }));
            }
            "click" | "dclick" => match proto::find(rest) {
                Some((name, rect)) => {
                    let pos = rect.center();
                    proto::log(
                        "control_click",
                        json!({ "widget": name, "double": verb == "dclick" }),
                    );
                    let button = |pressed| Event::PointerButton {
                        pos,
                        button: PointerButton::Primary,
                        pressed,
                        modifiers: Modifiers::NONE,
                    };
                    // One frame for press and release: a covered XWayland window draws
                    // about once a second, and a press held that long isn't a click.
                    let mut batch = vec![Event::PointerMoved(pos), button(true), button(false)];
                    if verb == "dclick" {
                        batch.extend([button(true), button(false)]);
                    }
                    self.inject.push_back(batch);
                }
                None => bad(),
            },
            _ => {
                let Some(view) = self.app.view() else {
                    return bad();
                };
                let today = self.app.today();
                let commands = self.app.commands().clone();
                if !self
                    .app
                    .recon()
                    .control(ctx, &view, &commands, verb, rest, today)
                {
                    bad();
                }
            }
        }
    }

    /// The prototype's own keys, all with Alt so none types into the Register.
    fn keys(&mut self, ctx: &egui::Context) {
        // XWayland sends Alt+A's letter as a Text event too, and consume_key leaves it, so
        // it typed "a" into a focused Register cell (session 2026-10-28). Drop it.
        ctx.input_mut(|i| {
            if i.modifiers.alt {
                i.events.retain(|e| !matches!(e, egui::Event::Text(_)));
            }
        });
        let alt =|key| ctx.input_mut(|i| i.consume_key(Modifiers::ALT, key));
        for (v, key) in Variant::ALL.into_iter().zip([Key::A, Key::B, Key::C]) {
            if alt(key) {
                self.set_variant(v, "key");
            }
        }
        let now = Variant::ALL
            .iter()
            .position(|v| *v == self.app.recon().variant())
            .unwrap();
        if alt(Key::ArrowLeft) {
            self.set_variant(Variant::ALL[(now + 2) % 3], "key");
        }
        if alt(Key::ArrowRight) {
            self.set_variant(Variant::ALL[(now + 1) % 3], "key");
        }
        for (i, key) in TOGGLE_KEYS.into_iter().enumerate() {
            if alt(key) {
                self.set_toggle(&(i + 1).to_string(), None, "key");
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
                egui::ScrollArea::vertical().show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("PROTOTYPE #374 · Reconcile").strong());
                        let r = ui.small_button(">").on_hover_text("Collapse (F4)");
                        proto::track(&r, "dock: collapse");
                        if r.clicked() {
                            self.dock_open = false;
                        }
                    });
                    ui.separator();
                    ui.label(RichText::new("Variant (Alt+A/B/C): where Reconcile lives").strong());
                    let now = self.app.recon().variant();
                    for v in Variant::ALL {
                        let r = ui.selectable_label(
                            now == v,
                            format!("   {}  {}", v.key().to_uppercase(), v.label()),
                        );
                        proto::track(&r, format!("dock: variant {}", v.key()));
                        if r.clicked() && now != v {
                            self.set_variant(v, "click");
                        }
                    }
                    ui.separator();
                    ui.label(RichText::new("Toggles (Alt+number cycles)").strong());
                    for (i, (key, label, applies, options)) in TOGGLES.iter().enumerate() {
                        let applies = if *applies == "all" {
                            "all".to_owned()
                        } else {
                            applies.to_uppercase().replace(',', ", ")
                        };
                        ui.label(format!("Alt+{}  {label}  ({applies})", i + 1));
                        let cur = self.app.recon().tog[i];
                        ui.horizontal_wrapped(|ui| {
                            ui.add_space(14.0);
                            for (n, option) in options.iter().enumerate() {
                                let r = ui.selectable_label(cur == n, *option);
                                proto::track(&r, format!("dock: toggle {key} {option}"));
                                if r.clicked() && cur != n {
                                    self.set_toggle(key, Some(&(n + 1).to_string()), "click");
                                }
                            }
                        });
                    }
                    ui.separator();
                    ui.label(RichText::new("The paper statement in your hand").strong());
                    let open = self.app.open_account();
                    match open.and_then(|id| self.scenario.paper.get(&id).map(|p| (id, p.clone()))) {
                        Some((id, p)) => {
                            let card = id == self.scenario.visa;
                            let (y, m, d) = data::civil(p.date);
                            ui.label(format!(
                                "{}: {} {m}/{d}/{y}, {} {}.",
                                if card { "Visa" } else { "Checking" },
                                if card { "statement closing" } else { "statement of" },
                                if card { "balance owed" } else { "ending balance" },
                                plain(p.ending)
                            ));
                            ui.label(RichText::new(p.note).small());
                        }
                        None => {
                            ui.label(RichText::new("Open Checking or Visa.").weak());
                        }
                    }
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
                        RichText::new(
                            "Alt+A/B/C or Alt+←/→ variant · Alt+1–6 toggles · ↑/↓ and Space in A's and C's lists · F2 note · F3 marker · F4 dock · F5 spotlight · F6 prompt",
                        )
                        .small()
                        .weak(),
                    );
                });
            });
    }
}

impl eframe::App for Proto {
    fn logic(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        self.app.logic(ctx, frame);
    }

    fn raw_input_hook(&mut self, ctx: &egui::Context, raw_input: &mut egui::RawInput) {
        self.injected_last_frame = false;
        if let Some(batch) = self.inject.pop_front() {
            raw_input.events.extend(batch);
            self.injected_last_frame = true;
            ctx.request_repaint();
        }
        self.app.raw_input_hook(ctx, raw_input);
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        self.app.save(storage);
    }

    fn on_exit(&mut self, gl: Option<&eframe::glow::Context>) {
        self.app.on_exit(gl);
        proto::log("session_end", json!({}));
        let _ = std::fs::remove_file(&self.data_path);
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
            self.pending.push_back(cmd);
        }
        // A command waits until the keys and clicks queued before it have landed, and
        // a frame has drawn their effect.
        let held = self
            .hold_until
            .is_some_and(|until| std::time::Instant::now() < until);
        if !held && self.inject.is_empty() && !self.injected_last_frame {
            self.hold_until = None;
            if let Some(cmd) = self.pending.pop_front() {
                if let Some(ms) = cmd
                    .strip_prefix("wait ")
                    .and_then(|ms| ms.trim().parse().ok())
                {
                    self.hold_until =
                        Some(std::time::Instant::now() + std::time::Duration::from_millis(ms));
                } else {
                    self.apply_control(&ctx, &cmd);
                    // A screenshot (and a toggle's or variant's own) waits 350 ms to settle,
                    // then encodes a full-resolution PNG inside a frame; nothing else runs
                    // until it is saved.
                    if matches!(
                        cmd.split(' ').next(),
                        Some("screenshot" | "toggle" | "variant" | "finish" | "ok")
                    ) {
                        self.hold_until = Some(
                            std::time::Instant::now() + std::time::Duration::from_millis(2500),
                        );
                    }
                }
            }
        }
        if !self.pending.is_empty() {
            ctx.request_repaint_after(std::time::Duration::from_millis(30));
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
                        .fill(
                            ui.visuals()
                                .warn_fg_color
                                .gamma_multiply(0.25)
                                .blend(ui.visuals().panel_fill),
                        )
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

/// Cents as an en-US plain amount.
fn plain(cents: i64) -> String {
    let f = format!("{:.2}", cents.abs() as f64 / 100.0);
    let (whole, frac) = f.split_once('.').unwrap();
    let mut grouped = String::new();
    for (i, ch) in whole.chars().enumerate() {
        if i > 0 && (whole.len() - i).is_multiple_of(3) {
            grouped.push(',');
        }
        grouped.push(ch);
    }
    format!("{}{grouped}.{frac}", if cents < 0 { "-" } else { "" })
}

/// `Ctrl+Shift+Enter` and the like, with egui's key names plus Esc, Up, Down, Left, Right.
fn parse_key(text: &str) -> Option<(Key, Modifiers)> {
    let mut modifiers = Modifiers::NONE;
    let mut key = None;
    for part in text.split('+') {
        match part.trim().to_lowercase().as_str() {
            "ctrl" | "cmd" => modifiers |= Modifiers::COMMAND,
            "shift" => modifiers |= Modifiers::SHIFT,
            "alt" => modifiers |= Modifiers::ALT,
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

/// The prototype's config dir. PROTOTYPE, wipe me: preferences live here, never in the
/// user's config.
fn config_dir() -> PathBuf {
    std::env::temp_dir().join("balanced-reconcile-prototype")
}

/// A new scratch data file path, so a reload never collides with the open file's lock.
fn fresh_path() -> PathBuf {
    let n = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis());
    config_dir().join(format!("PROTOTYPE-wipe-me-{n}.balanced"))
}

fn main() -> eframe::Result {
    let args = Args::from_env();
    let harness = Harness::new(&args);
    proto::install(harness);
    let config_dir = config_dir();
    let _ = std::fs::remove_dir_all(&config_dir);
    std::fs::create_dir_all(&config_dir).expect("create the prototype's config dir");
    let data_path = fresh_path();
    let started = std::time::Instant::now();
    let (ledger, scenario) = data::build(&data_path);
    if std::env::var_os("RECONCILE_PROTO_DUMP").is_some() {
        dump(&ledger, &scenario, started);
        return Ok(());
    }
    let variant = args.variant.as_deref().and_then(Variant::from_key);
    let options = eframe::NativeOptions {
        viewport: ViewportBuilder::default()
            .with_title("Balanced — PROTOTYPE Reconcile")
            .with_maximized(true)
            .with_inner_size([1600.0, 1000.0]),
        ..Default::default()
    };
    eframe::run_native(
        "balanced_reconcile_prototype",
        options,
        Box::new(move |cc| {
            let tag: Locale = locale!("en-US");
            let i18n = Localizer::new(&tag.id, &tag).expect("load the messages");
            let checking = scenario.checking;
            let mut app = App::on_ledger(cc, i18n, ledger, Clock::default(), &config_dir);
            app.recon().reset(&scenario);
            app.recon().variant = variant;
            app.send(AppCommand::Navigate(Route::Account(checking)));
            proto::log(
                "start",
                json!({ "variant": app.recon().variant().key(), "build_ms": started.elapsed().as_millis() }),
            );
            Ok(Box::new(Proto {
                app,
                scenario,
                data_path,
                dock_open: true,
                maximize_sent: false,
                inject: VecDeque::new(),
                pending: VecDeque::new(),
                hold_until: None,
                injected_last_frame: false,
            }))
        }),
    )
}

/// Prints the stub figures and exits, so they can be checked without a window.
fn dump(ledger: &balanced_core::Ledger, scenario: &data::Scenario, started: std::time::Instant) {
    use balanced_core::ClearedStatus;
    let view = ledger.snapshot();
    println!("built in {} ms", started.elapsed().as_millis());
    for s in view.accounts() {
        let reg = s.register();
        let n = reg.len();
        let sum = |f: &dyn Fn(ClearedStatus) -> bool| -> i64 {
            reg.iter()
                .map(|e| e.transaction())
                .filter(|t| f(t.cleared_status()))
                .map(|t| data::cents(t.amount()))
                .sum()
        };
        println!(
            "{}: {n} txns, balance {} cleared {} reconciled {}",
            s.account().name(),
            plain(data::cents(s.balance())),
            plain(sum(&|c| c != ClearedStatus::Uncleared)),
            plain(sum(&|c| c == ClearedStatus::Reconciled)),
        );
        let min = reg
            .iter()
            .map(|e| data::cents(e.balance()))
            .min()
            .unwrap_or(0);
        println!("  lowest running balance {}", plain(min));
        if let Some(p) = scenario.paper.get(&s.account().id()) {
            let card = s.account().id() == scenario.visa;
            let shown = |c: i64| if card { -c } else { c };
            let cleared_now = sum(&|c| c != ClearedStatus::Uncleared);
            let ticked_paper: i64 = p
                .items
                .iter()
                .filter(|(id, _)| {
                    reg.iter().any(|e| {
                        e.transaction().id() == *id
                            && e.transaction().cleared_status() == ClearedStatus::Uncleared
                    })
                })
                .map(|(id, _)| {
                    reg.iter()
                        .find(|e| e.transaction().id() == *id)
                        .map(|e| data::cents(e.transaction().amount()))
                        .unwrap()
                })
                .sum();
            println!(
                "  paper {}: ending {} ; {} items; difference now {} ; with every paper item ticked {}",
                p.date,
                plain(p.ending),
                p.items.len(),
                plain(p.ending - shown(cleared_now)),
                plain(p.ending - shown(cleared_now + ticked_paper)),
            );
        }
    }
}
