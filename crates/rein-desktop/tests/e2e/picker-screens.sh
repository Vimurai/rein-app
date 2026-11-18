#!/usr/bin/env bash
# picker-screens.sh <out-dir> [W H] — desktop.md V13 (D-067, E-92): the tab bar's + opens one
# Open project dialog. The real window (debug build) on Xvfb, the app's own tmux server on a
# private socket, a stand-in rein, both themes:
#   - a click on + shows the dialog: five recent projects, one open, one missing (screen
#     dialog-five), Choose a folder… last;
#   - a click on a recent row opens its tab; a click on the row of an open project switches to
#     it and opens no second tab;
#   - Esc leaves the tab count unchanged and the terminal takes keys again;
#   - a missing row opens nothing and says why (screen dialog-missing);
#   - a filter that matches nothing leaves Choose a folder… alone (screen dialog-nomatch);
#   - with no recent project the dialog shows its text and Choose a folder… (screen
#     dialog-empty).
# A local tool only: it refuses to run when CI is set. Needs: Xvfb, tmux, cc, Xlib + XTest
# headers, ffmpeg, python3. PROBE=1 only takes the screens, for finding the coordinates.
set -uo pipefail
if [ -n "${CI:-}" ]; then echo "picker-screens.sh: not in CI (it drives a real window on Xvfb)" >&2; exit 2; fi
OUT=$1
GW=${2:-1280} GH=${3:-800}
REPO="$(git -C "$(dirname "$0")" rev-parse --show-toplevel)"
. "$(dirname "$0")/window-binary.sh" && window_binary # cargo test's binary or a stale UI never reaches the window (E-84)
REAL_TMUX="$(command -v tmux)"
W="" APP="" XV=""
cleanup() {
  [ -n "$APP" ] && kill "$APP" 2>/dev/null
  if [ -n "$W" ]; then
    "$REAL_TMUX" -S "$W/app.sock" kill-server 2>/dev/null
    "$REAL_TMUX" -S "$W/default.sock" kill-server 2>/dev/null
  fi
  [ -n "$XV" ] && kill "$XV" 2>/dev/null
  [ -z "$W" ] && return
  [ -n "${KEEP:-}" ] && echo "workdir $W" >&2 || rm -rf "$W"
}
trap cleanup EXIT
W="$(mktemp -d /tmp/rein-pick.XXXXXX)"
W="$(cd "$W" && pwd -P)"
mkdir -p "$W/bin" "$W/home/.rein/bin" "$W/run" "$OUT" "$W/Dev"
chmod 700 "$W/run"
cc -O2 -o "$W/bin/xresize" "$REPO/crates/rein-desktop/tests/e2e/xresize.c" -lX11 || exit 2
cc -O2 -o "$W/bin/xinput" "$REPO/crates/rein-desktop/tests/e2e/xinput.c" -lX11 -lXtst || exit 2
# -S wins over -L: the app's socket name picks app.sock, anything else default.sock.
cat >"$W/bin/tmux" <<EOF
#!/bin/sh
sock="$W/default.sock"
for a in "\$@"; do [ "\$a" = rein-app ] && sock="$W/app.sock"; done
exec "$REAL_TMUX" -S "\$sock" -f /dev/null "\$@"
EOF
chmod +x "$W/bin/tmux"
cat >"$W/home/.rein/bin/rein" <<EOF
#!/bin/sh
LOG="$W/rein.log"
EOF
cat >>"$W/home/.rein/bin/rein" <<'EOF'
w=$(basename "$PWD")
case "$*" in
  start)
    echo "$w: start" >>"$LOG"
    if ! tmux has-session -t "$w" 2>/dev/null; then
      tmux new-session -d -s "$w" -n "$w" -x 200 -y 50 "sh"
      p=$(tmux list-panes -t "$w:$w" -F '#{pane_id}' | head -n1)
      tmux set-option -p -t "$p" @rein_role engineer
      sleep 1
      tmux send-keys -t "$p" "clear; echo $w engineer; sleep 600" Enter
    fi
    exec tmux attach -t "$w:$w" ;;
esac
exit 2
EOF
chmod +x "$W/home/.rein/bin/rein"
TA() { "$REAL_TMUX" -S "$W/app.sock" -f /dev/null "$@"; } # -f: never the operator's ~/.tmux.conf
TD() { "$REAL_TMUX" -S "$W/default.sock" -f /dev/null "$@"; }
X() { "$W/bin/xinput" "$@"; sleep 0.7; }
shot() { ffmpeg -loglevel error -y -f x11grab -draw_mouse 0 -video_size "${GW}x${GH}" -i "$DISPLAY" -frames:v 1 "$OUT/$1-${GW}x${GH}.png" && echo "  $OUT/$1-${GW}x${GH}.png"; }
fails=0
check() { if [ "$2" = "$3" ]; then echo "  ok   $1"; else echo "  FAIL $1 — expected [$2], got [$3]"; fails=$((fails + 1)); fi; }
# The open-tabs record's folder names, in order (projects.json, V12 component 4).
open_rec() { python3 -c 'import json,sys,os; d=json.load(open(sys.argv[1])); print(" ".join(os.path.basename(t["path"]) for t in d.get("open",[])))' "$W/xdg/state/rein-app/projects.json" 2>/dev/null; }
active() { python3 -c 'import json,sys; print(json.load(open(sys.argv[1])).get("active"))' "$W/xdg/state/rein-app/projects.json" 2>/dev/null; }
clients() { TA list-clients -F '#{session_name}' 2>/dev/null | sort | uniq -c | awk '{ print $2 "=" $1 }' | tr '\n' ' ' | sed 's/ $//'; }
starts() { grep -c ": start$" "$W/rein.log" 2>/dev/null || true; }

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

