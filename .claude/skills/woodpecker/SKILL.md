---
name: woodpecker
description: Read and drive this repo's CI through woodpecker-cli. Use when a pipeline is red, when asked whether CI passed or why it failed, to read a step's log, rerun or stop a pipeline, lint or locally run a workflow under .woodpecker/ before pushing, or manage a repo secret or cron.
---

# woodpecker-cli

`woodpecker-cli` talks to the Woodpecker server that runs this repo's CI. It reads
`WOODPECKER_SERVER` and `WOODPECKER_TOKEN` from the environment; both are set on the owner's
machine, and `woodpecker-cli info` printing a login confirms they still work. Those two
variables are the whole of the authentication, so a command that fails on auth means checking
that both are still set and the token still valid.

The repo is `StanleyGoldman/balanced`, id **6** on the server. A command whose only positional
is the repo (`pipeline ls`, `pipeline last`, `repo show`) infers it from the git remote when the
argument is dropped. A command that also takes a pipeline number (`ps`, `show`, `log show`,
`start`, `stop`) reads its first positional as the repo, so spell the repo out there, or the
number is read as a repo and the call fails with a bare `client error 404`. A pipeline is
addressed by its **number**, the `#n` the web UI shows and `pipeline ls` prints; the numbers
have gaps, and a missing number 404s.

## Reading a red run

1. **Find the pipeline number.**

   ```bash
   woodpecker-cli pipeline ls --limit 10 --output 'go-template={{range .}}#{{.Number}} {{.Status}} {{.Event}} {{.Branch}} {{.Ref}} {{.Refspec}}{{"\n"}}{{end}}'
   ```

   For a `pull_request` pipeline, `Branch` is the **base** branch. The PR number is in `Ref`
   (`refs/pull/<n>/merge`) and the head branch is the left half of `Refspec` (`head:base`), so
   filter on those. A `push` pipeline's `Branch` is the pushed branch, always `main` here.
   Done when you hold the number of the run the user means.

2. **Find the failing step.**

   ```bash
   woodpecker-cli pipeline ps StanleyGoldman/balanced <n> --format '{{.workflow.Name}}/{{.step.Name}} #{{.step.PID}} {{.step.State}}'
   ```

   `.woodpecker/ci.yml` has five steps: `clone`, `ci-image` (builds the CI image), then `ci`
   (`cargo xtask ci`, skipped when every changed path is prose), `typos` (`cargo xtask typos`)
   and `adrs` (`cargo xtask adrs`, against the target branch on a pull request) side by side.
   Done when one step reads `failure`. A `clone` failure means the checkout
   itself failed and no step of ours ran; a `ci-image` failure is the Dockerfile or the Docker
   daemon, not the code.

3. **Read its log.**

   ```bash
   woodpecker-cli pipeline log show StanleyGoldman/balanced <n> ci | tail -20
   ```

   A step is named as in the workflow file or by the `#PID` from `ps`. The `ci` log is
   thousands of lines of cargo output in ANSI colour (`CARGO_TERM_COLOR: always`). Its last
   line names the xtask task that failed and the command it ran (`snapshots failed: cargo test
   ... exited with exit status: 101`); the lines above it hold the error. Done when the
   failing command and its error text are in hand. Reproduce it locally with the same
   `cargo xtask <task>`.

## Output flags

| Use | Instead of | Why |
|---|---|---|
| `--output 'table=Number,Status,Event,Branch'` | bare `pipeline ls` | the default table carries the whole commit message per row |
| `--output 'go-template={{range .}}…{{end}}'` | `--output json` | `json` is accepted and silently ignored, printing the table |
| `{{range .}}` in every `--output` template | `{{.Number}}` | `ls`, `show` and `last` all hand the template a list, even for one pipeline; without it the call dies with `can't evaluate field Number` |
| `pipeline last --branch <b>` | `pipeline last` | defaults to `main`. It finds push pipelines only, so a PR's head branch is a bare `client error 404`; use `ls` and `Ref` for those |
| `--format '<go template>'` on `ps`, `queue`, `secret ls` | their defaults | the defaults are ANSI-coloured; these commands have no `--output`. `ps` ends each row with a newline itself |

## What the CLI cannot show

Durations, the agent that ran a step, and the queue are in the REST API only:

```bash
curl -s -H "Authorization: Bearer $WOODPECKER_TOKEN" "$WOODPECKER_SERVER/api/<path>" | jq .
```

| Need | `<path>` | Where it is |
|---|---|---|
| step durations and the agent | `repos/6/pipelines/<n>` | `.workflows[0].agent_id`; each `.workflows[0].children[]` has `name`, `state`, `started`, `finished` in unix seconds. A config error that stopped the pipeline before any step is in `.errors[]` |
| what is running or queued | `queue/info` | `.running[]` and `.pending[]`, each with `pipeline_number` and `agent_name` |
| agent ids to names and labels | `agents` | each agent's `custom_labels` |
| cron jobs | `repos/6/cron` | `null` while there are none |

A pull request queues for oldhermit and oldhermit-2 only; `ci.yml`'s `labels:` comment says why.

