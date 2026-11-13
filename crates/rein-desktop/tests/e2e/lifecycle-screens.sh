#!/usr/bin/env bash
# lifecycle-screens.sh <out-dir> [W H] — V11 (D-059, E-66) end to end in the real window (debug
# build) on Xvfb, and the screens of both confirms in catppuccin-mocha and github-light:
#   close-tab-confirm — closing a tab whose Engineer runs: "Closing this tab stops …";
#   quit-confirm      — quitting with projects on the app's server.
# Checked, with a `tmux` wrapper that maps the app's socket (`-L rein-app`) and the default
# server to two private sockets:
#   - a leftover app server (a session for the project) is gone before the first `rein start`,
#     and the stand-in rein never starts on a live session;
#   - closing a tab removes that project's session only; another tab's stays;
#   - quit ends the app's server; a same-named session on the default server stays;
#   - with tmux.server = "user" neither the sweep nor the quit touches anything.
# The stand-in rein builds one role pane per project running a stand-in agent. A local tool
# only: it refuses to run when CI is set. Needs: Xvfb, tmux, cc, Xlib + XTest headers, ffmpeg,
# a debug build.
set -uo pipefail
if [ -n "${CI:-}" ]; then echo "lifecycle-screens.sh: not in CI (it drives a real window on Xvfb)" >&2; exit 2; fi
OUT=$1
GW=${2:-1280} GH=${3:-800}
REPO="$(git -C "$(dirname "$0")" rev-parse --show-toplevel)"
REAL_TMUX="$(command -v tmux)"
W="" APP="" XV=""
cleanup() {
  [ -n "$APP" ] && kill "$APP" 2>/dev/null
  if [ -n "$W" ]; then
    "$REAL_TMUX" -S "$W/app.sock" kill-server 2>/dev/null
    "$REAL_TMUX" -S "$W/default.sock" kill-server 2>/dev/null
  fi
  [ -n "$XV" ] && kill "$XV" 2>/dev/null
  [ -z "$W" ] && return
  [ -n "${KEEP:-}" ] && echo "workdir $W" >&2 || rm -rf "$W"
}
trap cleanup EXIT
W="$(mktemp -d /tmp/rein-lcs.XXXXXX)"
W="$(cd "$W" && pwd -P)"
mkdir -p "$W/bin" "$W/home/.rein/bin" "$W/run" "$OUT" "$W/Dev"
chmod 700 "$W/run"
cc -O2 -o "$W/bin/xresize" "$REPO/crates/rein-desktop/tests/e2e/xresize.c" -lX11 || exit 2
cc -O2 -o "$W/bin/xinput" "$REPO/crates/rein-desktop/tests/e2e/xinput.c" -lX11 -lXtst || exit 2
# -S wins over -L: the app's socket name picks app.sock, anything else default.sock.
cat >"$W/bin/tmux" <<EOF
#!/bin/sh
sock="$W/default.sock"
for a in "\$@"; do [ "\$a" = rein-app ] && sock="$W/app.sock"; done
exec "$REAL_TMUX" -S "\$sock" "\$@"
EOF
chmod +x "$W/bin/tmux"
cat >"$W/home/.rein/bin/rein" <<EOF
#!/bin/sh
LOG="$W/rein.log"
EOF
cat >>"$W/home/.rein/bin/rein" <<'EOF'
w=$(basename "$PWD")
case "$*" in
  "start --kill --yes")
    echo "$w: kill" >>"$LOG"
    id=$(tmux list-windows -t "$w" -F '#{window_id} #{window_name}' 2>/dev/null | awk -v n="$w" '$2 == n { print $1 }')
    [ -n "$id" ] && tmux kill-window -t "$id"
    exit 0 ;;
  start)
    if tmux has-session -t "$w" 2>/dev/null; then echo "$w: start on a LIVE session" >>"$LOG"; else echo "$w: start" >>"$LOG"; fi
    if ! tmux has-session -t "$w" 2>/dev/null; then
      tmux new-session -d -s "$w" -n "$w" -x 200 -y 50 "sh"
      p=$(tmux list-panes -t "$w:$w" -F '#{pane_id}' | head -n1)
      tmux set-option -p -t "$p" @rein_role engineer
      sleep 1
      tmux send-keys -t "$p" 'clear; sleep 600' Enter
    fi
    exec tmux attach -t "$w:$w" ;;
esac
exit 2
EOF
chmod +x "$W/home/.rein/bin/rein"
TA() { "$REAL_TMUX" -S "$W/app.sock" "$@"; }
TD() { "$REAL_TMUX" -S "$W/default.sock" "$@"; }
X() { "$W/bin/xinput" "$@"; sleep 0.7; }
shot() { ffmpeg -loglevel error -y -f x11grab -draw_mouse 0 -video_size "${GW}x${GH}" -i "$DISPLAY" -frames:v 1 "$OUT/$1-${GW}x${GH}.png" && echo "  $OUT/$1-${GW}x${GH}.png"; }
fails=0
check() { if [ "$2" = "$3" ]; then echo "  ok   $1"; else echo "  FAIL $1 — expected [$2], got [$3]"; fails=$((fails + 1)); fi; }
has() { if "$@" >/dev/null 2>&1; then echo yes; else echo no; fi; }

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

