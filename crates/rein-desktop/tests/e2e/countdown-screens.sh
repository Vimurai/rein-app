#!/usr/bin/env bash
# countdown-screens.sh <out-dir> [W H] — desktop.md §Look and feel v2 V12 component 7 (E-71,
# D-061, D-052, D-060 ruling 3): the real window (debug build) on Xvfb, two real role panes
# (stand-in agents), close_when_agents_exit on (the default), both themes:
#   countdown-bar        — the Engineer exited: "The Engineer exited. Closing <project> in N s." with
#                          Keep open (focused) and Close now;
#   marked-background-tab — the user went to another tab mid-countdown: no close, the demo tab
#                          carries the marker;
# and checked on screen and in tmux: back on the tab the countdown starts from 5; Keep open
# (keyboard in one theme, pointer in the other) leaves both panes; Close now (the other way
# round) closes; zero closes the project and the Architect's process is gone; all agents
# exited closes at once (V9 as built).
# The `rein` is a stand-in in a throwaway HOME: `rein start` builds rein's layout (one session,
# the project's window, two panes tagged @rein_role) through plain `tmux` and attaches; each
# pane runs a stand-in agent (`sleep`, typed into its shell) that returns to the shell when
# interrupted. The window
# runs its real setup into that HOME; its shim forwards to a wrapper that pins a private -S
# socket, so the operator's own servers and any real `rein-app` socket are never touched. The
# window's own status script and close run unchanged.
# A local tool only: it refuses to run when CI is set. Needs: Xvfb, tmux, cc, Xlib + XTest
# headers, ffmpeg, a debug build (cargo build -p rein-desktop, after the UI build).
set -uo pipefail
if [ -n "${CI:-}" ]; then echo "countdown-screens.sh: not in CI (it drives a real window on Xvfb)" >&2; exit 2; fi
OUT=$1
GW=${2:-1280} GH=${3:-800}
REPO="$(git -C "$(dirname "$0")" rev-parse --show-toplevel)"
. "$(dirname "$0")/window-binary.sh" && window_binary # cargo test's binary or a stale UI never reaches the window (E-84)
REAL_TMUX="$(command -v tmux)"
W="" APP="" XV=""
cleanup() {
  [ -n "$APP" ] && kill "$APP" 2>/dev/null
  [ -n "$W" ] && "$REAL_TMUX" -S "$W/tmux.sock" kill-server 2>/dev/null
  [ -n "$XV" ] && kill "$XV" 2>/dev/null
  [ -z "$W" ] && return
  [ -n "${KEEP:-}" ] && echo "workdir $W" >&2 || rm -rf "$W"
}
trap cleanup EXIT
W="$(mktemp -d /tmp/rein-cds.XXXXXX)"
W="$(cd "$W" && pwd -P)"
P="$W/Dev/schraubr_laravel"
mkdir -p "$W/bin" "$W/home/.rein/bin" "$W/run" "$OUT" "$P/.ai"
chmod 700 "$W/run"
cc -O2 -o "$W/bin/xresize" "$REPO/crates/rein-desktop/tests/e2e/xresize.c" -lX11 || exit 2
cc -O2 -o "$W/bin/xinput" "$REPO/crates/rein-desktop/tests/e2e/xinput.c" -lX11 -lXtst || exit 2
cat >"$W/bin/tmux" <<EOF
#!/bin/sh
# Private server only: -S wins over the shim's -L.
exec "$REAL_TMUX" -S "$W/tmux.sock" -f /dev/null "\$@"
EOF
chmod +x "$W/bin/tmux"
cat >"$W/home/.rein/bin/rein" <<'EOF'
#!/bin/sh
# One session per project folder; the demo project gets two role panes with stand-in agents.
# `rein init --check`: up to date (no out-of-date bar over the strip the harness watches).
[ "$1" = init ] && exit 0
w=$(basename "$PWD")
if [ "$*" = "start --kill --yes" ]; then
  echo kill >>"$HOME/kill.log"
  tmux kill-session -t "$w" 2>/dev/null
  exit 0