A `killed` or `canceled` step's `finished - started` is the time until it was stopped, not a
build time. Keep `state == success` only before comparing durations.

## Rerunning and stopping

- `woodpecker-cli pipeline start StanleyGoldman/balanced <n>` reruns pipeline `<n>` on the
  same commit and event. `pipeline stop StanleyGoldman/balanced <n>` cancels it.
- `pipeline create --branch <b>` fires the `manual` event. `ci.yml` runs only on
  `pull_request` and on `push` to `main`, so a manual pipeline includes no workflow and does
  nothing. To exercise a change, push a commit or `start` an existing number.

## What a pull request runs

Every pull request runs the whole of `ci.yml`, a full `cargo xtask ci`, prose-only ones
included. A push to `main` runs it too; a push to any other branch starts nothing.

## Changing the pipeline config

Before pushing a changed workflow file:

1. `woodpecker-cli lint --strict <file>` from the repo root. It checks the schema, not whether
   the steps pass. Done when it prints `Config is valid`.
2. `.claude/scripts/woodpecker-exec.sh <file> [<event>]` runs it locally, the way the agents
   would, on the committed HEAD, so commit first. Done when its JSON line has `status` `green`.

The script wraps `woodpecker-cli exec`, which mounts the **live working tree**, not a checkout
of a commit: switching branches or editing a file while a bare `exec` runs corrupts the run.
The script's header lists the other ways a bare `exec` fails here and how it handles each.
One it does not handle yet: host volumes are refused unless `exec` gets
`--repo-trusted-volumes`, and without it a cache mount silently does nothing. The change that
first mounts a host volume in a workflow adds that flag to the script.

Four traps a config that lints clean still hits, each failing the pipeline before a step runs:

- **Dollar-brace substitution runs on the raw file, comments included.** Woodpecker expands
  the brace form of a variable as text before the YAML is parsed, so a comment that merely
  shows that syntax fails the pipeline with `unable to parse variable name`, and `lint`
  cannot see it. Write `$VAR` without braces in commands, and describe the brace form in prose.
- **A `from_secret` naming a missing secret is fatal, not empty.** The pipeline errors before
  the first step. The reference and the secret land together.
- **Build steps get no forge credentials.** Only the clone step has them, so `git fetch` in a
  build step dies with `could not read Username for 'https://github.com'` on this private
  repo. Ask for what you need from the forge in the `clone:` section
  (`fetch-target-branch: true` brings the target branch). Keep that section's image
  `woodpeckerci/plugin-git`: the credentials go only to images on the server's trusted-clone
  list. Ordinary network access works; steps reach crates.io.
- **A dependency on a step its `when:` filtered out rejects the whole pipeline.** Status
  `error`, no workflows, so `ps` prints nothing; the message is in `.errors[]`
  (`step 'ci' depends on step 'ci-image' which is filtered out by its conditions`). It fires
  only on the event-and-path combination where the dependent runs and its dependency does not,
  so test a `when:` change on a pull request that does *not* touch the gated path. `ci-image`
  stays unfiltered for this reason. For any other dependency that genuinely should not run,
  mark the edge optional; an optional edge is dropped when the named step is absent and
  waited on when present:

  ```yaml
  depends_on:
    - always-there
    - name: sometimes-filtered
      optional: true
  ```

## Secrets

The workflows read secrets by name through `from_secret`.
`woodpecker-cli repo secret ls --repo StanleyGoldman/balanced --format '{{.Name}} {{.Events}} {{.Images}}'`
lists names and the events and images each is limited to; values never come back out. To add
one:

```bash
woodpecker-cli repo secret add --repo StanleyGoldman/balanced --name <name> --value "$VALUE" --event <event> --image balanced-ci:latest
```

Limit it to the events and image of the steps that read it. A secret allowed on
`pull_request` reaches the steps of every pull request, so add that event only when a pull
request step reads the secret. Pass the value through a variable so it stays out of the
transcript. A wrong value is fixed with
`repo secret update`, or `rm` and a fresh `add`.

## Crons

`woodpecker-cli repo cron ls --repo StanleyGoldman/balanced` lists them; a cron's runs are
`pipeline ls --event cron`. To disable one, `repo cron update --enabled=false` exits cleanly
but leaves it enabled. Patch it instead, then read `repos/6/cron` back to confirm:

```bash
curl -s -X PATCH -H "Authorization: Bearer $WOODPECKER_TOKEN" -H 'Content-Type: application/json' \
  -d '{"enabled": false}' "$WOODPECKER_SERVER/api/repos/6/cron/<cron id>"
```

## Stacked pull requests

Woodpecker reads `.woodpecker/ci.yml` from the pull request's **head commit**, so each layer of
a stack runs the file *it* contains, and a change made at one layer is exercised by that layer
and the ones above it only. Before reading a run of a stack, find out which file it ran:

```bash
git rev-parse origin/<head-branch>:.woodpecker/ci.yml
git diff origin/<lower>:.woodpecker/ci.yml origin/<upper>:.woodpecker/ci.yml
```

Equal hashes, same file.

On a stacked pull request, `adrs` compares with the layer below, not `main`. A clash with a
record on `main` surfaces when the stack lands, in the push-to-`main` run.
