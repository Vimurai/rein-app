#!/usr/bin/env bash
# status-screens.sh <out-dir> [W H] — each project's status in the window (status.md §Components
# 2 and 3, D-062; E-77): the real window (debug build) on Xvfb, in catppuccin-mocha and
# github-light. Six projects, opened one by one (the first starts the window, the rest by a
# second start), each with a stand-in agent that behaves like the state it is for:
#   working   — prints a line every second;
#   quiet     — runs and prints nothing (quiet after 15 s); has a task in progress and handoffs;
#   ended     — runs for 14 s, then returns to the shell (seen running, then gone);
#   needs-you — runs quietly; a bell is rung in its other pane while the tab is in the background;
#   unknown   — no agent pane at all;
#   starting  — `rein start` never attaches (the core says starting for 20 s), opened last.
# Screens:
#   status-tabs      — the tab bar with one tab in each of those states (starting is current);
#   status-tooltip   — the pointer on the quiet tab: its tooltip;
#   status-dashboard — the quiet tab current: the status-bar segment and the Dashboard's first row;
#   status-hidden    — the same with `[tabs] status = "hidden"` set in Settings' file (ui.json):
#                      no marks, no segment; the Dashboard row stays.
# `setup` is not on screen: the core sends no status for a tab without a pty (a refused set-up
# opens none), so the window cannot show it — the render test draws that mark.
# Checked: the marks the window drew (data-status) are read from the screen's accessibility
# names by eye; the core's trace is not needed. The `rein` is a stand-in in a throwaway HOME;
# tmux runs on a private -S socket. A local tool only: it refuses to run when CI is set. Needs:
# Xvfb, tmux, cc, Xlib + XTest headers, ffmpeg, python3, a debug build.
set -uo pipefail
if [ -n "${CI:-}" ]; then echo "status-screens.sh: not in CI (it drives a real window on Xvfb)" >&2; exit 2; fi
OUT=$1
GW=${2:-1280} GH=${3:-800}
REPO="$(git -C "$(dirname "$0")" rev-parse --show-toplevel)"
. "$(dirname "$0")/window-binary.sh" && window_binary # cargo test's binary or a stale UI never reaches the window (E-84)
E2E="$REPO/crates/rein-desktop/tests/e2e"
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
W="$(mktemp -d /tmp/rein-sts.XXXXXX)"
W="$(cd "$W" && pwd -P)"
mkdir -p "$W/bin" "$W/home/.rein/bin" "$W/run" "$OUT" "$W/Dev"
chmod 700 "$W/run"
for h in xresize; do cc -O2 -o "$W/bin/$h" "$E2E/$h.c" -lX11 || exit 2; done
cc -O2 -o "$W/bin/xinput" "$E2E/xinput.c" -lX11 -lXtst || exit 2
cat >"$W/bin/tmux" <<EOF
#!/bin/sh
# Private server only: -S wins over the shim's -L.
exec "$REAL_TMUX" -S "$W/tmux.sock" -f /dev/null "\$@"
EOF
chmod +x "$W/bin/tmux"
# The stand-in agents: never a shell in the foreground (a role pane showing a shell has no
# agent), so each ends in exec; the pane shows the shell again once one exits.
cat >"$W/bin/agent-work" <<'SH'
#!/bin/sh
exec python3 -u -c 'import itertools, time; [print(time.ctime()) or time.sleep(1) for _ in itertools.count()]'
SH
printf '#!/bin/sh\nexec sleep 600\n' >"$W/bin/agent-idle"
printf '#!/bin/sh\nexec sleep 14\n' >"$W/bin/agent-brief"
chmod +x "$W/bin/agent-work" "$W/bin/agent-idle" "$W/bin/agent-brief"
cat >"$W/home/.rein/bin/rein" <<EOF
#!/bin/sh
[ "\$1" = start ] || exit 2
w=\$(basename "\$PWD")
[ "\$w" = starting ] && exec sleep 600 # never attaches
if ! tmux has-session -t "\$w" 2>/dev/null; then
  tmux new-session -d -s "\$w" -n "\$w" -x 200 -y 50
  [ "\$w" = needs-you ] && tmux split-window -h -d -t "\$w:\$w"
  case "\$w" in
    working) a=agent-work ;;
    quiet | needs-you) a=agent-idle ;;
    ended) a=agent-brief ;;
    *) a="" ;;
  esac
  if [ -n "\$a" ]; then
    tmux set-option -p -t "\$w:\$w.0" @rein_role engineer
    sleep 1 # the pane's shell ready, then the agent typed in as \`rein pane\` would
    tmux send-keys -t "\$w:\$w.0" "clear; $W/bin/\$a" Enter
  fi
fi
exec tmux attach -t "\$w"
EOF
chmod +x "$W/home/.rein/bin/rein"

