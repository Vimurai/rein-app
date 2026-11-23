#!/usr/bin/env bash
# app-update-screens.sh <out-dir> — the window's own update drawn, end to end against the test
# endpoint (updates.md §Components 4, §Tests "Window", D-066; E-87): the real window, built with
# the test-only `update-test` feature (its own target dir), on Xvfb, catppuccin-mocha and
# github-light. A loopback server serves a desktop-latest.json for 0.2.0 signed with a throwaway
# minisign key (REIN_DESKTOP_UPDATE_KEY / _URL) and its installer, slowly (about 20 s), so each
# state can be seen. Each theme:
#   app-card     — 30 s after start the watch finds 0.2.0: the Update button and the ask card;
#   app-later    — Later: the card goes, the button stays, ui.json records 0.2.0;
#   app-dialog   — the button opens the Update dialog: the window's row, its button and notes;
#   app-progress — "Update and restart" in the dialog: the card shows Downloading with the
#                  determinate line and a counting percent (two screens, the percent grows);
#   app-cancel   — Cancel: the card goes (the version was answered Later), no file is left;
#   app-failed   — the server now serves a tampered installer: Try again ends at the signature
#                  sentence with Try again, no file is left, nothing started;
#   app-updated  — a window started with ui.json `updated_from` 0.0.9 and a document for its own
#                  version: "Updated to 0.1.0", then "What's new" opens the notes.
# Then once, with GTK animations off (prefers-reduced-motion): app-reduced, the card at rest.
# Last, a good installer: the window reaches "would start … /P /UPDATE /R" (the update-test
# build prints instead of starting) and exits by itself.
# Checks print ok/FAIL; the screens are read by eye (each PNG opened before a handoff names it).
# A local tool only: it refuses to run when CI is set. Needs: Xvfb, tmux, cc, Xlib + XTest
# headers, ffmpeg, python3, minisign.
set -uo pipefail
if [ -n "${CI:-}" ]; then echo "app-update-screens.sh: not in CI (it drives a real window on Xvfb)" >&2; exit 2; fi
OUT=$1
GW=1280 GH=800
REPO="$(git -C "$(dirname "$0")" rev-parse --show-toplevel)"
. "$(dirname "$0")/window-binary.sh" && window_binary # the UI bundle must be current
BIN_DIR="$REPO/target/update-test"
(cd "$REPO" && CARGO_TARGET_DIR="$BIN_DIR" cargo build -q -p rein-desktop --features update-test) || { echo "update-test build failed" >&2; exit 2; }
REAL_TMUX="$(command -v tmux)"
W="" APP="" XV="" SRV=""
cleanup() {
  [ -n "$APP" ] && kill "$APP" 2>/dev/null
  [ -n "$SRV" ] && kill "$SRV" 2>/dev/null
  [ -n "$W" ] && "$REAL_TMUX" -S "$W/tmux.sock" kill-server 2>/dev/null
  [ -n "$XV" ] && kill "$XV" 2>/dev/null
  [ -z "$W" ] && return
  [ -n "${KEEP:-}" ] && echo "workdir $W" >&2 || rm -rf "$W"
}
trap cleanup EXIT
W="$(mktemp -d /tmp/rein-aup.XXXXXX)"
W="$(cd "$W" && pwd -P)"
mkdir -p "$W/bin" "$W/app" "$W/home/.rein/bin" "$W/run" "$W/srv" "$OUT" "$W/Dev/proj/.ai"
chmod 700 "$W/run"
cp "$BIN_DIR/debug/rein-desktop" "$W/app/rein-desktop" || exit 2
cc -O2 -o "$W/bin/xresize" "$REPO/crates/rein-desktop/tests/e2e/xresize.c" -lX11 || exit 2
cc -O2 -o "$W/bin/xinput" "$REPO/crates/rein-desktop/tests/e2e/xinput.c" -lX11 -lXtst || exit 2
printf '#!/bin/sh\nexec "%s" -S "%s/tmux.sock" -f /dev/null "$@"\n' "$REAL_TMUX" "$W" >"$W/bin/tmux"
chmod +x "$W/bin/tmux"
# Stand-ins: rein current with its project; rein-app's check says Rein is current (exit 10).
cat >"$W/home/.rein/bin/rein" <<'EOF'
#!/bin/sh
case "$*" in
  "init --check") echo "rein init --check: project 0.5.0 is current with installed 0.5.0 — nothing to do."; exit 0 ;;
  start)
    w=$(basename "$PWD")
    tmux has-session -t "$w" 2>/dev/null || tmux new-session -d -s "$w" -n "$w" -x 200 -y 50 "sh"
    exec tmux attach -t "$w" ;;
esac
exit 2
EOF
chmod +x "$W/home/.rein/bin/rein"
echo 0.5.0 >"$W/home/.rein/VERSION"
echo 0.5.0 >"$W/Dev/proj/.ai/version"
cat >"$W/app/rein-app" <<'EOF'
#!/bin/sh
[ "$*" = "update --check" ] || exit 2
echo "installed: 0.5.0"; echo "newest: 0.5.0"; exit 10
EOF
chmod +x "$W/app/rein-app"

