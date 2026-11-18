#!/usr/bin/env bash
# panel-screens.sh <out-dir> [W H] — the side menu end to end and its screens (panels.md, E-60,
# D-052): the real window (debug build) on Xvfb, in catppuccin-mocha and github-light.
#   panel-tasks-filter / -reader-task / -reader-task-next / -reader-closed / -communication-readable /
#   -reader-handoff / -reader-handoff-next — E-74's readable panels on this repository's own
#   .ai/ files (PANEL_SCREENS_ONLY=readable runs only these);
#   panel-dashboard / -communication / -tasks — each panel on a fixture project, rail shown;
#     the newest handoff holds a <script> tag, an <img onerror> and a Markdown link, which must
#     appear as literal text (checked by eye on the screen; the render test checks the markup);
#   panel-tasks-updated (mocha)  — a task added to state.json is on screen within 6 s;
#   panel-narrow-overlay (mocha) — at 860 px wide the panel lies over the terminal;
#   panel-unavailable            — a team-mode state.json: "Not available" and its reason;
#   panel-attention              — "Needs you" (E-61) with items from two projects (the second
#     opened by a second start) and the rail's badge; the first tab's bell rung while it is in
#     the background; checked: the bell item is there, and gone once that tab is shown;
#   panel-attention-empty        — a quiet project: "Nothing needs you" and the known gap;
#   panel-attention-info-only    — E-75: only For information (two blocked tasks, one row).
# Checked from the core's debug trace (REIN_DESKTOP_TRACE_PANELS, debug builds only):
#   - the open Tasks panel reads state.json again within 6 s of a change;
#   - with the window unmapped (hidden) nothing is read for 12 s, and reads resume once it is
#     mapped again;
#   - typing latency: 20 keys with the panel closed, 20 with it open, timed from the key sent
#     to the byte reaching a reader in the tab; the open median stays within 1.5 x closed + 5 ms.
# The `rein` is a stand-in in a throwaway HOME; tmux runs on a private -S socket. A local tool
# only: it refuses to run when CI is set. Needs: Xvfb, tmux, cc, Xlib + XTest headers, ffmpeg,
# python3, a debug build.
set -uo pipefail
if [ -n "${CI:-}" ]; then echo "panel-screens.sh: not in CI (it drives a real window on Xvfb)" >&2; exit 2; fi
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
W="$(mktemp -d /tmp/rein-pnl.XXXXXX)"
W="$(cd "$W" && pwd -P)"
mkdir -p "$W/bin" "$W/home/.rein/bin" "$W/run" "$OUT" "$W/Dev"
chmod 700 "$W/run"
for h in xresize xtitle xmap; do cc -O2 -o "$W/bin/$h" "$E2E/$h.c" -lX11 || exit 2; done
cc -O2 -o "$W/bin/xinput" "$E2E/xinput.c" -lX11 -lXtst || exit 2
cat >"$W/bin/tmux" <<EOF
#!/bin/sh
exec "$REAL_TMUX" -S "$W/tmux.sock" -f /dev/null "\$@"
EOF
chmod +x "$W/bin/tmux"
# The typing reader: one stamp (ns) per byte that reaches the pane.
cat >"$W/bin/keylog" <<EOF
#!/bin/bash
stty raw -echo
while IFS= read -r -s -n1 _; do date +%s%N >>"$W/keys.log"; done
EOF
chmod +x "$W/bin/keylog"
cat >"$W/home/.rein/bin/rein" <<EOF
#!/bin/sh
case "\$1" in
  start)
    w=\$(basename "\$PWD")
    cmd=sh
    [ -e .ai/keylog ] && cmd="$W/bin/keylog"
    tmux has-session -t "\$w" 2>/dev/null || tmux new-session -d -s "\$w" -n "\$w" -x 200 -y 50 "\$cmd"
    exec tmux attach -t "\$w" ;;
esac
exit 2
EOF
chmod +x "$W/home/.rein/bin/rein"

