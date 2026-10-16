#!/usr/bin/env bash
# Proves a test can go red: breaks the code it covers, runs the tests, then puts the
# file back exactly as it was.
#
# Usage: red-check.sh <file> <old> <new> <test command...>
#   <file>  the source file to break.
#   <old>   text that occurs exactly once in <file>; may span lines.
#   <new>   the text that replaces it (may be empty).
#   <test command...>  run as given, e.g. cargo test -p balanced_ui --test register -- name
#
# The file is restored by writing its saved text back, never by reversing the
# replacement, so a <new> that is empty or occurs elsewhere can't damage it. Writing
# moves the mtime forward, so cargo rebuilds it on the next run.
#
# Output: one JSON line on stdout:
#   {"status": "red"|"green"|"error", "failed": [test names], "panics": [string],
#    "restored": bool, "next": string?}
#   red:   the test command failed with at least one `test … FAILED` name, all listed
#          in "failed". "panics" holds one entry per failed test, in the order cargo
#          prints them: the `panicked at <file:line:col>` line plus the message after
#          it (`left:`/`right:`, "No nodes found", …), at most 6 lines joined by "\n".
#          Read each before trusting the red: a setup failure proves nothing.
#          Only a red result carries "panics".
#   green: the tests pass against the broken code; they miss this break.
#   error: bad arguments, or <old> not found exactly once (the file is untouched);
#          or the test command failed with no `test … FAILED` line, usually a break
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

failed=$({ grep -oE '^test [^ ]+ \.\.\. FAILED' "$log" || true; } | awk '{print $2}' | jq -R . | jq -sc .)
if [[ $code -ne 0 && $failed == "[]" ]]; then
  jq -nc --argjson restored "$restored" '{status: "error", failed: [], restored: $restored,
    next: "the break did not compile or no test failed: make a break that compiles, then run this again"}'
  exit 2
fi
if [[ $code -ne 0 ]]; then
  # One entry per failed test: the first panic in its `---- <name> stdout ----`
  # section, at most 6 lines, ending at a blank line, cargo's `note:` hint or the next
  # section. Entries are separated by the record separator \036 for jq to split on.
  panics=$(awk '
    function close_entry() { if (n > 0) printf "\036"; take = 0; n = 0 }
    /^---- .* stdout ----$/ { close_entry(); take = 1; next }
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
