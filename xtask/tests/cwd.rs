// SPDX-License-Identifier: GPL-3.0-or-later WITH AdditionRef-Balanced-Plugin-Exception

//! `cargo xtask` behaves the same from any directory in the workspace.

use std::process::Command;

#[test]
fn todo_check_passes_from_a_subdirectory() {
    let subdirectory = env!("CARGO_MANIFEST_DIR");

    let output = Command::new(env!("CARGO_BIN_EXE_xtask"))
        .arg("todo")
        .current_dir(subdirectory)
        .output()
        .expect("the xtask binary runs");

    // From the workspace root the repo is clean, so the same run from `xtask/` must
    // pass too, rather than scanning only `xtask/` and missing its own exclusion.
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
