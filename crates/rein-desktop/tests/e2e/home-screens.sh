#!/usr/bin/env bash
# home-screens.sh <out-dir> — Home, every project at a glance (status.md §Home H1–H5, D-064;
# E-80): the real window (debug build) on Xvfb, in catppuccin-mocha and github-light at
# 1280x800, and one screen at 700 px wide. Each run:
#   home-empty    — the window started with no project and three recent ones: Home is the
#                   start screen (recent projects only);
#   five projects opened by second starts, each with a stand-in agent:
#     working — prints every second; stopper — prints for 25 s, then nothing (it stops);
#     ended — runs 14 s, then returns to the shell; needs-you — quiet, a bell is rung in its
#     other pane later; setup — a folder with no .ai/ (a set-up tab);
#   home-projects — Ctrl+Shift+H: Home with the five cards (the stopper still working);
#   home-stopped  — once the stopper has been quiet 15 s: its card has moved above working;
#   home-needs-you — a bell rung in needs-you's background pane: a "Needs you now" row, the
#                   needs-you card first, the Home button's count;
#   home-700      — the same at 700 px wide (one column);
#   home-prev     — what Ctrl+Shift+Tab did from the first tab (see the comment at that step).
# Then (E-98, keys on a set-up card, from REIN_DESKTOP_TRACE_STAGE — what the stage shows once a
# second): Ctrl+Shift+H shows Home from the set-up tab; Ctrl+Tab leaves it for the next tab;
# Enter on the card presses its focused button (Shift+Tab twice to "Close tab": the tab closes).
# Checked from the core's debug traces (debug builds only, no project text in them):
#   - going to a tab from Home and back resizes no pty (REIN_DESKTOP_TRACE_PTY: no line);
#   - a second run with 16 open projects (the window's tab cap) on Home: no animation frame
#     over 50 ms while the pointer moves over the cards and the statuses change
#     (REIN_DESKTOP_TRACE_FRAMES, the longest gap per second; the first second after Home is
#     shown is the switch itself and is reported apart).
# The order of the cards and the badge are read from the screens by eye (each PNG is opened
# before a handoff names it). The `rein` is a stand-in in a throwaway HOME; tmux runs on a
# private -S socket. A local tool only: it refuses to run when CI is set. Needs: Xvfb, tmux, cc,
# Xlib + XTest headers, ffmpeg, python3, a debug build. Exit 0 when every check passed.
set -uo pipefail
if [ -n "${CI:-}" ]; then echo "home-screens.sh: not in CI (it drives a real window on Xvfb)" >&2; exit 2; fi
OUT=$1
GW=1280 GH=800
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
W="$(mktemp -d /tmp/rein-home.XXXXXX)"
W="$(cd "$W" && pwd -P)"
mkdir -p "$W/bin" "$W/home/.rein/bin" "$W/run" "$OUT" "$W/Dev"
chmod 700 "$W/run"
for h in xresize; do cc -O2 -o "$W/bin/$h" "$E2E/$h.c" -lX11 || exit 2; done
cc -O2 -o "$W/bin/xinput" "$E2E/xinput.c" -lX11 -lXtst || exit 2
cat >"$W/bin/tmux" <<EOF
#!/bin/sh
exec "$REAL_TMUX" -S "$W/tmux.sock" -f /dev/null "\$@"
EOF
chmod +x "$W/bin/tmux"
# agent-burst <wait-s> <work-s>: silent, then a line a second, then silent for good (0 0: idle).
cat >"$W/bin/agent-burst" <<'SH'
#!/bin/sh
exec python3 -u -c 'import sys, time
time.sleep(int(sys.argv[1]))
for _ in range(int(sys.argv[2])): print(time.ctime()); time.sleep(1)
time.sleep(3600)' "$@"
SH
printf '#!/bin/sh\nexec python3 -u -c "import itertools, time\n[print(time.ctime()) or time.sleep(1) for _ in itertools.count()]"\n' >"$W/bin/agent-work"
printf '#!/bin/sh\nexec sleep 14\n' >"$W/bin/agent-brief"
chmod +x "$W/bin/agent-burst" "$W/bin/agent-work" "$W/bin/agent-brief"
cat >"$W/home/.rein/bin/rein" <<EOF
#!/bin/sh
[ "\$1" = start ] || exit 2
w=\$(basename "\$PWD")
if ! tmux has-session -t "\$w" 2>/dev/null; then
  tmux new-session -d -s "\$w" -n "\$w" -x 200 -y 50
  [ "\$w" = needs-you ] && tmux split-window -h -d -t "\$w:\$w"
  case "\$w" in
    working) a=agent-work ;;
    stopper) a="agent-burst 0 25" ;;
    ended) a=agent-brief ;;
    *) a="agent-burst 3600 0" ;;
  esac
  tmux set-option -p -t "\$w:\$w.0" @rein_role engineer
  sleep 1
  tmux send-keys -t "\$w:\$w.0" "clear; exec $W/bin/\$a" Enter
