#!/usr/bin/env bash
# start-failed-screens.sh <out-dir> [W H] — a start that fails (desktop.md V14, D-068; E-94) end
# to end and its screens: the real window (debug build) on Xvfb, catppuccin-mocha and
# github-light:
#   failed-needs-init — a folder with a foreign .ai/ (no roles.json): `rein start` exits 2,
#                       `rein init --check` exits 1 with 58 lines: the bar with Set up;
#   failed-sheet      — Set up this project: the confirm with rein's rows, Cancel focused;
#   failed-sheet-row-52 — the sheet scrolled past row 40: `refresh CLAUDE.md` (line 52);
#   failed-set-up     — Run rein init: `rein init`, then the start, in the tab's own terminal;
#                       the bar is gone;
#   failed-checking   — while rein is asked (a slow `init --check`): "Asking rein…";
#   failed-no-fix     — `init --check` exits 0: the bar with Try again and Close tab only.
# Checked from rein's call log and the app's tmux server: the order of rein's calls (start,
# then one check; after Run: init, then start), that the session runs after set-up, and that a
# click where Set up would be does nothing in the no_fix bar. The bar itself is checked on the
# screens. The `rein` is a stand-in in a throwaway HOME; tmux runs on a private -S socket. A
# local tool only: it refuses to run when CI is set. Needs: Xvfb, tmux, cc, Xlib + XTest
# headers, ffmpeg, a debug build.
set -uo pipefail
if [ -n "${CI:-}" ]; then echo "start-failed-screens.sh: not in CI (it drives a real window on Xvfb)" >&2; exit 2; fi
OUT=$1
GW=${2:-1280} GH=${3:-800}
REPO="$(git -C "$(dirname "$0")" rev-parse --show-toplevel)"
. "$(dirname "$0")/window-binary.sh" && window_binary # cargo test's binary or a stale UI never reaches the window (E-84)
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
W="$(mktemp -d /tmp/rein-sfs.XXXXXX)"
W="$(cd "$W" && pwd -P)"
mkdir -p "$W/bin" "$W/home/.rein/bin" "$W/run" "$OUT" "$W/Dev"
chmod 700 "$W/run"
cc -O2 -o "$W/bin/xresize" "$REPO/crates/rein-desktop/tests/e2e/xresize.c" -lX11 || exit 2
cc -O2 -o "$W/bin/xinput" "$REPO/crates/rein-desktop/tests/e2e/xinput.c" -lX11 -lXtst || exit 2
cat >"$W/bin/tmux" <<EOF
#!/bin/sh
exec "$REAL_TMUX" -S "$W/tmux.sock" -f /dev/null "\$@"
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
    [ -e .ai/check-slow ] && sleep 5
    if [ -e .ai/roles.json ] || [ -e .ai/current ]; then
      echo "rein init --check: project 0.5.0 is current with installed 0.5.0 — nothing to do."
      exit 0
    fi
    # The operator's folder: 58 lines, `refresh CLAUDE.md` the 52nd.
    echo "rein init --check: no Rein project here — rein init would provision it:"
    i=2
    while [ $i -le 57 ]; do
      if [ $i = 52 ]; then echo "  refresh  CLAUDE.md"; else echo "  add      .claude/skills/s$i/SKILL.md"; fi
      i=$((i + 1))
    done
    echo "Run: rein init"
    exit 1 ;;
  init)
    echo "Initialized .ai/ (roles, state) with Rein v0.5.0"
    mkdir -p .ai && echo '{"engineer":{},"architect":{}}' >.ai/roles.json
    sleep 1
    exit 0 ;;
  start)
    if [ ! -e .ai/roles.json ]; then
      echo "no roles in .ai/roles.json — run 'rein init' or add engineer/architect." >&2
      exit 2
    fi
    w=$(basename "$PWD")
    tmux has-session -t "$w" 2>/dev/null || tmux new-session -d -s "$w" -n "$w" -x 200 -y 50 "sh"
    exec tmux attach -t "$w" ;;
