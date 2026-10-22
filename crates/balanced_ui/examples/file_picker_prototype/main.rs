// SPDX-License-Identifier: GPL-3.0-or-later WITH AdditionRef-Balanced-Plugin-Exception

//! PROTOTYPE, throwaway: the window behind the native file picker.
//!
//! Question (ticket #547, for #438): what does Balanced's window do while a file picker is open?
//!
//! Applied from earlier decisions: the welcome Dialog ignores clicks on the window behind it
//! (#440); its Quit button, Esc and the window's close button still quit.
//!
//! Four variants differ in who stops a second picker:
//!   A  No parent (today): the window stays live, and each Continue or menu item opens a picker
//!   B  `rfd`'s `set_parent` with the eframe window, so the desktop makes the picker modal
//!   C  B, plus Continue and File > New / Open greyed out until the picker closes
//!   D  B, plus a second request ignored while one is open; the controls stay live
//!
//! Every picker is the real `rfd::AsyncFileDialog`, built on the UI thread and waited for on a
//! worker thread, as `crates/balanced_ui/src/data_file.rs` does. Nothing is created or opened: a
//! picked path only logs the Command the real app would emit.
//!
//! Run natively on Wayland (not under XWayland: the portal attaches the picker by the window's
//! Wayland or X11 handle, so XWayland shows a different behaviour from the one that ships):
//!   cargo run -p balanced_ui --example file_picker_prototype -- \
//!     [--variant a|b|c|d] [--log <dir>/events.jsonl] [--control <dir>/control.txt]
//!
//! Control-file verbs (besides the harness's `prompt` and `spotlight`): `variant <key>`,
//! `scene welcome|main`, `choose new|open` (the welcome Dialog's radio), `continue`,
//! `menu new|open` (the File menu item), `screenshot [label]`, `dock on|off`, `reload`, `quit`.
//! `continue` and `menu` act as the click does, so they open a real picker on the desktop.
//!
//! English literals, default egui light visuals (the look isn't the question). None of this is
//! production code.

#![allow(
    clippy::allow_attributes,
    clippy::allow_attributes_without_reason,
    clippy::pedantic,
    clippy::too_many_lines,
    clippy::wildcard_enum_match_arm,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::str_to_string,
    clippy::let_underscore_must_use,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::as_conversions,
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::integer_division,
    clippy::shadow_unrelated,
    clippy::print_stdout,
    clippy::disallowed_methods,
    clippy::absolute_paths,
    clippy::single_match_else,
    clippy::manual_let_else,
    clippy::needless_pass_by_ref_mut,
    clippy::use_self,
    clippy::inconsistent_digit_grouping,
    clippy::manual_is_multiple_of,
    dead_code,
    missing_debug_implementations,
    unreachable_pub
)]

use std::path::PathBuf;
use std::sync::mpsc;
use std::thread;
use std::time::Instant;

use balanced_prototype_harness::{Args, Harness};
use eframe::egui;
use egui::{
    Align, Button, CentralPanel, Color32, Context, Frame, Id, Key, Layout, Modal, Panel, RichText,
    Sides, Stroke, Ui, Vec2, ViewportBuilder, ViewportCommand,
};
use rfd::AsyncFileDialog;
use serde_json::json;

// ---------------------------------------------------------------------------------------------
// Variants

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Variant {
    A,
    B,
    C,
    D,
}

impl Variant {
    const ALL: [Variant; 4] = [Variant::A, Variant::B, Variant::C, Variant::D];