fi
exec tmux attach -t "\$w"
EOF
chmod +x "$W/home/.rein/bin/rein"

project() { # project <dir>: a task in progress and a handoff (the card's fields)
  mkdir -p "$1/.ai"
  cat >"$1/.ai/state.json" <<'JSON'
{"project": {"focus": "E-80 — Home"},
 "tasks": [{"id": "E-78", "status": "DONE", "description": "Notifications."},
           {"id": "E-80", "status": "IN_PROGRESS", "description": "Build Home: every project at a glance."},
           {"id": "E-79", "status": "OPEN", "description": "System notifications."},
           {"id": "E-81", "status": "BLOCKED", "description": "Read rein's waiting signal."}]}
JSON
  cat >"$1/.ai/COMM.md" <<'MD'
# COMM

## Handoff — 2026-11-19 09:24 UTC
**From**: Architect → Engineer
**Trigger**: Review of the 09:16 handoff and the ruling on worked_s
### Next action needed
- Execute the open queue.
---
MD
}

T() { "$REAL_TMUX" -S "$W/tmux.sock" -f /dev/null "$@"; }
X() { "$W/bin/xinput" "$@"; sleep 0.7; }
shot() { ffmpeg -loglevel error -y -f x11grab -draw_mouse 0 -video_size "${GW}x${GH}" -i "$DISPLAY" -frames:v 1 "$OUT/$1.png" && echo "  $OUT/$1.png"; }
check() { if [ "$2" = yes ]; then echo "  ok   $1"; else echo "  FAIL $1 ($2)"; FAIL=1; fi; }
now_ms() { date +%s%3N; }

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

