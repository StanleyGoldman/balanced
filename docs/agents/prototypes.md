# Prototypes

A prototype is throwaway code that answers one question by giving the user something to use and react to. In this project that is a small egui app the agent launches and watches while the user drives it. The user never narrates what they did and never types a command.

## Branch and PR

- Each prototype gets its own branch, `prototype/<name>`, and a **draft** PR titled `PROTOTYPE: <what it prototypes>`.
- The PR description states the question the prototype set out to answer, plus a running Q&A of what it showed. Update it as answers land.
- When the prototype session ends, close the PR unmerged and leave a pointer to it on the prototype's issue. Decisions go back into real code or docs through their own PRs.

## Shape

- Put it next to the code it serves, as an example named `<name>_prototype`: e.g. `crates/balanced_ui/examples/register_prototype.rs`.
- Feed it fixed-seed stub data at realistic volume (thousands of rows where the real thing will have them). State lives in memory.
- Skip tests, error handling and polish. Spend the effort on variants, toggles and the observation harness.
- Before building, post the plan on the ticket: variants, toggles, stub data, open questions.

## Variants and toggles

- Build 2–5 **variants** that differ in structure or interaction model, not colour.
- Switch variants inside the running app: a floating ◀ ▶ bar at the bottom centre, styled so it reads as tooling rather than design, and the ←/→ keys while no text field has focus. A `--variant <key>` flag picks the starting one.
- A collapsible **Prototype panel** holds toggles. Tag each toggle as applying to one variant or to all. Toggle state and shared data (selection, sort) survive a variant switch, so the user compares like with like.

## Observation harness

The agent follows the user through the app's own output. Build these in:

- **Event log**: JSONL at the path given by `--log <path>` (use your scratchpad). One line per event, with a timestamp:
  - clicks, named by widget (`payee cell, row 1204`), not coordinates;
  - focus gained and lost, with the cause (click, Tab, app moved it);
  - typing: each keystroke tied to its field, and Tab, Enter, Esc and arrows as their own events;
  - app events: variant switches, toggle flips, saves and cancels with the Commands they would emit, suggestions offered and picked, sort changes;
  - one event per finished scroll gesture (rows moved, duration, worst frame), and a perf sample about once a second.

  Log intent, never a raw mouse-position stream.
- **Screenshots**: the app captures its own window (`ViewportCommand::Screenshot`) as PNGs beside the log on every variant switch and toggle flip.
- **F2** opens a one-line note box written into the log; **F3** drops a marker plus a screenshot.
- **Control file**: the app polls a file (`--control <path>`) for commands from the agent: switch variant, flip a toggle, jump to a row, reload data. Use it to set up a scenario before asking the user to try it.

The first prototype builds the harness; later prototypes copy it from that prototype's branch.

## Running a session

1. When the prototype builds, ask the user for clearance to launch.
2. On a yes, start it yourself as a background process; the window opens on the user's desktop. Tell them it's up.
3. Watch the event log as it grows (the Monitor tool), and read screenshots when a variant or toggle changes or the user drops a marker.
4. Turn what you see into questions and Q&A entries on the PR: hesitations, Esc right after starting an edit, Tab landing somewhere the user leaves at once, slow frames.
5. When the user's message refers to "this" or "here", line it up with the log by timestamp.
