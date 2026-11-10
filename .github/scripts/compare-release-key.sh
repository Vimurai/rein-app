#!/usr/bin/env bash
# Byte-compare two release-key files (D-017 tag gate; E-22).
# Usage: compare-release-key.sh OURS THEIRS
# Exit 0 only when both exist as regular files and are byte-identical; 1 otherwise.
set -euo pipefail

[ "$#" = 2 ] || { echo "usage: compare-release-key.sh OURS THEIRS" >&2; exit 1; }
for f in "$1" "$2"; do
  if [ -L "$f" ] || [ ! -f "$f" ]; then
    echo "::error::$f is missing or not a regular file" >&2
    exit 1
  fi
done
if ! cmp -s -- "$1" "$2"; then
  echo "::error::$1 differs from $2 — the key rein-app bakes is not the key rein's main publishes" >&2
  exit 1
fi
echo "release key matches: $1 == $2"
