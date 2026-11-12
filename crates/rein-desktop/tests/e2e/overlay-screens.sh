#!/usr/bin/env bash
# overlay-screens.sh <W> <H> <out-dir> [binary] — E-51 screens (D-052): the real window on Xvfb, an operator-style
# ~/.tmux.conf (coloured status line, plugin-free), three runs on ONE private tmux server:
#   hidden (default) → app → after both detached, the user's own look (overlay made to stop
#   before touching anything by a tmux that reports 3.1).
# Every tmux call goes to a private socket in a mktemp dir; never the user's server.
# Xvfb note: WebKit there paints an output batch only when the next one arrives, so each shot
# is preceded by three `refresh-client` redraws. Needs Xvfb, tmux, cc, Xlib headers, ffmpeg.
set -uo pipefail
GW=$1 GH=$2 OUT=$3
REPO="$(cd "$(dirname "$0")/../../../.." && pwd)"
BIN="${4:-$REPO/target/debug/rein-desktop}"
REAL="$(command -v tmux)"
W="$(mktemp -d /tmp/rein-ovs.XXXXXX)"
APP="" XV=""
cleanup() {
  [ -n "$APP" ] && kill "$APP" 2>/dev/null
  "$REAL" -S "$W/tmux.sock" kill-server 2>/dev/null
  [ -n "$XV" ] && kill "$XV" 2>/dev/null
  echo "workdir $W" >&2
}
trap cleanup EXIT
mkdir -p "$W/bin" "$W/home/.rein/bin" "$W/proj/.ai" "$W/run" "$OUT"
chmod 700 "$W/run"
cc -O2 -o "$W/bin/xresize" "$REPO/crates/rein-desktop/tests/e2e/xresize.c" -lX11 || exit 2

cat >"$W/bin/tmux" <<EOF
#!/bin/sh
if [ -n "\${FAKE_OLD_TMUX:-}" ] && [ "\$1" = -V ]; then echo "tmux 3.1c"; exit 0; fi
exec "$REAL" -S "$W/tmux.sock" "\$@"
EOF
cat >"$W/home/.rein/bin/rein" <<'EOF'
#!/bin/sh
[ "$1" = start ] || exit 1
if ! tmux has-session -t rein-demo 2>/dev/null; then
  tmux new-session -d -s rein-demo -n triad 'printf "architect pane\n"; exec sh'
  tmux split-window -h -t rein-demo 'printf "engineer pane\n"; exec sh'
  tmux split-window -v -t rein-demo 'printf "tests pane\n"; exec sh'
  tmux new-window -d -t rein-demo -n logs 'exec sh'
  tmux select-window -t rein-demo:triad
fi
exec tmux attach-session -t rein-demo
EOF
# The operator's look: a catppuccin-style coloured status line and borders, no plugin.
cat >"$W/home/.tmux.conf" <<'EOF'
set -g status-style "bg=#181825,fg=#cdd6f4"
set -g status-left-length 40
set -g status-left "#[bg=#f38ba8,fg=#11111b,bold]  #S #[bg=#181825] "
set -g status-right "#[bg=#fab387,fg=#11111b]  %H:%M #[bg=#a6e3a1,fg=#11111b,bold]  my-laptop "
set -g window-status-format "#[bg=#313244,fg=#cdd6f4] #I #W "
set -g window-status-current-format "#[bg=#89b4fa,fg=#11111b,bold] #I #W "
set -g pane-border-style "fg=#f9e2af"
set -g pane-active-border-style "fg=#f38ba8"
set -g message-style "bg=#f9e2af,fg=#11111b"
EOF
chmod +x "$W/bin/tmux" "$W/home/.rein/bin/rein"
T() { "$W/bin/tmux" "$@"; }

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

state() { # the session's own option values and the overlay hooks
  echo "  session: $(for o in status status-style message-style; do printf '%s=[%s] ' "$o" "$(T show-options -t rein-demo -v "$o" 2>/dev/null)"; done)"
  echo "  windows: $(for w in $(T list-windows -t rein-demo -F '#{window_id}'); do printf '%s border=[%s] ' "$w" "$(T show-options -w -t "$w" -v pane-border-style 2>/dev/null)"; done)"
  echo "  hooks:   [$(T show-hooks -t rein-demo 2>/dev/null | grep 4271 | tr '\n' ' ')]"
}

run() { # run <label> <status mode> [old]
  local label=$1 mode=$2 old=${3:-}
  mkdir -p "$W/xdg/state/rein-app"
  printf '{"schema":1,"input_style":"simple","first_run_done":true,"tmux_status":"%s"}' "$mode" \
    >"$W/xdg/state/rein-app/ui.json"
  env -u TMUX -u TMUX_PANE -u CLAUDECODE \
    HOME="$W/home" PATH="$W/bin:$PATH" SHELL=/bin/sh \
    XDG_RUNTIME_DIR="$W/run" XDG_CONFIG_HOME="$W/xdg/config" XDG_DATA_HOME="$W/xdg/data" \
    XDG_STATE_HOME="$W/xdg/state" XDG_CACHE_HOME="$W/xdg/cache" \
    WEBKIT_DISABLE_DMABUF_RENDERER=1 ${old:+FAKE_OLD_TMUX=1} \
    "$BIN" "$W/proj" >"$W/app-$label.log" 2>&1 &
  APP=$!
  for _ in $(seq 1 300); do "$W/bin/xresize" rein "$GW" "$GH" >/dev/null 2>&1 && break; sleep 0.05; done
  for _ in $(seq 1 120); do T list-clients 2>/dev/null | grep -q . && break; sleep 0.5; done
  for i in $(seq 1 20); do echo "  t+${i}x0.5s status=[$(T show-options -t rein-demo -v status 2>/dev/null)] border=[$(T show-options -w -t rein-demo -v pane-border-style 2>/dev/null)]"; sleep 0.5; done | uniq -c -f1 | head -5
  echo "[$label ${GW}x${GH}] client $(T list-clients -F '#{client_tty} #{client_width}x#{client_height}' 2>/dev/null)"
  state
  # Xvfb: WebKit paints a batch only when the next one arrives; redraws make the last
  # painted batch the current screen.
  ctty="$(T list-clients -F '#{client_tty}' | head -n1)"
  for _ in 1 2 3; do T refresh-client -t "$ctty"; sleep 1; done
  ffmpeg -loglevel error -y -f x11grab -video_size "${GW}x${GH}" -i "$DISPLAY" -frames:v 1 \
    "$OUT/${label}-${GW}x${GH}.png"
  kill "$APP" 2>/dev/null
  wait "$APP" 2>/dev/null
  APP=""
  for _ in $(seq 1 50); do T list-clients 2>/dev/null | grep -q . || break; sleep 0.1; done
  sleep 1
  echo "  after the window closed:"
  state
}

run overlay-hidden hidden
run overlay-app app
run restored-after-detach keep old
