#!/usr/bin/env bash
# Proves a test can go red: runs the tests on the code as it is, which must pass,
# then breaks the code they cover, runs them again, and puts the file back exactly as
# it was. A test that already fails would read as red from any break.
#
# Usage: red-check.sh <file> <old> <new> <test command...>
#   <file>  the source file to break.
#   <old>   text that occurs exactly once in <file>; may span lines.
#   <new>   the text that replaces it (may be empty).
#   <test command...>  a `cargo nextest run`, run as given, e.g.
#                      cargo nextest run -p balanced_ui --features test-support --test register name
#                      (pass `--features test-support` for any test file its
#                      crate's Cargo.toml gives `required-features`: every
#                      balanced_ui file and most of balanced_core's, split_lines
#                      included. Without it nextest refuses to build them.)
#                      Only nextest's captured output is parsed: a `cargo test`
#                      command always comes back as an error, and `--no-capture`
#                      leaves "panics" empty.
#
# The file is restored by writing its saved text back, never by reversing the
# replacement, so a <new> that is empty or occurs elsewhere can't damage it. Writing
# moves the mtime forward, so cargo rebuilds it on the next run. An interrupted run
# (Ctrl+C, SIGTERM, a hang-up) restores it too and stops the tests; only SIGKILL
# leaves it broken. A break that removes a loop guard can make a test loop or
# allocate without end: a hung test is killed by nextest's slow-timeout, and a test
# that hits the memory cap shows `SIGKILL`; both come back as status "error".
#
# Output: one JSON line on stdout:
#   {"status": "red"|"green"|"error", "failed": [test names], "panics": [string],
#    "restored": bool, "next": string?}
#   red:   the test command failed with at least one nextest `FAIL` line, every
#          failed test's name listed once in "failed". "panics" holds one entry per
#          failed test that panicked, in the order nextest prints them: the
#          `panicked at <file:line:col>` line plus the message after it
#          (`left:`/`right:`, "No nodes found", …), at most 6 lines joined by "\n".
#          A test that crashed (`SIGSEGV`) or timed out isn't a `FAIL`, so it is
#          missing from both.
#          Read each before trusting the red: a setup failure proves nothing.
#          A failing `.slt` file (balanced_store's `slt` target) doesn't panic, so
#          its "panics" is empty. Read its failing record by rerunning the test on
#          the broken code; a record that fails in setup proves nothing.
#          Only a red result carries "panics".
#   green: the tests pass against the broken code; they miss this break. Or another
#          guard covers the same behaviour, such as a stale cell address that a
#          reset clears and a later row lookup also drops, each alone. This
#          script breaks one place, so break every guard together by hand: `cp` each
#          file aside, edit, run the test, then `cp` the copies back so mtimes move
#          forward, and `cmp` to confirm the restore.
#   error: bad arguments, or <old> not found exactly once, or the tests fail before
#          the break, with their names in "failed" (the file is untouched in all three);
#          or the test command failed with no `FAIL` line, usually a break
#          that doesn't compile (the file is restored; "restored" says so). No test
#          ran red, so make a break that compiles, or one that doesn't loop forever.
# Exit codes: 0 red, 1 green, 2 error.
set -euo pipefail

if [[ $# -lt 4 ]]; then
  jq -nc '{status: "error", failed: [], restored: true,
           next: "usage: red-check.sh <file> <old> <new> <test command...>"}'
  exit 2
fi
file=$1 old=$2 new=$3
shift 3

# Runs the test command in the background, so a signal reaches the traps at once:
# bash holds a trap until a foreground child exits, and a looping test never does.
# `setsid` gives it a process group of its own, which the trap stops whole: cargo
# doesn't pass a signal on to nextest or the tests.
tests=
run_tests() {
  setsid "$@" &
  tests=$!
  wait "$tests"
}

saved=$(mktemp)
log=$(mktemp)
broken=false
# Put the file back on any exit while it is broken, then remove the temp files.
trap '$broken && cat -- "$saved" >"$file"; rm -f "$saved" "$log"' EXIT
trap '[[ -n $tests ]] && kill -- -"$tests" 2>/dev/null; exit 130' INT TERM HUP
cp -- "$file" "$saved"

# nextest prints each `FAIL [<time>] (<n>/<total>) <binary> <test>` line twice: as the
# test fails, and again in the closing summary.
# The counter is right-aligned to the total's width: `( 1/11)`.
fail_line='^ +FAIL \[ *[0-9.]+s\] \( *[0-9]+/[0-9]+\) [^ ]+ [^ ]+$'
failed_tests() {
  { grep -E "$fail_line" "$log" || true; } | awk '{print $NF}' | awk '!seen[$0]++' |
    jq -R . | jq -sc .
}

# The tests must pass before the break, or the break proves nothing.
if ! run_tests "$@" >"$log" 2>&1; then
  jq -nc --argjson failed "$(failed_tests)" '{status: "error", failed: $failed, restored: true,
    next: "the tests fail on the unbroken code: make them pass first, then run this again"}'
  exit 2
fi

# Break the file: replace the one occurrence of <old>. Marked broken first, so a
# signal during the write still restores it.
broken=true
if ! python3 -I -c '
import sys
path, old, new = sys.argv[1:4]
text = open(path).read()
if text.count(old) != 1:
    sys.exit(f"found {text.count(old)} times")
open(path, "w").write(text.replace(old, new))
' "$file" "$old" "$new" 2>"$log"; then
  broken=false
  jq -nc --arg why "$(cat "$log")" '{status: "error", failed: [], restored: true,
    next: ("<old> must occur exactly once in the file: " + $why)}'
  exit 2
fi

set +e
run_tests "$@" >"$log" 2>&1
code=$?
set -e

# Restore by writing the saved text back over the file, which keeps the newer mtime.
cat -- "$saved" >"$file"
broken=false
restored=false
cmp -s -- "$saved" "$file" && restored=true

failed=$(failed_tests)
if [[ $code -ne 0 && $failed == "[]" ]]; then
  jq -nc --argjson restored "$restored" '{status: "error", failed: [], restored: $restored,
    next: "the break did not compile, a test crashed, timed out or hit the memory cap, or the command was not `cargo nextest run`: rerun the command to read its output, make a break that compiles and fails an assertion, then run this again"}'
  exit 2
fi
if [[ $code -ne 0 ]]; then
  # One entry per failed test: the first panic after its `FAIL` line, at most 6 lines,
  # ending at a blank line, the `note:` hint or the next `FAIL`. nextest indents the
  # test's output by 4 spaces, which is stripped. The summary's repeated `FAIL` lines
  # have no panic after them, so they add nothing. Entries are separated by the record
  # separator \036 for jq to split on.
  panics=$(awk -v fail_line="$fail_line" '
    function close_entry() { if (n > 0) printf "\036"; take = 0; n = 0 }
    $0 ~ fail_line { close_entry(); take = 1; next }
    { sub(/^    /, "") }
    take && n == 0 { if (/panicked at/) { printf "%s", $0; n = 1 }; next }
    take {
      if ($0 == "" || /^note: / || n >= 6) { close_entry(); next }
      printf "\n%s", $0; n++
    }
    END { close_entry() }
  ' "$log" | jq -Rsc 'split("\u001e") | map(select(length > 0))')
  jq -nc --argjson failed "$failed" --argjson panics "$panics" --argjson restored "$restored" \
    '{status: "red", failed: $failed, panics: $panics, restored: $restored}'
  exit 0
fi
jq -nc --argjson restored "$restored" '{status: "green", failed: [], restored: $restored,
  next: "the tests pass against the broken code: strengthen them, then run this again"}'
exit 1
