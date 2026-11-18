# window-binary.sh — sourced by every window harness (the Xvfb screen scripts beside it; E-84,
# D-061 addendum 5 ruling 4). `cargo test` rebuilds target/debug/rein-desktop in a form that
# shows a blank window (E-70: two harness runs looked broken for that reason only, measured
# 2026-11-18: 220,449,560 bytes after `cargo test`, 220,657,688 after `cargo build`), and the
# binary embeds ui/dist, which `cargo build` does not rebuild from ui/src (E-91: a run tested
# the old UI). So before a harness starts the window:
#   - the web bundle older than any ui/src file → stop with one sentence (npm run build);
#   - then `cargo build -p rein-desktop`: a no-op when the binary is the right one, the right
#     one again when `cargo test` left its own.
# Only for the workspace's own debug binary: a harness given another binary uses it as given.
# Sets nothing in the caller; exits the harness (status 2) with one sentence on failure.

window_binary() { # window_binary [binary]
  local repo want got ui
  repo="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../../.." && pwd -P)"
  want="$repo/target/debug/rein-desktop"
  if [ -n "${1:-}" ]; then
    got="$(cd "$(dirname "$1")" 2>/dev/null && pwd -P)/$(basename "$1")"
    [ "$got" = "$want" ] || return 0
  fi
  ui="$repo/crates/rein-desktop/ui"
  if [ ! -f "$ui/dist/index.html" ] ||
    [ -n "$(find "$ui/src" "$ui/index.html" -newer "$ui/dist/index.html" -print -quit 2>/dev/null)" ]; then
    echo "window harness: the web UI bundle is older than ui/src — run npm run build in crates/rein-desktop/ui, then run this again." >&2
    exit 2
  fi
  (cd "$repo" && cargo build -q -p rein-desktop) ||
    { echo "window harness: cargo build -p rein-desktop failed — fix the build, then run this again." >&2; exit 2; }
}
