#!/usr/bin/env bash
# E-66 Windows interop: V11's start-up sweep (the V8 setup process with the word `sweep`) and
# quit's kill-server, each through Windows wsl.exe exactly as WslHost builds the argv, with the
# constant scripts on stdin. A private TMUX_TMPDIR (carried by WSLENV for this run only) holds
# a session on the app's socket and one of the same name on the default server.
set -uo pipefail
unset TMUX TMUX_PANE
WSL=/mnt/c/Windows/System32/wsl.exe
D="$WSL_DISTRO_NAME"
SRC="$(git -C "$(dirname "$0")" rev-parse --show-toplevel)/crates/rein-desktop/src"
REAL_TMUX="$(command -v tmux)"
P="$(mktemp -d /tmp/rXX.XXXXXX)"; chmod 700 "$P"
SETUP="$(mktemp /tmp/reine66sXXXXXX)"
cleanup() {
  TMUX_TMPDIR="$P" "$REAL_TMUX" -L rein-app kill-server 2>/dev/null
  TMUX_TMPDIR="$P" "$REAL_TMUX" kill-server 2>/dev/null
  rm -rf "$P" "$SETUP"
}
trap cleanup EXIT
export WSLENV=TMUX_TMPDIR TMUX_TMPDIR="$P"
awk -v shim="$SRC/tmux_shim.sh" -v conf="$SRC/tmux_app.conf" '
  $0 == "@@SHIM@@" { while ((getline l < shim) > 0) print l; next }
  $0 == "@@CONF@@" { while ((getline l < conf) > 0) print l; next }
  { print }' "$SRC/tmux_setup.sh" >"$SETUP"
both() { "$REAL_TMUX" -L rein-app new-session -d -s demo "sh"; "$REAL_TMUX" new-session -d -s demo "sh"; }
state() { echo "app server: $("$REAL_TMUX" -L rein-app has-session -t demo 2>/dev/null && echo running || echo gone); default server's demo: $("$REAL_TMUX" has-session -t demo 2>/dev/null && echo running || echo gone)"; }
ms() { echo $((($(date +%s%N) - $1) / 1000000)); }
echo "date: $(date -u +%FT%TZ)"
echo "wsl.exe: $("$WSL" --version 2>/dev/null | tr -d '\0\r' | head -1); distro: $D; tmux: $("$REAL_TMUX" -V)"
echo "private TMUX_TMPDIR: $P"
echo
both
echo "== start-up sweep: the first tab's V8 setup with the word sweep (no process of its own)"
echo "before: $(state)"
echo "\$ wsl.exe -d $D -e sh -s -- sweep   (stdin: tmux_setup.sh with the shim and config in)"
t0=$(date +%s%N); "$WSL" -d "$D" -e sh -s -- sweep <"$SETUP" >/dev/null 2>&1; code=$?
echo "  exit $code in $(ms "$t0") ms"
echo "after:  $(state)"
echo
echo "== a later tab's setup (no sweep word) ends nothing"
"$REAL_TMUX" -L rein-app new-session -d -s demo "sh"
echo "before: $(state)"
t0=$(date +%s%N); "$WSL" -d "$D" -e sh -s <"$SETUP" >/dev/null 2>&1; code=$?
echo "  exit $code in $(ms "$t0") ms"
echo "after:  $(state)"
echo
echo "== quit: kill-server on the app's socket (limit 3 s)"
echo "before: $(state)"
echo "\$ wsl.exe -d $D -e sh -ls   (stdin: tmux_kill_server.sh)"
t0=$(date +%s%N); "$WSL" -d "$D" -e sh -ls <"$SRC/tmux_kill_server.sh" >/dev/null 2>&1; code=$?
echo "  exit $code in $(ms "$t0") ms"
echo "after:  $(state)"
echo
echo "== quit again with no app server: a no-op"
t0=$(date +%s%N); "$WSL" -d "$D" -e sh -ls <"$SRC/tmux_kill_server.sh" >/dev/null 2>&1; code=$?
echo "  exit $code in $(ms "$t0") ms; $(state)"