# The throwaway key and the release: a 3 MiB "installer", good and tampered (same size).
minisign -G -W -p "$W/key.pub" -s "$W/key.sec" >/dev/null 2>&1 || { echo "minisign -G failed" >&2; exit 2; }
KEY_ID=$(head -1 "$W/key.pub" | awk '{print $NF}')
head -c 3145728 /dev/urandom >"$W/srv/good.exe"
cp "$W/srv/good.exe" "$W/srv/tampered.exe"
printf 'X' | dd of="$W/srv/tampered.exe" bs=1 seek=1000 conv=notrunc 2>/dev/null
minisign -S -s "$W/key.sec" -m "$W/srv/good.exe" -x "$W/srv/good.exe.minisig" >/dev/null 2>&1 || exit 2
PORT=$(python3 -c 'import socket; s=socket.socket(); s.bind(("127.0.0.1",0)); print(s.getsockname()[1])')
doc() { # doc <version> — write and sign desktop-latest.json
  python3 - "$1" "$KEY_ID" "$PORT" "$W/srv" <<'EOF'
import hashlib, json, sys
v, kid, port, d = sys.argv[1:]
b = open(f"{d}/good.exe", "rb").read()
json.dump({"schema": 1, "version": v, "key_id": kid,
  "notes": f"rein-app {v}\n\n- The window updates itself, verified with its own key.\n- **Later** asks once per version.",
  "windows_x64": {"url": f"http://127.0.0.1:{port}/rein-app_{v}_x64-setup.exe", "size": len(b),
    "sha256": hashlib.sha256(b).hexdigest(), "sig": open(f"{d}/good.exe.minisig").read()}},
  open(f"{d}/desktop-latest.json", "w"))
EOF
  minisign -S -s "$W/key.sec" -m "$W/srv/desktop-latest.json" -x "$W/srv/desktop-latest.json.minisig" >/dev/null 2>&1
}
# The server: the document and its signature; the installer at ~150 KiB/s from good.exe, or
# from tampered.exe while $W/srv/mode says so. Every request is logged.
cat >"$W/server.py" <<'EOF'
import http.server, sys, time, os
d = sys.argv[2]
class H(http.server.BaseHTTPRequestHandler):
    def log_message(self, *a): pass
    def do_GET(self):
        open(f"{d}/../requests.log", "a").write(self.path + "\n")
        p = self.path.lstrip("/")
        if "/" in p or ".." in p or not p:  # only the files of this one directory
            self.send_response(404); self.end_headers(); return
        if p.endswith(".exe"):
            mode = open(f"{d}/mode").read().strip() if os.path.exists(f"{d}/mode") else "good"
            body = open(f"{d}/{mode}.exe", "rb").read()
            self.send_response(200); self.send_header("Content-Length", str(len(body))); self.end_headers()
            for i in range(0, len(body), 32768):
                try:
                    self.wfile.write(body[i:i + 32768]); self.wfile.flush()
                except OSError:
                    return
                time.sleep(0.2)
            return
        if not os.path.exists(f"{d}/{p}"):
            self.send_response(404); self.end_headers(); return
        body = open(f"{d}/{p}", "rb").read()
        self.send_response(200); self.send_header("Content-Length", str(len(body))); self.end_headers()
        self.wfile.write(body)
http.server.ThreadingHTTPServer(("127.0.0.1", int(sys.argv[1])), H).serve_forever()
EOF
python3 "$W/server.py" "$PORT" "$W/srv" &
SRV=$!

T() { "$REAL_TMUX" -S "$W/tmux.sock" -f /dev/null "$@"; }
X() { "$W/bin/xinput" "$@"; sleep 0.8; }
shot() { ffmpeg -loglevel error -y -f x11grab -draw_mouse 0 -video_size "${GW}x${GH}" -i "$DISPLAY" -frames:v 1 "$OUT/$1.png" && echo "  $OUT/$1.png"; }
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
CACHE="$W/xdg/cache/rein-app/update"
app() { # app <theme> [ui.json extra]
  mkdir -p "$ST"
  printf '{"schema":1,"input_style":"simple","first_run_done":true,"theme":"%s"%s}' "$1" "${2:-}" >"$ST/ui.json"
  rm -f "$ST/projects.json"
  env -u TMUX -u TMUX_PANE -u CLAUDECODE \
    HOME="$W/home" PATH="$W/bin:$PATH" SHELL=/bin/sh \
    XDG_RUNTIME_DIR="$W/run" XDG_CONFIG_HOME="$W/xdg/config" XDG_DATA_HOME="$W/xdg/data" \
    XDG_STATE_HOME="$W/xdg/state" XDG_CACHE_HOME="$W/xdg/cache" \
    REIN_DESKTOP_UPDATE_URL="http://127.0.0.1:$PORT/desktop-latest.json" REIN_DESKTOP_UPDATE_KEY="$W/key.pub" \
    WEBKIT_DISABLE_DMABUF_RENDERER=1 \
    "$W/app/rein-desktop" "$W/Dev/proj" >>"$W/app.log" 2>&1 &
  APP=$!
  for _ in $(seq 1 300); do "$W/bin/xresize" rein "$GW" "$GH" >/dev/null 2>&1 && break; sleep 0.05; done
}
quit() { kill "$APP" 2>/dev/null; wait "$APP" 2>/dev/null; APP=""; T kill-server 2>/dev/null; sleep 0.5; }
leftover() { find "$CACHE" -type f 2>/dev/null | wc -l | tr -d ' '; }
fetched() { grep -c '\.exe$' "$W/requests.log" | tr -d ' '; } # installer requests so far

