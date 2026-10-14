//! PROTOTYPE observation harness, per `docs/agents/prototypes.md`. Throwaway, but written to be
//! copied: later prototypes take this file as-is from the `prototype/shell-layout` branch.
//!
//! It lets the agent follow the user through the app's own output:
//!
//! - `--log <path>`: a JSONL event log, one object per line with `ts_ms` (Unix ms, to line up
//!   with chat messages) and `t` (seconds since start). Screenshots land in the same directory.
//! - `--control <path>`: a file the agent appends commands to, one per line. A background thread
//!   reads it every 250 ms, empties it (commands already there at launch run first) and wakes the
//!   app, which applies one command per frame, waiting for any screenshot in progress so
//!   `variant b` then `screenshot b` capture the right thing. On Wayland a covered or unfocused
//!   window gets no frames, so commands wait until it is visible; `quit` still lands, because the
//!   thread exits the process itself once no frame has run for 2 s.
//! - `--variant <key>`: the variant to start on.
//! - F2 opens a one-line note box (Enter logs it, Esc cancels); F3 drops a marker + screenshot.
//!
//! The app's side of the contract:
//! - call `begin_frame` first in `ui()` and apply the commands it returns;
//! - call `track(&response, "payee cell, row 12")` on every widget worth naming;
//! - call `event(..)` for app-level events (variant switch, toggle flip, Commands it would emit);
//! - call `screenshot(label)` after a variant switch or toggle flip;
//! - call `scroll(area, offset_y, row_height)` each frame for big scroll areas;
//! - call `end_frame` last.

use std::collections::{HashMap, VecDeque};
use std::fs;
use std::io::Write as _;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use eframe::egui::{self, Align2, Context, Id, Key, Response};
use serde_json::{Value, json};

pub struct Args {
    pub variant: Option<String>,
    pub log: Option<PathBuf>,
    pub control: Option<PathBuf>,
}

pub fn parse_args() -> Args {
    let args: Vec<String> = std::env::args().collect();
    let value = |flag: &str| {
        args.iter()
            .position(|a| a == flag)
            .and_then(|p| args.get(p + 1))
            .cloned()
    };
    Args {
        variant: value("--variant"),
        log: value("--log").map(Into::into),
        control: value("--control").map(Into::into),
    }
}

struct Gesture {
    moving: bool,
    start_offset: f32,
    last_offset: f32,
    started: Instant,
    last_change: Instant,
    worst_ms: f32,
}

struct PendingShot {
    label: String,
    not_before: Instant,
}

pub struct Harness {
    log: Option<fs::File>,
    dir: Option<PathBuf>,
    control: Option<PathBuf>,
    start: Instant,
    frame: u64,
    control_rx: Option<mpsc::Receiver<String>>,
    /// Milliseconds since `start` at the last frame; the control thread reads it.
    last_frame_ms: Arc<AtomicU64>,
    commands: VecDeque<String>,
    names: HashMap<Id, String>,
    prev_names: HashMap<Id, String>,
    focused: Option<Id>,
    clicked_this_frame: bool,
    tab_this_frame: bool,
    shots: VecDeque<PendingShot>,
    shot_in_flight: Option<String>,
    shot_seq: u32,
    note: Option<String>,
    marker_seq: u32,
    dt_ms: f32,
    perf_since: Instant,
    perf_frames: u32,
    perf_sum_ms: f32,
    perf_worst_ms: f32,
    gestures: HashMap<String, Gesture>,
}

impl Harness {
    pub fn new(args: &Args) -> Self {
        let log = args.log.as_ref().map(|p| {
            if let Some(parent) = p.parent() {
                fs::create_dir_all(parent).expect("create log dir");
            }
            fs::File::create(p).expect("create log file")
        });
        let now = Instant::now();
        let mut h = Self {
            dir: args.log.as_ref().and_then(|p| p.parent().map(Into::into)),
            log,
            control: args.control.clone(),
            start: now,
            frame: 0,
            control_rx: None,
            last_frame_ms: Arc::new(AtomicU64::new(0)),
            commands: VecDeque::new(),
            names: HashMap::new(),
            prev_names: HashMap::new(),
            focused: None,
            clicked_this_frame: false,
            tab_this_frame: false,
            shots: VecDeque::new(),
            shot_in_flight: None,
            shot_seq: 0,
            note: None,
            marker_seq: 0,
            dt_ms: 0.0,
            perf_since: now,
            perf_frames: 0,
            perf_sum_ms: 0.0,
            perf_worst_ms: 0.0,
            gestures: HashMap::new(),
        };
        // Keep commands already in the control file: they set up the opening scenario.
        if let Some(control) = &h.control
            && !control.exists()
        {
            fs::write(control, "").expect("create control file");
        }
        h.event(
            "session_start",
            json!({ "args": std::env::args().skip(1).collect::<Vec<_>>() }),
        );
        h
    }

