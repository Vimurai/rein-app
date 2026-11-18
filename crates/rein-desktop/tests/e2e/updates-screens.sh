#!/usr/bin/env bash
# updates-screens.sh <out-dir> [W H] — the update watch end to end and its screens (updates.md
# §Tests "Window", D-065): the real window (debug build) on Xvfb, catppuccin-mocha and github-light:
#   watch-current — a started project, current with the installed Rein: no bar, no button;
#   watch-card    — 30 s after start the watch's own check finds a newer Rein: the Update
#                   button and the ask card appear, nobody opened anything;
#   watch-later   — Later: the card goes, the button stays, ui.json records the version;
#   watch-dialog  — the button opens the Update dialog with its two rows;
#   watch-behind  — Rein's VERSION changes under the running window: the open tab is asked
#                   again and its out-of-date bar appears without reopening it.
# The `rein` and `rein-app` are stand-ins in a throwaway HOME that keep their contracts:
# `rein init --check` exits 0 when the project's .ai/version equals ~/.rein/VERSION, else 1 with
# rows; `rein-app update --check` prints installed: / newest: and exits 0 newer, 10 current.
# The window is a copy of the debug build so the stand-in `rein-app` is the one beside it.
# tmux runs on a private -S socket. Every call is logged. A local tool only: it refuses to run
# when CI is set. Needs: Xvfb, tmux, cc, Xlib + XTest headers, ffmpeg, a debug build.
set -uo pipefail
if [ -n "${CI:-}" ]; then echo "updates-screens.sh: not in CI (it drives a real window on Xvfb)" >&2; exit 2; fi
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
W="$(mktemp -d /tmp/rein-ups.XXXXXX)"
W="$(cd "$W" && pwd -P)"
mkdir -p "$W/bin" "$W/app" "$W/home/.rein/bin" "$W/run" "$OUT" "$W/Dev"
chmod 700 "$W/run"
cp "$REPO/target/debug/rein-desktop" "$W/app/rein-desktop" || exit 2
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
    have=$(cat .ai/version) want=$(cat "$HOME/.rein/VERSION")
    if [ "$have" != "$want" ]; then
      echo "rein init --check: project $have → installed $want — 1 change(s) pending:"
      echo "  merge    .ai/state.json: project.rein_version $have → $want"
      echo "Run: rein init"
      exit 1
    fi
    echo "rein init --check: project $have is current with installed $want — nothing to do."
    exit 0 ;;
  start)
    w=$(basename "$PWD")
    tmux has-session -t "$w" 2>/dev/null || tmux new-session -d -s "$w" -n "$w" -x 200 -y 50 "sh"
    exec tmux attach -t "$w" ;;
esac
exit 2
EOF
chmod +x "$W/home/.rein/bin/rein"
cat >"$W/app/rein-app" <<EOF
#!/bin/sh
echo "rein-app: \$*" >>"$W/calls.log"
EOF
cat >>"$W/app/rein-app" <<'EOF'
[ "$*" = "update --check" ] || exit 2
i=$(cat "$HOME/.rein/VERSION") n=$(cat "$HOME/newest")
echo "installed: $i"
echo "newest: $n"
[ "$i" = "$n" ] && exit 10
echo
echo "- a stand-in release"
exit 0
EOF
chmod +x "$W/app/rein-app"
T() { "$REAL_TMUX" -S "$W/tmux.sock" -f /dev/null "$@"; }
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

ST="$W/xdg/state/rein-app"
app() { # app <theme> <dir>
  mkdir -p "$ST"
  printf '{"schema":1,"input_style":"simple","first_run_done":true,"theme":"%s"}' "$1" >"$ST/ui.json"
  rm -f "$ST/projects.json"
  env -u TMUX -u TMUX_PANE -u CLAUDECODE \
    HOME="$W/home" PATH="$W/bin:$PATH" SHELL=/bin/sh \
    XDG_RUNTIME_DIR="$W/run" XDG_CONFIG_HOME="$W/xdg/config" XDG_DATA_HOME="$W/xdg/data" \
    XDG_STATE_HOME="$W/xdg/state" XDG_CACHE_HOME="$W/xdg/cache" \
    WEBKIT_DISABLE_DMABUF_RENDERER=1 \
    "$W/app/rein-desktop" "$2" >>"$W/app.log" 2>&1 &
  APP=$!
  for _ in $(seq 1 300); do "$W/bin/xresize" rein "$GW" "$GH" >/dev/null 2>&1 && break; sleep 0.05; done
}
quit() { kill "$APP" 2>/dev/null; wait "$APP" 2>/dev/null; APP=""; T kill-server 2>/dev/null; sleep 0.5; }
calls() { grep "^$1: " "$W/calls.log" 2>/dev/null | sed "s/^$1: //" | tr '\n' ',' | sed 's/,$//'; }
# Positions at 1280×800 (probed): the card's Later, the title bar's Update button.
LATER_X=$((GW - 125)) LATER_Y=$((GH - 64))
BUTTON_X=$((GW - 130)) BUTTON_Y=20

for theme in catppuccin-mocha github-light; do
  echo "[$theme ${GW}x${GH}]"
  : >"$W/calls.log"
  echo 0.5.0 >"$W/home/.rein/VERSION"
  echo 0.5.1 >"$W/home/newest"
  p="$W/Dev/proj-$theme"; mkdir -p "$p/.ai"; echo 0.5.0 >"$p/.ai/version"
  app "$theme" "$p"
  sleep 8
  shot "watch-current-$theme"
  check "current: started, asked once, no check yet" "start,init --check" "$(calls "proj-$theme")"
  check "current: the watch has not checked before 30 s" "" "$(calls rein-app)"
  sleep 26 # the watch's first check, 30 s after start
  shot "watch-card-$theme"
  check "card: the watch checked by itself, once" "update --check" "$(calls rein-app)"
  X click "$LATER_X" "$LATER_Y"
  sleep 1
  shot "watch-later-$theme"
  check "later: the version is recorded" yes "$(grep -q '"rein": *"0.5.1"' "$ST/ui.json" && echo yes || echo no)"
  X click "$BUTTON_X" "$BUTTON_Y"
  sleep 2
  shot "watch-dialog-$theme"
  X key Escape
  sleep 1
  # Rein updated in another terminal: the open tab is asked again without reopening it.
  echo 0.5.1 >"$W/home/.rein/VERSION"
  sleep 8
  shot "watch-behind-$theme"
  check "behind: asked again once, no second start" "start,init --check,init --check" "$(calls "proj-$theme")"
  quit
done
echo "calls:"; sed 's/^/  /' "$W/calls.log"
if [ "$fails" -eq 0 ]; then echo "PASS: updates screens"; else echo "$fails failure(s)"; fi
[ "$fails" -eq 0 ]
