use std::collections::{HashMap, VecDeque};
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use egui::{
    Align2, Color32, Context, Id, Key, LayerId, Mesh, Order, Rect, Response, RichText, Stroke,
    StrokeKind, Ui,
};
use serde_json::{Value, json};

use crate::Args;
use crate::control::ControlReader;
use crate::log::write_event;
use crate::prompt::PromptCommand;
use crate::spotlight::{Spotlight, SpotlightCommand, outermost, overlay_rects};

/// Screenshots wait this long so scroll animations and egui's multi-pass layout settle.
const SHOT_SETTLE: Duration = Duration::from_millis(350);
/// A scroll gesture ends after this long without movement.
const SCROLL_REST: Duration = Duration::from_millis(300);
/// How long the prompt panel flashes after its text changes, so the change is noticed.
const PROMPT_FLASH: Duration = Duration::from_millis(1600);
/// How many times it pulses in that time.
const PROMPT_PULSES: f32 = 2.0;
/// How far a spotlight cut-out extends past the widget or region it lights.
const SPOTLIGHT_PAD: f32 = 4.0;

/// A widget named through [`Harness::track`], and where it was drawn this frame.
#[derive(Debug)]
struct Tracked {
    name: String,
    rect: Rect,
}

/// The spotlight and the bookkeeping for painting and logging it.
#[derive(Debug, Default)]
struct SpotlightState {
    current: Spotlight,
    /// A change made this frame, logged once `end_frame` knows what it matches.
    change: Option<SpotlightCause>,
    /// Whether "nothing matches" has been logged since the last change.
    unmatched_logged: bool,
    /// The panel `dock_spotlight` drew in this frame, which the overlay leaves undimmed.
    dock_rect: Option<Rect>,
}

/// The agent's current instruction and the panel [`Harness::prompt_panel`] draws it in.
#[derive(Debug, Default)]
struct PromptState {
    text: Option<String>,
    /// Whether the user put the panel away (F6 or the dock checkbox).
    hidden: bool,
    /// The panel drawn this frame, which the spotlight leaves undimmed.
    rect: Option<Rect>,
    /// When the text last changed, which starts the flash.
    changed_at: Option<Instant>,
}

/// What changed the spotlight, as logged.
#[derive(Debug, Clone, Copy)]
enum SpotlightCause {
    Control,
    Key,
    Click,
}

impl SpotlightCause {
    fn as_str(self) -> &'static str {
        match self {
            Self::Control => "control",
            Self::Key => "key",
            Self::Click => "click",
        }
    }
}

#[derive(Debug)]
struct Gesture {
    moving: bool,
    start_offset: f32,
    last_offset: f32,
    started: Instant,
    last_change: Instant,
    worst_ms: f32,
}

#[derive(Debug)]
struct PendingShot {
    label: String,
    not_before: Instant,
}

#[derive(Debug, Default)]
struct Perf {
    frames: u32,
    sum_ms: f64,
    worst_ms: f32,
}

/// The observation harness. See the crate docs for the app's side of the contract.
#[derive(Debug)]
pub struct Harness {
    log: Option<fs::File>,
    dir: Option<PathBuf>,
    control: Option<PathBuf>,
    control_rx: Option<mpsc::Receiver<String>>,
    /// Milliseconds since `start` at the last frame; the control thread reads it.
    last_frame_ms: Arc<AtomicU64>,
    commands: VecDeque<String>,
    start: Instant,
    names: HashMap<Id, Tracked>,
    prev_names: HashMap<Id, Tracked>,
    /// Regions named through [`Harness::region`] this frame.
    regions: Vec<(String, Rect)>,
    spotlight: SpotlightState,
    prompt: PromptState,
    /// Whether a text field had keyboard focus at the start of this frame.
    typing: bool,
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
    perf: Perf,
    gestures: HashMap<String, Gesture>,
}