esac
exit 2
EOF
chmod +x "$W/home/.rein/bin/rein"
T() { "$REAL_TMUX" -S "$W/tmux.sock" -f /dev/null "$@"; } # -f: never the operator's ~/.tmux.conf
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
  # "Don't ask" set for the folder: it must not hide a failed start's bar (V14 component 3).
  printf '{"schema":1,"input_style":"simple","first_run_done":true,"theme":"%s","update_dont_ask":["%s"]}' "$1" "$2" >"$st/ui.json"
  rm -f "$st/projects.json" # no restore
  env -u TMUX -u TMUX_PANE -u CLAUDECODE \
    HOME="$W/home" PATH="$W/bin:$PATH" SHELL=/bin/sh \
    XDG_RUNTIME_DIR="$W/run" XDG_CONFIG_HOME="$W/xdg/config" XDG_DATA_HOME="$W/xdg/data" \
    XDG_STATE_HOME="$W/xdg/state" XDG_CACHE_HOME="$W/xdg/cache" \
    WEBKIT_DISABLE_DMABUF_RENDERER=1 \
    "$REPO/target/debug/rein-desktop" "$2" >>"$W/app.log" 2>&1 &
  APP=$!
  for _ in $(seq 1 300); do "$W/bin/xresize" rein "$GW" "$GH" >/dev/null 2>&1 && break; sleep 0.05; done
}
fit() { "$W/bin/xresize" rein "$GW" "$GH" >/dev/null 2>&1; sleep 1; } # once it is up: the full screen
quit() { kill "$APP" 2>/dev/null; wait "$APP" 2>/dev/null; APP=""; T kill-server 2>/dev/null; sleep 0.5; }
calls() { grep "^$1: " "$W/calls.log" 2>/dev/null | sed "s/^$1: //" | tr '\n' ',' | sed 's/,$//'; }
until_calls() { # until_calls <project> <calls>: wait (at most 40 s) for rein's calls to reach <calls>
  for _ in $(seq 1 160); do [ "$(calls "$1")" = "$2" ] && return 0; sleep 0.25; done
  return 1
}
foreign() { mkdir -p "$1/.ai/memory" "$1/.ai/rules" "$1/.ai/skills"; } # another tool's .ai/, no roles.json

# The bar's buttons, right-aligned in the row under the title bar: Set up this project, Try
# again, Close tab (needs_init); Try again, Close tab (no_fix).
BAR_Y=${BAR_Y:-63}
SETUP_X=${SETUP_X:-$((GW - 235))}

for theme in catppuccin-mocha github-light; do
  echo "[$theme ${GW}x${GH}]"
  : >"$W/calls.log"
  # needs_init: the bar with Set up, the sheet, then Run rein init.
  p="$W/Dev/foreign-$theme"; foreign "$p"
  app "$theme" "$p"
  until_calls "foreign-$theme" "start,init --check"; fit; sleep 5 # the start failed, rein answered; one status poll
  shot "failed-needs-init-$theme"
  check "failed: the start, then one check" "start,init --check" "$(calls "foreign-$theme")"
  check "failed: no session" "" "$(T list-sessions -F '#S' 2>/dev/null | grep -x "foreign-$theme")"
  X click "$SETUP_X" "$BAR_Y" # Set up this project
  sleep 1.5
  shot "failed-sheet-$theme"
  check "sheet: nothing ran before the confirm" "start,init --check" "$(calls "foreign-$theme")"
  X click $((GW / 2)) $((GH / 2)) # into the rows: the scroll area takes the keys
  X key End; sleep 0.8 # the last rows: 52 (refresh CLAUDE.md) among them
  shot "failed-sheet-row-52-$theme"
  X key Escape; sleep 1 # Cancel: nothing ran
  check "sheet: Cancel ran nothing" "start,init --check" "$(calls "foreign-$theme")"
  X click "$SETUP_X" "$BAR_Y"; sleep 1.5
  X key Tab # Cancel has the focus; Run rein init is next
  X key Return
  until_calls "foreign-$theme" "start,init --check,init,start"; sleep 2
  shot "failed-set-up-$theme"
  check "set up: init, then the start, in the tab's terminal" "start,init --check,init,start" "$(calls "foreign-$theme")"
  check "set up: the session runs" "foreign-$theme" "$(T list-sessions -F '#S' | grep -x "foreign-$theme")"
  quit
  # checking: rein takes its time.
  p="$W/Dev/slow-$theme"; foreign "$p"; touch "$p/.ai/check-slow"
  app "$theme" "$p"
  until_calls "slow-$theme" "start,init --check"; fit # rein is still answering (5 s)
  shot "failed-checking-$theme"
  check "checking: the start, then the check (running)" "start,init --check" "$(calls "slow-$theme")"
  sleep 4
  quit
  # no_fix: rein's check says there is nothing to do.
  p="$W/Dev/current-$theme"; foreign "$p"; touch "$p/.ai/current"
  app "$theme" "$p"
  until_calls "current-$theme" "start,init --check"; fit; sleep 1
  shot "failed-no-fix-$theme"
  X click "$SETUP_X" "$BAR_Y"; sleep 2 # where Set up would be: nothing there
  check "no_fix: no Set up — nothing ran after the check" "start,init --check" "$(calls "current-$theme")"
  quit
done
echo "calls:"; sed 's/^/  /' "$W/calls.log"
if [ "$fails" -eq 0 ]; then echo "PASS: start-failed screens"; else echo "$fails failure(s)"; fi
[ "$fails" -eq 0 ]