    /// Write one event line. `fields` must be a JSON object (or null).
    pub fn event(&mut self, kind: &str, fields: Value) {
        if let Some(file) = &mut self.log {
            write_event(file, self.start, kind, fields);
        }
    }

    /// Name a widget for the log; logs clicks and right-clicks on it.
    pub fn track(&mut self, response: &Response, name: impl Into<String>) {
        let name = name.into();
        if response.clicked() {
            self.event("click", json!({ "widget": name }));
        }
        if response.secondary_clicked() {
            self.event("right_click", json!({ "widget": name }));
        }
        self.names.insert(response.id, name);
    }

    pub fn screenshot(&mut self, label: impl Into<String>) {
        if self.dir.is_some() {
            // Wait out scroll animations and egui's multi-pass layout settling.
            self.shots.push_back(PendingShot {
                label: label.into(),
                not_before: Instant::now() + Duration::from_millis(350),
            });
        }
    }

    /// Feed a scroll area's offset each frame; one `scroll` event per finished gesture.
    pub fn scroll(&mut self, area: &str, offset_y: f32, row_height: f32) {
        let now = Instant::now();
        let dt = self.dt_ms;
        let g = self.gestures.entry(area.to_owned()).or_insert(Gesture {
            moving: false,
            start_offset: offset_y,
            last_offset: offset_y,
            started: now,
            last_change: now,
            worst_ms: 0.0,
        });
        if (g.last_offset - offset_y).abs() > 0.5 {
            if !g.moving {
                g.moving = true;
                g.start_offset = g.last_offset;
                g.started = now;
                g.worst_ms = 0.0;
            }
            g.last_offset = offset_y;
            g.last_change = now;
            g.worst_ms = g.worst_ms.max(dt);
        } else if g.moving && now.duration_since(g.last_change) > Duration::from_millis(300) {
            g.moving = false;
            let fields = json!({
                "area": area,
                "rows_moved": ((g.last_offset - g.start_offset) / row_height).round(),
                "duration_ms": g.last_change.duration_since(g.started).as_millis(),
                "worst_frame_ms": g.worst_ms,
            });
            self.event("scroll", fields);
        }
    }

    fn name_of(&self, id: Option<Id>) -> Value {
        id.map_or(Value::Null, |id| {
            let name = self.names.get(&id).or_else(|| self.prev_names.get(&id));
            name.map_or_else(|| json!(format!("unnamed {id:?}")), |n| json!(n))
        })
    }