    fn key(self) -> &'static str {
        match self {
            Variant::A => "a",
            Variant::B => "b",
            Variant::C => "c",
            Variant::D => "d",
        }
    }

    fn from_key(key: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|v| v.key().eq_ignore_ascii_case(key.trim()))
    }

    fn label(self) -> &'static str {
        match self {
            Variant::A => "No parent (today)",
            Variant::B => "Parent set: the OS makes it modal",
            Variant::C => "Parent set + controls greyed out",
            Variant::D => "Parent set + second request ignored",
        }
    }

    fn letter_key(self) -> Key {
        match self {
            Variant::A => Key::A,
            Variant::B => Key::B,
            Variant::C => Key::C,
            Variant::D => Key::D,
        }
    }

    fn step(self, by: isize) -> Self {
        let i = Self::ALL.iter().position(|v| *v == self).unwrap() as isize;
        let n = Self::ALL.len() as isize;
        Self::ALL[(i + by).rem_euclid(n) as usize]
    }

    /// Whether the picker gets the eframe window as its parent.
    fn parented(self) -> bool {
        self != Variant::A
    }

    /// Whether Continue and the File menu's picker items are greyed out while one is open.
    fn greys_out(self) -> bool {
        self == Variant::C
    }

    /// Whether a request while a picker is open is dropped.
    fn ignores_second(self) -> bool {
        self == Variant::D
    }
}

// ---------------------------------------------------------------------------------------------
// Pickers

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PickerKind {
    New,
    Open,
}

impl PickerKind {
    fn name(self) -> &'static str {
        match self {
            PickerKind::New => "new",
            PickerKind::Open => "open",
        }
    }

    /// The Command the real app sends with the picked path.
    fn would_emit(self) -> &'static str {
        match self {
            PickerKind::New => "AppCommand::NewDataFile",
            PickerKind::Open => "AppCommand::OpenDataFile",
        }
    }
}

/// A picker showing on the desktop, waited for on a worker thread.
struct OpenPicker {
    id: u32,
    kind: PickerKind,
    /// Where it was opened from: "welcome Continue" or "File menu".
    from: &'static str,
    variant: Variant,
    parented: bool,
    since: Instant,
}

