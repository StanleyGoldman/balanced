// SPDX-License-Identifier: GPL-3.0-or-later WITH AdditionRef-Balanced-Plugin-Exception

use anyhow::{Context as _, Result, bail};

use crate::{Task, process};

pub(crate) const TASK: Task = Task {
    name: "todo",
    args: "",
    summary: "require every TODO and FIXME to name an issue, as TODO(#123)",
    run: |args| args.is_empty().then(run),
};

/// Files that spell out bare markers in their usage text and tests.
const SPELLS_OUT_MARKERS: [&str; 2] = ["xtask/src/todo.rs", "xtask/tests/usage.rs"];

pub(crate) fn run() -> Result<()> {
    let files = process::output("git", &["ls-files", "*.rs", "*.toml"])?;
    let mut bare = Vec::new();
    for path in files
        .lines()
        .filter(|path| !SPELLS_OUT_MARKERS.contains(path))
    {
        let text = std::fs::read_to_string(path).with_context(|| format!("reading {path}"))?;
        for (index, line) in text.lines().enumerate() {
            if has_bare_todo(line) {
                bare.push(format!("{path}:{}: {}", index + 1, line.trim()));
            }
        }
    }
    if !bare.is_empty() {
        bail!(
            "write TODO(#<issue>) instead of a bare TODO or FIXME:\n{}",
            bare.join("\n")
        );
    }
    Ok(())
}

/// Whether `line` holds a `TODO` or `FIXME` word that isn't followed by `(#<digits>)`.
fn has_bare_todo(line: &str) -> bool {
    ["TODO", "FIXME"].iter().any(|marker| {
        line.match_indices(marker).any(|(start, _)| {
            // `match_indices` yields char boundaries, so both slices are in range.
            let before = line[..start].chars().next_back();
            let after = &line[start + marker.len()..];
            let is_word = !before.is_some_and(is_word_char)
                && !after.chars().next().is_some_and(is_word_char);
            is_word && !names_issue(after)
        })
    })
}

fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Whether `rest` starts with `(#<digits>)`.
fn names_issue(rest: &str) -> bool {
    rest.strip_prefix("(#")
        .and_then(|rest| rest.split_once(')'))
        .is_some_and(|(number, _)| !number.is_empty() && number.chars().all(|c| c.is_ascii_digit()))
}

#[cfg(test)]
mod tests {
    use super::has_bare_todo;

    #[test]
    fn bare_markers_are_flagged() {
        assert!(has_bare_todo("// TODO fix"));
        assert!(has_bare_todo("// FIXME: x"));
        assert!(has_bare_todo("// TODO(stan) owner is not an issue"));
        assert!(has_bare_todo("// TODO(#) empty number"));
    }

    #[test]
    fn issue_markers_and_other_words_pass() {
        assert!(!has_bare_todo("// TODO(#12) tracked"));
        assert!(!has_bare_todo("// FIXME(#3) tracked"));
        assert!(!has_bare_todo("let todos = TODOS;"));
        assert!(!has_bare_todo("// todo lower case"));
    }

    #[test]
    fn a_bare_marker_after_a_tracked_one_is_flagged() {
        assert!(has_bare_todo("// TODO(#12) tracked, TODO not"));
    }

    #[test]
    fn malformed_issue_references_are_flagged() {
        assert!(has_bare_todo("// TODO(#12 unclosed"));
        assert!(has_bare_todo("// TODO(#12a) not a number"));
    }

    #[test]
    fn markers_inside_longer_words_pass() {
        assert!(!has_bare_todo("let MY_TODO = TODOS + FIXMEX;"));
    }
}
