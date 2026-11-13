#!/usr/bin/env bash
# E-62 Windows interop: the status script and the close, each through Windows wsl.exe exactly
# as WslHost builds the argv, against a private tmux (TMUX_TMPDIR carried by WSLENV for this
# run only) and a throwaway project. The real rein performs the kill.
set -uo pipefail
unset TMUX TMUX_PANE
SRC=/home/emir/Dev/rein-app/crates/rein-desktop/src
WSL=/mnt/c/Windows/System32/wsl.exe
REAL_TMUX="$(command -v tmux)"
P="$(mktemp -d /tmp/rXX.XXXXXX)"; chmod 700 "$P"
T="$(mktemp -d /tmp/reine62XXXXXX)"; T="$(cd "$T" && pwd -P)"
cleanup() {
  TMUX_TMPDIR="$P" "$REAL_TMUX" -L host kill-server 2>/dev/null
  TMUX_TMPDIR="$P" "$REAL_TMUX" -L rein-app kill-server 2>/dev/null
  rm -rf "$P" "$T"
}
trap cleanup EXIT
tm() { TMUX_TMPDIR="$P" "$REAL_TMUX" "$@" 2>/dev/null; }
export WSLENV=TMUX_TMPDIR TMUX_TMPDIR="$P"

mkdir "$T/.ai"
printf '{"roles":{"architect":{"provider":"claude","pane_identifier":"1"},"engineer":{"provider":"claude","pane_identifier":"0"}}}\n' >"$T/.ai/roles.json"
W="$(basename "$T")"
echo "date: $(date -u +%FT%TZ)"
echo "wsl.exe: $WSL ($("$WSL" --version 2>/dev/null | tr -d '\0' | head -1 | tr -d '\r'))"
echo "tmux: $("$REAL_TMUX" -V); distro: $WSL_DISTRO_NAME; shell: $SHELL"
echo "project: $T (throwaway); private TMUX_TMPDIR: $P"

# The project's session on the app's server, stamped as rein stamps it; two role panes.
tm -L rein-app -f ~/.local/state/rein-app/tmux/tmux.conf new-session -d -s e62 -n "$W" -c "$T" -x 120 -y 40
tm -L rein-app set-environment -t e62 REIN_PROJECT "$T"
tm -L rein-app split-window -d -t "e62:$W" -c "$T"
tm -L rein-app new-window -d -t e62 -n other
panes=($(tm -L rein-app list-panes -t "e62:$W" -F '#{pane_id}'))
tm -L rein-app set-option -p -t "${panes[0]}" @rein_role engineer
tm -L rein-app set-option -p -t "${panes[1]}" @rein_role architect
sleep 2 # each pane's login shell ready before the agent stand-in is typed
for p in "${panes[@]}"; do tm -L rein-app send-keys -t "$p" 'sleep 600' Enter; done
for _ in $(seq 50); do [ "$(tm -L rein-app list-panes -t "e62:$W" -F '#{pane_current_command}' | grep -c sleep)" = 2 ] && break; sleep 0.1; done
tm -L host new-session -d -s c -x 120 -y 40 "env -u TMUX $REAL_TMUX -L rein-app attach -t e62:$W"
for _ in $(seq 50); do [ -n "$(tm -L rein-app list-clients)" ] && break; sleep 0.1; done
tty="$(tm -L host display-message -p -t c '#{pane_tty}')"
sleep 1
echo "tab client tty: $tty"

status() {
  echo "\$ wsl.exe -d $WSL_DISTRO_NAME -e sh -ls -- $tty app   (stdin: src/tmux_agents.sh)"
  local t0 t1 out
  t0=$(date +%s%N)
  out="$("$WSL" -d "$WSL_DISTRO_NAME" -e sh -ls -- "$tty" app <"$SRC/tmux_agents.sh")"
  local code=$?
  t1=$(date +%s%N)
  printf '%s\n' "$out" | sed 's/^/  | /'
  echo "  exit $code in $(((t1 - t0) / 1000000)) ms"
}
echo; echo "== status: both agents running"; status
tm -L rein-app send-keys -t "${panes[0]}" C-c; sleep 1
echo; echo "== status: the Engineer exited"; status
echo; echo "== status: no client on that tty (fixed sentence: the tab's tmux client was not found)"
"$WSL" -d "$WSL_DISTRO_NAME" -e sh -ls -- /dev/pts/999 app <"$SRC/tmux_agents.sh"; echo "  exit $?"

KILL="$(cat "$SRC/tmux_start.sh") --kill --yes"
echo; echo "== close: the kill constant through wsl.exe (quote characters in argv: $(printf %s "$KILL" | tr -cd "'\"" | wc -c))"
echo "\$ wsl.exe -d $WSL_DISTRO_NAME --cd $T -e $SHELL -lic '<tmux_server::kill_script()>'"
echo "windows before: $(tm -L rein-app list-windows -t e62 -F '#W' | tr '\n' ' ')"
t0=$(date +%s%N)
"$WSL" -d "$WSL_DISTRO_NAME" --cd "$T" -e "$SHELL" -lic "$KILL" </dev/null 2>/dev/null | tr -d '\r' | sed 's/^/  | /'
code=${PIPESTATUS[0]}
t1=$(date +%s%N)
echo "  exit $code in $(((t1 - t0) / 1000000)) ms"
echo "windows after: $(tm -L rein-app list-windows -t e62 -F '#W' | tr '\n' ' ')"
echo "files rein wrote in the project: $(cd "$T" && find . -type f ! -path './.ai/roles.json' | sort | tr '\n' ' ')"
echo "the default server under this TMUX_TMPDIR: $(tm list-sessions -F '#S' || true)(none)"