/// What a worker thread sends back when its picker closes.
struct Closed {
    id: u32,
    path: Option<PathBuf>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Scene {
    /// No data file open: the empty window under the welcome Dialog.
    Welcome,
    /// A data file open: the Main window with its File menu.
    Main,
}

impl Scene {
    fn name(self) -> &'static str {
        match self {
            Scene::Welcome => "welcome",
            Scene::Main => "main",
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Stub data: a register to click on behind the picker

struct Row {
    date: String,
    payee: &'static str,
    cents: i64,
    balance: i64,
}

const SEED: u64 = 0x5eed_0547;
const PAYEES: [&str; 12] = [
    "Whole Foods",
    "Shell",
    "Comcast",
    "Payroll",
    "Amazon",
    "Trader Joe's",
    "PG&E",
    "Netflix",
    "Rent",
    "Costco",
    "Starbucks",
    "Target",
];

fn generate_rows() -> Vec<Row> {
    let mut state = SEED;
    let mut next = move || {
        state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        (state >> 33) as u32
    };
    let mut balance: i64 = 2_450_00;
    let mut rows = Vec::with_capacity(2400);
    let (mut year, mut month, mut day) = (2023, 1, 1);
    for _ in 0..2400 {
        let r = next();
        let payee = PAYEES[(r % PAYEES.len() as u32) as usize];
        let cents = match payee {
            "Payroll" => 2_400_00 + i64::from(next() % 400_00),
            "Rent" => -1_850_00,
            _ => -(500 + i64::from(next() % 180_00)),
        };
        balance += cents;
        rows.push(Row {
            date: format!("{month:02}/{day:02}/{year}"),
            payee,
            cents,
            balance,
        });
        if next() % 3 == 0 {
            day += 1;
            if day > 28 {
                day = 1;
                month += 1;
                if month > 12 {
                    month = 1;
                    year += 1;
                }
            }
        }
    }
    rows
}

fn fmt_cents(cents: i64) -> String {
    let sign = if cents < 0 { "-" } else { "" };
    let abs = cents.unsigned_abs();
    let dollars = (abs / 100).to_string();
    let mut grouped = String::new();
    for (i, c) in dollars.chars().enumerate() {
        if i > 0 && (dollars.len() - i) % 3 == 0 {
            grouped.push(',');
        }
        grouped.push(c);
    }
    format!("{sign}{grouped}.{:02}", abs % 100)
}

// ---------------------------------------------------------------------------------------------
// App

/// The light Dialog's content width (`Theme::LIGHT_DIALOG_WIDTH`).
const LIGHT_DIALOG_WIDTH: f32 = 360.0;

struct Proto {
    h: Harness,
    variant: Variant,
    scene: Scene,
    dock_open: bool,
    maximize_sent: bool,
    /// The welcome Dialog's radio.
    choice: PickerKind,
    /// Whether the welcome Dialog has drawn once, so Continue gets first focus.
    welcome_shown: bool,
    open: Vec<OpenPicker>,
    next_id: u32,
    closed_tx: mpsc::Sender<Closed>,
    closed_rx: mpsc::Receiver<Closed>,
    /// Requests made this frame, run once the frame handle is at hand.
    requests: Vec<(PickerKind, &'static str, &'static str)>,
    /// Pickers opened this session, and the most ever open at once.
    opened_total: u32,
    most_at_once: usize,
    /// Clicks and keys the window took while a picker was open, this session.
    clicks_behind: u32,
    keys_behind: u32,
    data_file: Option<String>,
    status: String,
    rows: Vec<Row>,
    selected_row: Option<usize>,
    closing: bool,
}

impl Proto {
    fn new(args: &Args) -> Self {
        let variant = args
            .variant
            .as_deref()
            .and_then(Variant::from_key)
            .unwrap_or(Variant::A);
        let mut h = Harness::new(args);
        h.event("variant", json!({ "to": variant.key(), "via": "start" }));
        let (closed_tx, closed_rx) = mpsc::channel();
        Self {
            h,
            variant,
            scene: Scene::Welcome,
            dock_open: true,
            maximize_sent: false,
            choice: PickerKind::New,
            welcome_shown: false,
            open: Vec::new(),
            next_id: 1,
            closed_tx,
            closed_rx,
            requests: Vec::new(),
            opened_total: 0,
            most_at_once: 0,
            clicks_behind: 0,
            keys_behind: 0,
            data_file: None,
            status: "No data file open".to_owned(),
            rows: generate_rows(),
            selected_row: None,
            closing: false,
        }
    }

    fn event(&mut self, kind: &str, fields: serde_json::Value) {
        self.h.event(kind, fields);
    }

    fn set_variant(&mut self, v: Variant, via: &str) {
        if v == self.variant {
            return;
        }
        let from = self.variant;
        self.variant = v;
        self.event(
            "variant",
            json!({ "from": from.key(), "to": v.key(), "via": via, "pickers_open": self.open.len() }),
        );
        self.h.screenshot(format!("variant-{}", v.key()));
    }

    fn set_scene(&mut self, scene: Scene, via: &str) {
        if scene == self.scene {
            return;
        }
        self.scene = scene;
        if scene == Scene::Welcome {
            self.welcome_shown = false;
            self.data_file = None;
            self.status = "No data file open".to_owned();
        } else if self.data_file.is_none() {
            self.data_file = Some("Household.balanced".to_owned());
            self.status = "Household.balanced".to_owned();
        }
        self.event("scene", json!({ "to": scene.name(), "via": via }));
        self.h.screenshot(format!("scene-{}", scene.name()));
    }

    fn set_dock(&mut self, open: bool, via: &str) {
        if open != self.dock_open {
            self.dock_open = open;
            self.event("dock", json!({ "open": open, "via": via }));
        }
    }

    /// Whether the picker controls take clicks now.
    fn pickers_enabled(&self) -> bool {
        !(self.variant.greys_out() && !self.open.is_empty())
    }

    /// Asks for a picker; it opens at the end of the frame, where the window handle is.
    fn request(&mut self, kind: PickerKind, from: &'static str, via: &'static str) {
        self.requests.push((kind, from, via));
    }

    fn run_requests(&mut self, ctx: &Context, frame: &eframe::Frame) {
        for (kind, from, via) in std::mem::take(&mut self.requests) {
            self.event(
                "picker_requested",
                json!({
                    "kind": kind.name(), "from": from, "via": via,
                    "variant": self.variant.key(), "pickers_open": self.open.len(),
                    "would_emit": "AppCommand::ChooseDataFile",
                }),
            );
            if !self.open.is_empty() && self.variant.ignores_second() {
                self.event(
                    "picker_request_ignored",
                    json!({ "kind": kind.name(), "from": from, "pickers_open": self.open.len() }),
                );
                self.status = "A file picker is already open".to_owned();
                continue;
            }
            self.show_picker(ctx, frame, kind, from);
        }
    }

    /// Shows the real picker as `DataFilePicker::show` does, with the window as its parent in
    /// every variant but A.
    fn show_picker(
        &mut self,
        ctx: &Context,
        frame: &eframe::Frame,
        kind: PickerKind,
        from: &'static str,
    ) {
        let id = self.next_id;
        self.next_id += 1;
        let parented = self.variant.parented();
        let mut picker = AsyncFileDialog::new().add_filter("Balanced data files", &["balanced"]);
        if parented {
            picker = picker.set_parent(frame);
        }
        let picked = match kind {
            PickerKind::New => {
                let picker = match directories::UserDirs::new()
                    .as_ref()
                    .and_then(directories::UserDirs::document_dir)
                {
                    Some(dir) => picker.set_directory(dir),
                    None => picker,
                };
                Box::pin(picker.set_file_name("Balanced.balanced").save_file())
                    as std::pin::Pin<Box<dyn Future<Output = Option<rfd::FileHandle>> + Send>>
            }
            PickerKind::Open => Box::pin(picker.pick_file()),
        };
        let tx = self.closed_tx.clone();
        let repaint = ctx.clone();
        thread::Builder::new()
            .name(format!("picker-{id}"))
            .spawn(move || {
                let path = pollster::block_on(picked).map(|f| f.path().to_owned());
                let _ = tx.send(Closed { id, path });
                repaint.request_repaint();
            })
            .expect("spawn the picker thread");
        self.open.push(OpenPicker {
            id,
            kind,
            from,
            variant: self.variant,
            parented,
            since: Instant::now(),
        });
        self.opened_total += 1;
        self.most_at_once = self.most_at_once.max(self.open.len());
        self.status = format!("File picker open ({})", self.open.len());
        self.event(
            "picker_opened",
            json!({
                "id": id, "kind": kind.name(), "from": from, "parented": parented,
                "variant": self.variant.key(), "pickers_open": self.open.len(),
            }),
        );
        if self.open.len() > 1 {
            self.event(
                "second_picker",
                json!({ "id": id, "pickers_open": self.open.len(), "variant": self.variant.key() }),
            );
        }
    }

    fn collect_closed(&mut self) {
        while let Ok(Closed { id, path }) = self.closed_rx.try_recv() {
            let Some(at) = self.open.iter().position(|p| p.id == id) else {
                continue;
            };
            let p = self.open.remove(at);
            let open_ms = p.since.elapsed().as_millis() as u64;
            match path {
                Some(path) => {
                    let file = path.file_name().map_or_else(
                        || path.display().to_string(),
                        |f| f.to_string_lossy().into_owned(),
                    );
                    self.event(
                        "picker_closed",
                        json!({
                            "id": id, "kind": p.kind.name(), "outcome": "chosen",
                            "path": path.display().to_string(), "open_ms": open_ms,
                            "pickers_open": self.open.len(), "would_emit": p.kind.would_emit(),
                        }),
                    );
                    self.event(
                        "command",
                        json!({ "would_emit": p.kind.would_emit(), "path": path.display().to_string() }),
                    );
                    // The real app opens the file and the welcome Dialog goes; here the Main
                    // window shows the file's name and nothing is read or written.
                    self.data_file = Some(file.clone());
                    self.status = format!(
                        "Would {} {file} (nothing was written)",
                        if p.kind == PickerKind::New {
                            "create"
                        } else {
                            "open"
                        }
                    );
                    if self.scene == Scene::Welcome {
                        self.scene = Scene::Main;
                        self.event("scene", json!({ "to": "main", "via": "picked" }));
                    }
                }
                None => {
                    self.event(
                        "picker_closed",
                        json!({
                            "id": id, "kind": p.kind.name(), "outcome": "cancelled",
                            "open_ms": open_ms, "pickers_open": self.open.len(),
                        }),
                    );
                    if self.open.is_empty() {
                        self.status = match &self.data_file {
                            Some(f) => f.clone(),
                            None => "No data file open".to_owned(),
                        };
                    }
                }
            }
            self.h.screenshot(format!("picker-{id}-closed"));
        }
    }

    /// Logs every click and key the window takes while a picker is open, whether or not a
    /// widget took it, so the log shows how modal the picker made the window.
    fn watch_behind(&mut self, ctx: &Context) {
        if self.open.is_empty() {
            return;
        }
        let (clicked, pos, keys) = ctx.input(|i| {
            let keys: Vec<String> = i
                .events
                .iter()
                .filter_map(|e| match e {
                    egui::Event::Key {
                        key, pressed: true, ..
                    } => Some(format!("{key:?}")),
                    _ => None,
                })
                .collect();
            (i.pointer.any_click(), i.pointer.interact_pos(), keys)
        });
        if clicked {
            self.clicks_behind += 1;
            let over = pos
                .and_then(|p| ctx.layer_id_at(p))
                .map(|l| format!("{:?}", l.order));
            self.event(
                "input_behind_picker",
                json!({
                    "input": "click", "layer": over, "pickers_open": self.open.len(),
                    "variant": self.variant.key(), "scene": self.scene.name(),
                }),
            );
        }
        for key in keys {
            self.keys_behind += 1;
            self.event(
                "input_behind_picker",
                json!({
                    "input": "key", "key": key, "pickers_open": self.open.len(),
                    "variant": self.variant.key(), "scene": self.scene.name(),
                }),
            );
        }
    }

    fn apply_control(&mut self, ctx: &Context, cmd: &str) {
        let (verb, rest) = cmd
            .split_once(' ')
            .map_or((cmd, ""), |(v, r)| (v, r.trim()));
        let parse_kind = |s: &str| match s {
            "new" | "create" => Some(PickerKind::New),
            "open" => Some(PickerKind::Open),
            _ => None,
        };
        match verb {
            "variant" => match Variant::from_key(rest) {
                Some(v) => self.set_variant(v, "control"),
                None => self.event("control_unknown", json!({ "command": cmd })),
            },
            "scene" => match rest {
                "welcome" => self.set_scene(Scene::Welcome, "control"),
                "main" => self.set_scene(Scene::Main, "control"),
                _ => self.event("control_unknown", json!({ "command": cmd })),
            },
            "choose" => match parse_kind(rest) {
                Some(k) => self.choice = k,
                None => self.event("control_unknown", json!({ "command": cmd })),
            },
            "continue" => {
                self.set_scene(Scene::Welcome, "control");
                if self.pickers_enabled() {
                    self.request(self.choice, "welcome Continue", "control");
                } else {
                    self.event("disabled_press", json!({ "control": "Continue" }));
                }
            }
            "menu" => match parse_kind(rest) {
                Some(k) => {
                    self.set_scene(Scene::Main, "control");
                    if self.pickers_enabled() {
                        self.request(k, "File menu", "control");
                    } else {
                        self.event(
                            "disabled_press",
                            json!({ "control": format!("File menu {}", k.name()) }),
                        );
                    }
                }
                None => self.event("control_unknown", json!({ "command": cmd })),
            },
            "screenshot" => self
                .h
                .screenshot(if rest.is_empty() { "control" } else { rest }),
            "dock" => self.set_dock(rest != "off", "control"),
            "reload" => {
                // Open pickers stay on the desktop: they belong to the OS until closed.
                self.scene = Scene::Welcome;
                self.welcome_shown = false;
                self.choice = PickerKind::New;
                self.data_file = None;
                self.selected_row = None;
                self.status = "No data file open".to_owned();
                self.event("reload", json!({ "pickers_open": self.open.len() }));
            }
            "quit" => {
                self.closing = true;
                ctx.send_viewport_cmd(ViewportCommand::Close);
            }
            _ => self.event("control_unknown", json!({ "command": cmd })),
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Frame

impl eframe::App for Proto {
    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.h.event(
            "session_end",
            json!({
                "pickers_open": self.open.len(), "opened_total": self.opened_total,
                "most_at_once": self.most_at_once, "clicks_behind": self.clicks_behind,
                "keys_behind": self.keys_behind,
            }),
        );
    }

    fn ui(&mut self, ui: &mut Ui, frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        if !self.maximize_sent {
            ctx.send_viewport_cmd(ViewportCommand::Maximized(true));
            self.maximize_sent = true;
        }
        if let Some(cmd) = self.h.begin_frame(&ctx) {
            self.apply_control(&ctx, &cmd);
        }
        self.collect_closed();
        self.watch_behind(&ctx);
        if ctx.input(|i| i.viewport().close_requested()) && !self.open.is_empty() {
            self.event(
                "close_with_picker_open",
                json!({ "pickers_open": self.open.len(), "variant": self.variant.key() }),
            );
        }

        let typing = ctx.egui_wants_keyboard_input();
        if !typing {
            if ctx.input(|i| i.key_pressed(Key::ArrowLeft)) {
                self.set_variant(self.variant.step(-1), "key");
            }
            if ctx.input(|i| i.key_pressed(Key::ArrowRight)) {
                self.set_variant(self.variant.step(1), "key");
            }
            for v in Variant::ALL {
                if ctx.input(|i| i.key_pressed(v.letter_key()) && i.modifiers.is_none()) {
                    self.set_variant(v, "key");
                }
            }
        }
        if ctx.input(|i| i.key_pressed(Key::F4)) {
            self.set_dock(!self.dock_open, "key");
        }

        self.dock(ui);
        self.h.prompt_panel(ui);
        match self.scene {
            Scene::Welcome => {
                let r = CentralPanel::default().show(ui, |_| {}).response;
                self.h.region(r.rect, "window behind");
                self.welcome_dialog(&ctx);
            }
            Scene::Main => {
                self.menu_bar(ui);
                Panel::bottom("status")
                    .exact_size(26.0)
                    .show(ui, |ui| ui.label(RichText::new(&self.status).weak()));
                CentralPanel::default().show(ui, |ui| self.register(ui));
            }
        }
        self.run_requests(&ctx, frame);
        if !self.open.is_empty() {
            // The picker's age in the dock ticks while it is open.
            ctx.request_repaint_after(std::time::Duration::from_millis(500));
        }
        self.h.end_frame(&ctx);
    }
}

// ---------------------------------------------------------------------------------------------
// The welcome Dialog, the File menu, the register

impl Proto {
    /// The welcome Dialog as `welcome.rs` draws it through `LightDialog`, ignoring backdrop
    /// clicks (#440).
    fn welcome_dialog(&mut self, ctx: &Context) {
        let first_frame = !self.welcome_shown;
        self.welcome_shown = true;
        let enabled = self.pickers_enabled();
        let room = (ctx.content_rect().size() - Vec2::splat(48.0)).max(Vec2::ZERO);
        let frame = Frame::popup(&ctx.global_style());
        let modal = Modal::new(Id::new("welcome_dialog"))
            .frame(frame)
            .backdrop_color(Color32::from_black_alpha(0x60))
            .show(ctx, |ui| {
                let inner = (room - frame.total_margin().sum()).max(Vec2::ZERO);
                ui.set_width(LIGHT_DIALOG_WIDTH.min(inner.x));
                ui.label("Balanced keeps your finances in a data file.");
                let new =
                    ui.radio_value(&mut self.choice, PickerKind::New, "Create a new data file…");
                let open = ui.radio_value(
                    &mut self.choice,
                    PickerKind::Open,
                    "Open an existing data file…",
                );
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let quit = ui.add(Button::new("Quit"));
                    let cont = ui.add_enabled(enabled, Button::new("Continue"));
                    if first_frame && enabled {
                        cont.request_focus();
                    }
                    (new, open, cont, quit)
                })
                .inner
            });
        let (new, open, cont, quit) = &modal.inner;
        self.h.track(new, "welcome: Create radio");
        self.h.track(open, "welcome: Open radio");
        self.h.track(cont, "welcome: Continue");
        self.h.track(quit, "welcome: Quit");
        self.h.region(modal.response.rect, "welcome dialog");
        if new.clicked() || open.clicked() {
            self.event("choice", json!({ "picker": self.choice.name() }));
        }
        let enter = enabled
            && modal.is_top_modal
            && !quit.has_focus()
            && ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::Enter));
        if cont.clicked() || enter {
            let via = if cont.clicked() { "click" } else { "Enter" };
            self.request(self.choice, "welcome Continue", via);
        } else if modal.backdrop_response.clicked() {
            self.event(
                "backdrop_click_ignored",
                json!({ "pickers_open": self.open.len() }),
            );
        } else if quit.clicked() || modal.should_close() {
            self.event(
                "command",
                json!({ "would_emit": "AppCommand::Quit", "pickers_open": self.open.len() }),
            );
            self.status = "Would quit (the prototype stays open)".to_owned();
        }
    }