# Positions at 1280×800 (probed from the screens): the card's right button and the one left of
# it, the title bar's Update button, the dialog's "Update and restart".
CARD_RIGHT_X=${CARD_RIGHT_X:-1205} CARD_LEFT_X=${CARD_LEFT_X:-1100} CARD_BTN_Y=${CARD_BTN_Y:-735}
BUTTON_X=${BUTTON_X:-1150} BUTTON_Y=${BUTTON_Y:-20}
DIALOG_UPDATE_X=${DIALOG_UPDATE_X:-880} DIALOG_UPDATE_Y=${DIALOG_UPDATE_Y:-348}

for theme in catppuccin-mocha github-light; do
  echo "[$theme]"
  doc 0.2.0
  echo good >"$W/srv/mode"
  : >"$W/requests.log"
  app "$theme"
  sleep 36 # the watch's first check is 30 s after start
  shot "app-card-$theme"
  check "card: the document and its signature were fetched" "yes" "$(grep -q '^/desktop-latest.json.minisig$' "$W/requests.log" && echo yes || echo no)"
  X click "$CARD_LEFT_X" "$CARD_BTN_Y" # Later
  sleep 1
  shot "app-later-$theme"
  check "later: 0.2.0 recorded" yes "$(grep -q '"app": *"0.2.0"' "$ST/ui.json" && echo yes || echo no)"
  X click "$BUTTON_X" "$BUTTON_Y"
  sleep 2
  shot "app-dialog-$theme"
  X click "$DIALOG_UPDATE_X" "$DIALOG_UPDATE_Y" # Update and restart
  sleep 4
  shot "app-progress-$theme"
  sleep 5
  shot "app-progress2-$theme"
  check "progress: the installer is being fetched" 1 "$(fetched)"
  check "progress: a partial file in the cache" 1 "$(leftover)"
  X click "$CARD_RIGHT_X" "$CARD_BTN_Y" # Cancel (the only button)
  sleep 2
  shot "app-cancel-$theme"
  check "cancel: no file left" 0 "$(leftover)"
  echo tampered >"$W/srv/mode"
  # The version was answered Later: start again from the dialog.
  X click "$BUTTON_X" "$BUTTON_Y"
  sleep 2
  X click "$DIALOG_UPDATE_X" "$DIALOG_UPDATE_Y"
  sleep 25 # the whole slow download, then the signature
  shot "app-failed-$theme"
  check "failed: the installer was fetched again" 2 "$(fetched)"
  check "failed: no file left" 0 "$(leftover)"
  check "failed: nothing started" 0 "$(grep -c 'would start' "$W/app.log")"
  quit
  # After a restart: updated_from in the state file, a document for the running version.
  doc 0.1.0
  app "$theme" ',"updated_from":"0.0.9"'
  sleep 3
  shot "app-updated-$theme"
  sleep 33 # the check brings the running version's notes: What's new appears
  shot "app-updated-notes-$theme"
  X click "${WHATSNEW_X:-1150}" "$CARD_BTN_Y" # What's new
  sleep 2
  shot "app-whatsnew-$theme"
  check "updated_from read once" yes "$(grep -q '"updated_from": *null' "$ST/ui.json" && echo yes || echo no)"
  quit
done

echo "[reduced motion]"
mkdir -p "$W/xdg/config/gtk-3.0"
printf '[Settings]\ngtk-enable-animations=false\n' >"$W/xdg/config/gtk-3.0/settings.ini"
doc 0.2.0
echo good >"$W/srv/mode"
: >"$W/requests.log"
app catppuccin-mocha
sleep 36
shot "app-reduced-catppuccin-mocha"
echo "[a good installer]"
: >"$W/app.log"
X click "$CARD_RIGHT_X" "$CARD_BTN_Y" # Update and restart
for _ in $(seq 1 60); do kill -0 "$APP" 2>/dev/null || break; sleep 1; done
check "good: the installer was fetched" 1 "$(fetched)"
check "good: the window exited by itself" gone "$(kill -0 "$APP" 2>/dev/null && echo running || echo gone)"
APP=""
check "good: the installer started with the constant argv" yes "$(grep -q 'would start .*rein-app-update-.*\.exe /P /UPDATE /R$' "$W/app.log" && echo yes || echo no)"
grep 'would start' "$W/app.log" | sed 's/^/  /'
T kill-server 2>/dev/null
echo "requests:"; sort "$W/requests.log" | uniq -c | sed 's/^/  /'
if [ "$fails" -eq 0 ]; then echo "PASS: app update screens"; else echo "$fails failure(s)"; fi
[ "$fails" -eq 0 ]
