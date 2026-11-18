#!/usr/bin/env bash
# E-76 Windows interop: the extended status script (tmux_agents.sh, status.md component 1) for
# eight open tabs, as the window asks it — ONE Windows wsl.exe for all tabs, with the argv
# WslHost::tmux_agents builds (`-d D -e sh -ls -- <tty> <server>...`) and the script on stdin.
# Timed over RUNS runs. The tmux server is a private one: TMUX_TMPDIR reaches the distro through
# WSLENV — the only difference from the app, which removes WSLENV — so the operator's own
# `rein-app` server is never read or changed. Each tab is a session with a real client attached
# on its own pty (script(1)), one pane carrying @rein_role, as rein start leaves it.
# A ninth pair names a tty with no client: it must answer `tab 8 5`.
set -uo pipefail
unset TMUX TMUX_PANE
REPO="$(git -C "$(dirname "$0")" rev-parse --show-toplevel)"
WSL=/mnt/c/Windows/System32/wsl.exe
D="$WSL_DISTRO_NAME"
SCRIPT="$REPO/crates/rein-desktop/src/tmux_agents.sh"
RUNS=${RUNS:-10}
P="$(mktemp -d /tmp/e76i.XXXX)"
export TMUX_TMPDIR="$P/t" WSLENV=TMUX_TMPDIR
mkdir -p "$TMUX_TMPDIR"; chmod 700 "$TMUX_TMPDIR"
pids=()
cleanup() { kill "${pids[@]}" 2>/dev/null; pkill -xf "sleep 6007" 2>/dev/null; tmux -L rein-app kill-server 2>/dev/null; rm -rf "$P"; }  # the private socket only
trap cleanup EXIT
ms() { echo $((($(date +%s%N) - $1) / 1000000)); }

echo "date: $(date -u +%FT%TZ)"
echo "wsl.exe: $("$WSL" --version 2>/dev/null | tr -d '\0\r' | head -1); distro: $D; tmux: $(tmux -V)"
echo "script: $(sha256sum "$SCRIPT" | cut -c1-12) (crates/rein-desktop/src/tmux_agents.sh at $(git -C "$REPO" rev-parse --short HEAD))"
echo
for n in 1 2 3 4 5 6 7 8; do
  tmux -L rein-app -f /dev/null new-session -d -s "s$n" -x 120 -y 40 "printf 'E76-$n\\n'; exec sleep 600"
  tmux -L rein-app split-window -t "s$n" "exec sleep 600"
  tmux -L rein-app set-option -p -t "s$n:0.0" @rein_role engineer
  tmux -L rein-app set-option -p -t "s$n:0.1" @rein_role architect
  # stdin is an idle pipe: /dev/zero would stream NUL keys into tmux and keep the server busy
  sleep 6007 | script -qfc "tmux -L rein-app attach -t s$n" /dev/null >/dev/null 2>&1 &
  pids+=($!)
done
for _ in $(seq 50); do
  [ "$(tmux -L rein-app list-clients 2>/dev/null | wc -l)" -ge 8 ] && break
  sleep 0.1
done
args=()
for n in 1 2 3 4 5 6 7 8; do
  args+=("$(tmux -L rein-app list-clients -t "s$n" -F '#{client_tty}' | head -1)" app)
done
args+=(/dev/pts/999 app)
echo "pairs (9: eight attached tabs and one tty with no client):"
echo "  ${args[*]}"
echo
echo "\$ wsl.exe -d $D -e sh -ls -- <the 18 words above> < tmux_agents.sh"
times=()
for r in $(seq "$RUNS"); do
  t0=$(date +%s%N)
  out="$("$WSL" -d "$D" -e sh -ls -- "${args[@]}" <"$SCRIPT" 2>&1 | tr -d '\r')"
  code=${PIPESTATUS[0]}
  times+=("$(ms "$t0")")
  if [ "$r" = 1 ]; then
    echo "--- output of run 1 (exit $code) ---"
    printf '%s\n' "$out" | sed 's/^/  | /'
    echo "---"
  fi
  [ "$code" = 0 ] || echo "run $r: exit $code"
done
echo
echo "runs: $RUNS; ms each: ${times[*]}"
printf '%s\n' "${times[@]}" | sort -n | awk '{a[NR]=$1} END {printf "min %d ms, median %d ms, max %d ms (target under 250 ms for eight tabs)\n", a[1], a[int((NR+1)/2)], a[NR]}'
