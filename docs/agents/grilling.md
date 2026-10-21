# Grilling resolution comment

When a grilling ticket resolves, post one comment on the issue with this shape. Then finish by where the ticket sits:

- **On a map:** close the issue and append the context pointer to the map's Decisions-so-far. The pointer links to this comment, so the first line answers the ticket without scrolling.
- **On no map, resolved into behaviour to build:** keep the issue open and swap its triage label for `ready-for-agent`, so the ticket tracks the build.

```
**Resolution:** <one or two sentences: the answer to the ticket's question>

| # | Decision | Tag | Reversibility | Source |
|---|----------|-----|---------------|--------|
| Q1 | <decision> | firm / leaning / coin-flip | cheap / expensive | <ADR, doc, or "user"> |

**User-decided:** <the `coin-flip` and `expensive` calls the user answered, with their reasoning if given>

**Assumed, shout if wrong:** <the `firm` questions settled without asking>

**Revises:** <earlier decisions or map entries this contradicts, or "none">

**Follow-ups:** <new tickets, ADRs, or `GLOSSARY.md` terms it produced, linked, or "none">
```

- List every question as a table row.
- Use the tags defined in the Grilling section of `CLAUDE.md`.
- Write `none` in an empty section.