app() { # app <theme> <server> <dir>
  local st="$W/xdg/state/rein-app"
  mkdir -p "$st"
  printf '{"schema":1,"input_style":"simple","first_run_done":true,"theme":"%s","tmux_server":"%s"}' "$1" "$2" >"$st/ui.json"
  env -u TMUX -u TMUX_PANE -u CLAUDECODE \
    HOME="$W/home" PATH="$W/bin:$PATH" SHELL=/bin/sh \
    XDG_RUNTIME_DIR="$W/run" XDG_CONFIG_HOME="$W/xdg/config" XDG_DATA_HOME="$W/xdg/data" \
    XDG_STATE_HOME="$W/xdg/state" XDG_CACHE_HOME="$W/xdg/cache" \
    WEBKIT_DISABLE_DMABUF_RENDERER=1 \
    "$REPO/target/debug/rein-desktop" "$3" >>"$W/app.log" 2>&1 &
  APP=$!
  for _ in $(seq 1 300); do "$W/bin/xresize" rein "$GW" "$GH" >/dev/null 2>&1 && break; sleep 0.05; done
}
second() { # open another project in the running window (single instance: a hand-off)
  env -u TMUX -u TMUX_PANE HOME="$W/home" PATH="$W/bin:$PATH" XDG_RUNTIME_DIR="$W/run" \
    XDG_STATE_HOME="$W/xdg/state" XDG_CONFIG_HOME="$W/xdg/config" "$REPO/target/debug/rein-desktop" "$1" >>"$W/app.log" 2>&1
}
gone() { for _ in $(seq 1 50); do kill -0 "$APP" 2>/dev/null || { APP=""; return 0; }; sleep 0.1; done; return 1; }

for theme in catppuccin-mocha github-light; do
  echo "[$theme ${GW}x${GH}]"
  : >"$W/rein.log"
  a="$W/Dev/alpha-$theme" b="$W/Dev/beta-$theme"
  mkdir -p "$a/.ai" "$b/.ai"
  # A leftover app server holding alpha's session, and the same name on the default server.
  TA new-session -d -s "alpha-$theme" -n "alpha-$theme" "sh"
  TD new-session -d -s "alpha-$theme" -n "alpha-$theme" "sh"
  app "$theme" app "$a"
  sleep 7
  check "sweep: alpha started fresh, never on a live session" "alpha-$theme: start" "$(cat "$W/rein.log")"
  second "$b"
  sleep 7
  check "two tabs: both sessions on the app's server" "yes yes" "$(has TA has-session -t "alpha-$theme") $(has TA has-session -t "beta-$theme")"
  # Close beta's tab (the current one): its Engineer runs → the confirm.
  X key ctrl+shift+w
  sleep 2.5
  shot "close-tab-confirm-$theme"
  X key Tab; X key Return # Cancel is focused; Close is next
  sleep 3
  check "tab close: beta's session ended, alpha's stays" "no yes" "$(has TA has-session -t "beta-$theme") $(has TA has-session -t "alpha-$theme")"
  # Quit with alpha open: the quit confirm, then the app's server ends.
  X key ctrl+shift+q
  sleep 1.5
  shot "quit-confirm-$theme"
  X key Tab; X key Return # Cancel is focused; Quit is next
  sleep 1
  check "quit: the window exited" yes "$(gone && echo yes || echo no)"
  check "quit: the app's server is gone" no "$(has TA list-sessions)"
  check "quit: the default server's same-named session stays" yes "$(has TD has-session -t "alpha-$theme")"
  [ -n "$APP" ] && { kill "$APP" 2>/dev/null; wait "$APP" 2>/dev/null; APP=""; }
  TD kill-server 2>/dev/null
  sleep 0.5
done

# tmux.server = "user": a leftover on the app's socket and the user's own session both stay.
echo "[user server]"
: >"$W/rein.log"
u="$W/Dev/user-proj"; mkdir -p "$u/.ai"
TA new-session -d -s leftover "sh"
app catppuccin-mocha user "$u"
sleep 7
X key ctrl+shift+w # close the tab: the session keeps running (the user's own)
sleep 2
X key q # the start screen's quit
sleep 1
check "user: the window exited" yes "$(gone && echo yes || echo no)"
check "user: no kill on the user's server" "user-proj: start" "$(cat "$W/rein.log")"
check "user: the user's session keeps running" yes "$(has TD has-session -t user-proj)"
check "user: the app's socket is not touched (no sweep, no quit kill)" yes "$(has TA has-session -t leftover)"
[ -n "$APP" ] && { kill "$APP" 2>/dev/null; wait "$APP" 2>/dev/null; APP=""; }

echo "rein log:"; sed 's/^/  /' "$W/rein.log"
if [ "$fails" -eq 0 ]; then echo "PASS: lifecycle screens"; else echo "$fails failure(s)"; fi
[ "$fails" -eq 0 ]
