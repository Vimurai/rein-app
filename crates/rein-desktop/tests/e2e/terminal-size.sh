#!/usr/bin/env bash
# terminal-size.sh [rein-desktop binary] — E-52 / D-052 end-to-end size check, Linux, Xvfb.
#
# Opens a project tab in the real window on a 2560 x 1440 Xvfb screen and reads the size of
# the app's own tmux client. A tab whose open is slow (held 3 s by slow-open.so, as wsl.exe
# holds it on Windows) must end at the same size as a fast one, and a maximised window must
# be far above xterm's default 80 x 24. Before the fix the slow tab stayed at 80 x 24.
#
# Hermetic: HOME, XDG dirs and the tmux socket live in a fresh temp dir. `rein start` is a
# stand-in that does what matters here — a detached session, then `tmux attach` in the pty.
# Every tmux call (the stand-in's too) goes through a wrapper pinned to a private socket, so
# no tmux server outside the temp dir is ever touched.
# Needs: Xvfb, tmux, cc, Xlib headers. SHOT_DIR=<dir> also saves screenshots (ffmpeg).
set -euo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
BIN="${1:-$HERE/../../../../target/debug/rein-desktop}"
BIN="$(cd "$(dirname "$BIN")" && pwd)/$(basename "$BIN")"
for tool in Xvfb tmux cc; do
  command -v "$tool" >/dev/null || { echo "terminal-size: $tool not found" >&2; exit 2; }
done
[ -x "$BIN" ] || { echo "terminal-size: no binary at $BIN" >&2; exit 2; }

W="$(mktemp -d /tmp/rein-e2e.XXXXXX)" # short: a tmux socket path has a length limit
REAL_TMUX="$(command -v tmux)"
APP="" XV=""
cleanup() {
  [ -n "$APP" ] && kill "$APP" 2>/dev/null || true
  "$W/bin/tmux" kill-server 2>/dev/null || true # the private socket only
  [ -n "$XV" ] && kill "$XV" 2>/dev/null || true
  if [ -n "${KEEP:-}" ]; then echo "kept $W" >&2; else rm -rf "$W"; fi
}
trap cleanup EXIT

mkdir -p "$W/bin" "$W/home/.rein/bin" "$W/proj/.ai" "$W/run"
chmod 700 "$W/run"
cc -shared -fPIC -O2 -o "$W/slow-open.so" "$HERE/slow-open.c" -ldl
cc -O2 -o "$W/bin/xresize" "$HERE/xresize.c" -lX11

cat >"$W/bin/tmux" <<EOF
#!/bin/sh
exec "$REAL_TMUX" -S "$W/tmux.sock" -f /dev/null "\$@"
EOF
cat >"$W/home/.rein/bin/rein" <<'EOF'
#!/bin/sh
# Stand-in for `rein start` (and a failing `rein doctor`): the session is created detached at
# tmux's default size, as rein does, then attached inside the window's pty.
[ "$1" = start ] || exit 1
tmux new-session -d -s e2e 2>/dev/null || true
exec tmux attach-session -t e2e
EOF
chmod +x "$W/bin/tmux" "$W/home/.rein/bin/rein"

# A free display: try numbers until an Xvfb stays up (one in use, even only as an abstract
# socket, makes Xvfb exit at once).
started=""
for n in $(seq 90 130); do
  [ -e "/tmp/.X$n-lock" ] && continue
  Xvfb ":$n" -screen 0 2560x1440x24 -nolisten tcp >"$W/xvfb.log" 2>&1 &
  XV=$!
  # Up when its socket file appears, or when it is still running after 1.5 s (under WSLg
  # /tmp/.X11-unix is read-only and Xvfb listens on its abstract socket only).
  for i in $(seq 1 15); do
    kill -0 "$XV" 2>/dev/null || break
    if [ -e "/tmp/.X11-unix/X$n" ] || [ "$i" -eq 15 ]; then started=":$n"; break; fi
    sleep 0.1
  done
  [ -n "$started" ] && break
  kill "$XV" 2>/dev/null || true
  XV=""
done
[ -n "$started" ] || { echo "terminal-size: Xvfb did not start" >&2; cat "$W/xvfb.log" >&2; exit 2; }
export DISPLAY="$started"

# run <label> <slow-ms> <maximise: yes|no> → prints the app client's "COLSxROWS"
run() {
  local label=$1 slow=$2 max=$3 size=""
  "$W/bin/tmux" kill-server 2>/dev/null || true
  rm -rf "$W/xdg"
  env -u TMUX -u TMUX_PANE -u CLAUDECODE \
    HOME="$W/home" PATH="$W/bin:$PATH" SHELL=/bin/sh \
    XDG_RUNTIME_DIR="$W/run" XDG_CONFIG_HOME="$W/xdg/config" XDG_DATA_HOME="$W/xdg/data" \
    XDG_STATE_HOME="$W/xdg/state" XDG_CACHE_HOME="$W/xdg/cache" \
    WEBKIT_DISABLE_DMABUF_RENDERER=1 \
    LD_PRELOAD="$([ "$slow" -gt 0 ] && echo "$W/slow-open.so")" \
    SLOW_OPEN_MARK="$W/proj" SLOW_OPEN_MS="$slow" \
    "$BIN" "$W/proj" >"$W/app-$label.log" 2>&1 &
  APP=$!
  if [ "$max" = yes ]; then
    for _ in $(seq 1 300); do "$W/bin/xresize" rein 2560 1440 >/dev/null 2>&1 && break; sleep 0.05; done
  fi
  for _ in $(seq 1 120); do
    "$W/bin/tmux" list-clients 2>/dev/null | grep -q . && break
    kill -0 "$APP" 2>/dev/null || { echo "terminal-size: the window exited:" >&2; cat "$W/app-$label.log" >&2; break; }
    sleep 0.5
  done
  sleep 4 # the open, then any resize, settle
  size="$("$W/bin/tmux" list-clients -F '#{client_width}x#{client_height}' 2>/dev/null | head -n1)"
  if [ -n "${SHOT_DIR:-}" ] && command -v ffmpeg >/dev/null; then
    mkdir -p "$SHOT_DIR"
    ffmpeg -loglevel error -y -f x11grab -video_size 2560x1440 -i "$DISPLAY" -frames:v 1 \
      "$SHOT_DIR/terminal-size-$label.png" || true
  fi
  kill "$APP" 2>/dev/null || true
  wait "$APP" 2>/dev/null || true
  APP=""
  echo "${size:-none}"
}

fail=0
fast_max="$(run fast-max 0 yes)"
slow_max="$(run slow-max 3000 yes)"
fast_min="$(run fast-default 0 no)"
slow_min="$(run slow-default 3000 no)"
echo "maximised 2560x1440: fast open $fast_max, slow open $slow_max"
echo "default window:      fast open $fast_min, slow open $slow_min"

[ "$slow_max" = "$fast_max" ] || { echo "FAIL: a slow open ends at $slow_max, a fast one at $fast_max"; fail=1; }
[ "$slow_min" = "$fast_min" ] || { echo "FAIL: a slow open ends at $slow_min, a fast one at $fast_min"; fail=1; }
cols="${slow_max%x*}" rows="${slow_max#*x}"
if ! [[ "$cols" =~ ^[0-9]+$ && "$rows" =~ ^[0-9]+$ ]] || [ "$cols" -lt 200 ] || [ "$rows" -lt 50 ]; then
  echo "FAIL: the maximised window's tmux client is $slow_max, expected well above 80x24"
  fail=1
fi
[ "$fail" -eq 0 ] && echo "PASS: the pty follows the stage"
exit "$fail"
