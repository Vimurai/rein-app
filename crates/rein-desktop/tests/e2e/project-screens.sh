#!/usr/bin/env bash
# project-screens.sh <out-dir> [W H] — V9 components 1–3 end to end and the screens (E-63, D-052):
# the real window (debug build) on Xvfb, catppuccin-mocha and github-light:
#   setup-card     — a folder without .ai/: the card with rein's own line, nothing started;
#   setup-started  — Set up this project: `rein init` then `rein start` in the tab's terminal
#                    (checked: init ran before start, the session exists);
#   setup-failed   — `rein init` fails: its output and the fixed line stay; start never ran
#                    (checked: the screen is not blank — it matches the same screen redrawn by
#                    a resize, the E-63 review's empty terminal);
#   setup-inside   — a folder inside a Rein project: the project and Open that project, no
#                    set-up and rein never asked (catppuccin-mocha only);
#   update-bar     — a project rein reports out of date: the bar after the tab started;
#   update-confirm — Update project clicked: rein's rows, the fixed line, Cancel focused —
#                    nothing has run (E-63 rework 2);
#   update-done    — Run rein init: `rein init` in the sheet, the check once more, the line.
# The `rein` is a stand-in in a throwaway HOME that keeps rein's contracts: `init --check`
# exits 0 current / 1 pending (rows, `Run: rein init`; one fixed line with no .ai/), `init`
# makes .ai/ and clears the pending state, `start` builds a session through plain `tmux` and
# attaches. Every call is logged. tmux runs on a private -S socket (the wrapper first on PATH),
# so the operator's own servers and any real `rein-app` socket are never touched.
# A local tool only: it refuses to run when CI is set. Needs: Xvfb, tmux, cc, Xlib + XTest
# headers, ffmpeg, a debug build (cargo build -p rein-desktop, after the UI build).
set -uo pipefail
if [ -n "${CI:-}" ]; then echo "project-screens.sh: not in CI (it drives a real window on Xvfb)" >&2; exit 2; fi
OUT=$1
GW=${2:-1280} GH=${3:-800}
REPO="$(git -C "$(dirname "$0")" rev-parse --show-toplevel)"
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
W="$(mktemp -d /tmp/rein-pjs.XXXXXX)"
W="$(cd "$W" && pwd -P)"
mkdir -p "$W/bin" "$W/home/.rein/bin" "$W/run" "$OUT" "$W/Dev"
chmod 700 "$W/run"
cc -O2 -o "$W/bin/xresize" "$REPO/crates/rein-desktop/tests/e2e/xresize.c" -lX11 || exit 2
cc -O2 -o "$W/bin/xinput" "$REPO/crates/rein-desktop/tests/e2e/xinput.c" -lX11 -lXtst || exit 2
cat >"$W/bin/tmux" <<EOF
#!/bin/sh
exec "$REAL_TMUX" -S "$W/tmux.sock" "\$@"
EOF
chmod +x "$W/bin/tmux"
cat >"$W/home/.rein/bin/rein" <<EOF
#!/bin/sh
LOG="$W/calls.log"
EOF
cat >>"$W/home/.rein/bin/rein" <<'EOF'
echo "$(basename "$PWD"): $*" >>"$LOG"
case "$*" in
  "init --check")
    if [ ! -d .ai ]; then
      echo "rein init --check: no .ai/ here — rein init would provision a new project."
      exit 1
    fi
    if [ -e .ai/stale ]; then
      echo "rein init --check: project 0.4.0 → installed 0.5.0 — 3 change(s) pending:"
      echo "  merge    .ai/state.json: project.rein_version 0.4.0 → 0.5.0"
      echo "  refresh  ENGINEER.md"
      echo "  refresh  .claude/skills/rein-task/SKILL.md"
      echo "Run: rein init"
      exit 1
    fi
    echo "rein init --check: project 0.5.0 is current with installed 0.5.0 — nothing to do."
    exit 0 ;;
  init)
    if [ -e fail-init ]; then
      echo "Initializing .ai/ ..."
      echo "rein init: cannot write .ai/state.json: Permission denied" >&2
      exit 3
    fi
    [ -d .ai ] && echo "Upgrading: project 0.4.0 → installed 0.5.0" || echo "Initialized .ai/ + .github/ (safe — no overwrites) with Rein v0.5.0"
    mkdir -p .ai && rm -f .ai/stale
    sleep 1
    echo "Rein v0.5.0 — user files untouched."
    exit 0 ;;
  start)
    w=$(basename "$PWD")
    # A plain, non-login sh: no host banner (MOTD) in the screens.
    tmux has-session -t "$w" 2>/dev/null || tmux new-session -d -s "$w" -n "$w" -x 200 -y 50 "sh"
    exec tmux attach -t "$w" ;;