# Where things are at 1280×800 (found with PROBE=1): the + right of one, two or no tab (a tab
# is 74 px wide at its narrowest, "alpha" and "beta" both), the five-project dialog's rows.
PLUS1_X=${PLUS1_X:-100} PLUS2_X=${PLUS2_X:-178} PLUS0_X=${PLUS0_X:-22} PLUS_Y=20
ROW1_Y=${ROW1_Y:-323} ROW_H=${ROW_H:-32} ROW_X=640

app() { # app <theme> <recent json list> [dir]
  local st="$W/xdg/state/rein-app"
  mkdir -p "$st"
  printf '{"schema":1,"input_style":"simple","first_run_done":true,"theme":"%s"}' "$1" >"$st/ui.json"
  printf '{"schema":1,"recent":%s}' "$2" >"$st/projects.json"
  env -u TMUX -u TMUX_PANE -u CLAUDECODE \
    HOME="$W/home" PATH="$W/bin:$PATH" SHELL=/bin/sh \
    XDG_RUNTIME_DIR="$W/run" XDG_CONFIG_HOME="$W/xdg/config" XDG_DATA_HOME="$W/xdg/data" \
    XDG_STATE_HOME="$W/xdg/state" XDG_CACHE_HOME="$W/xdg/cache" \
    WEBKIT_DISABLE_DMABUF_RENDERER=1 \
    "$REPO/target/debug/rein-desktop" ${3:+"$3"} >>"$W/app.log" 2>&1 &
  APP=$!
  for _ in $(seq 1 300); do "$W/bin/xresize" rein "$GW" "$GH" >/dev/null 2>&1 && break; sleep 0.05; done
}
stop_app() {
  kill "$APP" 2>/dev/null; wait "$APP" 2>/dev/null; APP=""
  TA kill-server 2>/dev/null; TD kill-server 2>/dev/null
  sleep 0.5
}
row() { echo $((ROW1_Y + ($1 - 1) * ROW_H)); } # row <n>: the y of the dialog's row n

now=$(date +%s)
for theme in catppuccin-mocha github-light; do
  echo "[$theme ${GW}x${GH}]"
  : >"$W/rein.log"; : >"$W/app.log"
  # Five recent projects, newest first: alpha (opened by the command line: open), beta, gamma,
  # delta, and epsilon whose folder is gone (missing).
  recent="" i=0
  for n in alpha beta gamma delta epsilon; do
    d="$W/Dev/$theme/$n"
    mkdir -p "$d/.ai"
    recent="$recent${recent:+,}{\"path\":\"$d\",\"last_opened\":$((now - i * 7200 - 600))}"
    i=$((i + 1))
  done
  rm -rf "$W/Dev/$theme/epsilon"
  app "$theme" "[$recent]" "$W/Dev/$theme/alpha"
  sleep 7
  X click "$PLUS1_X" "$PLUS_Y"; sleep 1
  shot "dialog-five-$theme"
  if [ -n "${PROBE:-}" ]; then stop_app; continue; fi
  check "+ opened no tab by itself" "alpha" "$(open_rec)"
  # A recent row (beta, row 2) opens its tab.
  X click "$ROW_X" "$(row 2)"; sleep 6
  check "a click on beta's row opens its tab" "alpha beta" "$(open_rec)"
  check "  ... one client per session" "alpha=1 beta=1" "$(clients)"
  # The row of an open project switches to it, no second tab: alpha, row 2 now that beta,
  # opened last, leads the list.
  X click "$PLUS2_X" "$PLUS_Y"; sleep 1
  X click "$ROW_X" "$(row 2)"; sleep 2
  check "a click on an open project's row switches to it" "0" "$(active)"
  check "  ... and opens no second tab" "alpha beta" "$(open_rec)"
  check "  ... nor a second start" 2 "$(starts)"
  # Esc: nothing opens, and the terminal takes keys again.
  X click "$PLUS2_X" "$PLUS_Y"; sleep 1
  X key Escape; sleep 1
  X type "zq9echo"; sleep 1
  check "Esc leaves the tabs as they were" "alpha beta" "$(open_rec)"
  check "  ... and the terminal takes keys again" yes "$(grep -q zq9echo <<<"$(TA capture-pane -p -t alpha)" && echo yes || echo no)"
  # A missing row (epsilon, row 5) opens nothing and says why.
  X click "$PLUS2_X" "$PLUS_Y"; sleep 1
  X click "$ROW_X" "$(row 5)"; sleep 1.5
  shot "dialog-missing-$theme"
  check "a missing row opens nothing" "alpha beta" "$(open_rec)"
  check "  ... and starts nothing" 2 "$(starts)"
  # A filter that matches nothing: Choose a folder… alone.
  X type "zzz"; sleep 1
  shot "dialog-nomatch-$theme"
  X key Escape; sleep 1
  stop_app
  # No recent project at all: the start screen, + opens the dialog with its text.
  app "$theme" "[]"
  sleep 5
  X click "$PLUS0_X" "$PLUS_Y"; sleep 1
  shot "dialog-empty-$theme"
  X key Escape; sleep 1
  check "no recent project: Esc opens nothing" "" "$(open_rec)"
  stop_app
done

if [ "$fails" -eq 0 ]; then echo "PASS: picker screens"; else echo "$fails failure(s)"; fi
[ "$fails" -eq 0 ]