fi
if ! tmux has-session -t "$w" 2>/dev/null; then
  tmux new-session -d -s "$w" -n "$w" -x 200 -y 50
  if [ "$w" = schraubr_laravel ]; then
    tmux split-window -h -d -t "$w:$w"
    i=0
    for p in $(tmux list-panes -t "$w:$w" -F '#{pane_id}'); do
      [ $i = 0 ] && r=engineer || r=architect
      tmux set-option -p -t "$p" @rein_role "$r"
      tmux set-option -p -t "$p" @rein_account default
      i=1
    done
    sleep 1 # each pane's shell ready, then the stand-in agent typed in as `rein pane` would
    for p in $(tmux list-panes -t "$w:$w" -F '#{pane_id}'); do tmux send-keys -t "$p" 'clear; sleep 600' Enter; done
  fi
fi
exec tmux attach -t "$w:$w"
EOF
chmod +x "$W/home/.rein/bin/rein"
T() { "$REAL_TMUX" -S "$W/tmux.sock" -f /dev/null "$@"; } # -f: never the operator's ~/.tmux.conf
X() { "$W/bin/xinput" "$@"; sleep 0.8; }
shot() { ffmpeg -loglevel error -y -f x11grab -draw_mouse 0 -video_size "${GW}x${GH}" -i "$DISPLAY" -frames:v 1 "$OUT/$1-${GW}x${GH}.png" && echo "  $OUT/$1-${GW}x${GH}.png"; }

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

# The bar is up when the top padding of its Close now button is there: that strip is filled with
# the theme's primary colour (saturated), where the terminal is grey, black or white.
bar_up() {
  ffmpeg -loglevel error -f x11grab -video_size 24x3 -i "$DISPLAY+1206,47" -frames:v 1 -vf scale=1:1 \
    -f rawvideo -pix_fmt rgb24 - 2>/dev/null | od -An -tu1 |
    awk '{ mx = $1; mn = $1; for (i = 2; i <= 3; i++) { if ($i > mx) mx = $i; if ($i < mn) mn = $i } exit !(mx - mn > 80) }'
}
wait_bar() { for _ in $(seq 1 60); do bar_up && return 0; sleep 0.2; done; return 1; }
role_pane() { T list-panes -a -F '#{pane_id} #{@rein_role}' | awk -v r="$1" '$2 == r { print $1 }'; }
agent_cmd() { T display-message -p -t "$(role_pane "$1")" '#{pane_current_command}' 2>/dev/null; }
# The bar's two buttons at 1280 px (read from countdown-bar): Keep open, then Close now.
KEEP_X=${KEEP_X:-1150} CLOSE_X=${CLOSE_X:-1232}
fails=0
check() { if [ "$2" = "$3" ]; then echo "  ok   $1"; else echo "  FAIL $1 — expected [$2], got [$3]"; fails=$((fails + 1)); fi; }
launch() { # launch <theme> <dir>
  st="$W/xdg/state/rein-app"
  mkdir -p "$st"
  printf '{"schema":1,"input_style":"simple","first_run_done":true,"theme":"%s"}' "$1" >"$st/ui.json"
  rm -f "$st/projects.json"
  env -u TMUX -u TMUX_PANE -u CLAUDECODE \
    HOME="$W/home" PATH="$W/bin:$PATH" SHELL=/bin/sh \
    XDG_RUNTIME_DIR="$W/run" XDG_CONFIG_HOME="$W/xdg/config" XDG_DATA_HOME="$W/xdg/data" \
    XDG_STATE_HOME="$W/xdg/state" XDG_CACHE_HOME="$W/xdg/cache" \
    WEBKIT_DISABLE_DMABUF_RENDERER=1 \
    "$REPO/target/debug/rein-desktop" "$2" >>"$W/app.log" 2>&1 &
  APP=$!
  for _ in $(seq 1 300); do "$W/bin/xresize" rein "$GW" "$GH" >/dev/null 2>&1 && break; sleep 0.05; done
  for _ in $(seq 1 120); do T list-clients 2>/dev/null | grep -q . && break; sleep 0.25; done
  sleep 12 # both agents seen running (one poll per 5 s)
}
second() {
  env -u TMUX -u TMUX_PANE HOME="$W/home" PATH="$W/bin:$PATH" XDG_RUNTIME_DIR="$W/run" \
    XDG_STATE_HOME="$W/xdg/state" XDG_CONFIG_HOME="$W/xdg/config" "$REPO/target/debug/rein-desktop" "$1" >>"$W/app.log" 2>&1
}
stop() { kill "$APP" 2>/dev/null; wait "$APP" 2>/dev/null; APP=""; T kill-server 2>/dev/null; : >"$W/home/kill.log"; sleep 0.5; }
exit_engineer() { T send-keys -t "$(role_pane engineer)" C-c; }

