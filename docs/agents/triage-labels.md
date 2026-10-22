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

Pick from the **unblocked** `needs-triage` tickets. `gh issue list` can't show which tickets are blocked, so list them with:

```sh
gh api "repos/StanleyGoldman/balanced/issues?labels=needs-triage&state=open&per_page=100" \
  --jq '.[] | select(.issue_dependencies_summary.blocked_by == 0) | "\(.number) \(.title)"'
```

## Waiting on an upstream release

Label a ticket `upstream`, in place of `needs-triage`, when it is fully specified but can't start until a dependency ships a release. Its body names the `cargo tree` check that shows the release has landed (#178, #353).

When a run upgrades a dependency, check every `upstream` ticket (`gh issue list --label upstream`) and do, in the same PR, each one whose check now passes.