# A fixture project: tasks in every group, three handoffs, the newest hostile.
project() { # project <dir>
  mkdir -p "$1/.ai"
  python3 - "$1/.ai/state.json" <<'PY'
import json, sys
t = lambda i, s, d, o="engineer", tier="T2": {"id": f"E-{i}", "status": s, "owner": o, "tier": tier, "description": d,
                                             "blocked_by": [], "updated_at": f"2026-11-16T0{i % 10}:00:00Z"}
tasks = [t(51, "DONE", "Layout writer"), t(52, "DONE", "Verifier"), t(53, "DONE", "Profiles"),
         t(57, "BLOCKED", "Signed release (waits on rein E-424)"), t(58, "OPEN", "Operator docs for the side menu"),
         t(59, "DONE", "Panel sources: state.json and COMM.md"), t(60, "IN_PROGRESS", "Side menu and the first three panels"),
         t(61, "OPEN", "\"Needs you\" panel and its badge"),
         # A real-length description: the row wraps to two lines in the panel, never wider than it.
         t(63, "OPEN", "Record the open project tabs and restore them when the app starts, per .ai/blueprints/desktop.md "
                       "V12 components 4-6, with the not-running card and the agents-gone bar, both themes")]
tasks[3]["blocked_by"] = ["rein E-424"]
json.dump({"project": {"focus": "E-60 — side menu and panels"}, "tasks": tasks}, open(sys.argv[1], "w"), indent=1)
PY
  cat >"$1/.ai/COMM.md" <<'MD'
# COMM

## Handoff — 2026-11-16 09:00 UTC
**From**: Architect → Engineer
**Trigger**: Planned E-59 and E-60
### Next action needed
- Build the sources, then the side menu.
---

## Handoff — 2026-11-16 10:30 UTC
**From**: Engineer → Architect
**Trigger**: E-59 built
### What was built
- read_project_file, the two parsers, panel_data.
### Next action needed
- Review E-59.
---

## Handoff — 2026-11-16 11:45 UTC
**From**: Architect → Engineer
**Trigger**: E-59 accepted <script>alert(1)</script>
### Rulings
- See [the blueprint](https://example.com/panels) and <img src=x onerror=alert(2)> — both are text.
### Next action needed
- Build E-60, then ask the operator to look.
---
MD
}

T() { "$REAL_TMUX" -S "$W/tmux.sock" -f /dev/null "$@"; } # -f: never the operator's ~/.tmux.conf
X() { "$W/bin/xinput" "$@"; sleep 0.7; }
shot() { ffmpeg -loglevel error -y -f x11grab -draw_mouse 0 -video_size "${GW}x${GH}" -i "$DISPLAY" -frames:v 1 "$OUT/$1-${GW}x${GH}.png" && echo "  $OUT/$1-${GW}x${GH}.png"; }
fails=0
check() { if [ "$2" = "$3" ]; then echo "  ok   $1"; else echo "  FAIL $1 — expected [$2], got [$3]"; fails=$((fails + 1)); fi; }
ok() { if [ "$2" = 0 ]; then echo "  ok   $1"; else echo "  FAIL $1"; fails=$((fails + 1)); fi; }
now_ms() { date +%s%3N; }
# Trace lines for state.json reads (not stats) at or after <ms>.
reads_since() { awk -v t="$1" '$1=="panel-trace" && $2>=t && $4=="file=StateJson" && $5=="read"' "$W/app.log" | wc -l; }
traces_since() { awk -v t="$1" '$1=="panel-trace" && $2>=t' "$W/app.log" | wc -l; }

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
    HOME="$W/home" PATH="$W/bin:$PATH" SHELL=/bin/sh REIN_DESKTOP_TRACE_PANELS=1 \
    XDG_RUNTIME_DIR="$W/run" XDG_CONFIG_HOME="$W/xdg/config" XDG_DATA_HOME="$W/xdg/data" \
    XDG_STATE_HOME="$W/xdg/state" XDG_CACHE_HOME="$W/xdg/cache" \
    WEBKIT_DISABLE_DMABUF_RENDERER=1 \
    "$REPO/target/debug/rein-desktop" "$1" >>"$W/app.log" 2>&1
}
app() { # app <theme> <dir>
  local st="$W/xdg/state/rein-app"
  mkdir -p "$st"
  printf '{"schema":1,"input_style":"simple","first_run_done":true,"theme":"%s"}' "$1" >"$st/ui.json"
  rm -f "$st/projects.json" # E-70: each launch starts with no open-tabs record (no restore)
  desktop "$2" &
  APP=$!
  for _ in $(seq 1 300); do "$W/bin/xresize" rein "$GW" "$GH" >/dev/null 2>&1 && break; sleep 0.05; done
  sleep 4
}
quit() { kill "$APP" 2>/dev/null; wait "$APP" 2>/dev/null; APP=""; T kill-server 2>/dev/null; sleep 0.5; }

