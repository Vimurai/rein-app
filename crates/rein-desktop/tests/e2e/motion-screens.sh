#!/usr/bin/env bash
# motion-screens.sh <out-dir> [W H] — the screens of desktop.md §Look and feel v2 V7 (E-54,
# D-052): the real window (debug build) on Xvfb, driven through XTest (xinput.c), with every
# wait frozen mid-run in catppuccin-mocha and github-light:
#   start   — the recent projects as skeleton rows and the health badge "Checking…";
#   starting — a tab whose `rein start` has not drawn yet: progress line, spinner in the pill,
#              "Starting rein…" in the status bar;
#   running — the same tab drawn, the tmux overlay applied over the operator's own
#              ~/.tmux.conf (catppuccin, status line at the top): a missing overlay would show it;
#   busy    — Undo last update confirmed: the button busy ("Rolling back…").
# Plus, at 1280x800 in the default theme, `keep`: the operator's status line left as it is, to
# show the config really has one.
# The `rein start` is the real one (~/.rein/bin/rein, the operator's HOME, so their tmux config
# and Claude account) in a throwaway project; tmux runs on a private socket and the wrapper
# drops `send-keys`, so the panes stay shells and no Claude starts. The waits are stretched by
# wrappers: doctor 8 s, start 4 s before attaching, `rein-app rollback` 30 s, the recent list
# 6 s (REIN_DESKTOP_SLOW_LIST_MS, debug builds only).
# Needs: Xvfb, tmux, cc, Xlib + XTest headers, ffmpeg, a debug build and the real rein install.
set -uo pipefail
OUT=$1
GW=${2:-1280} GH=${3:-800}
REPO="$(cd "$(dirname "$0")/../../../.." && pwd)"
REAL_HOME=$HOME
REAL_REIN="$REAL_HOME/.rein/bin/rein"
REAL_TMUX="$(command -v tmux)"
[ -x "$REAL_REIN" ] || { echo "no rein install at $REAL_REIN" >&2; exit 2; }
W="" APP="" XV=""
cleanup() {
  [ -n "$APP" ] && kill "$APP" 2>/dev/null
  "$REAL_TMUX" -S "$W/tmux.sock" kill-server 2>/dev/null
  [ -n "$XV" ] && kill "$XV" 2>/dev/null
  [ -z "$W" ] && return
  [ -n "${KEEP:-}" ] && echo "workdir $W" >&2 || rm -rf "$W"
}
trap cleanup EXIT # set before anything is created
W="$(mktemp -d /tmp/rein-mos.XXXXXX)"
mkdir -p "$W/bin" "$W/app" "$W/home/.rein/bin" "$W/run" "$OUT" "$W/Dev/energy-api/.ai" "$W/Dev/docs/.ai" "$W/Dev/schraubr_laravel/.ai"
chmod 700 "$W/run"
cc -O2 -o "$W/bin/xresize" "$REPO/crates/rein-desktop/tests/e2e/xresize.c" -lX11 || exit 2
cc -O2 -o "$W/bin/xinput" "$REPO/crates/rein-desktop/tests/e2e/xinput.c" -lX11 -lXtst || exit 2
# The window binary beside a slow `rein-app` stub (the CLI is found beside the window first).
cp "$REPO/target/debug/rein-desktop" "$W/app/rein-desktop"
cat >"$W/app/rein-app" <<'EOF'
#!/bin/sh
echo "Restoring the previous Rein version from its snapshot…"
sleep 30
EOF
cat >"$W/bin/tmux" <<EOF
#!/bin/sh
# Private server only; send-keys dropped so the panes stay shells (no Claude starts).
[ "\$1" = send-keys ] && exit 0
exec "$REAL_TMUX" -S "$W/tmux.sock" "\$@"
EOF
cat >"$W/home/.rein/bin/rein" <<EOF
#!/bin/sh
# The real rein, with the operator's HOME and the private tmux; its waits stretched.
case "\$1" in
  doctor) sleep 8 ;;
  start) sleep 4 ;;
