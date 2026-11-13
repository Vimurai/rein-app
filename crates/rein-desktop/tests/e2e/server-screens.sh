#!/usr/bin/env bash
# server-screens.sh <out-dir> [W H] — the screens of desktop.md §Look and feel v2 V8 (E-57,
# D-052): a running project tab of the real window (debug build) on Xvfb, the session on the
# window's own tmux server, with the operator's own ~/.tmux.conf in place in the HOME the
# window runs with — so its status line, key bindings and plugins would show if they reached
# the window's server. catppuccin-mocha and github-light.
# The `rein start` is the real one: a throwaway HOME links to the operator's ~/.rein install
# and Claude account directories and holds a COPY of their ~/.tmux.conf; the project is bound
# to the `default` account by its .ai/start.json. The window runs its real setup into that
# throwaway HOME, so the shim (first on the tab's PATH) forwards to the next `tmux` on PATH:
# a wrapper that pins a private -S socket. The private server is therefore started with the
# app's config only — the operator's config is never read, so its plugin bootstrap (git
# clone) never runs — and the operator's own servers and any real `rein-app` socket are never
# touched. The wrapper drops `send-keys`, so the panes stay shells and no Claude starts.
# A local tool only: it refuses to run when CI is set. Needs: Xvfb, tmux, cc, Xlib + XTest
# headers, ffmpeg, a debug build, the real rein install and ~/.tmux.conf.
set -uo pipefail
if [ -n "${CI:-}" ]; then echo "server-screens.sh: not in CI (it links the operator's real rein install)" >&2; exit 2; fi
OUT=$1
GW=${2:-1280} GH=${3:-800}
REPO="$(git -C "$(dirname "$0")" rev-parse --show-toplevel)"
REAL_HOME=$HOME
REAL_TMUX="$(command -v tmux)"
[ -x "$REAL_HOME/.rein/bin/rein" ] || { echo "no rein install" >&2; exit 2; }
[ -f "$REAL_HOME/.tmux.conf" ] || { echo "no ~/.tmux.conf to put in place" >&2; exit 2; }
W="" APP="" XV=""
cleanup() {
  [ -n "$APP" ] && kill "$APP" 2>/dev/null
  [ -n "$W" ] && "$REAL_TMUX" -S "$W/tmux.sock" kill-server 2>/dev/null
  [ -n "$XV" ] && kill "$XV" 2>/dev/null
  [ -z "$W" ] && return
  [ -n "${KEEP:-}" ] && echo "workdir $W" >&2 || rm -rf "$W"
}
trap cleanup EXIT
W="$(mktemp -d /tmp/rein-svs.XXXXXX)"
W="$(cd "$W" && pwd -P)"
mkdir -p "$W/bin" "$W/home" "$W/run" "$OUT" "$W/Dev/schraubr_laravel/.ai"
chmod 700 "$W/run"
cc -O2 -o "$W/bin/xresize" "$REPO/crates/rein-desktop/tests/e2e/xresize.c" -lX11 || exit 2
cc -O2 -o "$W/bin/xinput" "$REPO/crates/rein-desktop/tests/e2e/xinput.c" -lX11 -lXtst || exit 2
cat >"$W/bin/tmux" <<EOF
#!/bin/sh
# Private server only (-S wins over the shim's -L); send-keys dropped (no Claude starts).
[ "\$1" = send-keys ] && exit 0
for a in "\$@"; do [ "\$a" = send-keys ] && exit 0; done
exec "$REAL_TMUX" -S "$W/tmux.sock" "\$@"
EOF
chmod +x "$W/bin/tmux"
ln -s "$REAL_HOME/.rein" "$W/home/.rein"
for a in .claude .claude-personal .claude-work; do [ -e "$REAL_HOME/$a" ] && ln -s "$REAL_HOME/$a" "$W/home/$a"; done
cp "$REAL_HOME/.tmux.conf" "$W/home/.tmux.conf" # the operator's config, in place
cp "$REPO/.ai/roles.json" "$W/Dev/schraubr_laravel/.ai/roles.json"
# The `default` account (~/.claude, linked above): rein refuses an account whose directory is
# not under HOME, and the throwaway HOME only links to the real ones.
printf '{\n  "account": "default"\n}\n' >"$W/Dev/schraubr_laravel/.ai/start.json"
T() { "$REAL_TMUX" -S "$W/tmux.sock" "$@"; }

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
    "$REPO/target/debug/rein-desktop" "$W/Dev/schraubr_laravel" >"$W/app-$theme.log" 2>&1 &
  APP=$!
  for _ in $(seq 1 300); do "$W/bin/xresize" rein "$GW" "$GH" >/dev/null 2>&1 && break; sleep 0.05; done
  for _ in $(seq 1 120); do T list-clients 2>/dev/null | grep -q . && break; sleep 0.25; done
  sleep 3
  c="$(T list-clients -F '#{client_tty}' | head -n1)"
  for _ in 1 2 3; do T refresh-client -t "$c"; sleep 0.7; done
  # Proof the operator's config did not reach the server: it sets status-position top and a
  # status line; the app's config leaves the position at tmux's default and the line off.
  echo "  server: status=$(T show-options -gv status) status-position=$(T show-options -gv status-position) prefix=$(T show-options -gv prefix) config-files=[$(T display-message -p '#{config_files}')]"
  echo "  tmux home: $(ls -la "$W/home/.local/state/rein-app/tmux" "$W/home/.local/state/rein-app/tmux/bin" | grep -E ' (tmux|tmux.conf|bin)$' | awk '{print $1, $NF}' | tr '\n' ' ')"
  ffmpeg -loglevel error -y -f x11grab -draw_mouse 0 -video_size "${GW}x${GH}" -i "$DISPLAY" -frames:v 1 "$OUT/tab-own-server-$theme-${GW}x${GH}.png"
  echo "  $OUT/tab-own-server-$theme-${GW}x${GH}.png"
  kill "$APP" 2>/dev/null
  wait "$APP" 2>/dev/null
  APP=""
  T kill-server 2>/dev/null
  sleep 0.5
done
