#!/usr/bin/env bash
# lifecycle-screens.sh <out-dir> [W H] — V11 component 1 and V12 (D-059, D-061, E-66, E-69) end
# to end in the real window (debug build) on Xvfb, with screens in catppuccin-mocha and
# github-light:
#   close-tab-confirm — closing a tab whose Engineer runs: "Closing this tab stops …";
#   reattached        — the second run's tab, attached to the session the first run left: the
#                       agent pane's line carries the first run's start time.
# Checked, with a `tmux` wrapper that maps the app's socket (`-L rein-app`) and the default
# server to two private sockets:
#   - each session the app opens carries @rein_app_project = SHA-256 of its project path;
#   - closing a tab removes that project's session only; another tab's stays;
#   - quit asks nothing, kills nothing: the window exits, the session keeps running, a
#     same-named session on the default server stays;
#   - a second run on the project attaches: the stand-in rein records no second start;
#   - a session tagged for another project is not attached to (a new project starts);
#   - E-70: the open tabs are recorded (a closed project leaves the record) and a run with no
#     project restores them attached, starting nothing; a session that ended between runs
#     opens as the "not running" card and starts only on Start; an attached tab whose agents
#     are gone shows the "no longer running" bar (screens restored-tabs, not-running-card,
#     agents-gone-bar);
#   - with tmux.server = "user" no tag is set and quit touches nothing.
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
      tmux send-keys -t "$p" "clear; echo $w engineer, started $(date +%T); sleep 600" Enter
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
tag() { printf %s "$1" | sha256sum | cut -d' ' -f1; }
tag_of() { TA show-options -v -t "$1" @rein_app_project 2>/dev/null; }
# The open-tabs record's folder names, in order (projects.json, V12 component 4).
open_rec() { python3 -c 'import json,sys,os; d=json.load(open(sys.argv[1])); print(" ".join(os.path.basename(t["path"]) for t in d.get("open",[])))' "$W/xdg/state/rein-app/projects.json" 2>/dev/null; }
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
    "$REPO/target/debug/rein-desktop" ${3:+"$3"} >>"$W/app.log" 2>&1 &
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
  rm -f "$W/xdg/state/rein-app/projects.json" # each theme starts with no open-tabs record
  a="$W/Dev/alpha-$theme" b="$W/Dev/beta-$theme" g="$W/Dev/gamma-$theme"
  mkdir -p "$a/.ai" "$b/.ai" "$g/.ai"
  # The same name on the default server: never touched.
  TD new-session -d -s "alpha-$theme" -n "alpha-$theme" "sh"
  app "$theme" app "$a"
  sleep 7
  second "$b"
  sleep 7
  check "first run: both started once" "alpha-$theme: start|beta-$theme: start" "$(paste -sd'|' "$W/rein.log")"
  check "each session carries its project's tag" "$(tag "$a") $(tag "$b")" "$(tag_of "alpha-$theme") $(tag_of "beta-$theme")"
  # Close beta's tab (the current one): its Engineer runs → the confirm.
  X key ctrl+shift+w
  sleep 2.5
  shot "close-tab-confirm-$theme"
  X key Tab; X key Return # Cancel is focused; Close is next
  sleep 3
  check "tab close: beta's session ended, alpha's stays" "no yes" "$(has TA has-session -t "beta-$theme") $(has TA has-session -t "alpha-$theme")"
  # Quit with alpha open: no confirm, nothing killed.
  X key ctrl+shift+q
  check "quit: the window exited at once" yes "$(gone && echo yes || echo no)"
  check "quit: alpha's session keeps running on the app's server" yes "$(has TA has-session -t "alpha-$theme")"
  check "quit: no client is left attached" 0 "$(TA list-clients 2>/dev/null | wc -l | tr -d ' ')"
  check "quit: the default server's same-named session stays" yes "$(has TD has-session -t "alpha-$theme")"
  [ -n "$APP" ] && { kill "$APP" 2>/dev/null; wait "$APP" 2>/dev/null; APP=""; }
  # Second run on alpha: attached to the session, no second start.
  first=$(TA capture-pane -p -t "alpha-$theme" | grep -m1 'engineer, started')
  app "$theme" app "$a"
  sleep 6
  check "second run: no second start (attached)" "1 0" "$(grep -c "^alpha-$theme: start" "$W/rein.log") $(grep -c LIVE "$W/rein.log")"
  check "second run: the app's client is on alpha's session" "alpha-$theme" "$(TA list-clients -F '#{session_name}' 2>/dev/null)"
  check "second run: the same agent pane" "$first" "$(TA capture-pane -p -t "alpha-$theme" | grep -m1 'engineer, started')"
  shot "reattached-$theme"
  # A project whose tag no session carries starts, even with alpha's tagged session alive.
  second "$g"
  sleep 7
  check "another project's tagged session is not attached to" "gamma-$theme: start" "$(tail -n1 "$W/rein.log")"
  check "  ... and gamma got its own tag" "$(tag "$g")" "$(tag_of "gamma-$theme")"
  X key ctrl+shift+q
  gone >/dev/null
  [ -n "$APP" ] && { kill "$APP" 2>/dev/null; wait "$APP" 2>/dev/null; APP=""; }
  # E-70 (V12 components 4-5): the record is what was open — closed beta is not in it.
  check "record: alpha and gamma, in tab order (beta was closed)" "alpha-$theme gamma-$theme" "$(open_rec)"
  starts=$(grep -c ': start$' "$W/rein.log")
  # Third run, no project given: both tabs come back attached, nothing is started again.
  app "$theme" app ""
  sleep 9
  check "restore: no start on a restored tab" "$starts" "$(grep -c ': start$' "$W/rein.log")"
  check "restore: both sessions have the app's client (attached, in order)" "alpha-$theme gamma-$theme" "$(TA list-clients -F '#{session_name}' 2>/dev/null | sort | tr '\n' ' ' | sed 's/ $//')"
  shot "restored-tabs-$theme"
  X key ctrl+shift+q
  gone >/dev/null
  [ -n "$APP" ] && { kill "$APP" 2>/dev/null; wait "$APP" 2>/dev/null; APP=""; }
  # Between runs: alpha's agent stops (its pane back at a shell); gamma's session ends.
  TA send-keys -t "alpha-$theme" C-c
  TA kill-session -t "gamma-$theme"
  sleep 1
  app "$theme" app ""
  sleep 8 # the restore; gamma was the current tab when the last run quit
  check "restore: still no start (gamma waits on its card)" "$starts" "$(grep -c ': start$' "$W/rein.log")"
  shot "not-running-card-$theme"
  check "the card started nothing" "no" "$(has TA has-session -t "gamma-$theme")"
  X key ctrl+Tab; sleep 7 # alpha: its first 5 s status read finds no agent running
  shot "agents-gone-bar-$theme"
  X key ctrl+Tab; sleep 1.5
  X key Return; sleep 7 # back on gamma: Start is focused on the card
  check "Start runs rein start for gamma, once" "$((starts + 1))" "$(grep -c ': start$' "$W/rein.log")"
  check "  ... and gamma runs again" yes "$(has TA has-session -t "gamma-$theme")"
  X key ctrl+shift+q
  gone >/dev/null
  [ -n "$APP" ] && { kill "$APP" 2>/dev/null; wait "$APP" 2>/dev/null; APP=""; }
  TA kill-server 2>/dev/null # the test's own cleanup between themes, not the app's
  TD kill-server 2>/dev/null
  sleep 0.5
done

# tmux.server = "user": something on the app's socket and the user's own session both stay.
echo "[user server]"
: >"$W/rein.log"
u="$W/Dev/user-proj"; mkdir -p "$u/.ai"
rm -f "$W/xdg/state/rein-app/projects.json" # no record from the theme runs
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
check "user: the app's socket is not touched" yes "$(has TA has-session -t leftover)"
check "user: no tag on the user's session" "" "$(TD show-options -v -t user-proj @rein_app_project 2>/dev/null)"
[ -n "$APP" ] && { kill "$APP" 2>/dev/null; wait "$APP" 2>/dev/null; APP=""; }

echo "rein log:"; sed 's/^/  /' "$W/rein.log"
if [ "$fails" -eq 0 ]; then echo "PASS: lifecycle screens"; else echo "$fails failure(s)"; fi
[ "$fails" -eq 0 ]
