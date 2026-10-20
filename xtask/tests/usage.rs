// SPDX-License-Identifier: GPL-3.0-or-later WITH AdditionRef-Balanced-Plugin-Exception

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
  ci                                 run fmt, clippy, test, snapshots, locales, deny, layers, typos, todo, spdx, adrs and dco, stopping at the first failure
  fmt                                check formatting
  clippy                             lint with warnings denied
  test                               run every test except the image snapshots
  snapshots                          check the image snapshots, on Linux only; UPDATE_SNAPSHOTS=1 rewrites failing ones
  locales                            report untranslated messages per locale
  deny                               check licences, advisories, duplicate versions and layer bans
  layers                             require workspace crates to depend only on the layers below them
  typos                              spell-check the repo
  todo                               require every TODO and FIXME to name an issue, as TODO(#123)
  spdx                               require every Rust source file to open with the SPDX license header
  adrs [--against <ref>]             require every decision record to have its own number
  dco                                require a Signed-off-by trailer on every non-merge commit since the DCO began
  mutants [<base>] [--re <pattern>]  mutation-test code changed since <base> (default origin/main), or with --re alone every mutant it matches; advisory
  mutants-shard <shard> [--dry-run]  a CI task: mutation-test one shard (0 to 6) of every mutant and update the mutant issues, or with --dry-run print the updates; by hand, use mutants
  mutants-landed [--dry-run]         a CI task: mutation-test the diff the pull request that landed HEAD brought into main and update the mutant issues, or with --dry-run print the updates; by hand, use mutants
"
    );
}