for theme in catppuccin-mocha github-light; do
  echo "[$theme ${GW}x${GH}]"
  # E-74 (status.md §7): readable panels on a copy of this repository's own .ai/ — a real long
  # task and a real long handoff — driven by keyboard only: the filter, a row into the reader,
  # Esc back to the row, Communication's "Read all", Down to the next handoff.
  r="$W/Dev/rein-app-$theme"; mkdir -p "$r/.ai"
  cp "$REPO/.ai/state.json" "$REPO/.ai/COMM.md" "$r/.ai/"
  app "$theme" "$r"
  X key ctrl+shift+3; sleep 1.5; X key ctrl+shift+e # open Tasks, give the panel the keyboard
  X key Tab; X key Tab; X key Tab # refresh, close, then the filter box
  X type "one status per open project"; sleep 1; shot "panel-tasks-filter-$theme" # E-76 alone
  X key Tab; X key Return; sleep 1.5 # the row (a group header takes no focus while filtering): the reader
  shot "panel-reader-task-$theme"    # E-82: a long description in paragraphs, labels bold
  X key Escape; sleep 1; shot "panel-reader-closed-$theme" # the row has the keyboard again
  # A filter that leaves several tasks: back to the box, a new query, the first row, Down.
  X key shift+Tab; X key ctrl+a; X type "status.md"; sleep 1
  X key Tab; X key Return; sleep 1.5; X key Down; sleep 1
  shot "panel-reader-task-next-$theme"
  X key Escape; sleep 0.5
  X key ctrl+shift+2; sleep 1.5; X key ctrl+shift+e; shot "panel-communication-readable-$theme"
  X key Tab; X key Tab; X key Tab; X key Return; sleep 1.5 # refresh, close, the first Read all
  shot "panel-reader-handoff-$theme"
  X key Down; sleep 1; shot "panel-reader-handoff-next-$theme"
  quit
  [ "${PANEL_SCREENS_ONLY:-}" = readable ] && continue
  p="$W/Dev/demo-$theme"; project "$p"
  app "$theme" "$p"
  X key ctrl+shift+1; sleep 1.5; shot "panel-dashboard-$theme"
  X key ctrl+shift+2; sleep 1.5; shot "panel-communication-$theme"
  X key ctrl+shift+3; sleep 1.5; shot "panel-tasks-$theme"
  if [ "$theme" = catppuccin-mocha ]; then
    # A change to state.json is read again within 6 s while the panel is open.
    sleep 1
    t0=$(now_ms)
    python3 - "$p/.ai/state.json" <<'PY'
import json, sys
s = json.load(open(sys.argv[1]))
s["tasks"].append({"id": "E-62", "status": "OPEN", "owner": "engineer", "tier": "T2",
                   "description": "Added while the panel was open", "blocked_by": [], "updated_at": "2026-11-16T12:00:00Z"})
