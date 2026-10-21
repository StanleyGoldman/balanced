// SPDX-License-Identifier: GPL-3.0-or-later WITH AdditionRef-Balanced-Plugin-Exception

//! Turning line coverage, and the lines a change added, into the Markdown report that
//! CI comments on a pull request and puts in the job summary.

use std::collections::BTreeMap;

use super::diff::AddedLines;
use super::lcov::FileCoverage;

/// The first line of the report. `.github/post-coverage-comment.sh` finds the
/// comments to replace by it.
pub(crate) const MARKER: &str = "<!-- balanced-coverage-report -->";

/// A percentage at or above this shows green, at or above `YELLOW_FROM` yellow, else red.
const GREEN_FROM: f64 = 80.0;
const YELLOW_FROM: f64 = 60.0;

/// GitHub rejects a comment over 65 536 characters; the report stops short of that.
const MAX_LEN: usize = 60_000;

/// Hit and found counts of something that can be covered.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(super) struct Tally {
    pub(super) hit: u64,
    pub(super) found: u64,
}

impl Tally {
    /// `None` when nothing was found, so an empty file isn't called 100% covered.
    pub(super) fn percent(self) -> Option<f64> {
        let count = |n: u64| f64::from(u32::try_from(n).unwrap_or(u32::MAX));
        (self.found > 0).then(|| count(self.hit) * 100.0 / count(self.found))
    }

    fn add(&mut self, other: Self) {
        self.hit += other.hit;
        self.found += other.found;
    }
}

/// How much of the lines a change added the tests ran.
#[derive(Debug, Default, PartialEq, Eq)]
pub(super) struct DiffCoverage {
    pub(super) total: Tally,
    /// Per changed file with code on added lines: its tally and its uncovered lines.
    pub(super) files: BTreeMap<String, (Tally, Vec<u32>)>,
}

/// The added lines that hold code, and which of them ran. A line `lcov` has no
/// record of (a comment, a signature, a blank) isn't code, and is left out.
pub(super) fn diff_coverage(
    files: &BTreeMap<String, FileCoverage>,
    added: &AddedLines,
) -> DiffCoverage {
    let mut result = DiffCoverage::default();
    for (path, numbers) in added {
        let Some(file) = files.get(path) else {
            continue;
        };
        let mut tally = Tally::default();
        let mut uncovered = Vec::new();
        for number in numbers {
            if let Some(&count) = file.lines.get(number) {
                tally.found += 1;
                if count > 0 {
                    tally.hit += 1;
                } else {
                    uncovered.push(*number);
                }
            }
        }
        if tally.found > 0 {
            result.total.add(tally);
            result.files.insert(path.clone(), (tally, uncovered));
        }
    }
    result
}

/// The crate a file belongs to: `balanced_ui` for `crates/balanced_ui/src/lib.rs`, else
/// the file's first directory.
fn crate_of(path: &str) -> &str {
    let mut parts = path.split('/');
    match parts.next() {
        Some("crates") => parts.next().unwrap_or("crates"),
        Some(first) => first,
        None => path,
    }
}

fn lines_of(file: &FileCoverage) -> Tally {
    Tally {
        hit: file.lines_hit(),
        found: file.lines_found(),
    }
}

fn functions_of(file: &FileCoverage) -> Tally {
    Tally {
        hit: file.functions_hit,
        found: file.functions_found,
    }
}

fn emoji(percent: Option<f64>) -> &'static str {
    match percent {
        None => "⚪",
        Some(p) if p >= GREEN_FROM => "🟢",
        Some(p) if p >= YELLOW_FROM => "🟡",
        Some(_) => "🔴",
    }
}

/// Ten cells, one per ten percent, filled to the nearest cell.
fn bar(tally: Tally) -> String {
    let filled = (tally.hit * 10 + tally.found / 2)
        .checked_div(tally.found)
        .map_or(0, |cells| usize::try_from(cells).unwrap_or(10).min(10));
    format!("{}{}", "▰".repeat(filled), "▱".repeat(10 - filled))
}

/// `87.3% (131/150)`, or a dash when nothing was found.
fn cell(tally: Tally) -> String {
    tally.percent().map_or_else(
        || "–".to_owned(),
        |p| format!("{p:.1}% ({}/{})", tally.hit, tally.found),
    )
}

fn meter(tally: Tally) -> String {
    format!("{} {} {}", emoji(tally.percent()), bar(tally), cell(tally))
}

/// `[3, 4, 5, 9]` as `3-5, 9`.
fn ranges(numbers: &[u32]) -> String {
    let mut out: Vec<String> = Vec::new();
    let mut iter = numbers.iter().copied().peekable();
    while let Some(start) = iter.next() {
        let mut end = start;
        while iter.next_if_eq(&(end + 1)).is_some() {
            end += 1;
        }
        out.push(if end == start {
            start.to_string()
        } else {
            format!("{start}-{end}")
        });
    }
    out.join(", ")
}

/// A crate's lines, its functions, and its files.
type CrateFiles<'a> = (Tally, Tally, Vec<(&'a str, &'a FileCoverage)>);

