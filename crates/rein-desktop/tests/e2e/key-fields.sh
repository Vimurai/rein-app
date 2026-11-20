#!/usr/bin/env bash
# key-fields.sh <out-dir> — what the page receives for Ctrl+Shift+Tab on Xvfb (D-070 ruling 2;
# E-99): the real window (debug build) with one project tab, the keydown fields recorded by the
# page itself (REIN_DESKTOP_TRACE_KEYS, debug builds only) for the key sent several ways:
#   1. xinput key ctrl+shift+Tab            — the harnesses' present way (Tab's keycode)
#   2. xinput keysym ctrl+ISO_Left_Tab      — the keysym bound at level 1 of a spare keycode
#   3. Shift held (down/up) around 2        — ISO_Left_Tab under Ctrl and Shift
#   4. xinput key ctrl+Tab                  — control
#   5. xinput key ctrl+shift+h              — control
# Each way's keydowns (modifiers included) are written with the token ui/src/keys.ts keyName
# gives them to <out-dir>/key-fields.txt. Nothing is asserted: it is a measurement. A local tool
# only: it refuses to run when CI is set. Needs: Xvfb, tmux, cc, Xlib + XTest headers, node,
# python3, a debug build.
set -uo pipefail
if [ -n "${CI:-}" ]; then echo "key-fields.sh: not in CI (it drives a real window on Xvfb)" >&2; exit 2; fi
OUT=$1
GW=1280 GH=800
REPO="$(git -C "$(dirname "$0")" rev-parse --show-toplevel)"
. "$(dirname "$0")/window-binary.sh" && window_binary
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
W="$(mktemp -d /tmp/rein-keys.XXXXXX)"
W="$(cd "$W" && pwd -P)"
mkdir -p "$W/bin" "$W/home/.rein/bin" "$W/run" "$OUT" "$W/Dev/one/.ai"
chmod 700 "$W/run"
printf '{"tasks": []}\n' >"$W/Dev/one/.ai/state.json"
cc -O2 -o "$W/bin/xresize" "$E2E/xresize.c" -lX11 || exit 2
cc -O2 -o "$W/bin/xinput" "$E2E/xinput.c" -lX11 -lXtst || exit 2
cat >"$W/bin/tmux" <<EOF
#!/bin/sh
exec "$REAL_TMUX" -S "$W/tmux.sock" -f /dev/null "\$@"
EOF
chmod +x "$W/bin/tmux"
cat >"$W/home/.rein/bin/rein" <<'EOF'
#!/bin/sh
[ "$1" = start ] || exit 2
w=$(basename "$PWD")
tmux has-session -t "$w" 2>/dev/null || tmux new-session -d -s "$w" -n "$w" -x 200 -y 50
exec tmux attach -t "$w"
EOF
chmod +x "$W/home/.rein/bin/rein"

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

mkdir -p "$W/xdg/state/rein-app"
printf '{"schema":1,"input_style":"simple","first_run_done":true}' >"$W/xdg/state/rein-app/ui.json"
env -u TMUX -u TMUX_PANE -u CLAUDECODE \
  HOME="$W/home" PATH="$W/bin:$PATH" SHELL=/bin/sh REIN_DESKTOP_TRACE_KEYS=1 \
  XDG_RUNTIME_DIR="$W/run" XDG_CONFIG_HOME="$W/xdg/config" XDG_DATA_HOME="$W/xdg/data" \
  XDG_STATE_HOME="$W/xdg/state" XDG_CACHE_HOME="$W/xdg/cache" \
  WEBKIT_DISABLE_DMABUF_RENDERER=1 \
  "$REPO/target/debug/rein-desktop" "$W/Dev/one" >>"$W/app.log" 2>&1 &
APP=$!
for _ in $(seq 1 300); do "$W/bin/xresize" rein "$GW" "$GH" >/dev/null 2>&1 && break; sleep 0.05; done
sleep 8 # the tab is up and the terminal has the keyboard

X() { "$W/bin/xinput" "$@"; }
keys_now() { grep -c '^key-trace' "$W/app.log" || true; }
: >"$W/ways.txt"
way() { # way <label> <command…>: send, then the key-trace lines it produced (2.5 s later)
  local label=$1 before
  shift
  before=$(keys_now)
  "$@"
  sleep 2.5
  grep '^key-trace' "$W/app.log" | tail -n +"$((before + 1))" | sed "s/^key-trace [0-9]* /$label\t/" >>"$W/ways.txt"
}
shift_held() { X down Shift_L; sleep 0.2; X keysym ctrl+ISO_Left_Tab; sleep 0.2; X up Shift_L; }
way "1 key ctrl+shift+Tab" X key ctrl+shift+Tab
way "2 keysym ctrl+ISO_Left_Tab" X keysym ctrl+ISO_Left_Tab
way "3 Shift held + keysym ctrl+ISO_Left_Tab" shift_held
way "4 key ctrl+Tab (control)" X key ctrl+Tab
way "5 key ctrl+shift+h (control)" X key ctrl+shift+h

# The token keys.ts keyName gives each keydown.
node --input-type=module -e '
import { readFileSync } from "node:fs";
const { keyName } = await import(process.argv[1]);
const rows = readFileSync(process.argv[2], "utf8").trim().split("\n").filter(Boolean);
console.log("way\tkey\tcode\tctrlKey\tshiftKey\taltKey\tkeyName");
for (const r of rows) {
  const [label, json] = r.split("\t");
  const f = JSON.parse(json);
  console.log([label, f.key, f.code, f.ctrlKey, f.shiftKey, f.altKey, keyName(f) ?? "(modifier)"].join("\t"));
}' "$REPO/crates/rein-desktop/ui/src/keys.ts" "$W/ways.txt" >"$OUT/key-fields.txt"
column -t -s "$(printf '\t')" "$OUT/key-fields.txt"
echo "  $OUT/key-fields.txt"
