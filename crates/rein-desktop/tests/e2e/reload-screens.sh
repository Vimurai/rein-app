#!/usr/bin/env bash
# reload-screens.sh <out-dir> [W H] — desktop.md V12 component 10 (D-061 addendum 6, E-89): the
# real window (debug build) on Xvfb, two project tabs on the app's own tmux server (private
# sockets, stand-in rein), both themes:
#   - the web view's reload / history / find / print keys and its right-click menu do nothing
#     outside the terminal (screen no-native-menu: right-click on the title bar, no menu; the
#     core's trace shows no page reset);
#   - a reload anyway (debug builds: REIN_DESKTOP_RELOAD_AFTER_MS) ends the held ptys, and the
#     page's restore brings the same tabs back: one client per session, no second rein start,
#     the open-tabs record unchanged (screen after-reload).
# A local tool only: it refuses to run when CI is set. Needs: Xvfb, tmux, cc, Xlib + XTest
# headers, ffmpeg, a debug build made with `cargo build` (not left by `cargo test`).
set -uo pipefail
if [ -n "${CI:-}" ]; then echo "reload-screens.sh: not in CI (it drives a real window on Xvfb)" >&2; exit 2; fi
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
W="$(mktemp -d /tmp/rein-rls.XXXXXX)"
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

app() { # app <theme> <dir> [reload-after-ms]
  local st="$W/xdg/state/rein-app"
  mkdir -p "$st"
  printf '{"schema":1,"input_style":"simple","first_run_done":true,"theme":"%s"}' "$1" >"$st/ui.json"
  rm -f "$st/projects.json"
  env -u TMUX -u TMUX_PANE -u CLAUDECODE \
    HOME="$W/home" PATH="$W/bin:$PATH" SHELL=/bin/sh \
    XDG_RUNTIME_DIR="$W/run" XDG_CONFIG_HOME="$W/xdg/config" XDG_DATA_HOME="$W/xdg/data" \
    XDG_STATE_HOME="$W/xdg/state" XDG_CACHE_HOME="$W/xdg/cache" \
    WEBKIT_DISABLE_DMABUF_RENDERER=1 REIN_DESKTOP_RELOAD_AFTER_MS="${3:-}" \
    "$REPO/target/debug/rein-desktop" "$2" >>"$W/app.log" 2>&1 &
  APP=$!
  for _ in $(seq 1 300); do "$W/bin/xresize" rein "$GW" "$GH" >/dev/null 2>&1 && break; sleep 0.05; done
}
second() {
  env -u TMUX -u TMUX_PANE HOME="$W/home" PATH="$W/bin:$PATH" XDG_RUNTIME_DIR="$W/run" \
    XDG_STATE_HOME="$W/xdg/state" XDG_CONFIG_HOME="$W/xdg/config" "$REPO/target/debug/rein-desktop" "$1" >>"$W/app.log" 2>&1
}
clients() { TA list-clients -F '#{session_name}' 2>/dev/null | sort | uniq -c | awk '{ print $2 "=" $1 }' | tr '\n' ' ' | sed 's/ $//'; }
resets() { grep -c '^page-reset' "$W/app.log"; }
open_rec() { python3 -c 'import json,sys,os; d=json.load(open(sys.argv[1])); print(" ".join(os.path.basename(t["path"]) for t in d.get("open",[])))' "$W/xdg/state/rein-app/projects.json" 2>/dev/null; }

for theme in catppuccin-mocha github-light; do
  echo "[$theme ${GW}x${GH}]"
  : >"$W/rein.log"; : >"$W/app.log"
  a="$W/Dev/alpha-$theme" b="$W/Dev/beta-$theme"
  mkdir -p "$a/.ai" "$b/.ai"
  app "$theme" "$a" 40000 # the page reloads once, 40 s after start
  sleep 7
  second "$b"
  sleep 7
  check "two tabs, one client each" "alpha-$theme=1 beta-$theme=1" "$(clients)"
  # The browser's own keys and menu with the keyboard outside the terminal: in the side panel
  # (Dashboard, focused). The title bar is the window's drag area and keeps the terminal focused.
  X key ctrl+shift+1; sleep 1.5; X key ctrl+shift+e
  for k in F5 ctrl+r ctrl+shift+r alt+Left ctrl+p ctrl+f ctrl+u F7; do X key "$k"; done
  sleep 2
  X click 240 400 3; sleep 1.5 # right-click inside the panel
  shot "no-native-menu-$theme"
  X key Escape; X key ctrl+shift+0
  check "no key or menu reloaded the page" 0 "$(resets)"
  check "  ... and nothing started again" 2 "$(grep -c ': start$' "$W/rein.log")"
  record=$(open_rec)
  # The reload (debug switch): wait for the core's trace, then for the restore.
  for _ in $(seq 1 120); do [ "$(resets)" -gt 0 ] && break; sleep 0.25; done
  check "the page reloaded once, with both tabs held" "page-reset held=2" "$(grep '^page-reset' "$W/app.log")"
  sleep 8
  check "after the reload: one client per session (none left behind)" "alpha-$theme=1 beta-$theme=1" "$(clients)"
  check "  ... no second rein start" 2 "$(grep -c ': start$' "$W/rein.log")"
  check "  ... the same tabs" "$record" "$(open_rec)"
  check "  ... and the same one in front (beta: the command-line alpha is not focused again)" 1 "$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1])).get("active"))' "$W/xdg/state/rein-app/projects.json")"
  shot "after-reload-$theme"
  kill "$APP" 2>/dev/null; wait "$APP" 2>/dev/null; APP=""
  TA kill-server 2>/dev/null; TD kill-server 2>/dev/null
  sleep 0.5
done

if [ "$fails" -eq 0 ]; then echo "PASS: reload screens"; else echo "$fails failure(s)"; fi
[ "$fails" -eq 0 ]