    /// Call first in `ui()`. Returns at most one control command to apply this frame.
    pub fn begin_frame(&mut self, ctx: &Context) -> Option<String> {
        self.frame += 1;
        let now_ms = u64::try_from(self.start.elapsed().as_millis()).unwrap_or(u64::MAX);
        self.last_frame_ms.store(now_ms, Ordering::Relaxed);
        if self.control_rx.is_none()
            && let Some(path) = self.control.clone()
        {
            let log = self.log.as_ref().and_then(|f| f.try_clone().ok());
            self.control_rx = Some(spawn_control_reader(
                path,
                ctx.clone(),
                log,
                self.start,
                Arc::clone(&self.last_frame_ms),
            ));
        }
        self.prev_names = std::mem::take(&mut self.names);
        self.dt_ms = ctx.input(|i| i.unstable_dt) * 1000.0;
        let field = self.name_of(self.focused);

        let events = ctx.input(|i| i.events.clone());
        for e in events {
            match e {
                egui::Event::Key {
                    key,
                    pressed: true,
                    repeat,
                    modifiers,
                    ..
                } => {
                    if key == Key::F2 && self.note.is_none() {
                        self.note = Some(String::new());
                        continue;
                    }
                    if key == Key::F3 {
                        self.marker_seq += 1;
                        let n = self.marker_seq;
                        self.event("marker", json!({ "n": n }));
                        self.screenshot(format!("marker-{n}"));
                        continue;
                    }
                    if key == Key::Tab {
                        self.tab_this_frame = true;
                    }
                    let logged = matches!(
                        key,
                        Key::Tab
                            | Key::Enter
                            | Key::Escape
                            | Key::ArrowUp
                            | Key::ArrowDown
                            | Key::ArrowLeft
                            | Key::ArrowRight
                            | Key::Backspace
                            | Key::Delete
                            | Key::PageUp
                            | Key::PageDown
                            | Key::Home
                            | Key::End
                    ) || modifiers.command;
                    if logged && self.note.is_none() {
                        self.event(
                            "key",
                            json!({ "key": key.name(), "repeat": repeat, "ctrl": modifiers.command,
                                    "shift": modifiers.shift, "alt": modifiers.alt, "field": field }),
                        );
                    }
                }
                egui::Event::Text(text) if self.note.is_none() => {
                    self.event("type", json!({ "text": text, "field": field }));
                }
                egui::Event::PointerButton { pressed: true, .. } => self.clicked_this_frame = true,
                // An unfocused or hidden window gets throttled (Wayland stops frame callbacks), and
                // keystrokes typed elsewhere can land here; both show up next to this event.
                egui::Event::WindowFocused(focused) => {
                    self.event("window_focus", json!({ "focused": focused }))
                }
                egui::Event::Screenshot { image, .. } => self.save_shot(&image),
                _ => {}
            }
        }

        if self.perf_since.elapsed() >= Duration::from_secs(1) {
            let secs = self.perf_since.elapsed().as_secs_f32();
            let frames = self.perf_frames;
            let fields = json!({
                "fps": (frames as f32 / secs).round(),
                "mean_frame_ms": if frames == 0 { 0.0 } else { self.perf_sum_ms / frames as f32 },
                "worst_frame_ms": self.perf_worst_ms,
            });
            if frames > 1 {
                self.event("perf", fields);
            }
            self.perf_since = Instant::now();
            self.perf_frames = 0;
            self.perf_sum_ms = 0.0;
            self.perf_worst_ms = 0.0;
        }
        self.perf_frames += 1;
        self.perf_sum_ms += self.dt_ms;
        self.perf_worst_ms = self.perf_worst_ms.max(self.dt_ms);

        self.poll_control();
        if self.shots.is_empty() && self.shot_in_flight.is_none() {
            let cmd = self.commands.pop_front();
            if let Some(cmd) = &cmd {
                self.event("control", json!({ "command": cmd }));
            }
            cmd
        } else {
            None
        }
    }

    fn poll_control(&mut self) {
        if let Some(rx) = &self.control_rx {
            self.commands.extend(rx.try_iter());
        }
    }

    fn save_shot(&mut self, image: &egui::ColorImage) {
        let (Some(dir), Some(label)) = (&self.dir, self.shot_in_flight.take()) else {
            return;
        };
        self.shot_seq += 1;
        let path = dir.join(format!("{:03}-{label}.png", self.shot_seq));
        let bytes: Vec<u8> = image.pixels.iter().flat_map(|c| c.to_array()).collect();
        let [w, h] = image.size;
        let saved = image::save_buffer(
            &path,
            &bytes,
            w as u32,
            h as u32,
            image::ExtendedColorType::Rgba8,
        );
        let path = path.display().to_string();
        match saved {
            Ok(()) => self.event("screenshot", json!({ "label": label, "path": path })),
            Err(e) => self.event(
                "screenshot_failed",
                json!({ "label": label, "error": e.to_string() }),
            ),
        }
    }

