use anyhow::Result;

use crate::{Task, process};

pub(crate) const TASK: Task = Task {
    name: "snapshots",
    args: "",
    summary: "check the image snapshots, on Linux only; UPDATE_SNAPSHOTS=1 rewrites failing ones",
    run: |args| args.is_empty().then(run),
};

/// Runs the image snapshot tests, which `test` skips as ignored. Linux with the lavapipe
/// software rasterizer is their source of truth, so elsewhere this skips them.
pub(crate) fn run() -> Result<()> {
    if !cfg!(target_os = "linux") {
        eprintln!("skipped: image snapshots are checked on Linux only");
        return Ok(());
    }
    process::run(
        "cargo",
        &[
            "test",
            "--locked",
            "--package",
            "balanced_ui",
            "--all-features",
            "--test",
            "snapshots",
            "--",
            "--ignored",
        ],
    )
}
