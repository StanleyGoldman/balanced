use anyhow::Result;

use crate::{Task, process};

pub(crate) const TASK: Task = Task {
    name: "deny",
    args: "",
    summary: "check licences, advisories, duplicate versions and layer bans",
    run: |args| args.is_empty().then(run),
};

pub(crate) fn run() -> Result<()> {
    process::run("cargo", &["deny", "--locked", "check"])
}
