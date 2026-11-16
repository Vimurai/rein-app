#!/usr/bin/env bash
# team-screens.sh <out-dir> [W H] — the Project sheet's Team section end to end and the screens
# (desktop.md §Look and feel v2 V10, E-64, D-052): the real window (debug build) on Xvfb, in
# catppuccin-mocha and github-light:
#   team-sheet        — a project in team shape: rein's text, the add form, Go back to
#                       single-developer mode;
#   team-single       — the "no .ai/team.json" shape: rein's line, the form, Turn on team mode
#                       (catppuccin-mocha only);
#   team-form-error   — an add rein refused (an overlapping range), its words under the form;
#   team-confirm      — the sheet's confirm (default No);
#   team-native       — the core's own dialog before rein runs (catppuccin-mocha only).
# Checked from the stand-in's argv log: the add reaches rein as five words; the migrate does
# not run when the native dialog says No, and runs once when it says Yes.
# The `rein` is a stand-in in a throwaway HOME that records its argv one word per line and
# answers as rein 0.5.0 does; tmux runs on a private -S socket. A local tool only: it refuses
# to run when CI is set. Needs: Xvfb, tmux, cc, Xlib + XTest headers, ffmpeg, a debug build.
set -uo pipefail
if [ -n "${CI:-}" ]; then echo "team-screens.sh: not in CI (it drives a real window on Xvfb)" >&2; exit 2; fi
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
W="$(mktemp -d /tmp/rein-tms.XXXXXX)"
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
LOG="$W/argv.log"
EOF
cat >>"$W/home/.rein/bin/rein" <<'EOF'
# One call per line: its words, each in brackets — a split value would show as more words.
line=""; for a in "$@"; do line="$line[$a]"; done; echo "$line" >>"$LOG"
case "$1 $2" in
  "team show")
    if [ -e .ai/team-shape ]; then
      echo "planner: lead@example.com   claim_ttl_days: 7   shared_log: false"
      echo "  lead@example.com (planner)  E:1-100 (12/100 used)  P:1-20 (3/20 used)"
      echo "  dev@example.com  E:101-200 (0/100 used)"
    else
      echo "no .ai/team.json — ids are allocated max+1 (single-operator project)"
    fi
    exit 0 ;;
  "trunk ") echo master; exit 0 ;;
  "team add")
    echo "✗ [RANGE_OVERLAP] $3 E:150-160 overlaps dev@example.com E:101-200"
    exit 1 ;;
  "state migrate")
    echo "✓ migrated to team records: 66 task(s), 206 stamp(s), 0 archived task(s)."
    echo "  next: git add .ai && git commit -m \"chore(.ai): migrate state to team records (D-102)\""
    exit 0 ;;
  "start ")
    w=$(basename "$PWD")
    tmux has-session -t "$w" 2>/dev/null || tmux new-session -d -s "$w" -n "$w" -x 200 -y 50 "sh"
    exec tmux attach -t "$w" ;;
esac
exit 2
EOF
chmod +x "$W/home/.rein/bin/rein"
T() { "$REAL_TMUX" -S "$W/tmux.sock" "$@"; }
X() { "$W/bin/xinput" "$@"; sleep 0.7; }
shot() { ffmpeg -loglevel error -y -f x11grab -draw_mouse 0 -video_size "${GW}x${GH}" -i "$DISPLAY" -frames:v 1 "$OUT/$1-${GW}x${GH}.png" && echo "  $OUT/$1-${GW}x${GH}.png"; }
fails=0
check() { if [ "$2" = "$3" ]; then echo "  ok   $1"; else echo "  FAIL $1 — expected [$2], got [$3]"; fails=$((fails + 1)); fi; }
calls() { grep -v '^\[start\]$' "$W/argv.log" 2>/dev/null | tr '\n' ',' | sed 's/,$//'; }

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
  sleep 4
}
quit() { kill "$APP" 2>/dev/null; wait "$APP" 2>/dev/null; APP=""; T kill-server 2>/dev/null; sleep 0.5; }

for theme in catppuccin-mocha github-light; do
  echo "[$theme ${GW}x${GH}]"
  # A project in team shape.
  p="$W/Dev/team-$theme"; mkdir -p "$p/.ai"; touch "$p/.ai/team-shape"
  : >"$W/argv.log"
  app "$theme" "$p"
  : >"$W/argv.log" # only the sheet's calls count, not start-up's health and project check
  X key ctrl+shift+m
  sleep 2
  shot "team-sheet-$theme"
  check "open: team show, then trunk — nothing else" "[team][show],[trunk]" "$(calls)"
  # The add form: type a good row; rein refuses it (overlap) and its words show under the form.
  X click ${ADD_EMAIL_X:-500} ${ADD_EMAIL_Y:-430}
  X type "new.dev+x@example.com"
  X key Tab; X key Tab
  X type "150"
  X key Tab
  X type "160"
  X key Return
  sleep 2
  shot "team-form-error-$theme"
  check "add: five words, each value one word" "[team][show],[trunk],[team][add][new.dev+x@example.com][--range][E:150-160]" "$(calls)"
  # Go back to single-developer mode: the sheet's confirm (default No).
  X click ${MODE_X:-1000} ${MODE_Y:-560}
  sleep 1
  shot "team-confirm-$theme"
  if [ "$theme" = catppuccin-mocha ]; then
    X key Tab; X key Return # the confirm's own button → the core's native dialog
    sleep 2
    shot "team-native-$theme"
    X key Escape # No
    sleep 1.5
    check "native No: the migrate did not run" 0 "$(grep -c migrate "$W/argv.log")"
  fi
  quit
done
# The "no .ai/team.json" shape, and the native Yes running the migrate once.
p="$W/Dev/single"; mkdir -p "$p/.ai"
: >"$W/argv.log"
app catppuccin-mocha "$p"
: >"$W/argv.log"
X key ctrl+shift+m
sleep 2
shot "team-single-catppuccin-mocha"
X click ${SINGLE_MODE_X:-880} ${SINGLE_MODE_Y:-526} # shorter sheet: the button sits higher
sleep 1
X key Tab; X key Return
sleep 2
X key Return # the native dialog's default is No
sleep 1.5
check "native default (Return) is No" 0 "$(grep -c migrate "$W/argv.log")"
X click ${SINGLE_MODE_X:-880} ${SINGLE_MODE_Y:-526}
sleep 1
X key Tab; X key Return
sleep 2
X click ${NATIVE_YES_X:-467} ${NATIVE_YES_Y:-162} # GTK's Yes, the dialog at the screen's top left on Xvfb
sleep 3
check "native Yes: migrate --team ran once" "[state][migrate][--team]" "$(grep migrate "$W/argv.log" | tr '\n' ',' | sed 's/,$//')"
quit
echo "argv log:"; sed 's/^/  /' "$W/argv.log"
if [ "$fails" -eq 0 ]; then echo "PASS: team screens"; else echo "$fails failure(s)"; fi
[ "$fails" -eq 0 ]
