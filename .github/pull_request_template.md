Closes #<n>

<!--
Crew fills this by hand with `gh pr edit --body-file -`; GitHub does not apply templates to
PRs created from the command line. Keep the first line exactly as above with the real number.
-->

## Stack position

none: one PR on main

<!-- Phase 1 is always this. Later: "stack <name>, layer N of M, based on #<pr>". -->

## Summary

<!-- What changed and why, in a few sentences. -->

## Acceptance criteria

<!-- Copy each criterion from the issue. Check it only when a test covers it, and name the test. -->

- [x] **AC1** Given … When … Then … → tests/cli.rs::get_prints_status

## Test seams

<!-- The public seams the tests exercise. Mark any still assumed as `Seam (assumed): …`. -->

- …

## How to try it

<!-- Exact commands a reviewer can paste, from a fresh checkout of this branch. -->

```bash
…
```

## Out of scope

- …

## Notes

<!--
Assumptions crew made, new glossary terms (CONTEXT.md), a root-cause note for bug fixes,
and anything the reviewer should know.
-->
