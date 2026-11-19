#!/usr/bin/env bash
# notify-screens.sh <out-dir> [W H] — notifications in the window (status.md §Components 6
# channels 1 and 2, D-062; E-78): the real window (debug build) on Xvfb, in catppuccin-mocha
# and github-light. Three projects, each with a stand-in agent:
#   worker — prints a line every second for 25 s, then nothing (a background tab that works
#            and stops);
#   later  — nothing for 75 s, then prints for 20 s, then nothing (it stops while the window
#            is hidden);
#   front  — runs and prints nothing; opened last, so it is the current tab.
# Checked from the core's trace (REIN_DESKTOP_TRACE_NOTIFY, debug builds only; no sentence in
# it) and from the screen:
#   - worker raises `stopped` once, with the window shown; its toast is on screen
#     (notify-toast-<theme>): proof the web side drew it;
#   - with the window unmapped (hidden), later raises `stopped` once and the core asks for the
#     window's attention (attention=true: the window is not focused);
#   - Settings shows the notification rows (notify-settings-<theme>);
#   - front raises nothing (it never worked), nothing is raised twice, and nothing in the
#     first 30 s after the tabs open.
# The `rein` is a stand-in in a throwaway HOME; tmux runs on a private -S socket. A local tool
# only: it refuses to run when CI is set. Needs: Xvfb, tmux, cc, Xlib + XTest headers, ffmpeg,
# python3, a debug build. Exit 0 when every check passed.
set -uo pipefail
if [ -n "${CI:-}" ]; then echo "notify-screens.sh: not in CI (it drives a real window on Xvfb)" >&2; exit 2; fi
OUT=$1
GW=${2:-1280} GH=${3:-800}
REPO="$(git -C "$(dirname "$0")" rev-parse --show-toplevel)"
. "$(dirname "$0")/window-binary.sh" && window_binary
E2E="$REPO/crates/rein-desktop/tests/e2e"
REAL_TMUX="$(command -v tmux)"
W="" APP="" XV="" FAIL=0
cleanup() {
  [ -n "$APP" ] && kill "$APP" 2>/dev/null
  [ -n "$W" ] && "$REAL_TMUX" -S "$W/tmux.sock" kill-server 2>/dev/null
  [ -n "$XV" ] && kill "$XV" 2>/dev/null
  [ -z "$W" ] && return
  [ -n "${KEEP:-}" ] && echo "workdir $W" >&2 || rm -rf "$W"
}
trap cleanup EXIT
W="$(mktemp -d /tmp/rein-ntf.XXXXXX)"
W="$(cd "$W" && pwd -P)"
mkdir -p "$W/bin" "$W/home/.rein/bin" "$W/run" "$OUT" "$W/Dev"
chmod 700 "$W/run"
for h in xresize xmap; do cc -O2 -o "$W/bin/$h" "$E2E/$h.c" -lX11 || exit 2; done
cc -O2 -o "$W/bin/xinput" "$E2E/xinput.c" -lX11 -lXtst || exit 2
cat >"$W/bin/tmux" <<EOF
#!/bin/sh
exec "$REAL_TMUX" -S "$W/tmux.sock" -f /dev/null "\$@"
EOF
chmod +x "$W/bin/tmux"
# agent-burst <wait-s> <work-s>: silent, then a line a second, then silent for good.
cat >"$W/bin/agent-burst" <<'SH'
#!/bin/sh
exec python3 -u -c 'import sys, time
time.sleep(int(sys.argv[1]))
for _ in range(int(sys.argv[2])): print(time.ctime()); time.sleep(1)
time.sleep(3600)' "$@"
SH
chmod +x "$W/bin/agent-burst"
cat >"$W/home/.rein/bin/rein" <<EOF
#!/bin/sh
[ "\$1" = start ] || exit 2
w=\$(basename "\$PWD")
if ! tmux has-session -t "\$w" 2>/dev/null; then
  tmux new-session -d -s "\$w" -n "\$w" -x 200 -y 50
  case "\$w" in
    worker) a="agent-burst 0 25" ;;
    later) a="agent-burst 75 20" ;;
    *) a="agent-burst 3600 0" ;;
  esac
  tmux set-option -p -t "\$w:\$w.0" @rein_role engineer
  sleep 1
  tmux send-keys -t "\$w:\$w.0" "clear; exec $W/bin/\$a" Enter
fi
exec tmux attach -t "\$w"
EOF
chmod +x "$W/home/.rein/bin/rein"

project() { mkdir -p "$1/.ai"; printf '{"tasks": []}\n' >"$1/.ai/state.json"; }
X() { "$W/bin/xinput" "$@"; sleep 0.7; }
T() { "$REAL_TMUX" -S "$W/tmux.sock" -f /dev/null "$@"; }
shot() { ffmpeg -loglevel error -y -f x11grab -draw_mouse 0 -video_size "${GW}x${GH}" -i "$DISPLAY" -frames:v 1 "$OUT/$1-${GW}x${GH}.png" && echo "  $OUT/$1-${GW}x${GH}.png"; }
check() { # check <what> <ok?>
  if [ "$2" = yes ]; then echo "  ok   $1"; else echo "  FAIL $1 ($2)"; FAIL=1; fi
}
traced() { grep "^notify-trace" "$W/app.log" | grep -- "$1" | grep -c .; }
wait_for() { # wait_for <pattern> <seconds>: 0 once the trace has a line matching it
  local i
  for i in $(seq 1 "$(($2 * 4))"); do
    grep "^notify-trace" "$W/app.log" | grep -q -- "$1" && return 0
    sleep 0.25
  done
  return 1
}

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