    fn menu_bar(&mut self, ui: &mut Ui) {
        let enabled = self.pickers_enabled();
        Panel::top("menu_bar").show(ui, |ui| {
            egui::MenuBar::new().ui(ui, |ui| {
                let file = ui.menu_button("File", |ui| {
                    let new = ui.add_enabled(enabled, Button::new("New Data File…"));
                    self.h.track(&new, "menu: New Data File");
                    let open = ui.add_enabled(enabled, Button::new("Open Data File…"));
                    self.h.track(&open, "menu: Open Data File");
                    ui.separator();
                    ui.add_enabled(false, Button::new("New Account…"));
                    ui.separator();
                    let quit = ui.button("Quit");
                    self.h.track(&quit, "menu: Quit");
                    if new.clicked() {
                        ui.close();
                        self.request(PickerKind::New, "File menu", "click");
                    }
                    if open.clicked() {
                        ui.close();
                        self.request(PickerKind::Open, "File menu", "click");
                    }
                    if quit.clicked() {
                        ui.close();
                        self.event("command", json!({ "would_emit": "AppCommand::Quit" }));
                        self.status = "Would quit (the prototype stays open)".to_owned();
                    }
                });
                self.h.track(&file.response, "menu: File");
                for name in ["Edit", "View", "Tools", "Help"] {
                    let r = ui.menu_button(name, |ui| {
                        ui.label(RichText::new("Not part of this prototype").weak());
                    });
                    self.h.track(&r.response, format!("menu: {name}"));
                }
            });
        });
    }