/// The report: the totals, the change's own coverage if `diff` is given, a table per
/// crate, and a table per file in a collapsed section under each crate.
pub(super) fn render(
    files: &BTreeMap<String, FileCoverage>,
    diff: Option<&DiffCoverage>,
    footer: &str,
) -> String {
    let mut total_lines = Tally::default();
    let mut total_functions = Tally::default();
    let mut crates: BTreeMap<&str, CrateFiles<'_>> = BTreeMap::new();
    for (path, file) in files {
        total_lines.add(lines_of(file));
        total_functions.add(functions_of(file));
        let entry = crates.entry(crate_of(path)).or_default();
        entry.0.add(lines_of(file));
        entry.1.add(functions_of(file));
        entry.2.push((path, file));
    }

    let mut out: Vec<String> = vec![
        MARKER.to_owned(),
        "## 📊 Code coverage".to_owned(),
        String::new(),
        "| | Coverage |".to_owned(),
        "|---|---|".to_owned(),
    ];
    if let Some(diff) = diff {
        let note = if diff.total.found == 0 {
            "no changed lines hold code".to_owned()
        } else {
            meter(diff.total)
        };
        out.push(format!("| **Changed lines** | {note} |"));
    }
    out.push(format!("| **Lines** | {} |", meter(total_lines)));
    out.push(format!("| **Functions** | {} |", meter(total_functions)));
    out.push(String::new());

    if let Some(diff) = diff.filter(|diff| !diff.files.is_empty()) {
        out.push("### Changed files".to_owned());
        out.push(String::new());
        out.push("| File | Changed lines covered | Not covered |".to_owned());
        out.push("|---|---|---|".to_owned());
        for (path, (tally, uncovered)) in &diff.files {
            out.push(format!(
                "| `{path}` | {} | {} |",
                meter(*tally),
                ranges(uncovered)
            ));
        }
        out.push(String::new());
    }

    out.push("### By crate".to_owned());
    out.push(String::new());
    out.push("| Crate | Lines | Functions |".to_owned());
    out.push("|---|---|---|".to_owned());
    for (name, (lines, functions, _)) in &crates {
        out.push(format!(
            "| `{name}` | {} | {} |",
            meter(*lines),
            cell(*functions)
        ));
    }
    out.push(String::new());

    for (name, (lines, _, members)) in &crates {
        out.push(format!(
            "<details><summary><code>{name}</code> files ({})</summary>",
            cell(*lines)
        ));
        out.push(String::new());
        out.push("| File | Lines | Functions |".to_owned());
        out.push("|---|---|---|".to_owned());
        for (path, file) in members {
            out.push(format!(
                "| `{path}` | {} | {} |",
                meter(lines_of(file)),
                cell(functions_of(file))
            ));
        }
        out.push(String::new());
        out.push("</details>".to_owned());
        out.push(String::new());
    }
    out.push(footer.to_owned());
    out.push(String::new());
    truncate(out.join("\n"))
}

/// `report` cut at a line boundary below GitHub's comment limit, with a note saying so.
fn truncate(report: String) -> String {
    if report.len() <= MAX_LEN {
        return report;
    }
    let mut cut = MAX_LEN;
    while !report.is_char_boundary(cut) {
        cut -= 1;
    }
    let end = report[..cut].rfind('\n').unwrap_or(cut);
    format!(
        "{}\n\n_The rest of the report is cut to fit a comment; the artifact holds all of it._\n",
        &report[..end]
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(lines: &[(u32, u64)], functions: (u64, u64)) -> FileCoverage {
        FileCoverage {
            lines: lines.iter().copied().collect(),
            functions_hit: functions.0,
            functions_found: functions.1,
        }
    }

    fn files() -> BTreeMap<String, FileCoverage> {
        BTreeMap::from([
            (
                "crates/a/src/lib.rs".to_owned(),
                file(&[(1, 1), (2, 0), (3, 0), (4, 2)], (1, 2)),
            ),
            ("crates/b/src/lib.rs".to_owned(), file(&[(1, 1)], (1, 1))),
        ])
    }

    #[test]
    fn diff_coverage_counts_only_added_lines_that_hold_code() {
        let added = AddedLines::from([
            ("crates/a/src/lib.rs".to_owned(), [1, 2, 3, 9].into()),
            ("README.md".to_owned(), [1].into()),
        ]);
        let diff = diff_coverage(&files(), &added);
        assert_eq!(diff.total, Tally { hit: 1, found: 3 });
        assert_eq!(diff.files["crates/a/src/lib.rs"].1, [2, 3]);
        assert_eq!(diff.files.len(), 1);
    }

    #[test]
    fn the_comment_script_finds_the_report_by_its_marker() {
        let script = include_str!("../../../.github/post-coverage-comment.sh");
        assert!(script.contains(MARKER));
    }

    #[test]
    fn ranges_join_runs() {
        assert_eq!(ranges(&[3, 4, 5, 9, 11, 12]), "3-5, 9, 11-12");
    }

    #[test]
    fn the_report_shows_totals_changed_lines_and_each_crate() {
        let added = AddedLines::from([("crates/a/src/lib.rs".to_owned(), [1, 2, 3].into())]);
        let diff = diff_coverage(&files(), &added);
        let report = render(&files(), Some(&diff), "footer note");
        assert!(report.starts_with(MARKER));
        assert!(
            report.contains("| **Lines** | 🟡 ▰▰▰▰▰▰▰▰▱▱ 60.0% (3/5) |")
                || report.contains("60.0% (3/5)")
        );
        assert!(report.contains("33.3% (1/3)"), "{report}");
        assert!(
            report.contains("| `crates/a/src/lib.rs` | 🔴 ▰▰▰▱▱▱▱▱▱▱ 33.3% (1/3) | 2-3 |"),
            "{report}"
        );
        assert!(report.contains("| `a` |"));
        assert!(report.contains("| `b` |"));
        assert!(report.ends_with("footer note\n"));
    }

    #[test]
    fn a_report_over_the_comment_limit_is_cut_with_a_note() {
        let long = format!("{}\nmore\n", "x".repeat(MAX_LEN));
        let cut = truncate(long);
        assert!(cut.len() < 65_536);
        assert!(cut.contains("cut to fit"));
    }
}
