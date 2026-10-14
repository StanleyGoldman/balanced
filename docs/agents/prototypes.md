# Prototypes

A prototype is throwaway code that answers one question by giving the user something to use and react to. In this project that is a small egui app the agent launches and watches while the user drives it. The user never narrates what they did and never types a command.

## Branch and PR

- Each prototype gets its own branch, `prototype/<name>`, and a **draft** PR titled `PROTOTYPE: <what it prototypes>`.
- The PR description states the question the prototype set out to answer, plus a running Q&A of what it showed. Update it as answers land.
- When the prototype session ends, post the resolution comment (below) on the prototype's issue and close the PR unmerged. Decisions go back into real code or docs through their own PRs.

## Shape

- Put it next to the code it serves, as an example named `<name>_prototype`: e.g. `crates/balanced_ui/examples/register_prototype.rs`.
- Feed it fixed-seed stub data at realistic volume (thousands of rows where the real thing will have them). State lives in memory.
- Skip tests, error handling and polish. Spend the effort on variants, toggles and the observation harness.
- Before building, post the plan on the ticket: variants, toggles, stub data, open questions.

## Variants and toggles

- Build 2–5 **variants** that differ in structure or interaction model, not colour.
- Dock all prototype tooling (variant switcher, toggles) in one panel along the window's right edge, added before the variant's own panels so every variant lays out in the remaining space. Style it as tooling, and let F4 collapse it to a thin edge. Floating overlays cover the design under review. A separate OS window can't stay above the app on Wayland: winit ignores window levels and parent windows there.
- In the app window, the ←/→ keys switch variants while no text field has focus. A `--variant <key>` flag picks the starting one.
- Tag each toggle as applying to one variant or to all. Toggle state and shared data (selection, sort) survive a variant switch, so the user compares like with like.

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

The harness lives in `crates/balanced_ui/examples/shell_layout_prototype/harness.rs` on branch `prototype/shell-layout`. Copy that file into each new prototype and follow the contract in its header.

## Running a session

The user learns what to look at from you. Drive the session through an **agenda**: the questions the prototype must answer, in the order the user should meet them.

You drive the prototype as well as the conversation. Through the control file you switch variants, flip toggles, and open the account, tab or row each step needs, so the user's hands stay on looking, trying and reacting. Each step takes the user through every variant and toggle that bears on its question, side by side, and you make each switch yourself as the step reaches it.

1. Before launch, write the agenda into the PR's Q&A, one entry per question.
2. When the prototype builds, ask the user for clearance to launch. Every launch needs it, including your own scripted control-file runs: the window opens on the user's desktop and takes keyboard focus, so it logs whatever they were typing elsewhere. An unfocused window is also throttled on Wayland, so a scripted run stalls while the user works in another app.
3. On a yes, start it yourself as a background process and tell the user it's up.
4. Guide one agenda question at a time, numbered `Step <n>/<total>`. For each step, give the user:
   - **context**: what the variants or toggles in play change, and why it matters for the question;
   - **look at**: where on screen the difference lives;
   - **try**: concrete actions in a scenario you have already set up through the control file; when the step compares variants or toggles, switch them yourself one at a time and say what changed after each switch;
   - **notice**: what to pay attention to while trying it.

   End the step by asking for their reaction.
5. Watch the event log as it grows (the Monitor tool), and read screenshots when a variant or toggle changes or the user drops a marker.
6. Turn what the log shows and the user doesn't say into follow-up questions: hesitations, Esc right after starting an edit, Tab landing somewhere the user leaves at once, a control never touched, slow frames.
7. When the user's message refers to "this" or "here", line it up with the log by timestamp.
8. A step closes when the user's answer and the log agree. Record it as a Q&A entry on the PR, then move to the next step.
9. The session ends when every agenda question has an answer. Post the resolution comment, then close the PR unmerged.

## Resolution comment

When the session ends, post one comment on the prototype's issue with this shape, then close the issue and append the context pointer to the map's Decisions-so-far. The pointer links to this comment, so the first line answers the ticket without scrolling.

```
**Resolution:** <one or two sentences: the answer the prototype gave to the ticket's question>

| # | Question | Answer | Tag | Reversibility | Evidence |
|---|----------|--------|-----|---------------|----------|
| S1 | <agenda question> | <what was chosen> | firm / leaning / coin-flip | cheap / expensive | <user quote, log events, screenshot> |

**Variants:** <each variant: chosen, merged into the winner (name the parts), or dropped, with the reason>

**Observed in the log:** <what the log showed beyond what the user said: hesitations, ignored controls, slow frames, or "none">

**User-decided:** <the `coin-flip` and `expensive` calls the user answered, with their reasoning if given>

**Assumed, shout if wrong:** <answers taken from the log or defaults without asking>

**Revises:** <earlier decisions or map entries this contradicts, or "none">

**Follow-ups:** <new tickets, ADRs, or `CONTEXT.md` terms it produced, linked, or "none">

**Prototype:** <the closed draft PR and its branch>
```

- List every agenda question as a table row, numbered by its step.
- Use the tags defined in the Grilling section of `CLAUDE.md`. Here `firm` means the user chose it and the log agrees.
- Write `none` in an empty section.
