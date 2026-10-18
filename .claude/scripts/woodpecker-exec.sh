#!/usr/bin/env bash
# Runs a Woodpecker workflow locally with `woodpecker-cli exec`, the way the
# oldhermit agents would, against the current worktree's HEAD commit.
#
# Usage: woodpecker-exec.sh <workflow> [<event>]
#   <workflow>  path to the workflow file, relative to the repository root,
#               e.g. .woodpecker/ci.yml.
#   <event>     the pipeline event to simulate: pull_request (the default) or push.
#
# Run it from inside the worktree (or main checkout) whose HEAD you want tested.
# Only committed work runs: uncommitted changes are left out, so commit first.
#
# Two things make a bare `woodpecker-cli exec` fail here, and this script handles both:
# - A worktree's `.git` is a file pointing outside the mount, so git inside the
#   step container sees no repository. The script runs from a real clone at
#   ~/.cache/balanced-woodpecker-exec/clone, reused between runs so its `target/`
#   stays warm, and reset to HEAD each time.
# - `ci-image` reaches Docker at tcp://socket-proxy:2375, which only the server's
#   agents provide. The script keeps a `socket-proxy` container on a Docker
#   network named `woodpecker-exec` and attaches the steps to it. Both stay up
#   after the run for the next one.
#
# The run is trusted with volumes, as balanced is on the server, so a step's
# `volumes:` mounts host paths here too: `ci`'s cargo registry lands in
# /var/lib/woodpecker/cache/balanced/cargo on this machine, created by Docker.
#
# Output: one JSON line on stdout:
#   {"status": "green"|"red"|"error", "commit": sha, "log": path,
#    "failed_step": string?, "tail": [string]?, "next": string?}
#   green: every step passed.
#   red:   a step failed. "failed_step" names the last step that printed output,
#          "tail" holds the last 15 lines of the log. Read the full log for more.
#   error: the run couldn't start (bad arguments, no workflow file, Docker or
#          woodpecker-cli missing, the clone or the proxy failed). "next" says why.
# Exit codes: 0 green, 1 red, 2 error.
set -euo pipefail

cache=${HOME}/.cache/balanced-woodpecker-exec
clone=$cache/clone
log=$cache/exec.log
network=woodpecker-exec
proxy_image=tecnativa/docker-socket-proxy:v0.5.0

fail() {
  jq -nc --arg next "$1" '{status: "error", next: $next}'
  exit 2
}

[[ $# -ge 1 && $# -le 2 ]] || fail "usage: woodpecker-exec.sh <workflow> [<event>]"
workflow=$1 event=${2:-pull_request}
command -v woodpecker-cli >/dev/null || fail "woodpecker-cli is not on PATH"
docker info >/dev/null 2>&1 || fail "docker is unreachable; is the daemon running and the user in the docker group?"

repo=$(git rev-parse --show-toplevel 2>/dev/null) || fail "run this from inside the repository"
commit=$(git -C "$repo" rev-parse HEAD)
[[ -f $repo/$workflow ]] || fail "no workflow file at $workflow"

mkdir -p "$cache"
if [[ ! -d $clone/.git ]]; then
  git clone -q --no-checkout "$repo" "$clone" >/dev/null 2>&1 || fail "git clone of $repo failed"
fi
git -C "$clone" fetch -q "$repo" "$commit" >/dev/null 2>&1 || fail "git fetch of $commit into $clone failed"
git -C "$clone" reset -q --hard "$commit" || fail "git reset in $clone failed"
git -C "$clone" clean -qfdx -e target || fail "git clean in $clone failed"

docker network inspect "$network" >/dev/null 2>&1 ||
  docker network create "$network" >/dev/null || fail "docker network create $network failed"
if [[ $(docker inspect -f '{{.State.Running}}' socket-proxy 2>/dev/null) != true ]]; then
  docker rm -f socket-proxy >/dev/null 2>&1 || true
  docker run -d --name socket-proxy --network "$network" \
    -e BUILD=1 -e IMAGES=1 -e POST=1 -e CONTAINERS=1 \
    -v /var/run/docker.sock:/var/run/docker.sock:ro "$proxy_image" >/dev/null 2>&1 ||
    fail "starting the socket-proxy container failed"
elif ! docker inspect -f '{{json .NetworkSettings.Networks}}' socket-proxy | jq -e --arg n "$network" 'has($n)' >/dev/null; then
  docker network connect "$network" socket-proxy || fail "attaching socket-proxy to $network failed"
fi

if (cd "$clone" && woodpecker-cli exec --local --network "$network" --repo-trusted-volumes \
      --repo StanleyGoldman/balanced --pipeline-event "$event" "$workflow") >"$log" 2>&1; then
  jq -nc --arg commit "$commit" --arg log "$log" '{status: "green", commit: $commit, log: $log}'
  exit 0
fi

# Step output lines start `[<step>:L<n>:<secs>s]`.
step=$(grep -aoE '^\[[^]:]+:L[0-9]+:' "$log" | tail -1 | sed -E 's/^\[([^]:]+):.*/\1/' || true)
jq -nc --arg commit "$commit" --arg log "$log" --arg step "$step" \
  --rawfile tail <(tail -15 "$log" | sed 's/\x1b\[[0-9;]*m//g') \
  '{status: "red", commit: $commit, log: $log, failed_step: $step,
    tail: ($tail | rtrimstr("\n") | split("\n"))}'
exit 1
