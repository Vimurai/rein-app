#!/usr/bin/env bash
# notify-system.sh <out-dir> — the system's own notification on Linux (status.md §Components 6
# channel 3, D-069; E-79): the real window (debug build) on Xvfb, with a private D-Bus session
# bus (dbus-daemon on a socket in the run's own directory — never the user's bus) where
# notifyd.c stands in for the desktop's notification server. Two projects:
#   worker — prints a line every second for 25 s, then nothing (a background tab that stops);
#   front  — runs and prints nothing; opened last, the current tab.
# The window is unmapped (hidden) before the worker stops. Checked:
#   - the server receives one Notify with app and summary "rein-app" and the body "worker has
#     stopped — it is finished or waiting." (the fixed sentence: no other text);
#   - a click (the server's ActionInvoked "default") maps the window again — it comes forward;
#   - front sends nothing.
# Written to <out-dir>: linux-system-notification.txt (the server's log and the core's trace
# lines; no project text in either but the two folder names) and a screen after the click.
# A local tool only: it refuses to run when CI is set. Needs: Xvfb, tmux, cc, Xlib + XTest
# headers, libdbus-1 headers (pkg-config dbus-1), dbus-daemon, ffmpeg, python3, a debug build.
set -uo pipefail
if [ -n "${CI:-}" ]; then echo "notify-system.sh: not in CI (it drives a real window on Xvfb)" >&2; exit 2; fi
OUT=$1
GW=1280 GH=800
REPO="$(git -C "$(dirname "$0")" rev-parse --show-toplevel)"
. "$(dirname "$0")/window-binary.sh" && window_binary
E2E="$REPO/crates/rein-desktop/tests/e2e"
REAL_TMUX="$(command -v tmux)"
W="" APP="" XV="" BUS="" ND="" FAIL=0
cleanup() {
  [ -n "$APP" ] && kill "$APP" 2>/dev/null
  [ -n "$ND" ] && kill "$ND" 2>/dev/null
  [ -n "$BUS" ] && kill "$BUS" 2>/dev/null
  [ -n "$W" ] && "$REAL_TMUX" -S "$W/tmux.sock" kill-server 2>/dev/null
  [ -n "$XV" ] && kill "$XV" 2>/dev/null
  [ -z "$W" ] && return
  [ -n "${KEEP:-}" ] && echo "workdir $W" >&2 || rm -rf "$W"
}
trap cleanup EXIT
W="$(mktemp -d /tmp/rein-nsys.XXXXXX)"
W="$(cd "$W" && pwd -P)"
mkdir -p "$W/bin" "$W/home/.rein/bin" "$W/run" "$OUT" "$W/Dev"
chmod 700 "$W/run"
for h in xresize xmap xtitle; do cc -O2 -o "$W/bin/$h" "$E2E/$h.c" -lX11 || exit 2; done
# shellcheck disable=SC2046 # pkg-config prints several words on purpose
cc -O2 -o "$W/bin/notifyd" "$E2E/notifyd.c" $(pkg-config --cflags --libs dbus-1) || exit 2
cat >"$W/bin/tmux" <<EOF
#!/bin/sh
exec "$REAL_TMUX" -S "$W/tmux.sock" -f /dev/null "\$@"
EOF
chmod +x "$W/bin/tmux"
cat >"$W/bin/agent-burst" <<'SH'
#!/bin/sh
exec python3 -u -c 'import sys, time
time.sleep(int(sys.argv[1]))
for _ in range(int(sys.argv[2])): print(time.ctime()); time.sleep(1)
time.sleep(3600)' "$@"
SH
chmod +x "$W/bin/agent-burst"
cat >"$W/home/.rein/bin/rein" <<EOF
#!/bin/sh
[ "\$1" = start ] || exit 2
w=\$(basename "\$PWD")
if ! tmux has-session -t "\$w" 2>/dev/null; then
  tmux new-session -d -s "\$w" -n "\$w" -x 200 -y 50
  case "\$w" in
    worker) a="agent-burst 0 25" ;;
    *) a="agent-burst 3600 0" ;;
  esac
  tmux set-option -p -t "\$w:\$w.0" @rein_role engineer
  sleep 1
  tmux send-keys -t "\$w:\$w.0" "clear; exec $W/bin/\$a" Enter
fi
exec tmux attach -t "\$w"
EOF
chmod +x "$W/home/.rein/bin/rein"
project() { mkdir -p "$1/.ai"; printf '{"tasks": []}\n' >"$1/.ai/state.json"; }
T() { "$REAL_TMUX" -S "$W/tmux.sock" -f /dev/null "$@"; }
shot() { ffmpeg -loglevel error -y -f x11grab -draw_mouse 0 -video_size "${GW}x${GH}" -i "$DISPLAY" -frames:v 1 "$OUT/$1.png" && echo "  $OUT/$1.png"; }
check() { if [ "$2" = yes ]; then echo "  ok   $1"; else echo "  FAIL $1 ($2)"; FAIL=1; fi; }
mapped() { "$W/bin/xtitle" rein-app >/dev/null 2>&1; }
wait_for() { # wait_for <file> <pattern> <seconds>
  local i
  for i in $(seq 1 "$(($3 * 4))"); do grep -q -- "$2" "$1" 2>/dev/null && return 0; sleep 0.25; done
  return 1
}

