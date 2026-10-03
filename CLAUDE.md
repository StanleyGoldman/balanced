# CLAUDE.md

This project is developed with **crew** (https://github.com/StanleyGoldman/crew), a Claude Code plugin.

- Run `crew` (`alias crew='claude --agent crew:orchestrator'`) to work with the orchestrator. Plain `claude` stays normal Claude; `/crew:status` shows the board and `/crew:file` files an idea or bug.
- The project profile, `.claude/crew/project.md`, has the language, the build, test and run commands, and the conventions. Read it before working here.
- Domain language: `CONTEXT.md` (once it exists). Decisions: `docs/adr/`.
- There's no app code yet.
