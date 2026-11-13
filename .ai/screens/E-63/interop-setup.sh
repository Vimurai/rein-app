#!/usr/bin/env bash
# E-63 Windows interop: each new or changed call through Windows wsl.exe exactly as WslHost builds
# its argv. Throwaway folders only; the real rein answers. The set-up child is run in a
# read-only folder so its `rein init` fails: the transport, the tty line first and exit 119
# are shown without `rein start` (no agents) ever running.
set -uo pipefail
unset TMUX TMUX_PANE
WSL=/mnt/c/Windows/System32/wsl.exe
D="$WSL_DISTRO_NAME"
SH="$SHELL"
REPO="$(git -C "$(dirname "$0")" rev-parse --show-toplevel)"
VALIDATE='case $PWD in /) exit 5;; esac; [ -x ~/.rein/bin/rein ] || exit 4; pwd -P; [ -d .ai ] && exit 0; IFS=; set -f; d=$(pwd -P); while [ $d != / ]; do d=${d%/*}; [ -z $d ] && d=/; [ -d $d/.ai ] && cd $d && pwd -P && exit 6; done; exit 3'
SETUP_USER='tty; ~/.rein/bin/rein init || exit 119; exec ~/.rein/bin/rein start'
N="$(mktemp -d /tmp/reine63nXXXXXX)"
R="$(mktemp -d /tmp/reine63rXXXXXX)"
cleanup() { chmod 755 "$R" 2>/dev/null; rm -rf "$N" "$R"; }
trap cleanup EXIT
ms() { echo $((($(date +%s%N) - $1) / 1000000)); }
run() { # run <label> <argv…> — stdout (CR stripped), exit, time
  local label=$1 t0 out code
  shift
  echo "== $label"
  echo "\$ wsl.exe $*" | sed "s|$REPO|<repo>|g"
  t0=$(date +%s%N)
  out="$("$WSL" "$@" </dev/null 2>&1 | tr -d '\r')"
  code=${PIPESTATUS[0]}
  printf '%s\n' "$out" | sed "s|$REPO|<repo>|g; s/^/  | /"
  echo "  exit $code in $(ms "$t0") ms"
  echo
}
echo "date: $(date -u +%FT%TZ)"
echo "wsl.exe: $("$WSL" --version 2>/dev/null | tr -d '\0\r' | head -1); distro: $D; shell: $SH"
echo "argv rule (no quote character): validate $(printf %s "$VALIDATE" | tr -cd "'\"" | wc -c), set-up $(printf %s "$SETUP_USER" | tr -cd "'\"" | wc -c)"
echo
run "validate: a folder without .ai/ — exit 3 and the canonical path, in the one call" -d "$D" --cd "$N" -e sh -c "$VALIDATE"
mkdir -p "$N/proj/.ai" "$N/proj/inner dir"
run "validate: a folder inside a Rein project (a space in its name) — exit 6, its path, then the project's, in the one call" -d "$D" --cd "$N/proj/inner dir" -e sh -c "$VALIDATE"
run "validate: the project itself — exit 0" -d "$D" --cd "$N/proj" -e sh -c "$VALIDATE"
rm -rf "$N/proj"
run "the card's rows: rein init --check in that folder" -d "$D" --cd "$N" -e "$SH" -lic "exec ~/.rein/bin/rein init --check"
run "the out-of-date verdict: rein init --check in a project (this repository)" -d "$D" --cd "$REPO" -e "$SH" -lic "exec ~/.rein/bin/rein init --check"
chmod 555 "$R"
run "set up (user server) in a read-only folder: init fails → 119, start never runs" -d "$D" --cd "$R" -e "$SH" -lic "$SETUP_USER"
echo "files in the read-only folder afterwards: [$(ls -A "$R" | tr '\n' ' ')]"