started=""
for n in $(seq 90 130); do
  [ -e "/tmp/.X$n-lock" ] && continue
  Xvfb ":$n" -screen 0 "${GW}x${GH}x24" -nolisten tcp >/dev/null 2>&1 &
  XV=$!
  sleep 1.5
  kill -0 "$XV" 2>/dev/null && { started=":$n"; break; }
  XV=""
done
[ -n "$started" ] || { echo "no Xvfb" >&2; exit 2; }
export DISPLAY=$started

# The run's own session bus and the stand-in server on it.
dbus-daemon --session --nofork --address="unix:path=$W/bus" >/dev/null 2>&1 &
BUS=$!
for _ in $(seq 1 40); do [ -S "$W/bus" ] && break; sleep 0.1; done
export DBUS_SESSION_BUS_ADDRESS="unix:path=$W/bus"
"$W/bin/notifyd" "$W/click" >"$W/notifyd.log" 2>&1 &
ND=$!
wait_for "$W/notifyd.log" '^ready' 10 || { echo "notifyd did not start: $(cat "$W/notifyd.log")" >&2; exit 2; }

desktop() {
  exec env -u TMUX -u TMUX_PANE -u CLAUDECODE \
    HOME="$W/home" PATH="$W/bin:$PATH" SHELL=/bin/sh REIN_DESKTOP_TRACE_NOTIFY=1 \
    DBUS_SESSION_BUS_ADDRESS="unix:path=$W/bus" \
    XDG_RUNTIME_DIR="$W/run" XDG_CONFIG_HOME="$W/xdg/config" XDG_DATA_HOME="$W/xdg/data" \
    XDG_STATE_HOME="$W/xdg/state" XDG_CACHE_HOME="$W/xdg/cache" \
    WEBKIT_DISABLE_DMABUF_RENDERER=1 \
    "$REPO/target/debug/rein-desktop" "$1" >>"$W/app.log" 2>&1
}
second() {
  local n
  for n in 1 2; do
    timeout 20 bash -c "$(declare -f desktop); W='$W' REPO='$REPO' desktop '$1'"
    for _ in $(seq 1 24); do T has-session -t "$(basename "$1")" 2>/dev/null && { sleep 2; return; }; sleep 0.25; done
  done
}

st="$W/xdg/state/rein-app"
mkdir -p "$st"
printf '{"schema":1,"input_style":"simple","first_run_done":true}' >"$st/ui.json"
for p in worker front; do project "$W/Dev/$p"; done
desktop "$W/Dev/worker" &
APP=$!
for _ in $(seq 1 300); do "$W/bin/xresize" rein "$GW" "$GH" >/dev/null 2>&1 && break; sleep 0.05; done
sleep 6
second "$W/Dev/front"
"$W/bin/xresize" rein "$GW" "$GH" >/dev/null
sleep 2
"$W/bin/xmap" rein unmap >/dev/null # hidden before the worker stops
sleep 1
check "the window is hidden" "$(mapped && echo "still mapped" || echo yes)"
if wait_for "$W/notifyd.log" 'body=worker has stopped' 120; then
  check "a system notification for the stop" yes
else
  check "a system notification for the stop" "none in 120 s: $(cat "$W/notifyd.log")"
fi
line=$(grep -m1 '^notify ' "$W/notifyd.log")
check "its text is the fixed sentence, named rein-app" "$([ "${line#*app=}" = "rein-app summary=rein-app body=worker has stopped — it is finished or waiting." ] && echo yes || echo "$line")"
touch "$W/click"
shown=""
for _ in $(seq 1 40); do mapped && { shown=1; break; }; sleep 0.25; done
check "a click brings the window forward" "$([ -n "$shown" ] && echo yes || echo "not mapped after 10 s")"
grep -q "system clicked" "$W/app.log" && check "the core saw the click" yes || check "the core saw the click" "no 'system clicked' trace"
sleep 2
"$W/bin/xresize" rein "$GW" "$GH" >/dev/null; sleep 1
shot "linux-after-click"
fronts=$(grep -c 'body=front' "$W/notifyd.log" || true) # grep -c prints 0 and exits 1 on none
check "front sent nothing" "$([ "$fronts" = 0 ] && echo yes || echo "$fronts notifications")"
{
  echo "# notifyd (the stand-in server on the run's private session bus)"
  cat "$W/notifyd.log"
  echo "# the core's trace (REIN_DESKTOP_TRACE_NOTIFY)"
  grep '^notify-trace' "$W/app.log"
} >"$OUT/linux-system-notification.txt"
echo "  $OUT/linux-system-notification.txt"
[ "$FAIL" = 0 ] && echo "PASS" || echo "FAIL"
exit "$FAIL"
