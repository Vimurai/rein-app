#!/usr/bin/env bash
# agents.sh — E-62 / D-057, desktop.md §Look and feel v2 V9 "Tests": the agent-exit bar's two
# processes at the boundary they are made for, Linux and macOS. The status script
# (src/tmux_agents.sh) on stdin for the tab's client tty and server; the close — the start
# constant (src/tmux_start.sh) made into `rein start --kill --yes`, as tmux_server::kill_script
# builds it — against a stand-in rein. New suite: nothing asked tmux about agents before; the
# V8 start/shim/setup stay in tests/e2e/tmux-server.sh.
# Isolation: HOME is a mktemp dir; every tmux server lives under a private TMUX_TMPDIR (TMUX
# unset); the user's own servers are never reached. Needs: tmux >= 3.2, sh.
set -uo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
SRC="$(git -C "$HERE" rev-parse --show-toplevel)/crates/rein-desktop/src"
REAL_TMUX="$(command -v tmux)" || { echo "agents: tmux not found" >&2; exit 2; }
W=""
cleanup() {
  [ -z "$W" ] && return
  for s in rein-app host default; do
    if [ "$s" = default ]; then TMUX_TMPDIR="$W/t" "$REAL_TMUX" kill-server 2>/dev/null || true
    else TMUX_TMPDIR="$W/t" "$REAL_TMUX" -L "$s" kill-server 2>/dev/null || true; fi
  done
  rm -rf "$W"
}
trap cleanup EXIT
unset TMUX TMUX_PANE XDG_STATE_HOME
W="$(mktemp -d /tmp/rein-ag.XXXXXX)"
W="$(cd "$W" && pwd -P)"
mkdir -p "$W/t" "$W/proj" && chmod 700 "$W/t"

fails=0
ok() { echo "ok   $1"; }
no() { echo "FAIL $1"; fails=$((fails + 1)); }
check() { if [ "$2" = "$3" ]; then ok "$1"; else no "$1 — expected [$2], got [$3]"; fi; }

H="$W/home"
mkdir -p "$H/.rein/bin"
# rein's own shape: one session per project with the project's window; --kill removes that
# window by name through plain `tmux` (the shim on the app's server).
cat >"$H/.rein/bin/rein" <<'EOF'
#!/bin/sh
if [ "$*" = "start --kill --yes" ]; then
  w=$(tmux list-windows -t demo -F '#{window_id} #{window_name}' 2>/dev/null | awk '$2 == "proj" { print $1 }')
  [ -n "$w" ] && tmux kill-window -t "$w" && echo killed
  exit 0
fi
tmux new-session -d -s demo -n proj -x 120 -y 40 && tmux split-window -d -t demo:proj && tmux new-window -d -t demo -n other
EOF
chmod +x "$H/.rein/bin/rein"
awk -v shim="$SRC/tmux_shim.sh" -v conf="$SRC/tmux_app.conf" '
  $0 == "@@SHIM@@" { while ((getline l < shim) > 0) print l; next }
  $0 == "@@CONF@@" { while ((getline l < conf) > 0) print l; next }
  { print }' "$SRC/tmux_setup.sh" | HOME="$H" /bin/sh -s >/dev/null 2>&1 || { echo "setup failed" >&2; exit 1; }
START="$(cat "$SRC/tmux_start.sh")"
KILL="$START --kill --yes"
ENV=(env -i HOME="$H" PATH="$(dirname "$REAL_TMUX"):/usr/bin:/bin" TMUX_TMPDIR="$W/t" SHELL=/bin/sh)
tm() { "${ENV[@]}" "$REAL_TMUX" "$@" 2>/dev/null; }
status() { (cd / && "${ENV[@]}" /bin/sh -s -- "$1" "$2" <"$SRC/tmux_agents.sh" 2>/dev/null); }
code() { status "$@" >/dev/null; echo $?; }
wait_for() { for _ in $(seq 1 100); do "$@" && return 0; sleep 0.1; done; return 1; }

echo "tmux under test: $("$REAL_TMUX" -V)"

