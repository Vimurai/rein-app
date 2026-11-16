#!/usr/bin/env bash
# panel-screens.sh <out-dir> [W H] — the side menu end to end and its screens (panels.md, E-60,
# D-052): the real window (debug build) on Xvfb, in catppuccin-mocha and github-light.
#   panel-dashboard / -communication / -tasks — each panel on a fixture project, rail shown;
#     the newest handoff holds a <script> tag, an <img onerror> and a Markdown link, which must
#     appear as literal text (checked by eye on the screen; the render test checks the markup);
#   panel-tasks-updated (mocha)  — a task added to state.json is on screen within 6 s;
#   panel-narrow-overlay (mocha) — at 860 px wide the panel lies over the terminal;
#   panel-unavailable            — a team-mode state.json: "Not available" and its reason.
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
exec "$REAL_TMUX" -S "$W/tmux.sock" "\$@"
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
         t(61, "OPEN", "\"Needs you\" panel and its badge")]
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

T() { "$REAL_TMUX" -S "$W/tmux.sock" "$@"; }
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

app() { # app <theme> <dir>
  local st="$W/xdg/state/rein-app"
  mkdir -p "$st"
  printf '{"schema":1,"input_style":"simple","first_run_done":true,"theme":"%s"}' "$1" >"$st/ui.json"
  env -u TMUX -u TMUX_PANE -u CLAUDECODE \
    HOME="$W/home" PATH="$W/bin:$PATH" SHELL=/bin/sh REIN_DESKTOP_TRACE_PANELS=1 \
    XDG_RUNTIME_DIR="$W/run" XDG_CONFIG_HOME="$W/xdg/config" XDG_DATA_HOME="$W/xdg/data" \
    XDG_STATE_HOME="$W/xdg/state" XDG_CACHE_HOME="$W/xdg/cache" \
    WEBKIT_DISABLE_DMABUF_RENDERER=1 \
    "$REPO/target/debug/rein-desktop" "$2" >>"$W/app.log" 2>&1 &
  APP=$!
  for _ in $(seq 1 300); do "$W/bin/xresize" rein "$GW" "$GH" >/dev/null 2>&1 && break; sleep 0.05; done
  sleep 4
}
quit() { kill "$APP" 2>/dev/null; wait "$APP" 2>/dev/null; APP=""; T kill-server 2>/dev/null; sleep 0.5; }

for theme in catppuccin-mocha github-light; do
  echo "[$theme ${GW}x${GH}]"
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