    /// Call last in `ui()`.
    pub fn end_frame(&mut self, ctx: &Context) {
        let now = ctx.memory(|m| m.focused());
        if now != self.focused {
            let cause = if self.clicked_this_frame {
                "click"
            } else if self.tab_this_frame {
                "tab"
            } else {
                "app"
            };
            if self.focused.is_some() {
                let w = self.name_of(self.focused);
                self.event("focus_lost", json!({ "widget": w, "cause": cause }));
            }
            if now.is_some() {
                let w = self.name_of(now);
                self.event("focus_gained", json!({ "widget": w, "cause": cause }));
            }
            self.focused = now;
        }
        self.clicked_this_frame = false;
        self.tab_this_frame = false;

        self.note_box(ctx);

        if self.shot_in_flight.is_none()
            && self
                .shots
                .front()
                .is_some_and(|s| s.not_before <= Instant::now())
            && let Some(shot) = self.shots.pop_front()
        {
            self.shot_in_flight = Some(shot.label);
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default()));
        }

        if !self.shots.is_empty() || self.shot_in_flight.is_some() || !self.commands.is_empty() {
            ctx.request_repaint();
        } else if self.log.is_some() || self.control.is_some() {
            ctx.request_repaint_after(Duration::from_millis(250));
        }
    }

    fn note_box(&mut self, ctx: &Context) {
        let Some(mut text) = self.note.take() else {
            return;
        };
        let mut done = None;
        egui::Area::new(Id::new("harness_note"))
            .anchor(Align2::CENTER_TOP, [0.0, 60.0])
            .order(egui::Order::Tooltip)
            .show(ctx, |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label("Note (F2):");
                        let r = ui.add(egui::TextEdit::singleline(&mut text).desired_width(420.0));
                        r.request_focus();
                        if ui.input(|i| i.key_pressed(Key::Enter)) {
                            done = Some(true);
                        } else if ui.input(|i| i.key_pressed(Key::Escape)) {
                            done = Some(false);
                        }
                    });
                });
            });
        match done {
            Some(true) => {
                self.event("note", json!({ "text": text }));
                self.screenshot("note");
            }
            Some(false) => {}
            None => self.note = Some(text),
        }
    }
}

fn write_event(file: &mut fs::File, start: Instant, kind: &str, fields: Value) {
    let ts_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis());
    let mut obj = json!({
        "ts_ms": ts_ms,
        "t": (start.elapsed().as_secs_f64() * 1000.0).round() / 1000.0,
        "event": kind,
    });
    if let (Some(o), Value::Object(f)) = (obj.as_object_mut(), fields) {
        o.extend(f);
    }
    let _ = writeln!(file, "{obj}");
    let _ = file.flush();
}

/// Read the control file off the UI thread, so commands arrive even while the window gets no
/// frames. Commands go to the app in order; `quit` also arms a fallback that exits the process
/// when no frame has run for 2 s (a throttled window would never apply it).
fn spawn_control_reader(
    path: PathBuf,
    ctx: Context,
    mut log: Option<fs::File>,
    start: Instant,
    last_frame_ms: Arc<AtomicU64>,
) -> mpsc::Receiver<String> {
    let (tx, rx) = mpsc::channel();
    std::thread::Builder::new()
        .name("prototype-control".to_owned())
        .spawn(move || {
            let mut quit_requested = false;
            loop {
                std::thread::sleep(Duration::from_millis(250));
                if quit_requested {
                    let now_ms = u64::try_from(start.elapsed().as_millis()).unwrap_or(u64::MAX);
                    if now_ms.saturating_sub(last_frame_ms.load(Ordering::Relaxed)) > 2_000 {
                        if let Some(file) = &mut log {
                            write_event(
                                file,
                                start,
                                "session_end",
                                json!({ "via": "control thread, no frames" }),
                            );
                        }
                        #[allow(
                            clippy::disallowed_methods,
                            reason = "the window gets no frames, so it cannot close itself"
                        )]
                        std::process::exit(0);
                    }
                }
                let Ok(text) = fs::read_to_string(&path) else {
                    continue;
                };
                if text.trim().is_empty() {
                    continue;
                }
                let _ = fs::write(&path, "");
                for line in text.lines().map(str::trim).filter(|l| !l.is_empty()) {
                    quit_requested |= line == "quit";
                    if tx.send(line.to_owned()).is_err() {
                        return;
                    }
                }
                ctx.request_repaint();
            }
        })
        .expect("spawn the control reader thread");
    rx
}