impl Harness {
    /// Open the log (truncating it) and create the control file if it doesn't exist. Commands
    /// already in the control file are kept: they set up the opening scenario.
    pub fn new(args: &Args) -> Self {
        let log = args.log.as_ref().map(|p| {
            if let Some(parent) = p.parent() {
                fs::create_dir_all(parent).expect("create the log directory");
            }
            fs::File::create(p).expect("create the log file")
        });
        if let Some(control) = &args.control
            && !control.exists()
        {
            fs::write(control, "").expect("create the control file");
        }
        let now = Instant::now();
        let mut harness = Self {
            log,
            dir: args.log.as_ref().and_then(|p| p.parent().map(Into::into)),
            control: args.control.clone(),
            control_rx: None,
            last_frame_ms: Arc::new(AtomicU64::new(0)),
            commands: VecDeque::new(),
            start: now,
            names: HashMap::new(),
            prev_names: HashMap::new(),
            regions: Vec::new(),
            spotlight: SpotlightState::default(),
            prompt: PromptState::default(),
            typing: false,
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
            perf: Perf::default(),
            gestures: HashMap::new(),
        };
        harness.event(
            "session_start",
            json!({ "args": std::env::args().skip(1).collect::<Vec<_>>() }),
        );
        harness
    }

    /// Write one event line. `fields` is a JSON object (or `null`).
    pub fn event(&mut self, kind: &str, fields: Value) {
        if let Some(file) = &mut self.log {
            write_event(file, self.start, kind, fields);
        }
    }

    /// Name a widget for the log and the spotlight, and log clicks and right-clicks on it.
    pub fn track(&mut self, response: &Response, name: impl Into<String>) {
        let name = name.into();
        if response.clicked() {
            self.event("click", json!({ "widget": name }));
        }
        if response.secondary_clicked() {
            self.event("right_click", json!({ "widget": name }));
        }
        let rect = response.rect.intersect(response.interact_rect);
        self.names.insert(response.id, Tracked { name, rect });
    }

    /// Name a region larger than one widget (`"register"`, `"account bar"`) for the spotlight.
    /// Call it every frame the region is drawn, with its screen rect.
    pub fn region(&mut self, rect: Rect, name: impl Into<String>) {
        self.regions.push((name.into(), rect));
    }

    /// Draw the dock's spotlight row: a "Spotlight (F5)" checkbox and the current targets.
    /// The panel it is drawn in stays undimmed while the spotlight is on.
    pub fn dock_spotlight(&mut self, ui: &mut Ui) {
        self.spotlight.dock_rect = Some(ui.clip_rect());
        let mut on = self.spotlight.current.is_on();
        let response = ui.checkbox(&mut on, "Spotlight (F5)");
        self.track(&response, "dock: spotlight");
        if response.changed() {
            self.set_spotlight_on(on, SpotlightCause::Click);
        }
        let targets = if self.spotlight.current.targets().is_empty() {
            "no targets".to_owned()
        } else {
            self.spotlight.current.targets().join("; ")
        };
        ui.label(RichText::new(targets).small().weak());
    }

