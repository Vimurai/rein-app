#!/usr/bin/env bash
# Check that a minisign public-key file is well formed (D-014, D-017; E-16, E-22).
# Usage: check-release-key.sh [FILE]   (default: docs/contract/release-key.pub)
# Mirrors MinisignVerifier::from_pub_file in crates/rein-release/src/signature.rs: a file
# this script accepts is one the binary trusts, so a release never ships a key that would
# make every install fail. Exit 0 = valid; 1 = refused (reason on stderr as ::error::).
set -euo pipefail

F=${1:-docs/contract/release-key.pub}
fail() {
  echo "::error::$F $*" >&2
  exit 1
}

if [ -L "$F" ] || [ ! -f "$F" ]; then
  fail "is missing or not a regular file (D-014: the release trust root is a committed copy of rein's file) — refusing to build a release that cannot verify rein"
fi
if ! tr -d '\000' < "$F" | cmp -s - "$F"; then
  fail "contains a NUL byte"
fi
# Checked on the raw bytes: Git Bash's command substitution drops a CR, so the line checks
# below would not see one (E-22 hosted windows-latest run 42875792979).
if ! tr -d '\r' < "$F" | cmp -s - "$F"; then
  fail "contains a carriage return (CRLF line endings)"
fi
if [ "$(awk 'END { print NR }' "$F")" != 2 ]; then
  fail "does not have exactly two lines"
fi
COMMENT=$(sed -n 1p "$F")
KEY=$(sed -n 2p "$F")
printf '%s\n' "$COMMENT" | grep -Eq '^untrusted comment: minisign public key:? [0-9A-F]{16}$' \
  || fail "line 1 is not 'untrusted comment: minisign public key <ID>'"
printf '%s\n' "$KEY" | grep -Eq '^[A-Za-z0-9+/]{56}$' \
  || fail "line 2 is not a 56-character base64 minisign key"
# A minisign Ed25519 public key starts with the algorithm bytes "Ed".
[ "$(printf '%s' "$KEY" | base64 -d | head -c 2)" = Ed ] \
  || fail "is not a minisign Ed25519 public key"
# The id in the comment must be the id inside the key (bytes 2..10, little-endian).
NAMED=${COMMENT##* }
ACTUAL=$(printf '%s' "$KEY" | base64 -d | od -An -tx1 -j2 -N8 \
  | awk '{ for (i = NF; i > 0; i--) printf "%s", toupper($i); print "" }') \
  || fail "line 2 is not valid base64"
[ "$NAMED" = "$ACTUAL" ] || fail "comment names key $NAMED but the key is $ACTUAL"
echo "release key: $ACTUAL"