esac
exec env HOME="$REAL_HOME" PATH="$W/bin:\$PATH" "$REAL_REIN" "\$@"
EOF
chmod +x "$W/app/rein-app" "$W/bin/tmux" "$W/home/.rein/bin/rein"
cp "$REPO/.ai/roles.json" "$W/Dev/schraubr_laravel/.ai/roles.json"
T() { "$W/bin/tmux" "$@"; }
X() { "$W/bin/xinput" "$@"; sleep "${PAUSE:-0.6}"; }
# Bind the throwaway project to an account once (rein asks otherwise), then start fresh.
(cd "$W/Dev/schraubr_laravel" && HOME="$REAL_HOME" PATH="$W/bin:$PATH" "$REAL_REIN" start --account personal --detach >/dev/null 2>&1)
T kill-server 2>/dev/null

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

shot() { # shot <name>
  ffmpeg -loglevel error -y -f x11grab -draw_mouse 0 -video_size "${GW}x${GH}" -i "$DISPLAY" -frames:v 1 "$OUT/$1-${GW}x${GH}.png"
  echo "  $OUT/$1-${GW}x${GH}.png"
}

launch() { # launch <theme> <tmux status>
  local now st="$W/xdg/state/rein-app"
  now=$(date +%s)
  mkdir -p "$st"
  printf '{"schema":1,"input_style":"simple","first_run_done":true,"theme":"%s","tmux_status":"%s"}' "$1" "$2" >"$st/ui.json"
  printf '{"schema":1,"recent":[{"path":"%s","last_opened":%d},{"path":"%s","last_opened":%d},{"path":"%s","last_opened":%d}]}' \
    "$W/Dev/schraubr_laravel" $((now - 7200)) "$W/Dev/energy-api" $((now - 90000)) "$W/Dev/docs" $((now - 5 * 86400)) >"$st/projects.json"
  env -u TMUX -u TMUX_PANE -u CLAUDECODE \
    HOME="$W/home" PATH="$W/bin:$PATH" SHELL=/bin/sh \
    XDG_RUNTIME_DIR="$W/run" XDG_CONFIG_HOME="$W/xdg/config" XDG_DATA_HOME="$W/xdg/data" \
    XDG_STATE_HOME="$W/xdg/state" XDG_CACHE_HOME="$W/xdg/cache" \
    WEBKIT_DISABLE_DMABUF_RENDERER=1 REIN_DESKTOP_SLOW_LIST_MS=6000 \
    "$W/app/rein-desktop" >"$W/app-$1.log" 2>&1 &
  APP=$!
  for _ in $(seq 1 300); do "$W/bin/xresize" rein "$GW" "$GH" >/dev/null 2>&1 && break; sleep 0.05; done
  X move $((GW / 2)) $((GH / 2)) # no window manager: the keyboard follows the pointer
}

quit() {
  kill "$APP" 2>/dev/null
  wait "$APP" 2>/dev/null
  APP=""
  sleep 0.5
  T kill-server 2>/dev/null # the next run is a fresh start again
}

drawn() { # the tab's tmux client is attached, then redraws so Xvfb's WebKit paints the frame
  for _ in $(seq 1 80); do T list-clients 2>/dev/null | grep -q . && break; sleep 0.25; done
  sleep 3 # the overlay lands just after the attach
  local c
  c="$(T list-clients -F '#{client_tty}' | head -n1)"
  for _ in 1 2 3; do T refresh-client -t "$c"; sleep 0.7; done
}

for theme in catppuccin-mocha github-light; do
  echo "[$theme ${GW}x${GH}]"
  launch "$theme" hidden
  sleep 2.5
  shot "start-loading-$theme"  # skeleton rows (list held 6 s) + badge Checking… (doctor 8 s)
  sleep 4
  X key 1                      # recent project 1: a fresh `rein start`, held 4 s before attaching
  sleep 2
  shot "tab-starting-$theme"
  drawn
  shot "tab-running-$theme"    # overlay applied: no status line from the operator's config
  X key ctrl+shift+p
  X type "undo last"
  X key Return
  X key y                      # confirmed: `rein-app rollback` runs (30 s)
  sleep 1
  shot "confirm-busy-$theme"
  quit
done

if [ "$GW" = 1280 ]; then
  echo "[keep: the operator's status line, catppuccin-mocha ${GW}x${GH}]"
  launch catppuccin-mocha keep
  sleep 6.5
  X key 1
  drawn
  shot "tab-status-keep-catppuccin-mocha"
  quit
fi
