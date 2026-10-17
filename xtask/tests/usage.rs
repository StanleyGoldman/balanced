//! `cargo xtask` with no task lists every task.

use std::process::Command;

#[test]
fn no_task_prints_the_usage_and_fails() {
    let output = Command::new(env!("CARGO_BIN_EXE_xtask"))
        .output()
        .expect("the xtask binary runs");

    assert!(!output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stderr),
        "\
usage: cargo xtask <task>

tasks:
  ci                run fmt, clippy, test, snapshots, locales, deny, typos and todo, stopping at the first failure
  fmt               check formatting
  clippy            lint with warnings denied
  test              run every test except the image snapshots
  snapshots         check the image snapshots, on Linux only; UPDATE_SNAPSHOTS=1 rewrites failing ones
  locales           report untranslated messages per locale
  deny              check licences, advisories, duplicate versions and layer bans
  typos             spell-check the repo
  todo              require every TODO and FIXME to name an issue, as TODO(#123)
  mutants [<base>]  mutation-test code changed since <base> (default origin/main); advisory
"
    );
}