    fn register(&mut self, ui: &mut Ui) {
        let rect = ui.available_rect_before_wrap();
        self.h.region(rect, "register");
        let file = self.data_file.clone().unwrap_or_default();
        ui.label(
            RichText::new(format!(
                "{file} · Checking · {} transactions",
                self.rows.len()
            ))
            .weak(),
        );
        let row_h = 22.0;
        egui_extras::TableBuilder::new(ui)
            .id_salt("register")
            .striped(true)
            .sense(egui::Sense::click())
            .cell_layout(Layout::left_to_right(Align::Center))
            .column(egui_extras::Column::exact(100.0))
            .column(egui_extras::Column::initial(280.0))
            .column(egui_extras::Column::initial(140.0))
            .column(egui_extras::Column::remainder().at_least(140.0))
            .header(row_h, |mut header| {
                for label in ["Date", "Payee", "Amount", "Balance"] {
                    header.col(|ui| {
                        ui.strong(label);
                    });
                }
            })
            .body(|body| {
                body.rows(row_h, self.rows.len(), |mut row| {
                    let i = row.index();
                    row.set_selected(self.selected_row == Some(i));
                    let r = &self.rows[i];
                    row.col(|ui| {
                        ui.label(&r.date);
                    });
                    row.col(|ui| {
                        ui.label(r.payee);
                    });
                    row.col(|ui| {
                        ui.label(fmt_cents(r.cents));
                    });
                    row.col(|ui| {
                        ui.label(fmt_cents(r.balance));
                    });
                    let resp = row.response();
                    if resp.clicked() {
                        self.selected_row = Some(i);
                        self.h.event(
                            "click",
                            json!({ "widget": format!("register row {i}"), "pickers_open": self.open.len() }),
                        );
                    }
                });
            });
    }