project() { # project <dir>: a task in progress and two handoffs (the Dashboard's rows)
  mkdir -p "$1/.ai"
  cat >"$1/.ai/state.json" <<'JSON'
{"project": {"focus": "E-77 — status in the tab bar"},
 "tasks": [{"id": "E-76", "status": "DONE", "owner": "engineer", "tier": "T2", "description": "One status per open project, computed in the core.", "blocked_by": [], "updated_at": "2026-11-18T16:00:00Z"},
           {"id": "E-77", "status": "IN_PROGRESS", "owner": "engineer", "tier": "T2", "description": "Show each project's status in the tab bar, the status bar and the Dashboard.", "blocked_by": [], "updated_at": "2026-11-18T17:00:00Z"},
           {"id": "E-78", "status": "OPEN", "owner": "engineer", "tier": "T2", "description": "Raise a notification when a project needs the operator.", "blocked_by": [], "updated_at": "2026-11-18T17:00:00Z"}]}
JSON
  cat >"$1/.ai/COMM.md" <<'MD'
# COMM

## Handoff — 2026-11-18 15:53 UTC
**From**: Architect → Engineer
**Trigger**: Rulings on E-76's reads
### Next action needed
- Continue E-76, then E-77.
---
MD
}

T() { "$REAL_TMUX" -S "$W/tmux.sock" -f /dev/null "$@"; } # -f: never the operator's ~/.tmux.conf
X() { "$W/bin/xinput" "$@"; sleep 0.7; }
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

desktop() { # desktop <dir>: the window, or (a second start) a handoff to the running one
  exec env -u TMUX -u TMUX_PANE -u CLAUDECODE \
    HOME="$W/home" PATH="$W/bin:$PATH" SHELL=/bin/sh \
    XDG_RUNTIME_DIR="$W/run" XDG_CONFIG_HOME="$W/xdg/config" XDG_DATA_HOME="$W/xdg/data" \
    XDG_STATE_HOME="$W/xdg/state" XDG_CACHE_HOME="$W/xdg/cache" \
    WEBKIT_DISABLE_DMABUF_RENDERER=1 \
    "$REPO/target/debug/rein-desktop" "$1" >>"$W/app.log" 2>&1
}
second() { # second <dir>: a second start; once more if no session came of it (one can be lost
  # while the window is still busy with the tab before)
  local n
  for n in 1 2; do
    timeout 20 bash -c "$(declare -f desktop); W='$W' REPO='$REPO' desktop '$1'"
    for _ in $(seq 1 24); do T has-session -t "$(basename "$1")" 2>/dev/null && { sleep 2; return; }; sleep 0.25; done
    echo "  no session for $(basename "$1") after start $n" >&2
  done
}
quit() { kill "$APP" 2>/dev/null; wait "$APP" 2>/dev/null; APP=""; T kill-server 2>/dev/null; sleep 0.5; }

run() { # run <theme> <tabs-status>
  local theme=$1 shown=$2 st="$W/xdg/state/rein-app" d="$W/Dev/$1-$2"
  mkdir -p "$st"
  printf '{"schema":1,"input_style":"simple","first_run_done":true,"theme":"%s","tabs_status":"%s"}' "$theme" "$shown" >"$st/ui.json"
  rm -f "$st/projects.json" "$st/dismissed.json" # no restore, no dismissed rows from a run before
  for p in working quiet ended needs-you unknown starting; do project "$d/$p"; done
  desktop "$d/working" &
  APP=$!
  for _ in $(seq 1 300); do "$W/bin/xresize" rein "$GW" "$GH" >/dev/null 2>&1 && break; sleep 0.05; done
  sleep 8 # the first tab is up and the window takes a second start's folder
  for p in quiet ended needs-you unknown; do second "$d/$p"; done
  "$W/bin/xresize" rein "$GW" "$GH" >/dev/null
  sleep 16 # quiet has been quiet for 15 s; ended's agent was seen and has exited
  T send-keys -t "needs-you:needs-you.1" "printf '\\a'" Enter # a bell in a background tab
  timeout 20 bash -c "$(declare -f desktop); W='$W' REPO='$REPO' desktop '$d/starting'" # last: starting for 20 s; never a session
  sleep 6 # one status poll after the bell and the start
  echo "  tabs: $(T list-sessions -F '#S' | tr '\n' ' ')"
  echo "  panes: $(T list-panes -a -F '#{session_name}:#{@rein_role}=#{pane_current_command}' | tr '\n' ' ')"
  if [ "$shown" = shown ]; then
    shot "status-tabs-$theme"
    X move 160 20; sleep 1.5 # the pointer on the second tab (quiet): its tooltip
    shot "status-tooltip-$theme"
    X move 700 600; X move 900 700; sleep 1 # off the tab bar: the tooltip closes
  fi
  X key alt+2; sleep 1.5 # the quiet tab current: the status bar's segment
  X key ctrl+shift+1; sleep 2.5 # the Dashboard
  if [ "$shown" = shown ]; then shot "status-dashboard-$theme"; else shot "status-hidden-$theme"; fi
  quit
}

for theme in catppuccin-mocha github-light; do
  echo "[$theme ${GW}x${GH}]"
  run "$theme" shown
done
echo "[catppuccin-mocha, [tabs] status = hidden]"
run catppuccin-mocha hidden
echo "done: screens in $OUT"