# The tab's project: started through the start constant on the app's server, as the tab does.
(cd "$W/proj" && "${ENV[@]}" sh -c "$START" >/dev/null 2>&1)
# The same names on the user's default server: the close must never reach them.
tm new-session -d -s demo -n proj -x 80 -y 24 && tm new-window -d -t demo -n other
panes=($(tm -L rein-app list-panes -t demo:proj -F '#{pane_id}'))
tm -L rein-app set-option -p -t "${panes[0]}" @rein_role architect
tm -L rein-app set-option -p -t "${panes[1]}" @rein_role engineer
for p in "${panes[@]}"; do tm -L rein-app send-keys -t "$p" 'sleep 600' Enter; done
# The tab's client: a real tty, from a pane of a second server that attaches to the session.
tm -L host new-session -d -s c -x 120 -y 40 "env -u TMUX $REAL_TMUX -L rein-app attach -t demo:proj"
attached() { [ "$(tm -L rein-app list-clients | wc -l | tr -d ' ')" -ge 1 ]; }
wait_for attached || { echo "the client did not attach" >&2; exit 1; }
tty="$(tm -L host display-message -p -t c '#{pane_tty}')"
echo "tab client: $tty"

running() { [ "$(status "$tty" app | grep -c '|sleep|0$')" = "$1" ]; }
wait_for running 2
check "both agents run: one line per pane, role|command|dead" "architect|sleep|0 engineer|sleep|0" "$(status "$tty" app | sort | tr '\n' ' ' | sed 's/ $//')"
check "only the window the client shows (not the session's other window)" 2 "$(status "$tty" app | wc -l | tr -d ' ')"

before="$(tm -L rein-app show-options -g; tm -L rein-app show-options -p -t "${panes[1]}"; tm -L rein-app list-sessions)"
tm -L rein-app send-keys -t "${panes[1]}" C-c
ended() { status "$tty" app | grep -q '^engineer|sh|0$\|^engineer|bash|0$\|^engineer|dash|0$'; }
wait_for ended
check "the Engineer exited: its pane shows its shell" yes "$(ended && echo yes || echo no)"
check "the Architect still runs" 1 "$(status "$tty" app | grep -c '^architect|sleep|0$')"
after="$(tm -L rein-app show-options -g; tm -L rein-app show-options -p -t "${panes[1]}"; tm -L rein-app list-sessions)"
check "the status run changed nothing in tmux" yes "$([ "$before" = "$after" ] && echo yes || echo no)"

check "a tty tmux would not name exits 2" 2 "$(code /dev/pts/x app)"
check "a server word not in the list exits 2" 2 "$(code "$tty" other)"
check "no client on that tty exits 5 (no wait)" 5 "$(code /dev/pts/999 app)"
check "the user's server has no such client: exit 5" 5 "$(code "$tty" user)"
check "no tmux exits 3" 3 "$( (cd / && env -i HOME="$H" PATH=/nonexistent /bin/sh -s -- "$tty" app <"$SRC/tmux_agents.sh" >/dev/null 2>&1); echo $?)"

# An ordinary tab close detaches its client: the session keeps running (D-034).
tm -L host kill-server
gone() { [ "$(tm -L rein-app list-clients | wc -l | tr -d ' ')" = 0 ]; }
wait_for gone
check "tab closed the ordinary way: the project's window still runs" proj "$(tm -L rein-app list-windows -t demo -F '#W' | grep -x proj)"

# Close project: the kill constant in the project, on the app's server only.
check "kill: no quote character" 0 "$(printf '%s' "$KILL" | tr -cd "'\"" | wc -c | tr -d ' ')"
out="$(cd "$W/proj" && "${ENV[@]}" TMUX=/x TMUX_PANE=%1 sh -c "$KILL" 2>&1)"
check "kill: rein's --kill ran through the shim" killed "$out"
check "kill: the project's window is gone from the app's server" "" "$(tm -L rein-app list-windows -t demo -F '#W' | grep -x proj)"
check "kill: the same-named window on the default server is untouched" proj "$(tm list-windows -t demo -F '#W' | grep -x proj)"
# The user's server: rein itself, with plain tmux reaching the default server.
out="$(cd "$W/proj" && "${ENV[@]}" "$H/.rein/bin/rein" start --kill --yes 2>&1)"
check "kill (user): the default server's window is gone" "killed " "$out $(tm list-windows -t demo -F '#W' | grep -x proj)"
check "kill (user): the app's server is untouched" other "$(tm -L rein-app list-windows -t demo -F '#W')"

if [ "$fails" -eq 0 ]; then echo "PASS: agents"; else echo "$fails failure(s)"; fi
[ "$fails" -eq 0 ]
