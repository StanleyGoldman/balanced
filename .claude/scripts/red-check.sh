#!/usr/bin/env bash
# Proves a test can go red: breaks the code it covers, runs the tests, then puts the
# file back exactly as it was.
#
# Usage: red-check.sh <file> <old> <new> <test command...>
#   <file>  the source file to break.
#   <old>   text that occurs exactly once in <file>; may span lines.
#   <new>   the text that replaces it (may be empty).
#   <test command...>  a `cargo nextest run`, run as given, e.g.
#                      cargo nextest run -p balanced_ui --features test-support --test register name
#                      Only nextest's captured output is parsed: a `cargo test`
#                      command always comes back as an error, and `--no-capture`
#                      leaves "panics" empty.
#
# The file is restored by writing its saved text back, never by reversing the
# replacement, so a <new> that is empty or occurs elsewhere can't damage it. Writing
# moves the mtime forward, so cargo rebuilds it on the next run.
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
#          Only a red result carries "panics".
#   green: the tests pass against the broken code; they miss this break.
#   error: bad arguments, or <old> not found exactly once (the file is untouched);
#          or the test command failed with no `FAIL` line, usually a break
#          that doesn't compile (the file is restored; "restored" says so). No test
#          ran red, so make a break that compiles.
# Exit codes: 0 red, 1 green, 2 error.
set -euo pipefail

if [[ $# -lt 4 ]]; then
  jq -nc '{status: "error", failed: [], restored: true,
           next: "usage: red-check.sh <file> <old> <new> <test command...>"}'
  exit 2
fi
file=$1 old=$2 new=$3
shift 3

saved=$(mktemp)
log=$(mktemp)
trap 'rm -f "$saved" "$log"' EXIT
cp -- "$file" "$saved"

# Break the file: replace the one occurrence of <old>.
if ! python3 -I -c '
import sys
path, old, new = sys.argv[1:4]
text = open(path).read()
if text.count(old) != 1:
    sys.exit(f"found {text.count(old)} times")
open(path, "w").write(text.replace(old, new))
' "$file" "$old" "$new" 2>"$log"; then
  jq -nc --arg why "$(cat "$log")" '{status: "error", failed: [], restored: true,
    next: ("<old> must occur exactly once in the file: " + $why)}'
  exit 2
fi

set +e
"$@" >"$log" 2>&1
code=$?
set -e

# Restore by writing the saved text back over the file, which keeps the newer mtime.
cat -- "$saved" >"$file"
restored=false
cmp -s -- "$saved" "$file" && restored=true

# nextest prints each `FAIL [<time>] (<n>/<total>) <binary> <test>` line twice: as the
# test fails, and again in the closing summary.
# The counter is right-aligned to the total's width: `( 1/11)`.
fail_line='^ +FAIL \[ *[0-9.]+s\] \( *[0-9]+/[0-9]+\) [^ ]+ [^ ]+$'
failed=$({ grep -E "$fail_line" "$log" || true; } | awk '{print $NF}' | awk '!seen[$0]++' |
  jq -R . | jq -sc .)
if [[ $code -ne 0 && $failed == "[]" ]]; then
  jq -nc --argjson restored "$restored" '{status: "error", failed: [], restored: $restored,
    next: "the break did not compile, a test crashed or timed out, or the command was not `cargo nextest run`: rerun the command to read its output, make a break that compiles and fails an assertion, then run this again"}'
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
