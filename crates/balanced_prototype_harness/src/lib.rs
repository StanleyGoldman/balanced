//! Observation and remote control for egui prototypes, per `docs/agents/prototypes.md`.
//!
//! A prototype (an example under some crate's `examples/`) takes this crate as a
//! dev-dependency, so the agent can follow the user through the app's own output and drive it:
//!
//! - `--log <path>`: a JSONL event log, one object per line with `ts_ms` (Unix ms, to line up
//!   with chat messages) and `t` (seconds since start). Screenshots land in the same directory as
//!   `<seq>-<label>.png`.
//! - `--control <path>`: a file the agent appends commands to, one per line. A background thread
//!   reads it every 250 ms, empties it (commands already there at launch run first) and wakes the
//!   app, which applies one command per frame, waiting for any screenshot in progress so
//!   `variant b` then `screenshot b` capture the right thing. On Wayland a covered or unfocused
//!   window gets no frames, so commands wait until it is visible; `quit` still lands, because the
//!   thread exits the process itself once no frame has run for 2 s.
//! - `--variant <key>`: the variant to start on.
//! - F2 opens a one-line note box (Enter logs it, Esc cancels); F3 drops a marker and a screenshot;
//!   F5 turns the spotlight on or off for its current targets while no text field has focus.
//!
//! The harness handles one control verb itself and never returns it to the app:
//! `spotlight <target>[; <target>…]` sets the targets and turns the spotlight on, and
//! `spotlight off` / `spotlight on` turn it off or back on without changing the targets. A target
//! lights every tracked widget or named region whose name contains it, ignoring case; targets are
//! separated by `;` because names contain spaces and commas. While the spotlight is on, a
//! translucent overlay dims the window except an outlined cut-out around each match and the panel
//! [`Harness::dock_spotlight`] is drawn in. It only paints, so input still reaches every widget.
//! Each change logs a `spotlight` event (`on`, `targets`, `matched`, `cause`: control, key or
//! click) and takes a screenshot; when nothing on screen matches, nothing is drawn and one
//! `spotlight_unmatched` event is logged per change.
//!
//! The app's side of the contract:
//! - build a [`Harness`] from [`Args::from_env`] before `eframe::run_native`;
//! - call [`Harness::begin_frame`] first in `ui()` and apply the command it returns;
//! - call [`Harness::track`] on every widget worth naming (`"payee cell, row 12"`);
//! - call [`Harness::region`] each frame for larger areas worth spotlighting (`"register"`,
//!   `"account bar"`);
//! - call [`Harness::dock_spotlight`] in the prototype dock;
//! - call [`Harness::event`] for app-level events (variant switch, toggle flip, the Command an
//!   action would emit);
//! - call [`Harness::screenshot`] after a variant switch or toggle flip;
//! - call [`Harness::scroll`] each frame for big scroll areas;
//! - call [`Harness::end_frame`] last, and log `session_end` from `App::on_exit`.
//!
//! The harness knows no app domain: what a command such as `variant b` means is the app's call.

mod args;
mod control;
mod harness;
mod log;
mod spotlight;

pub use args::Args;
pub use harness::Harness;