    fn dock(&mut self, ui: &mut Ui) {
        let frame = {
            let v = ui.visuals();
            Frame::side_top_panel(ui.style())
                .fill(v.warn_fg_color.gamma_multiply(0.10).blend(v.panel_fill))
                .stroke(Stroke::new(1.0, v.warn_fg_color))
        };
        if !self.dock_open {
            Panel::right("prototype_dock_edge")
                .exact_size(22.0)
                .resizable(false)
                .frame(frame)
                .show(ui, |ui| {
                    let r = ui
                        .small_button("◀")
                        .on_hover_text("Show prototype controls (F4)");
                    self.h.track(&r, "dock: expand");
                    if r.clicked() {
                        self.set_dock(true, "click");
                    }
                });
            return;
        }
        Panel::right("prototype_dock")
            .exact_size(330.0)
            .resizable(false)
            .frame(frame)
            .show(ui, |ui| {
                Sides::new().show(
                    ui,
                    |ui| ui.label(RichText::new("PROTOTYPE").strong()),
                    |ui| {
                        let r = ui.small_button("▶").on_hover_text("Collapse (F4)");
                        self.h.track(&r, "dock: collapse");
                        if r.clicked() {
                            self.set_dock(false, "click");
                        }
                    },
                );
                ui.separator();
                ui.label(RichText::new("Variant (applies to pickers opened from now on)").strong());
                for v in Variant::ALL {
                    let r = ui.selectable_label(
                        self.variant == v,
                        format!("{}   {}", v.key().to_uppercase(), v.label()),
                    );
                    self.h.track(&r, format!("dock: variant {}", v.key()));
                    if r.clicked() {
                        self.set_variant(v, "click");
                    }
                }
                ui.separator();
                ui.label(RichText::new("Screen").strong());
                ui.horizontal(|ui| {
                    for (scene, label) in [(Scene::Welcome, "Welcome (no file)"), (Scene::Main, "Main window")] {
                        let r = ui.selectable_label(self.scene == scene, label);
                        self.h.track(&r, format!("dock: scene {}", scene.name()));
                        if r.clicked() {
                            self.set_scene(scene, "click");
                        }
                    }
                });
                ui.separator();
                ui.label(RichText::new("File pickers").strong());
                ui.label(format!(
                    "Open now: {}   ·   opened: {}   ·   most at once: {}",
                    self.open.len(),
                    self.opened_total,
                    self.most_at_once
                ));
                for p in &self.open {
                    ui.label(
                        RichText::new(format!(
                            "#{} {} from {}, {}, {}, open {:.0} s",
                            p.id,
                            p.kind.name(),
                            p.from,
                            p.variant.key().to_uppercase(),
                            if p.parented { "parented" } else { "no parent" },
                            p.since.elapsed().as_secs_f32()
                        ))
                        .small(),
                    );
                }
                ui.label(format!(
                    "Taken while one was open: {} clicks, {} keys",
                    self.clicks_behind, self.keys_behind
                ));
                ui.separator();
                self.h.dock_spotlight(ui);
                self.h.dock_prompt(ui);
                ui.separator();
                ui.label(
                    RichText::new(
                        "A / B / C / D or left / right: variant · F2 note · F3 marker · F4 hide this dock · F5 spotlight · F6 prompt",
                    )
                    .small()
                    .weak(),
                );
            });
    }
}

fn main() -> eframe::Result {
    // Prints the stub figures and exits, so they can be checked without a window.
    if std::env::var_os("FILE_PICKER_PROTO_DUMP").is_some() {
        let rows = generate_rows();
        for r in rows.iter().take(5).chain(rows.iter().rev().take(5)) {
            println!(
                "{} {:<14} {:>12} {:>14}",
                r.date,
                r.payee,
                fmt_cents(r.cents),
                fmt_cents(r.balance)
            );
        }
        return Ok(());
    }
    let args = Args::from_env();
    let options = eframe::NativeOptions {
        viewport: ViewportBuilder::default()
            .with_title("Balanced — PROTOTYPE file picker")
            .with_maximized(true)
            .with_inner_size([1600.0, 1000.0]),
        ..Default::default()
    };
    eframe::run_native(
        "balanced_file_picker_prototype",
        options,
        Box::new(move |cc| {
            cc.egui_ctx.set_theme(egui::Theme::Light);
            Ok(Box::new(Proto::new(&args)))
        }),
    )
}
