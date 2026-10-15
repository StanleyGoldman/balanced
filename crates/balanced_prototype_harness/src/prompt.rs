//! Prompt: the agent's next instruction to the user, shown in a bottom panel of the window.
//!
//! This module parses the `prompt` control command; [`crate::Harness`] stores, draws and logs it.

/// A `prompt` control command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum PromptCommand {
    /// `prompt off`, or a bare `prompt`: hide the panel.
    Clear,
    /// `prompt <text>`: show `text`, with each literal `\n` as a line break.
    Show(String),
}

impl PromptCommand {
    /// Parse one control line, or return `None` when it isn't a `prompt` command.
    pub(crate) fn parse(line: &str) -> Option<Self> {
        let rest = line.trim().strip_prefix("prompt")?;
        if !(rest.is_empty() || rest.starts_with(char::is_whitespace)) {
            return None;
        }
        let rest = rest.trim();
        if rest.is_empty() || rest.eq_ignore_ascii_case("off") {
            Some(Self::Clear)
        } else {
            Some(Self::Show(rest.replace("\\n", "\n")))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_is_shown_with_line_breaks() {
        assert_eq!(
            PromptCommand::parse("prompt Step 1/8\\nClick B in the dock"),
            Some(PromptCommand::Show(
                "Step 1/8\nClick B in the dock".to_owned()
            ))
        );
    }

    #[test]
    fn off_and_bare_clear_it() {
        assert_eq!(
            PromptCommand::parse("prompt off"),
            Some(PromptCommand::Clear)
        );
        assert_eq!(PromptCommand::parse("prompt"), Some(PromptCommand::Clear));
    }

    #[test]
    fn other_commands_are_not_prompt() {
        assert_eq!(PromptCommand::parse("prompts here"), None);
        assert_eq!(PromptCommand::parse("variant b"), None);
    }
}
