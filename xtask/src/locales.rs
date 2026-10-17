use anyhow::Result;

use crate::{Task, process};

pub(crate) const TASK: Task = Task {
    name: "locales",
    args: "",
    summary: "report untranslated messages per locale",
    run: |args| args.is_empty().then(run),
};

/// Reruns the locale-consistency test uncaptured, so its untranslated counts show.
/// It uses `test`'s features, so it reuses that build.
pub(crate) fn run() -> Result<()> {
    process::run(
        "cargo",
        &[
            "nextest",
            "run",
            "--locked",
            "--all-features",
            "--package",
            "balanced_i18n",
            "--test",
            "locales",
            "--cargo-quiet",
            "--no-capture",
        ],
    )
}
