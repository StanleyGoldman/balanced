// SPDX-License-Identifier: GPL-3.0-or-later WITH AdditionRef-Balanced-Plugin-Exception

//! Reading the lcov file `cargo llvm-cov` writes.

use std::collections::BTreeMap;

use anyhow::{Context as _, Result};

/// What one source file's tests covered.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub(super) struct FileCoverage {
    /// Execution count of each line that holds code; a line missing here holds none.
    pub(super) lines: BTreeMap<u32, u64>,
    pub(super) functions_found: u64,
    pub(super) functions_hit: u64,
}

impl FileCoverage {
    pub(super) fn lines_hit(&self) -> u64 {
        self.lines.values().filter(|&&count| count > 0).count() as u64
    }

    pub(super) fn lines_found(&self) -> u64 {
        self.lines.len() as u64
    }
}

/// Every file in `lcov`, keyed by its path made relative to `root`; a path outside
/// `root` (a dependency's source) is dropped.
pub(super) fn parse(lcov: &str, root: &str) -> Result<BTreeMap<String, FileCoverage>> {
    let mut files = BTreeMap::new();
    let mut current: Option<(String, FileCoverage)> = None;
    for line in lcov.lines() {
        let (tag, value) = line.split_once(':').unwrap_or((line, ""));
        match tag {
            "SF" => {
                let relative = value
                    .strip_prefix(root)
                    .map(|rest| rest.trim_start_matches('/').to_owned());
                current = relative.map(|path| (path, FileCoverage::default()));
            }
            "DA" => {
                if let Some((_, file)) = current.as_mut() {
                    let (number, count) = value
                        .split_once(',')
                        .with_context(|| format!("malformed DA record: {line}"))?;
                    // A trailing checksum, `DA:1,2,abc`, is ignored.
                    let count = count.split(',').next().unwrap_or(count);
                    let number: u32 = number.parse().with_context(|| format!("line {line}"))?;
                    let count: u64 = count.parse().with_context(|| format!("count {line}"))?;
                    *file.lines.entry(number).or_default() += count;
                }
            }
            "FNF" => {
                if let Some((_, file)) = current.as_mut() {
                    file.functions_found = value.parse().with_context(|| line.to_owned())?;
                }
            }
            "FNH" => {
                if let Some((_, file)) = current.as_mut() {
                    file.functions_hit = value.parse().with_context(|| line.to_owned())?;
                }
            }
            "end_of_record" => {
                if let Some((path, file)) = current.take() {
                    let merged: &mut FileCoverage = files.entry(path).or_default();
                    for (number, count) in file.lines {
                        *merged.lines.entry(number).or_default() += count;
                    }
                    merged.functions_found += file.functions_found;
                    merged.functions_hit += file.functions_hit;
                }
            }
            _ => {}
        }
    }
    Ok(files)
}

#[cfg(test)]
mod tests {
    use super::*;

    const LCOV: &str = "\
SF:/work/crates/a/src/lib.rs
FN:1,f
FNDA:2,f
FNF:2
FNH:1
DA:1,2
DA:2,0
DA:3,5
LF:3
LH:2
end_of_record
SF:/usr/local/cargo/registry/dep/src/lib.rs
DA:1,1
end_of_record
";

    #[test]
    fn it_reads_lines_and_functions_of_files_under_the_root() {
        let files = parse(LCOV, "/work").unwrap();
        assert_eq!(files.keys().collect::<Vec<_>>(), ["crates/a/src/lib.rs"]);
        let file = &files["crates/a/src/lib.rs"];
        assert_eq!((file.lines_hit(), file.lines_found()), (2, 3));
        assert_eq!((file.functions_hit, file.functions_found), (1, 2));
    }

    #[test]
    fn a_malformed_record_is_an_error() {
        assert!(parse("SF:/work/a.rs\nDA:nonsense\nend_of_record\n", "/work").is_err());
    }
}
