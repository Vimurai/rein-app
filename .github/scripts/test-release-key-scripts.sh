#!/usr/bin/env bash
# Corpus test for check-release-key.sh and compare-release-key.sh (D-017 ruling 3; E-22).
# Runs in ci.yml on Linux, macOS and Windows (Git Bash) with no network. Fixtures are built
# in a fresh temp dir from the committed key, so no malformed key is ever committed.
# Usage (from the repo root): bash .github/scripts/test-release-key-scripts.sh
set -uo pipefail

HERE=$(cd "$(dirname "$0")" && pwd)
KEYFILE=${1:-docs/contract/release-key.pub}
CHECK="$HERE/check-release-key.sh"
COMPARE="$HERE/compare-release-key.sh"

WORK=$(mktemp -d "${TMPDIR:-/tmp}/release-key-corpus.XXXXXXXX")
trap 'rm -rf -- "$WORK"' EXIT

failures=0
skips=0
expect() { # expect <want-exit 0|1> <reason-substring> <label> <command…>
  # A refusal must be exit 1 with its own reason on stderr: a crash (exit 2, 127, …) or a
  # refusal for another reason fails the case.
  local want=$1 reason=$2 label=$3
  shift 3
  local err
  err=$("$@" 2>&1 >/dev/null)
  local got=$?
  if [ "$got" = "$want" ] && { [ -z "$reason" ] || [[ "$err" == *"$reason"* ]]; }; then
    echo "ok    $label"
  else
    echo "FAIL  $label (exit $got, want $want${reason:+ with '$reason'}): $err"
    failures=$((failures + 1))
  fi
}
fixture() { # fixture <name> <printf-format> [args…] → path
  local f="$WORK/$1"
  shift
  # shellcheck disable=SC2059
  printf "$@" > "$f"
  echo "$f"
}

COMMENT=$(sed -n 1p "$KEYFILE")
KEY=$(sed -n 2p "$KEYFILE")
ID=${COMMENT##* }
# The same key with algorithm bytes "Eg" instead of "Ed" (still 56 base64 chars).
EG_KEY=$(printf '%s' "$KEY" | base64 -d | { printf 'Eg'; tail -c +3; } | base64 | tr -d '\n')

# --- check-release-key.sh: accepted ---------------------------------------------------
expect 0 "" "committed key"                 bash "$CHECK" "$KEYFILE"
expect 0 "" "no trailing newline"           bash "$CHECK" "$(fixture no-eol '%s\n%s' "$COMMENT" "$KEY")"
expect 0 "" "comment with colon form"       bash "$CHECK" "$(fixture colon 'untrusted comment: minisign public key: %s\n%s\n' "$ID" "$KEY")"

# --- check-release-key.sh: refused ----------------------------------------------------
expect 1 "not a regular file" "missing file"                  bash "$CHECK" "$WORK/does-not-exist"
expect 1 "exactly two lines" "empty file"                    bash "$CHECK" "$(fixture empty '')"
expect 1 "not a regular file" "directory"                     bash "$CHECK" "$WORK"
expect 1 "exactly two lines" "one line (key only)"           bash "$CHECK" "$(fixture one-line '%s\n' "$KEY")"
expect 1 "exactly two lines" "three lines"                   bash "$CHECK" "$(fixture three '%s\n%s\nextra\n' "$COMMENT" "$KEY")"
expect 1 "exactly two lines" "trailing blank line"           bash "$CHECK" "$(fixture blank '%s\n%s\n\n' "$COMMENT" "$KEY")"
expect 1 "line 1 is not" "CRLF line endings"             bash "$CHECK" "$(fixture crlf '%s\r\n%s\r\n' "$COMMENT" "$KEY")"
expect 1 "NUL byte" "NUL byte after the key"        bash "$CHECK" "$(fixture nul '%s\n%s\n\000' "$COMMENT" "$KEY")"
expect 1 "not a minisign Ed25519" "non-Ed algorithm bytes"        bash "$CHECK" "$(fixture eg '%s\n%s\n' "$COMMENT" "$EG_KEY")"
expect 1 "comment names key" "comment id != key id"          bash "$CHECK" "$(fixture wrong-id 'untrusted comment: minisign public key 0000000000000000\n%s\n' "$KEY")"
expect 1 "line 1 is not" "lower-case key id"             bash "$CHECK" "$(fixture lower-id '%s\n%s\n' "$(printf '%s' "$COMMENT" | tr 'A-F' 'a-f')" "$KEY")"
expect 1 "line 1 is not" "other comment"                 bash "$CHECK" "$(fixture comment 'untrusted comment: hello\n%s\n' "$KEY")"
expect 1 "line 2 is not" "truncated key"                 bash "$CHECK" "$(fixture short '%s\n%s\n' "$COMMENT" "${KEY:0:40}")"
expect 1 "line 2 is not" "space before the key"          bash "$CHECK" "$(fixture space '%s\n %s\n' "$COMMENT" "$KEY")"
if ln -s "$KEYFILE" "$WORK/link" 2>/dev/null && [ -L "$WORK/link" ]; then
  expect 1 "not a regular file" "symlink to the committed key" bash "$CHECK" "$WORK/link"
else
  echo "SKIP  symlink to the committed key (this host cannot create symlinks)"
  skips=$((skips + 1))
fi

# --- compare-release-key.sh ------------------------------------------------------------
# Comparison fixtures reuse files built above: wrong-id (other bytes), crlf (same key, CRLF).
cp "$KEYFILE" "$WORK/same"
expect 0 "" "compare: byte-identical"       bash "$COMPARE" "$KEYFILE" "$WORK/same"
expect 1 "differs from" "compare: different bytes"      bash "$COMPARE" "$KEYFILE" "$WORK/wrong-id"
expect 1 "differs from" "compare: same key, CRLF"       bash "$COMPARE" "$KEYFILE" "$WORK/crlf"
expect 1 "not a regular file" "compare: theirs absent"        bash "$COMPARE" "$KEYFILE" "$WORK/does-not-exist"
expect 1 "not a regular file" "compare: ours absent"          bash "$COMPARE" "$WORK/does-not-exist" "$WORK/same"
expect 1 "usage" "compare: one argument"         bash "$COMPARE" "$KEYFILE"

echo "release-key corpus: $failures failed, $skips skipped"
[ "$failures" = 0 ]