B="$W/Dev/other"; mkdir -p "$B/.ai"
for theme in catppuccin-mocha github-light; do
  echo "[$theme ${GW}x${GH}]"
  : >"$W/home/kill.log"
  # 1. The countdown, then away to another tab mid-countdown: no close, the tab is marked.
  launch "$theme" "$P"
  second "$B"; sleep 4
  X key alt+1; sleep 1
  exit_engineer
  if wait_bar; then echo "  ok   the countdown bar is up"; else echo "  FAIL no countdown bar"; fails=$((fails + 1)); fi
  shot "countdown-bar-$theme"
  X key alt+2; sleep 8
  check "away mid-countdown: nothing closed" "0 yes" "$(wc -l <"$W/home/kill.log" | tr -d ' ') $([ "$(agent_cmd architect)" = sleep ] && echo yes || echo no)"
  shot "marked-background-tab-$theme"
  # 2. Back: from 5 again; Keep open (keyboard in mocha, pointer in light) leaves both panes.
  X key alt+1
  if wait_bar; then echo "  ok   back on the tab the countdown is up again"; else echo "  FAIL no countdown on return"; fails=$((fails + 1)); fi
  if [ "$theme" = catppuccin-mocha ]; then X key Return; else X click "$KEEP_X" 58; fi
  sleep 8
  check "Keep open: nothing closed, the Architect still runs" "0 sleep" "$(wc -l <"$W/home/kill.log" | tr -d ' ') $(agent_cmd architect)"
  check "Keep open: the bar is gone" no "$(bar_up && echo yes || echo no)"
  stop
  # 3. Zero (mocha) or Close now (light, keyboard: Tab from Keep open, Enter): the project
  #    closes, the Architect's process is gone.
  launch "$theme" "$P"
  exit_engineer
  wait_bar >/dev/null
  if [ "$theme" = github-light ]; then X key Tab; X key Return; fi
  sleep 8
  check "the project closed (rein start --kill ran once)" 1 "$(wc -l <"$W/home/kill.log" | tr -d ' ')"
  check "  ... and the Architect's pane and process are gone" "0 0" "$(T list-panes -a -F '#{@rein_role}' 2>/dev/null | grep -c architect) $(T list-panes -a -F '#{pane_current_command}' 2>/dev/null | grep -c sleep)"
  stop
  # 4. Close now by pointer (mocha) — the other way round from 3.
  if [ "$theme" = catppuccin-mocha ]; then
    launch "$theme" "$P"
    exit_engineer
    wait_bar >/dev/null
    X click "$CLOSE_X" 58; sleep 6
    check "Close now by pointer closes" 1 "$(wc -l <"$W/home/kill.log" | tr -d ' ')"
    stop
  fi
done

if [ "$fails" -eq 0 ]; then echo "PASS: countdown screens"; else echo "$fails failure(s)"; fi
[ "$fails" -eq 0 ]
