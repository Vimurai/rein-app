#!/usr/bin/env bash
# team-screens.sh <out-dir> [W H] — the Project sheet's Team section end to end and the screens
# (desktop.md §Look and feel v2 V10 and its "As built and reviewed" part, E-64, E-67, D-052,
# D-060): the real window (debug build) on Xvfb, in catppuccin-mocha and github-light:
#   team-sheet / team-single      — the team shape and the "no .ai/team.json" shape;
#   team-form-error               — an add rein refused (an overlapping range), its words shown;
#   team-confirm-after-error      — the confirm reached with the sheet made taller by that error;
#   team-confirm-legacy / -team   — the sheet's confirm with each mode's sentences;
#   team-native-legacy / -team    — the core's own dialog (catppuccin-mocha only).
# Controls that move with content are reached by keyboard focus, never a fixed pixel. Every
# "did not run" check is paired with proof that the native dialog was a mapped window with its
# title (xtitle) before the answer and gone after it: Return (its default, No), Escape, and Yes
# (clicked, its place read from the dialog's geometry) running `state migrate --legacy` / `--team` exactly once.
# The `rein` is a stand-in in a throwaway HOME that records its argv one word per line and
# answers as rein 0.5.0 does; tmux runs on a private -S socket. A local tool only: it refuses
# to run when CI is set. Needs: Xvfb, tmux, cc, Xlib + XTest headers, ffmpeg, a debug build.
set -uo pipefail
if [ -n "${CI:-}" ]; then echo "team-screens.sh: not in CI (it drives a real window on Xvfb)" >&2; exit 2; fi
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
W="$(mktemp -d /tmp/rein-tms.XXXXXX)"
W="$(cd "$W" && pwd -P)"
mkdir -p "$W/bin" "$W/home/.rein/bin" "$W/run" "$OUT" "$W/Dev"
chmod 700 "$W/run"
cc -O2 -o "$W/bin/xresize" "$REPO/crates/rein-desktop/tests/e2e/xresize.c" -lX11 || exit 2
cc -O2 -o "$W/bin/xinput" "$REPO/crates/rein-desktop/tests/e2e/xinput.c" -lX11 -lXtst || exit 2
cc -O2 -o "$W/bin/xtitle" "$REPO/crates/rein-desktop/tests/e2e/xtitle.c" -lX11 || exit 2
cat >"$W/bin/tmux" <<EOF
#!/bin/sh
exec "$REAL_TMUX" -S "$W/tmux.sock" -f /dev/null "\$@"
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
T() { "$REAL_TMUX" -S "$W/tmux.sock" -f /dev/null "$@"; } # -f: never the operator's ~/.tmux.conf
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
  rm -f "$st/projects.json" # E-70: each launch starts with no open-tabs record (no restore)
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

TEAM_T="Turn on team mode?" LEGACY_T="Go back to single-developer mode?"
up() { "$W/bin/xtitle" "$1" >/dev/null 2>&1; } # the core's native dialog is a mapped window titled $1
migrations() { grep -c "^\[state\]\[migrate\]\[--$1\]$" "$W/argv.log"; }
# The sheet opens with focus on Close; the mode button is the control just before it, wherever
# the content above has moved it. Its confirm opens on No; Tab is the confirm's own button.
sheet() { X key ctrl+shift+m; sleep 2; }
to_confirm() { X key shift+Tab; X key Return; sleep 1; }
to_native() { X key Tab; X key Return; sleep 2; }
reset() { X key Escape; sleep 0.5; } # after a native answer only the sheet is left; it closes
# proof <title> <what>: the native dialog is a mapped window on screen, or a failure.
proof() { if up "$1"; then echo "  ok   native dialog \"$1\" on screen $2"; else echo "  FAIL native dialog \"$1\" not on screen $2"; fails=$((fails + 1)); fi; }
# native <mode> <title> <keys…> <want>: open the confirm and the native dialog, prove the dialog is
# on screen, answer with <keys>, prove it is gone, and count the migrations.
native() {
  local m=$1 t=$2 label=$3 want=$4; shift 4
  sheet; to_confirm; to_native
  proof "$t" "before $label"
  for k in "$@"; do
    case $k in
      yes) # GTK's Yes: the right half of the dialog's bottom button row, from its geometry
        local g
        g=$("$W/bin/xtitle" "$t" -g) && set -- $g && X click $(($1 + $3 * 3 / 4)) $(($2 + $4 - 17)) ;;
      *) X key "$k" ;;
    esac
  done
  sleep 2
  if up "$t"; then echo "  FAIL native dialog still on screen after $label"; fails=$((fails + 1)); fi
  check "$label: state migrate --$m ran $want time(s)" "$want" "$(migrations "$m")"
  reset
}
# shape <theme> <dir> <mode> <title>: the confirm and native dialog screens, then No by
# Return (the default), No by Escape, and Yes (clicked) running the migrate once.
shape() {
  local theme=$1 p=$2 m=$3 t=$4
  sheet; to_confirm
  shot "team-confirm-$m-$theme"
  to_native
  proof "$t" "for the screens"
  [ "$theme" = catppuccin-mocha ] && shot "team-native-$m-$theme"
  X key Escape; sleep 1.5; reset
  : >"$W/argv.log"
  native "$m" "$t" "native Return (default No)" 0 Return
  native "$m" "$t" "native Escape" 0 Escape
  native "$m" "$t" "native Yes (a click on it)" 1 yes
}

for theme in catppuccin-mocha github-light; do
  echo "[$theme ${GW}x${GH}] team shape (--legacy)"
  p="$W/Dev/team-$theme"; mkdir -p "$p/.ai"; touch "$p/.ai/team-shape"
  app "$theme" "$p"
  : >"$W/argv.log" # only the sheet's calls count, not start-up's health and project check
  sheet
  shot "team-sheet-$theme"
  check "open: team show, then trunk — nothing else" "[team][show],[trunk]" "$(calls)"
  # The add form (its fields sit above everything that moves): rein refuses (overlap), its words show.
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
  # The confirm with the sheet made taller by the add error: from the "to" field, Tab is Add and
  # the next Tab the mode button. The native dialog behind its confirm proves the confirm was up.
  X key Tab; X key Tab; X key Return; sleep 1
  shot "team-confirm-after-error-$theme"
  to_native
  proof "$LEGACY_T" "after the add error"
  X key Escape; sleep 1.5; reset
  shape "$theme" "$p" legacy "$LEGACY_T"
  quit
  echo "[$theme ${GW}x${GH}] no .ai/team.json shape (--team)"
  p="$W/Dev/single-$theme"; mkdir -p "$p/.ai"
  app "$theme" "$p"
  sheet
  shot "team-single-$theme"
  reset
  shape "$theme" "$p" team "$TEAM_T"
  quit
done
echo "argv log (last project):"; sed 's/^/  /' "$W/argv.log"
if [ "$fails" -eq 0 ]; then echo "PASS: team screens"; else echo "$fails failure(s)"; fi
[ "$fails" -eq 0 ]