desktop() { # desktop [dir]: the window, or (a second start) a handoff to the running one
  exec env -u TMUX -u TMUX_PANE -u CLAUDECODE \
    HOME="$W/home" PATH="$W/bin:$PATH" SHELL=/bin/sh REIN_DESKTOP_TRACE_PTY=1 REIN_DESKTOP_TRACE_FRAMES=1 REIN_DESKTOP_TRACE_STAGE=1 \
    XDG_RUNTIME_DIR="$W/run" XDG_CONFIG_HOME="$W/xdg/config" XDG_DATA_HOME="$W/xdg/data" \
    XDG_STATE_HOME="$W/xdg/state" XDG_CACHE_HOME="$W/xdg/cache" \
    WEBKIT_DISABLE_DMABUF_RENDERER=1 \
    "$REPO/target/debug/rein-desktop" "$@" >>"$W/app.log" 2>&1
}
second() { # second <dir>: a second start; once more if no session came of it
  local n
  if [ ! -d "$1/.ai" ]; then
    timeout 20 bash -c "$(declare -f desktop); W='$W' REPO='$REPO' desktop '$1'"; sleep 3; return
  fi
  for n in 1 2; do
    timeout 20 bash -c "$(declare -f desktop); W='$W' REPO='$REPO' desktop '$1'"
    for _ in $(seq 1 24); do T has-session -t "$(basename "$1")" 2>/dev/null && { sleep 2; return; }; sleep 0.25; done
    echo "  no session for $(basename "$1") after start $n" >&2
  done
}
quit() { kill "$APP" 2>/dev/null; wait "$APP" 2>/dev/null; APP=""; T kill-server 2>/dev/null; sleep 0.5; }
fresh_state() { # fresh_state <theme> <dir>: ui.json, and three recent projects that are not opened
  local st="$W/xdg/state/rein-app" t
  mkdir -p "$st"
  printf '{"schema":1,"input_style":"simple","first_run_done":true,"theme":"%s"}' "$1" >"$st/ui.json"
  rm -f "$st/dismissed.json"
  t=$(date +%s)
  mkdir -p "$2/recent/billing-api" "$2/recent/docs-site" "$2/recent/infra"
  printf '{"schema":1,"recent":[{"path":"%s","last_opened":%d},{"path":"%s","last_opened":%d},{"path":"%s","last_opened":%d}]}' \
    "$2/recent/billing-api" $((t - 3600)) "$2/recent/docs-site" $((t - 90000)) "$2/recent/infra" $((t - 600000)) >"$st/projects.json"
}
stage_after() { # stage_after <unix s> <pattern> [seconds]: 0 once a stage-trace line at or after then matches
  local i
  for i in $(seq 1 $((${3:-6} * 4))); do
    grep "^stage-trace" "$W/app.log" | awk -v t="$1" '$2 >= t' | grep -q -- "$2" && return 0
    sleep 0.25
  done
  return 1
}
last_stage() { grep "^stage-trace" "$W/app.log" | tail -1; }
pty_since() { grep "^pty-trace" "$W/app.log" | awk -v t="$1" '$2 >= t' | grep -c .; }

run() { # run <theme>
  local theme=$1 d="$W/Dev/$1" t0
  fresh_state "$theme" "$d"
  for p in working stopper ended needs-you; do project "$d/$p"; done
  mkdir -p "$d/setup"
  : >"$W/app.log"
  desktop &
  APP=$!
  for _ in $(seq 1 300); do "$W/bin/xresize" rein "$GW" "$GH" >/dev/null 2>&1 && break; sleep 0.05; done
  sleep 5
  shot "home-empty-$theme"
  for p in working setup stopper ended needs-you; do second "$d/$p"; done # needs-you current; setup not first (its card keeps every key)
  "$W/bin/xresize" rein "$GW" "$GH" >/dev/null
  sleep 3
  X move 900 20 # the pointer off the grid (the title bar's empty part): cards may move
  X key ctrl+shift+h; sleep 3
  shot "home-projects-$theme"
  # Back to a tab and Home again: no pty resize.
  t0=$(now_ms)
  X key alt+1; sleep 2
  X key ctrl+shift+h; sleep 2
  X key ctrl+Tab; sleep 2 # Home → the first tab
  # Ctrl+Shift+Tab (tab.prev, which goes Home from the first tab): on Xvfb the page has been
  # seen to get it without Ctrl, as Shift+Tab into the terminal (E-80 handoff). The screen
  # records what happened; Ctrl+Shift+H then goes Home whatever it did.
  X key ctrl+shift+Tab; sleep 2
  shot "home-prev-$theme"
  X key ctrl+shift+h; sleep 2
  check "going to a tab from Home and back resizes no pty" "$([ "$(pty_since "$t0")" -eq 0 ] && echo yes || echo "$(pty_since "$t0") resizes")"
  sleep 25 # the stopper worked 25 s from its start; it is quiet for 15 s by now
  shot "home-stopped-$theme"
  T send-keys -t "needs-you:needs-you.1" "printf '\\a'" Enter
  sleep 7 # one status poll and one attention poll
  shot "home-needs-you-$theme"
  if [ "$theme" = catppuccin-mocha ]; then
    "$W/bin/xresize" rein 700 "$GH" >/dev/null; sleep 2
    ffmpeg -loglevel error -y -f x11grab -draw_mouse 0 -video_size "700x${GH}" -i "$DISPLAY" -frames:v 1 "$OUT/home-700-$theme.png" && echo "  $OUT/home-700-$theme.png"
    "$W/bin/xresize" rein "$GW" "$GH" >/dev/null; sleep 1
  fi
  # E-98 (desktop.md §V9 "Keys on a set-up card"): the set-up tab is the second tab.
  local t
  X key alt+2; t=$(date +%s)
  stage_after "$t" "setup=1 tabs=5 current=setup" || echo "  (not on the set-up card: $(last_stage))"
  X key ctrl+shift+h; t=$(date +%s)
  check "Ctrl+Shift+H shows Home from a set-up tab" "$(stage_after "$t" "home=1 setup=0" && echo yes || echo "$(last_stage)")"
  X key alt+2; sleep 1.5
  X key ctrl+Tab; t=$(date +%s)
  check "Ctrl+Tab leaves a set-up tab for the next tab" "$(stage_after "$t" "home=0 setup=0 tabs=5 current=stopper" && echo yes || echo "$(last_stage)")"
  X key alt+2; sleep 1.5 # the card's first focus is "Set up this project"; Shift+Tab twice: "Close tab"
  X key shift+Tab; X key shift+Tab
  X key Return; t=$(date +%s)
  check "Enter on the card presses its focused button (Close tab)" "$(stage_after "$t" "tabs=4" && echo yes || echo "$(last_stage)")"
  quit
}