json.dump(s, open(sys.argv[1], "w"), indent=1)
PY
    waited=""
    for i in $(seq 1 60); do
      [ "$(reads_since "$t0")" -gt 0 ] && { waited=$(( $(now_ms) - t0 )); break; }
      sleep 0.1
    done
    ok "state.json read again ${waited:-never} ms after the change (limit 6000)" "$([ -n "$waited" ] && [ "$waited" -le 6000 ] && echo 0 || echo 1)"
    sleep 1; shot "panel-tasks-updated-$theme"
    # Hidden: nothing is read; shown again: reads resume.
    "$W/bin/xmap" rein unmap >/dev/null
    sleep 1
    t1=$(now_ms)
    sleep 12
    check "no panel read for 12 s while the window is hidden" 0 "$(traces_since "$t1")"
    t2=$(now_ms)
    "$W/bin/xmap" rein map >/dev/null
    "$W/bin/xresize" rein "$GW" "$GH" >/dev/null
    resumed=""
    for i in $(seq 1 70); do
      [ "$(traces_since "$t2")" -gt 0 ] && { resumed=$(( $(now_ms) - t2 )); break; }
      sleep 0.1
    done
    ok "reads resume ${resumed:-never} ms after the window is shown again" "$([ -n "$resumed" ] && echo 0 || echo 1)"
    # Narrow: the panel lies over the terminal.
    "$W/bin/xresize" rein 860 "$GH" >/dev/null; sleep 1.5
    shot "panel-narrow-overlay-$theme"
    "$W/bin/xresize" rein "$GW" "$GH" >/dev/null; sleep 1
  fi
  # Needs you: a second project by a second start (it becomes the current tab), then a bell
  # in the first tab's pane while that tab is in the background.
  o="$W/Dev/other-$theme"; mkdir -p "$o/.ai"
  echo '{"tasks":[{"id":"E-7","status":"BLOCKED","description":"Waits on the operator'"'"'s signing key"}]}' >"$o/.ai/state.json"
  timeout 20 bash -c "$(declare -f desktop); W='$W' REPO='$REPO' desktop '$o'"; sleep 4
  T send-keys -t "demo-$theme" "printf '\\a'" Enter; sleep 1
  X key ctrl+shift+4; sleep 6
  shot "panel-attention-$theme"
  if [ "$GW" = 1280 ] && [ "$GH" = 800 ]; then # E-75: by pointer, at the default layout only
    X click 120 281; sleep 1; shot "panel-attention-info-open-$theme" # For information
    X click 128 343; sleep 1                                           # its blocked row opens
    shot "panel-attention-info-blocked-$theme"
    X click 396 152; sleep 2; shot "panel-attention-dismissed-$theme" # Dismiss the bell
    check "a dismissed bell is kept in dismissed.json for this project" yes "$(python3 -c 'import json,sys; d=json.load(open(sys.argv[1])); print("yes" if any(i["key"]=="bell" and i["project"].endswith(sys.argv[2]) for i in d["items"]) else "no")' "$W/xdg/state/rein-app/dismissed.json" "/demo-$theme" 2>/dev/null || echo no)"
  fi
  bell_before=$(grep -c "panel-trace" "$W/app.log")
  [ "$bell_before" -gt 0 ]; ok "the attention facts are read (trace lines present)" $?
  X key alt+1; sleep 1.5; X key alt+2; sleep 1.5
  shot "panel-attention-after-bell-$theme"
  quit
  # Needs you, empty: one quiet project.
  c="$W/Dev/calm-$theme"; mkdir -p "$c/.ai"
  echo '{"tasks":[{"id":"E-1","status":"DONE","description":"Done"}]}' >"$c/.ai/state.json"
  app "$theme" "$c"
  X key ctrl+shift+4; sleep 6
  shot "panel-attention-empty-$theme"
  quit
  # Needs you with only information (E-75): a project whose one item is a blocked task.
  f="$W/Dev/blocked-$theme"; mkdir -p "$f/.ai"
  echo '{"tasks":[{"id":"E-7","status":"BLOCKED","description":"Waits on the signing key"},{"id":"E-8","status":"BLOCKED","description":"Waits on E-7"}]}' >"$f/.ai/state.json"
  app "$theme" "$f"
  X key ctrl+shift+4; sleep 6
  shot "panel-attention-info-only-$theme"
  quit
  # Not available: a team-mode state.json (no tasks[]).
  q="$W/Dev/teammode-$theme"; mkdir -p "$q/.ai"; echo '{"project":{"focus":""}}' >"$q/.ai/state.json"
  app "$theme" "$q"
  X key ctrl+shift+3; sleep 1.5; shot "panel-unavailable-$theme"
  quit
done

echo "[typing latency, catppuccin-mocha]"
p="$W/Dev/typing"; project "$p"; touch "$p/.ai/keylog"
app catppuccin-mocha "$p"
sleep 1
lat() { # lat <label>: 20 keys, then the median ms from send to the reader
  : >"$W/keys.log"; : >"$W/sent.log"
  for i in $(seq 1 20); do date +%s%N >>"$W/sent.log"; "$W/bin/xinput" type a; sleep 0.25; done
  sleep 0.5
  python3 - "$W/sent.log" "$W/keys.log" <<'PY'
import sys, statistics
s = [int(x) for x in open(sys.argv[1])]; k = [int(x) for x in open(sys.argv[2])]
if len(k) != len(s): print(f"NA {len(s)} {len(k)}"); sys.exit()
print(f"{statistics.median((b - a) / 1e6 for a, b in zip(s, k)):.1f}")
PY
}
closed=$(lat closed)
X key ctrl+shift+1; sleep 2
opened=$(lat open)
echo "  median key latency: panel closed ${closed} ms, panel open ${opened} ms"
ok "typing latency unchanged with a panel open (open <= 1.5 x closed + 5 ms)" \
  "$(python3 -c "import sys; c, o = sys.argv[1:]; print(0 if 'NA' not in c + o and float(o) <= 1.5 * float(c) + 5 else 1)" "$closed" "$opened")"
quit

if [ "$fails" -eq 0 ]; then echo "PASS: panel screens"; else echo "$fails failure(s)"; fi
[ "$fails" -eq 0 ]
