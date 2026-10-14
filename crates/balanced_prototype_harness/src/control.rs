use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use egui::Context;
use serde_json::json;

use crate::log::write_event;

const POLL: Duration = Duration::from_millis(250);
/// How long without a frame before `quit` exits the process from the reader thread.
const NO_FRAME_QUIT_MS: u64 = 2_000;

/// Reads the control file off the UI thread, so commands arrive while the window draws no frames.
#[derive(Debug)]
pub(crate) struct ControlReader {
    pub(crate) path: PathBuf,
    pub(crate) ctx: Context,
    pub(crate) log: Option<fs::File>,
    pub(crate) start: Instant,
    /// Milliseconds since `start` at the app's last frame.
    pub(crate) last_frame_ms: Arc<AtomicU64>,
}

impl ControlReader {
    /// Start the reader thread. Commands come out of the returned channel in file order; `quit`
    /// also arms a fallback that exits the process once no frame has run for 2 s.
    pub(crate) fn spawn(self) -> mpsc::Receiver<String> {
        let (tx, rx) = mpsc::channel();
        std::thread::Builder::new()
            .name("prototype-control".to_owned())
            .spawn(move || self.run(&tx))
            .expect("spawn the control reader thread");
        rx
    }

    fn run(mut self, tx: &mpsc::Sender<String>) {
        let mut quit_requested = false;
        loop {
            std::thread::sleep(POLL);
            if quit_requested && self.frames_stalled() {
                self.exit_without_frames();
            }
            let Ok(text) = fs::read_to_string(&self.path) else {
                continue;
            };
            if text.trim().is_empty() {
                continue;
            }
            fs::write(&self.path, "").ok();
            for line in text.lines().map(str::trim).filter(|l| !l.is_empty()) {
                quit_requested |= line == "quit";
                if tx.send(line.to_owned()).is_err() {
                    return; // the app is gone
                }
            }
            self.ctx.request_repaint();
        }
    }

    fn frames_stalled(&self) -> bool {
        let now_ms = u64::try_from(self.start.elapsed().as_millis()).unwrap_or(u64::MAX);
        now_ms.saturating_sub(self.last_frame_ms.load(Ordering::Relaxed)) > NO_FRAME_QUIT_MS
    }

    fn exit_without_frames(&mut self) -> ! {
        if let Some(file) = &mut self.log {
            write_event(
                file,
                self.start,
                "session_end",
                json!({ "via": "control thread, no frames" }),
            );
        }
        #[expect(
            clippy::disallowed_methods,
            reason = "the window gets no frames, so it cannot close itself and return from main"
        )]
        std::process::exit(0)
    }
}
