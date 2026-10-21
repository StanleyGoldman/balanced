// SPDX-License-Identifier: GPL-3.0-or-later WITH AdditionRef-Balanced-Plugin-Exception

//! Reading which lines a change added, from `git diff -U0`.

use std::collections::{BTreeMap, BTreeSet};

/// The added lines of each changed file, keyed by path.
pub(super) type AddedLines = BTreeMap<String, BTreeSet<u32>>;

/// The lines `diff` adds, from a zero-context unified diff.
pub(super) fn added_lines(diff: &str) -> AddedLines {
    let mut added = AddedLines::new();
    let mut path: Option<&str> = None;
    for line in diff.lines() {
        if let Some(new) = line.strip_prefix("+++ ") {
            path = new.strip_prefix("b/");
        } else if let (Some(hunk), Some(path)) = (line.strip_prefix("@@ "), path) {
            // `-a,b +c,d @@`: the new side starts at c and runs d lines (1 if omitted).
            let Some(new_side) = hunk.split_whitespace().nth(1) else {
                continue;
            };
            let new_side = new_side.trim_start_matches('+');
            let (start, count) = new_side.split_once(',').unwrap_or((new_side, "1"));
            let (Ok(start), Ok(count)) = (start.parse::<u32>(), count.parse::<u32>()) else {
                continue;
            };
            added
                .entry(path.to_owned())
                .or_default()
                .extend(start..start + count);
        }
    }
    added
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_lists_added_lines_per_file() {
        let diff = "\
diff --git a/src/a.rs b/src/a.rs
--- a/src/a.rs
+++ b/src/a.rs
@@ -3,0 +4,2 @@ fn x
+one
+two
@@ -10 +12 @@
-old
+new
@@ -20,2 +21,0 @@
-gone
-gone
diff --git a/src/b.rs b/src/b.rs
--- /dev/null
+++ b/src/b.rs
@@ -0,0 +1,3 @@
+a
+b
+c
";
        let added = added_lines(diff);
        assert_eq!(added["src/a.rs"], BTreeSet::from([4, 5, 12]));
        assert_eq!(added["src/b.rs"], BTreeSet::from([1, 2, 3]));
    }

    #[test]
    fn a_deleted_file_adds_nothing() {
        let diff = "--- a/gone.rs\n+++ /dev/null\n@@ -1,2 +0,0 @@\n-a\n-b\n";
        assert!(added_lines(diff).is_empty());
    }
}
