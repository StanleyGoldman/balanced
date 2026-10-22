# Triage Labels

The skills speak in terms of five canonical triage roles. This file maps those roles to the actual label strings used in this repo's issue tracker.

| Label in mattpocock/skills | Label in our tracker | Meaning                                  |
| -------------------------- | -------------------- | ---------------------------------------- |
| `needs-triage`             | `needs-triage`       | Maintainer needs to evaluate this issue  |
| `needs-info`               | `needs-info`         | Waiting on reporter for more information |
| `ready-for-agent`          | `ready-for-agent`    | Fully specified, ready for an AFK agent  |
| `ready-for-human`          | `ready-for-human`    | Requires human implementation            |
| `wontfix`                  | `wontfix`            | Will not be actioned                     |

When a skill mentions a role (e.g. "apply the AFK-ready triage label"), use the corresponding label string from this table.

Edit the right-hand column to match whatever vocabulary you actually use.

## Picking a ticket to triage

Pick from the **unblocked** `needs-triage` tickets. A blocked one keeps its `needs-triage` label until its blocker resolves it, often a prototype whose agenda already holds its question, so grilling it repeats that work:

```sh
gh api "repos/StanleyGoldman/balanced/issues?labels=needs-triage&state=open&per_page=100" \
  --jq '.[] | select(.issue_dependencies_summary.blocked_by == 0) | "\(.number) \(.title)"'
```

`gh issue list` has no blocked-by field, so its list mixes blocked and unblocked tickets.
