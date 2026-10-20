// SPDX-License-Identifier: GPL-3.0-or-later WITH AdditionRef-Balanced-Plugin-Exception

use std::path::PathBuf;

/// The harness's command-line flags. Other arguments are ignored.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Args {
    /// `--variant <key>`: the variant to start on.
    pub variant: Option<String>,
    /// `--log <path>`: the JSONL event log; screenshots go beside it.
    pub log: Option<PathBuf>,
    /// `--control <path>`: the file the agent writes commands to.
    pub control: Option<PathBuf>,
}

impl Args {
    /// Parse the process's own arguments.
    pub fn from_env() -> Self {
        Self::parse(std::env::args().skip(1))
    }

    /// Parse `args` (without the program name).
    pub fn parse(args: impl IntoIterator<Item = String>) -> Self {
        let args: Vec<String> = args.into_iter().collect();
        let value = |flag: &str| {
            args.iter()
                .position(|a| a == flag)
                .and_then(|p| args.get(p + 1))
                .cloned()
        };
        Self {
            variant: value("--variant"),
            log: value("--log").map(Into::into),
            control: value("--control").map(Into::into),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Args {
        Args::parse(list.iter().map(|s| (*s).to_owned()))
    }

    #[test]
    fn reads_each_flag_value() {
        let parsed = args(&[
            "--variant",
            "b",
            "--log",
            "/tmp/x/events.jsonl",
            "--control",
            "/tmp/x/c.txt",
        ]);
        assert_eq!(parsed.variant.as_deref(), Some("b"));
        assert_eq!(parsed.log, Some(PathBuf::from("/tmp/x/events.jsonl")));
        assert_eq!(parsed.control, Some(PathBuf::from("/tmp/x/c.txt")));
    }

    #[test]
    fn missing_flags_and_dangling_flags_are_none() {
        assert_eq!(args(&[]), Args::default());
        assert_eq!(args(&["--variant"]).variant, None);
    }
}