    /// Draw the agent's current `prompt` text in a panel along the bottom of the window, or
    /// nothing while there is none. Call it before the app's `CentralPanel`, so the app's content
    /// sits above it rather than under it. The panel stays undimmed while the spotlight is on.
    pub fn prompt_panel(&mut self, ui: &mut Ui) {
        let Some(text) = self.prompt.text.clone().filter(|_| !self.prompt.hidden) else {
            return;
        };
        let visuals = ui.visuals().clone();
        let base = visuals
            .selection
            .bg_fill
            .gamma_multiply(0.25)
            .blend(visuals.panel_fill);
        let flash = self.prompt_flash();
        if flash > 0.0 {
            ui.ctx().request_repaint();
        }
        let frame = egui::Frame::side_top_panel(ui.style())
            .fill(base.lerp_to_gamma(visuals.warn_fg_color, flash * 0.6))
            .stroke(Stroke::new(
                1.0 + flash * 2.0,
                visuals
                    .selection
                    .stroke
                    .color
                    .lerp_to_gamma(visuals.warn_fg_color, flash),
            ));
        egui::Panel::bottom("harness_prompt")
            .resizable(false)
            .frame(frame)
            .show(ui, |ui| {
                self.prompt.rect = Some(ui.clip_rect());
                ui.horizontal(|ui| {
                    ui.label(RichText::new(text).size(16.0).strong());
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(RichText::new("F2 reply · F6 hide").small().weak());
                    });
                });
            });
    }

    /// The flash strength now, from 1 just after the text changed down to 0 once it settles.
    fn prompt_flash(&self) -> f32 {
        let Some(since) = self.prompt.changed_at.map(|t| t.elapsed()) else {
            return 0.0;
        };
        if since >= PROMPT_FLASH {
            return 0.0;
        }
        let progress = since.as_secs_f32() / PROMPT_FLASH.as_secs_f32();
        let pulse = 0.5 + 0.5 * (progress * PROMPT_PULSES * std::f32::consts::TAU).cos();
        pulse * (1.0 - progress)
    }

    /// Draw the dock's prompt row: a "Prompt (F6)" checkbox that puts the prompt panel away or
    /// brings it back.
    pub fn dock_prompt(&mut self, ui: &mut Ui) {
        let mut shown = !self.prompt.hidden;
        let response = ui.checkbox(&mut shown, "Prompt (F6)");
        self.track(&response, "dock: prompt");
        if response.changed() {
            self.set_prompt_shown(shown, "click");
        }
    }

    fn set_prompt_shown(&mut self, shown: bool, cause: &str) {
        self.prompt.hidden = !shown;
        self.event("prompt_shown", json!({ "shown": shown, "cause": cause }));
    }

    /// Queue a screenshot of the window, saved beside the log as `<seq>-<label>.png`.
    pub fn screenshot(&mut self, label: impl Into<String>) {
        if self.dir.is_some() {
            self.shots.push_back(PendingShot {
                label: label.into(),
                not_before: Instant::now() + SHOT_SETTLE,
            });
        }
    }

    /// Feed a scroll area's offset each frame; logs one `scroll` event per finished gesture.
    pub fn scroll(&mut self, area: &str, offset_y: f32, row_height: f32) {
        let now = Instant::now();
        let dt = self.dt_ms;
        let gesture = self
            .gestures
            .entry(area.to_owned())
            .or_insert_with(|| Gesture {
                moving: false,
                start_offset: offset_y,
                last_offset: offset_y,
                started: now,
                last_change: now,
                worst_ms: 0.0,
            });
        if (gesture.last_offset - offset_y).abs() > 0.5 {
            if !gesture.moving {
                gesture.moving = true;
                gesture.start_offset = gesture.last_offset;
                gesture.started = now;
                gesture.worst_ms = 0.0;
            }
            gesture.last_offset = offset_y;
            gesture.last_change = now;
            gesture.worst_ms = gesture.worst_ms.max(dt);
        } else if gesture.moving && now.duration_since(gesture.last_change) > SCROLL_REST {
            gesture.moving = false;
            let fields = json!({
                "area": area,
                "rows_moved": ((gesture.last_offset - gesture.start_offset) / row_height).round(),
                "duration_ms": gesture.last_change.duration_since(gesture.started).as_millis(),
                "worst_frame_ms": gesture.worst_ms,
            });
            self.event("scroll", fields);
        }
    }

    /// Call first in `ui()`. Returns at most one control command for the app to apply; the
    /// harness applies its own `spotlight` commands and returns `None` for them.
    pub fn begin_frame(&mut self, ctx: &Context) -> Option<String> {
        let now_ms = u64::try_from(self.start.elapsed().as_millis()).unwrap_or(u64::MAX);
        self.last_frame_ms.store(now_ms, Ordering::Relaxed);
        self.start_control_reader(ctx);
        // Swapping keeps both maps' capacity, so thousands of tracked cells don't reallocate.
        std::mem::swap(&mut self.prev_names, &mut self.names);
        self.names.clear();
        self.regions.clear();
        self.spotlight.dock_rect = None;
        self.prompt.rect = None;
        self.typing = ctx.egui_wants_keyboard_input();
        self.dt_ms = ctx.input(|i| i.unstable_dt) * 1000.0;

        let events = ctx.input(|i| i.events.clone());
        for event in events {
            self.on_input(event);
        }
        self.sample_perf();

        if let Some(rx) = &self.control_rx {
            self.commands.extend(rx.try_iter());
        }
        if self.shots.is_empty() && self.shot_in_flight.is_none() {
            let command = self.commands.pop_front();
            if let Some(command) = &command {
                self.event("control", json!({ "command": command }));
            }
            if let Some(spotlight) = command.as_deref().and_then(SpotlightCommand::parse) {
                self.spotlight.current.apply(spotlight);
                self.spotlight.change = Some(SpotlightCause::Control);
                return None;
            }
            if let Some(prompt) = command.as_deref().and_then(PromptCommand::parse) {
                self.prompt.text = match prompt {
                    PromptCommand::Clear => None,
                    PromptCommand::Show(text) => Some(text),
                };
                // A new instruction brings a put-away panel back, so it isn't missed.
                self.prompt.hidden = false;
                self.prompt.changed_at = Some(Instant::now());
                self.event("prompt", json!({ "text": self.prompt.text }));
                return None;
            }
            command
        } else {
            None
        }
    }

    /// Call last in `ui()`.
    pub fn end_frame(&mut self, ctx: &Context) {
        self.log_focus_change(ctx);
        self.paint_spotlight(ctx);
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
        } else if self.log.is_some() {
            ctx.request_repaint_after(Duration::from_millis(250));
        }
    }

    fn start_control_reader(&mut self, ctx: &Context) {
        if self.control_rx.is_some() {
            return;
        }
        let Some(path) = self.control.clone() else {
            return;
        };
        let reader = ControlReader {
            path,
            ctx: ctx.clone(),
            log: self.log.as_ref().and_then(|f| f.try_clone().ok()),
            start: self.start,
            last_frame_ms: Arc::clone(&self.last_frame_ms),
        };
        self.control_rx = Some(reader.spawn());
    }

    fn name_of(&self, id: Option<Id>) -> Value {
        id.map_or(Value::Null, |id| {
            let tracked = self.names.get(&id).or_else(|| self.prev_names.get(&id));
            tracked.map_or_else(|| json!(format!("unnamed {id:?}")), |t| json!(t.name))
        })
    }

    #[expect(clippy::wildcard_enum_match_arm, reason = "foreign enum")]
    fn on_input(&mut self, event: egui::Event) {
        match event {
            egui::Event::Key {
                key,
                pressed: true,
                repeat,
                modifiers,
                ..
            } => self.on_key(key, repeat, modifiers),
            egui::Event::Text(text) if self.note.is_none() => {
                let field = self.name_of(self.focused);
                self.event("type", json!({ "text": text, "field": field }));
            }
            egui::Event::PointerButton { pressed: true, .. } => self.clicked_this_frame = true,
            // A covered or unfocused window gets throttled (Wayland stops frame callbacks), and
            // keystrokes typed elsewhere can land here; both show up next to this event.
            egui::Event::WindowFocused(focused) => {
                self.event("window_focus", json!({ "focused": focused }));
            }
            egui::Event::Screenshot { image, .. } => self.save_shot(&image),
            _ => {}
        }
    }

    fn on_key(&mut self, key: Key, repeat: bool, modifiers: egui::Modifiers) {
        if key == Key::F2 && self.note.is_none() {
            self.note = Some(String::new());
            return;
        }
        if key == Key::F3 {
            self.marker_seq += 1;
            let n = self.marker_seq;
            self.event("marker", json!({ "n": n }));
            self.screenshot(format!("marker-{n}"));
            return;
        }
        if key == Key::F5 && !repeat && !self.typing && self.note.is_none() {
            self.set_spotlight_on(!self.spotlight.current.is_on(), SpotlightCause::Key);
            return;
        }
        if key == Key::F6 && !repeat && !self.typing && self.note.is_none() {
            self.set_prompt_shown(self.prompt.hidden, "key");
            return;
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
            let field = self.name_of(self.focused);
            self.event(
                "key",
                json!({ "key": key.name(), "repeat": repeat, "ctrl": modifiers.command,
                        "shift": modifiers.shift, "alt": modifiers.alt, "field": field }),
            );
        }
    }

    fn set_spotlight_on(&mut self, on: bool, cause: SpotlightCause) {
        self.spotlight.current.set_on(on);
        self.spotlight.change = Some(cause);
    }

    /// The screen rects of this frame's tracked widgets and regions that the spotlight matches,
    /// leaving out any that lie inside another match.
    fn spotlight_matches(&self, screen: Rect) -> Vec<Rect> {
        let widgets = self.names.values().map(|t| (t.name.as_str(), t.rect));
        let regions = self
            .regions
            .iter()
            .map(|(name, rect)| (name.as_str(), *rect));
        let matches: Vec<Rect> = widgets
            .chain(regions)
            .filter(|(name, rect)| rect.intersects(screen) && self.spotlight.current.matches(name))
            .map(|(_, rect)| rect)
            .collect();
        outermost(&matches)
    }

    /// Log a spotlight change made this frame, then dim everything but the matches.
    fn paint_spotlight(&mut self, ctx: &Context) {
        let change = self.spotlight.change.take();
        if change.is_none() && !self.spotlight.current.is_on() {
            return;
        }
        let screen = ctx.content_rect();
        let lit = self.spotlight_matches(screen);
        let on = self.spotlight.current.is_on();
        if let Some(cause) = change {
            let fields = json!({
                "on": on,
                "targets": self.spotlight.current.targets(),
                "matched": lit.len(),
                "cause": cause.as_str(),
            });
            self.event("spotlight", fields);
            self.screenshot(if on { "spotlight-on" } else { "spotlight-off" });
            self.spotlight.unmatched_logged = false;
        }
        if !on || self.spotlight.current.targets().is_empty() {
            return;
        }
        if lit.is_empty() {
            if !self.spotlight.unmatched_logged {
                self.spotlight.unmatched_logged = true;
                let fields = json!({ "targets": self.spotlight.current.targets() });
                self.event("spotlight_unmatched", fields);
            }
            return;
        }

        let lit: Vec<Rect> = lit.iter().map(|r| r.expand(SPOTLIGHT_PAD)).collect();
        let holes: Vec<Rect> = lit
            .iter()
            .copied()
            .chain(self.spotlight.dock_rect)
            .chain(self.prompt.rect)
            .collect();
        // The overlay must darken in light and dark themes alike, so it isn't a theme colour.
        let shade = Color32::from_black_alpha(140);
        // One unfeathered mesh, so neighbouring rects meet without seams.
        let mut mesh = Mesh::default();
        for rect in overlay_rects(screen, &holes) {
            mesh.add_colored_rect(rect, shade);
        }
        let painter = ctx.layer_painter(LayerId::new(
            Order::Foreground,
            Id::new("harness_spotlight"),
        ));
        painter.add(mesh);
        let stroke = Stroke::new(2.0, ctx.global_style().visuals.selection.stroke.color);
        for rect in lit {
            painter.rect_stroke(rect, 0.0, stroke, StrokeKind::Outside);
        }
    }

    fn sample_perf(&mut self) {
        if self.perf_since.elapsed() >= Duration::from_secs(1) {
            let secs = self.perf_since.elapsed().as_secs_f64();
            let Perf {
                frames,
                sum_ms,
                worst_ms,
            } = std::mem::take(&mut self.perf);
            if frames > 1 {
                let frames = f64::from(frames);
                self.event(
                    "perf",
                    json!({
                        "fps": (frames / secs).round(),
                        "mean_frame_ms": sum_ms / frames,
                        "worst_frame_ms": worst_ms,
                    }),
                );
            }
            self.perf_since = Instant::now();
        }
        self.perf.frames += 1;
        self.perf.sum_ms += f64::from(self.dt_ms);
        self.perf.worst_ms = self.perf.worst_ms.max(self.dt_ms);
    }

    fn log_focus_change(&mut self, ctx: &Context) {
        let now = ctx.memory(egui::Memory::focused);
        if now != self.focused {
            let cause = if self.clicked_this_frame {
                "click"
            } else if self.tab_this_frame {
                "tab"
            } else {
                "app"
            };
            if self.focused.is_some() {
                let widget = self.name_of(self.focused);
                self.event("focus_lost", json!({ "widget": widget, "cause": cause }));
            }
            if now.is_some() {
                let widget = self.name_of(now);
                self.event("focus_gained", json!({ "widget": widget, "cause": cause }));
            }
            self.focused = now;
        }
        self.clicked_this_frame = false;
        self.tab_this_frame = false;
    }

    fn save_shot(&mut self, image: &egui::ColorImage) {
        let (Some(dir), Some(label)) = (&self.dir, self.shot_in_flight.take()) else {
            return;
        };
        self.shot_seq += 1;
        let path = dir.join(format!("{:03}-{label}.png", self.shot_seq));
        let bytes: Vec<u8> = image
            .pixels
            .iter()
            .flat_map(egui::Color32::to_array)
            .collect();
        let [width, height] = image.size;
        let saved = match (u32::try_from(width), u32::try_from(height)) {
            (Ok(w), Ok(h)) => {
                image::save_buffer(&path, &bytes, w, h, image::ExtendedColorType::Rgba8)
                    .map_err(|e| e.to_string())
            }
            _ => Err("screenshot larger than u32".to_owned()),
        };
        let path = path.display().to_string();
        match saved {
            Ok(()) => self.event("screenshot", json!({ "label": label, "path": path })),
            Err(error) => self.event(
                "screenshot_failed",
                json!({ "label": label, "error": error }),
            ),
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
                        let field =
                            ui.add(egui::TextEdit::singleline(&mut text).desired_width(420.0));
                        field.request_focus();
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