esac
exit 2
EOF
chmod +x "$W/home/.rein/bin/rein"
T() { "$REAL_TMUX" -S "$W/tmux.sock" "$@"; }
X() { "$W/bin/xinput" "$@"; sleep 0.8; }
shot() { ffmpeg -loglevel error -y -f x11grab -draw_mouse 0 -video_size "${GW}x${GH}" -i "$DISPLAY" -frames:v 1 "$OUT/$1-${GW}x${GH}.png" && echo "  $OUT/$1-${GW}x${GH}.png"; }
fails=0
check() { if [ "$2" = "$3" ]; then echo "  ok   $1"; else echo "  FAIL $1 — expected [$2], got [$3]"; fails=$((fails + 1)); fi; }

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
    HOME="$W/home" PATH="$W/bin:$PATH" SHELL=/bin/sh \
    XDG_RUNTIME_DIR="$W/run" XDG_CONFIG_HOME="$W/xdg/config" XDG_DATA_HOME="$W/xdg/data" \
    XDG_STATE_HOME="$W/xdg/state" XDG_CACHE_HOME="$W/xdg/cache" \
    WEBKIT_DISABLE_DMABUF_RENDERER=1 \
    "$REPO/target/debug/rein-desktop" "$2" >>"$W/app.log" 2>&1 &
  APP=$!
  for _ in $(seq 1 300); do "$W/bin/xresize" rein "$GW" "$GH" >/dev/null 2>&1 && break; sleep 0.05; done
}
quit() { kill "$APP" 2>/dev/null; wait "$APP" 2>/dev/null; APP=""; T kill-server 2>/dev/null; sleep 0.5; }
calls() { grep "^$1: " "$W/calls.log" 2>/dev/null | sed "s/^$1: //" | tr '\n' ',' | sed 's/,$//'; }

for theme in catppuccin-mocha github-light; do
  echo "[$theme ${GW}x${GH}]"
  : >"$W/calls.log"
  # 1–2: a folder without .ai/ — the card, then Set up.
  p="$W/Dev/new-$theme"; mkdir -p "$p"
  app "$theme" "$p"
  sleep 4
  shot "setup-card-$theme"
  check "card: rein asked what set-up does, nothing else ran" "init --check" "$(calls "new-$theme")"
  check "card: no session yet" "" "$(T list-sessions -F '#S' 2>/dev/null | grep -x "new-$theme")"
  X key Return # Set up this project has the focus
  sleep 5
  shot "setup-started-$theme"
  check "set up: init, then start, in that order" "init --check,init,start" "$(calls "new-$theme" | sed 's/,init --check$//')"
  check "set up: the session runs" "new-$theme" "$(T list-sessions -F '#S' | grep -x "new-$theme")"
  check "set up: .ai/ made by rein's init" yes "$([ -d "$p/.ai" ] && echo yes || echo no)"
  quit
  # 3: init fails — its output and the fixed line stay; start never runs.
  p="$W/Dev/fail-$theme"; mkdir -p "$p"; touch "$p/fail-init"
  app "$theme" "$p"
  sleep 4
  X key Return
  sleep 3
  shot "setup-failed-$theme"
  check "failed init: start never ran" "init --check,init" "$(calls "fail-$theme")"
  # Not blank: the same screen redrawn by a resize must look the same (sizes within 10 %).
  "$W/bin/xresize" rein $((GW - 80)) $((GH - 40)); sleep 1.2
  "$W/bin/xresize" rein "$GW" "$GH"; sleep 1.2
  shot "dbg-redrawn-$theme" >/dev/null
  a=$(stat -c %s "$OUT/setup-failed-$theme-${GW}x${GH}.png") b=$(stat -c %s "$OUT/dbg-redrawn-$theme-${GW}x${GH}.png")
  rm -f "$OUT/dbg-redrawn-$theme-${GW}x${GH}.png"
  check "failed init: drawn without a redraw (not blank)" yes "$([ $((a * 10)) -ge $((b * 9)) ] && echo yes || echo "no ($a vs $b bytes)")"
  quit
  # A folder inside a Rein project: no set-up, rein not asked.
  if [ "$theme" = catppuccin-mocha ]; then
    p="$W/Dev/whole-project"; mkdir -p "$p/.ai" "$p/inner"
    app "$theme" "$p/inner"
    sleep 4
    shot "setup-inside-$theme"
    check "inside: rein was not asked about the inner folder" "" "$(calls inner)"
    check "inside: nothing started" "" "$(calls whole-project)"
    quit
  fi
  # 4–5: a project rein reports out of date — the bar, then Update project.
  p="$W/Dev/stale-$theme"; mkdir -p "$p/.ai"; touch "$p/.ai/stale"
  app "$theme" "$p"
  sleep 9 # started, then the one check
  shot "update-bar-$theme"
  check "stale: start, then the one check" "start,init --check" "$(calls "stale-$theme")"
  X click $((GW - 68)) 58 # Update project, at the bar's right end
  sleep 1.5
  shot "update-confirm-$theme"
  check "confirm: nothing ran before the confirm" "start,init --check" "$(calls "stale-$theme")"
  X key Tab # Cancel has the focus; Run rein init is next
  X key Return
  sleep 5
  shot "update-done-$theme"
  check "update: init, then the check once more" "start,init --check,init,init --check" "$(calls "stale-$theme")"
  quit
done
echo "calls:"; sed 's/^/  /' "$W/calls.log"
if [ "$fails" -eq 0 ]; then echo "PASS: project screens"; else echo "$fails failure(s)"; fi
[ "$fails" -eq 0 ]