desktop() {
  exec env -u TMUX -u TMUX_PANE -u CLAUDECODE \
    HOME="$W/home" PATH="$W/bin:$PATH" SHELL=/bin/sh REIN_DESKTOP_TRACE_NOTIFY=1 \
    XDG_RUNTIME_DIR="$W/run" XDG_CONFIG_HOME="$W/xdg/config" XDG_DATA_HOME="$W/xdg/data" \
    XDG_STATE_HOME="$W/xdg/state" XDG_CACHE_HOME="$W/xdg/cache" \
    WEBKIT_DISABLE_DMABUF_RENDERER=1 \
    "$REPO/target/debug/rein-desktop" "$1" >>"$W/app.log" 2>&1
}
second() { # second <dir>: a second start; once more if no session came of it
  local n
  for n in 1 2; do
    timeout 20 bash -c "$(declare -f desktop); W='$W' REPO='$REPO' desktop '$1'"
    for _ in $(seq 1 24); do T has-session -t "$(basename "$1")" 2>/dev/null && { sleep 2; return; }; sleep 0.25; done
    echo "  no session for $(basename "$1") after start $n" >&2
  done
}
quit() { kill "$APP" 2>/dev/null; wait "$APP" 2>/dev/null; APP=""; T kill-server 2>/dev/null; sleep 0.5; }

run() { # run <theme>
  local theme=$1 st="$W/xdg/state/rein-app" d="$W/Dev/$1" t0 first
  mkdir -p "$st"
  : >"$W/app.log"
  printf '{"schema":1,"input_style":"simple","first_run_done":true,"theme":"%s"}' "$theme" >"$st/ui.json"
  rm -f "$st/projects.json" "$st/dismissed.json"
  for p in worker later front; do project "$d/$p"; done
  t0=$(date +%s)
  desktop "$d/worker" &
  APP=$!
  for _ in $(seq 1 300); do "$W/bin/xresize" rein "$GW" "$GH" >/dev/null 2>&1 && break; sleep 0.05; done
  sleep 6
  second "$d/later"
  second "$d/front"
  "$W/bin/xresize" rein "$GW" "$GH" >/dev/null
  # worker: 25 s of work from ~1 s after its start, then 30 s quiet, then the next 5 s poll.
  if wait_for "event=stopped .*project=worker" 90; then
    shot "notify-toast-$theme"
    check "worker raised stopped with the window shown" yes
    # Settings: the notification rows (E-78), the body scrolled to them.
    sleep 4; X key ctrl+comma; sleep 1.5
    for _ in 1 2 3 4; do X click 640 400 5; done # button 5: the wheel down
    shot "notify-settings-$theme"
    X key Escape; sleep 1
  else
    check "worker raised stopped with the window shown" "not within 90 s"
  fi
  first=$(grep "^notify-trace .* event=" "$W/app.log" | awk 'NR == 1 {print $2}')
  check "nothing in the first 30 s after the tabs opened" "$([ -n "$first" ] && [ $((first - t0)) -ge 30 ] && echo yes || echo "first at $((${first:-0} - t0)) s")"
  # Hidden: later starts working at ~75 s after its start and stops 20 s on; the hidden poll
  # runs every 15 s.
  "$W/bin/xmap" rein unmap >/dev/null
  if wait_for "event=stopped .*project=later" 150; then
    check "later raised stopped with the window hidden" yes
    check "the window's attention was asked for (not focused)" "$([ "$(traced 'project=later attention=true')" -ge 1 ] && echo yes || echo "$(grep 'project=later' "$W/app.log")")"
  else
    check "later raised stopped with the window hidden" "not within 150 s"
  fi
  "$W/bin/xmap" rein map >/dev/null
  "$W/bin/xresize" rein "$GW" "$GH" >/dev/null
  sleep 6
  check "front raised nothing" "$([ "$(traced 'project=front')" -eq 0 ] && echo yes || echo "$(traced 'project=front') lines")"
  check "worker's stop raised once" "$([ "$(traced 'event=stopped .*project=worker')" -eq 1 ] && echo yes || echo "$(traced 'event=stopped .*project=worker') times")"
  check "later's stop raised once" "$([ "$(traced 'event=stopped .*project=later')" -eq 1 ] && echo yes || echo "$(traced 'event=stopped .*project=later') times")"
  echo "  trace:"; grep "^notify-trace" "$W/app.log" | sed 's/^/    /'
  quit
}

for theme in catppuccin-mocha github-light; do
  echo "[$theme ${GW}x${GH}]"
  run "$theme"
done
[ "$FAIL" = 0 ] && echo "PASS: screens in $OUT" || echo "FAIL"
exit "$FAIL"
