// SPDX-License-Identifier: GPL-3.0-or-later WITH AdditionRef-Balanced-Plugin-Exception

use anyhow::{Context as _, Result, bail};

use crate::{Task, process};

pub(crate) const TASK: Task = Task {
    name: "spdx",
    args: "",
    summary: "require every Rust source file to open with the SPDX license header",
    run: |args| args.is_empty().then(run),
};

/// The first line of every Rust source file. The expression matches `license` in
/// `Cargo.toml`; the exception is `LICENSE-EXCEPTION.md`.
pub(crate) const HEADER: &str =
    "// SPDX-License-Identifier: GPL-3.0-or-later WITH AdditionRef-Balanced-Plugin-Exception";

pub(crate) fn run() -> Result<()> {
    // Untracked files too, so a new file fails before it is staged.
    let files = process::output(
        "git",
        &[
            "ls-files",
            "--cached",
            "--others",
            "--exclude-standard",
            "*.rs",
        ],
    )?;
    let mut missing = Vec::new();
    for path in files.lines() {
        let text = std::fs::read_to_string(path).with_context(|| format!("reading {path}"))?;
        if !has_header(&text) {
            missing.push(path);
        }
    }
    if !missing.is_empty() {
        bail!(
            "make this the first line of each file below:\n{HEADER}\n\n{}",
            missing.join("\n")
        );
    }
    eprintln!("spdx: {} Rust source files checked", files.lines().count());
    Ok(())
}

/// Whether `text` opens with [`HEADER`] as its own first line.
fn has_header(text: &str) -> bool {
    text.lines().next() == Some(HEADER)
}

#[cfg(test)]
mod tests {
    use super::{HEADER, has_header};

    #[test]
    fn a_file_opening_with_the_header_passes() {
        assert!(has_header(&format!("{HEADER}\n\n//! Docs.\n")));
        assert!(has_header(&format!("{HEADER}\r\nfn main() {{}}\r\n")));
        assert!(has_header(HEADER));
    }

    #[test]
    fn a_file_without_the_header_on_its_first_line_fails() {
        assert!(!has_header(""));
        assert!(!has_header("//! Docs.\n"));
        assert!(!has_header(&format!("//! Docs.\n{HEADER}\n")));
        assert!(!has_header(&format!("\n{HEADER}\n")));
    }

    #[test]
    fn another_license_expression_fails() {
        assert!(!has_header("// SPDX-License-Identifier: MIT\n"));
        assert!(!has_header(
            "// SPDX-License-Identifier: GPL-3.0-or-later\n"
        ));
        assert!(!has_header(&format!("{HEADER} OR MIT\n")));
    }
}