frames() { # 16 open projects on Home: the longest animation frame per second
  local d="$W/Dev/many" i t0 max lines
  fresh_state catppuccin-mocha "$d"
  for i in $(seq -w 1 16); do project "$d/project-$i"; done
  : >"$W/app.log"
  desktop "$d/project-01" &
  APP=$!
  for _ in $(seq 1 300); do "$W/bin/xresize" rein "$GW" "$GH" >/dev/null 2>&1 && break; sleep 0.05; done
  sleep 6
  for i in $(seq -w 2 16); do second "$d/project-$i"; done
  echo "  tabs: $(T list-sessions | grep -c .)"
  X key ctrl+shift+h; sleep 1.5
  t0=$(date +%s)
  # The pointer over the cards (each hover redraws a card), then off; the statuses change by
  # themselves (quiet counts up), so the grid is redrawn while held and after.
  for y in 200 300 400 500 600 400 200; do for x in 200 500 800 1100; do X move "$x" "$y"; done; done
  X move 900 20; sleep 6
  lines=$(grep "^frame-trace" "$W/app.log" | awk -v t="$t0" '$2 > t')
  max=$(printf '%s\n' "$lines" | sed -n 's/.*max_ms=\([0-9]*\).*/\1/p' | sort -n | tail -1)
  echo "  frame-trace on Home (per second, after the switch): $(printf '%s\n' "$lines" | sed -n 's/.*max_ms=\([0-9]*\).*/\1/p' | tr '\n' ' ')"
  echo "  the switch to Home: $(grep "^frame-trace" "$W/app.log" | awk -v t="$t0" '$2 == t' | sed -n 's/.*max_ms=\([0-9]*\).*/\1 ms/p' | head -1)"
  check "16 open projects on Home: no frame over 50 ms" "$([ -n "$max" ] && [ "$max" -le 50 ] && echo yes || echo "longest ${max:-none} ms")"
  shot "home-16-catppuccin-mocha"
  quit
}

for theme in catppuccin-mocha github-light; do
  echo "[$theme ${GW}x${GH}]"
  run "$theme"
done
echo "[16 projects]"
frames
[ "$FAIL" = 0 ] && echo "PASS: screens in $OUT" || echo "FAIL"
exit "$FAIL"
