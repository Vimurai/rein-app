#!/usr/bin/env bash
# v2-screens.sh <out-dir> [binary] [W H] — the screens of desktop.md §Look and feel v2 (E-50,
# D-052): the real window on Xvfb, driven by keys through XTest (xinput.c), in the default
# theme and in github-light: first run, start screen, a project tab, the menu, Settings, the
# health view and a confirm dialog (Undo last update — the confirm step runs nothing).
# Isolation: HOME, XDG_* and the tmux socket are in a mktemp dir; the `rein` the window finds
# is a stub (`start` builds a demo session on the private socket, `doctor --scope install
# --json` answers OK), so no real install, tmux server or Claude is touched.
# INPUT_STYLE=modal takes the shots in the modal input style (E-55: the menu in both styles).
# Xvfb note: WebKit there paints an output batch only when the next one arrives, so the
# terminal shot is preceded by `refresh-client` redraws. Needs Xvfb, tmux, cc, Xlib + XTest
# headers, ffmpeg.
set -uo pipefail
OUT=$1
REPO="$(cd "$(dirname "$0")/../../../.." && pwd)"
BIN="${2:-$REPO/target/debug/rein-desktop}"
GW=${3:-1280} GH=${4:-800}
REAL="$(command -v tmux)"
W="$(mktemp -d /tmp/rein-v2s.XXXXXX)"
APP="" XV=""
cleanup() {
  [ -n "$APP" ] && kill "$APP" 2>/dev/null
  "$REAL" -S "$W/tmux.sock" kill-server 2>/dev/null
  [ -n "$XV" ] && kill "$XV" 2>/dev/null
  [ -n "${KEEP:-}" ] && echo "workdir $W" >&2 || rm -rf "$W"
}
trap cleanup EXIT
mkdir -p "$W/bin" "$W/home/.rein/bin" "$W/run" "$OUT" "$W/Dev/schraubr_laravel/.ai" "$W/Dev/energy-api/.ai" "$W/Dev/rein-app/.ai"
chmod 700 "$W/run"
cc -O2 -o "$W/bin/xresize" "$REPO/crates/rein-desktop/tests/e2e/xresize.c" -lX11 || exit 2
cc -O2 -o "$W/bin/xinput" "$REPO/crates/rein-desktop/tests/e2e/xinput.c" -lX11 -lXtst || exit 2

cat >"$W/bin/tmux" <<EOF
#!/bin/sh
exec "$REAL" -S "$W/tmux.sock" "\$@"
EOF
cat >"$W/home/.rein/bin/rein" <<'EOF'
#!/bin/sh
case "$1" in
doctor)
  printf '%s\n' '{"schema":1,"version":"0.5.0","status":"ok","checks":[
{"id":"install_layout","scope":"install","subject":null,"status":"ok","line":"252 files match the manifest","fix":null},
{"id":"install_version","scope":"install","subject":null,"status":"ok","line":"rein 0.5.0 (signed release)","fix":null},
{"id":"install_hooks","scope":"install","subject":null,"status":"ok","line":"6 hooks installed and executable","fix":null},
{"id":"install_node","scope":"install","subject":null,"status":"ok","line":"node v22.11.0 on PATH","fix":null},
{"id":"install_tmux","scope":"install","subject":null,"status":"ok","line":"tmux 3.4 (3.2 or newer)","fix":null}]}'
  exit 0 ;;
start)
  s=$(basename "$PWD")
  echo "rein start: account personal (~/.claude-personal)"
  if ! tmux has-session -t "$s" 2>/dev/null; then
    tmux new-session -d -s "$s" -n "$s" 'printf "\033[1mEngineer\033[0m · opus\n\n> E-50 restyle the overlays\n"; exec sh'
    tmux split-window -h -t "$s" 'printf "\033[1mArchitect\033[0m · fable\n\n> reviewing E-49 screens\n"; exec sh'
    tmux set-option -t "$s" pane-border-status top
    tmux set-option -t "$s" pane-border-format ' #{pane_index}: #{?pane_active,active,idle} '
  fi
  exec tmux attach-session -t "$s" ;;
esac
exit 1
EOF
chmod +x "$W/bin/tmux" "$W/home/.rein/bin/rein"
T() { "$W/bin/tmux" "$@"; }
X() { "$W/bin/xinput" "$@"; sleep "${PAUSE:-0.6}"; }

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

shot() { # shot <name>: a frame of the whole screen
  sleep 0.8
  ffmpeg -loglevel error -y -f x11grab -draw_mouse 0 -video_size "${GW}x${GH}" -i "$DISPLAY" -frames:v 1 "$OUT/$1-${GW}x${GH}.png"
  echo "  $OUT/$1-${GW}x${GH}.png"
}

launch() { # launch <theme> <first_run_done> [dir]
  local now st="$W/xdg/state/rein-app"
  now=$(date +%s)
  mkdir -p "$st"
  printf '{"schema":1,"input_style":"%s","first_run_done":%s,"theme":"%s"}' "${INPUT_STYLE:-simple}" "$2" "$1" >"$st/ui.json"
  printf '{"schema":1,"recent":[{"path":"%s","last_opened":%d},{"path":"%s","last_opened":%d},{"path":"%s","last_opened":%d}]}' \
    "$W/Dev/schraubr_laravel" $((now - 7200)) "$W/Dev/energy-api" $((now - 90000)) "$W/Dev/rein-app" $((now - 5 * 86400)) >"$st/projects.json"
  env -u TMUX -u TMUX_PANE -u CLAUDECODE \
    HOME="$W/home" PATH="$W/bin:$PATH" SHELL=/bin/sh \
    XDG_RUNTIME_DIR="$W/run" XDG_CONFIG_HOME="$W/xdg/config" XDG_DATA_HOME="$W/xdg/data" \
    XDG_STATE_HOME="$W/xdg/state" XDG_CACHE_HOME="$W/xdg/cache" \
    WEBKIT_DISABLE_DMABUF_RENDERER=1 \
    "$BIN" ${3:+"$3"} >"$W/app-$1.log" 2>&1 &
  APP=$!
  for _ in $(seq 1 300); do "$W/bin/xresize" rein "$GW" "$GH" >/dev/null 2>&1 && break; sleep 0.05; done
  sleep 2.5
  X move $((GW / 2)) $((GH / 2)) # no window manager: the keyboard follows the pointer
}

quit() {
  kill "$APP" 2>/dev/null
  wait "$APP" 2>/dev/null
  APP=""
  sleep 0.5
}

for theme in catppuccin-mocha github-light; do
  echo "[$theme ${GW}x${GH}]"
  launch "$theme" false
  shot "first-run-$theme"
  [ "${INPUT_STYLE:-simple}" = modal ] && X key Tab # the second card: modal
  X key Return # the focused card
  shot "start-$theme"
  X key 1 # recent project 1
  for _ in $(seq 1 60); do T list-clients 2>/dev/null | grep -q . && break; sleep 0.25; done
  sleep 2
  ctty="$(T list-clients -F '#{client_tty}' | head -n1)"
  for _ in 1 2 3; do T refresh-client -t "$ctty"; sleep 0.7; done
  shot "tab-$theme"
  X key ctrl+shift+p
  shot "menu-$theme"
  X key Escape
  X key ctrl+comma
  shot "settings-$theme"
  X key Escape
  X key ctrl+shift+p
  X type "health"
  X key Return
  sleep 1
  shot "health-$theme"
  X key Escape
  X key ctrl+shift+p
  X type "undo last"
  X key Return
  shot "confirm-$theme"
  X key Escape
  quit
  T kill-server 2>/dev/null # the next theme starts from a fresh session
done
