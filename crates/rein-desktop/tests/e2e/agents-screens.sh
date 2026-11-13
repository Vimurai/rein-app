#!/usr/bin/env bash
# agents-screens.sh <out-dir> [W H] — the screens of desktop.md §Look and feel v2 V9 component 4
# (E-62, D-052): the real window (debug build) on Xvfb, a project tab on the window's own tmux
# server, in catppuccin-mocha and github-light:
#   agent-bar     — the Engineer's agent exited: "The Engineer has exited." with Keep open and
#                   Close project, the Architect still running;
#   close-confirm — Close project clicked while the Architect runs: the confirm, No focused;
#   all-exited-closed — No, then the Architect exits too: the project closes by itself (the
#                   window's session is gone; the start screen is back).
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
if [ -n "${CI:-}" ]; then echo "agents-screens.sh: not in CI (it drives a real window on Xvfb)" >&2; exit 2; fi
OUT=$1
GW=${2:-1280} GH=${3:-800}
REPO="$(git -C "$(dirname "$0")" rev-parse --show-toplevel)"
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
W="$(mktemp -d /tmp/rein-ags.XXXXXX)"
W="$(cd "$W" && pwd -P)"
P="$W/Dev/schraubr_laravel"
mkdir -p "$W/bin" "$W/home/.rein/bin" "$W/run" "$OUT" "$P/.ai"
chmod 700 "$W/run"
cc -O2 -o "$W/bin/xresize" "$REPO/crates/rein-desktop/tests/e2e/xresize.c" -lX11 || exit 2
cc -O2 -o "$W/bin/xinput" "$REPO/crates/rein-desktop/tests/e2e/xinput.c" -lX11 -lXtst || exit 2
cat >"$W/bin/tmux" <<EOF
#!/bin/sh
# Private server only: -S wins over the shim's -L.
exec "$REAL_TMUX" -S "$W/tmux.sock" "\$@"
EOF
chmod +x "$W/bin/tmux"
cat >"$W/home/.rein/bin/rein" <<'EOF'
#!/bin/sh
w=schraubr_laravel
if [ "$*" = "start --kill --yes" ]; then
  id=$(tmux list-windows -t demo -F '#{window_id} #{window_name}' 2>/dev/null | awk -v n="$w" '$2 == n { print $1 }')
  [ -n "$id" ] && tmux kill-window -t "$id"
  exit 0
fi
if ! tmux has-session -t demo 2>/dev/null; then
  tmux new-session -d -s demo -n "$w" -x 200 -y 50
  tmux split-window -h -d -t "demo:$w"
  i=0
  for p in $(tmux list-panes -t "demo:$w" -F '#{pane_id}'); do
    [ $i = 0 ] && r=engineer || r=architect
    tmux set-option -p -t "$p" @rein_role "$r"
    tmux set-option -p -t "$p" @rein_account default
    i=1
  done
  sleep 1 # each pane's shell ready, then the stand-in agent typed in as `rein pane` would
  for p in $(tmux list-panes -t "demo:$w" -F '#{pane_id}'); do tmux send-keys -t "$p" 'clear; sleep 600' Enter; done
fi
exec tmux attach -t "demo:$w"
EOF
chmod +x "$W/home/.rein/bin/rein"
T() { "$REAL_TMUX" -S "$W/tmux.sock" "$@"; }
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

for theme in catppuccin-mocha github-light; do
  echo "[$theme ${GW}x${GH}]"
  st="$W/xdg/state/rein-app"
  mkdir -p "$st"
  printf '{"schema":1,"input_style":"simple","first_run_done":true,"theme":"%s"}' "$theme" >"$st/ui.json"
  env -u TMUX -u TMUX_PANE -u CLAUDECODE \
    HOME="$W/home" PATH="$W/bin:$PATH" SHELL=/bin/sh \
    XDG_RUNTIME_DIR="$W/run" XDG_CONFIG_HOME="$W/xdg/config" XDG_DATA_HOME="$W/xdg/data" \
    XDG_STATE_HOME="$W/xdg/state" XDG_CACHE_HOME="$W/xdg/cache" \
    WEBKIT_DISABLE_DMABUF_RENDERER=1 \
    "$REPO/target/debug/rein-desktop" "$P" >"$W/app-$theme.log" 2>&1 &
  APP=$!
  for _ in $(seq 1 300); do "$W/bin/xresize" rein "$GW" "$GH" >/dev/null 2>&1 && break; sleep 0.05; done
  for _ in $(seq 1 120); do T list-clients 2>/dev/null | grep -q . && break; sleep 0.25; done
  sleep 12 # the window has seen both agents running (one poll per 5 s)
  eng="$(T list-panes -a -F '#{pane_id} #{@rein_role}' | awk '$2 == "engineer" { print $1 }')"
  T send-keys -t "$eng" C-c
  for _ in $(seq 1 40); do T display-message -p -t "$eng" '#{pane_current_command}' | grep -qx sh && break; sleep 0.25; done
  T send-keys -t "$eng" clear Enter
  sleep 11 # the next poll shows the bar
  echo "  panes: $(T list-panes -a -F '#{@rein_role}=#{pane_current_command}' | tr '\n' ' ')"
  shot "agent-bar-$theme"
  X click $((GW - 62)) 58 # Close project, at the bar's right end
  sleep 2
  shot "close-confirm-$theme"
  echo "  window still there after the confirm is shown: $(T list-windows -a -F '#W' | tr '\n' ' ')"
  X key Escape # No is the default
  echo "  after No: $(T list-windows -a -F '#W' | tr '\n' ' ')"
  # The Architect exits too: all ended, close_when_agents_exit on (the default) — the project
  # closes by itself on the next poll.
  arch="$(T list-panes -a -F '#{pane_id} #{@rein_role}' | awk '$2 == "architect" { print $1 }')"
  T send-keys -t "$arch" C-c
  sleep 12
  echo "  after both exited: windows=[$(T list-windows -a -F '#W' 2>/dev/null | tr '\n' ' ')]"
  shot "all-exited-closed-$theme"
  kill "$APP" 2>/dev/null
  wait "$APP" 2>/dev/null
  APP=""
  T kill-server 2>/dev/null
  sleep 0.5
done
