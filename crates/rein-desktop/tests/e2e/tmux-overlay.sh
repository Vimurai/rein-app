#!/usr/bin/env bash
# tmux-overlay.sh — E-51 / D-050: the window's tmux overlay script (src/tmux_overlay.sh)
# against a real tmux server, Linux and macOS. About 75 s: the slow-start case waits 12 s and
# "no client" waits the script's full 60 s.
#
# Two private sockets, never the default one: `a` is the "user's" server holding the
# sessions; `b` hosts the clients — each pane of `b` runs `tmux attach` to `a`, so every
# client has a real tty (`#{pane_tty}` in `b`) and is detached by killing its pane.
# HOME points at a directory whose .tmux.conf would touch a sentinel file if any tmux read it
# (a server started by the script would load it): the script must never start one.
# Needs: tmux >= 3.2.
set -euo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
SCRIPT="$HERE/../../src/tmux_overlay.sh"
REAL="$(command -v tmux)" || { echo "tmux-overlay: tmux not found" >&2; exit 2; }
W="$(mktemp -d /tmp/rein-ov.XXXXXX)"
cleanup() {
  "$REAL" -S "$W/b.sock" kill-server 2>/dev/null || true # private sockets only
  "$REAL" -S "$W/a.sock" kill-server 2>/dev/null || true
  chmod 600 "$W/home/.tmux.conf" 2>/dev/null || true
  rm -rf "$W"
}
trap cleanup EXIT
unset TMUX TMUX_PANE

mkdir -p "$W/bin" "$W/home" "$W/old"
printf 'run-shell "touch %s/conf-was-read"\n' "$W" >"$W/home/.tmux.conf"
cat >"$W/bin/tmux" <<EOF
#!/bin/sh
exec "$REAL" -S "$W/a.sock" "\$@"
EOF
cat >"$W/old/tmux" <<'EOF'
#!/bin/sh
[ "$1" = -V ] && { echo "tmux 3.1c"; exit 0; }
exit 1
EOF
chmod +x "$W/bin/tmux" "$W/old/tmux"
export PATH="$W/bin:$PATH"
b() { "$REAL" -S "$W/b.sock" -f /dev/null "$@"; }

echo "tmux under test: $(tmux -V)"
fails=0
ok() { echo "ok   $1"; }
no() { echo "FAIL $1"; fails=$((fails + 1)); }
check() { # check <label> <expected> <actual>
  if [ "$2" = "$3" ]; then ok "$1"; else no "$1 — expected [$2], got [$3]"; fi
}
C='#1e1e2e #cdd6f4 #a6adc8 #6c7086 #cba6f7'
overlay() { # overlay <tty> <mode> → exit code
  local code=0
  # shellcheck disable=SC2086
  HOME="$W/home" /bin/sh -s -- "$1" "$2" $C <"$SCRIPT" >/dev/null 2>&1 || code=$?
  echo "$code"
}
sopt() { tmux show-options -t "$1" -v "$2" 2>/dev/null || true; }
wopt() { tmux show-options -w -t "$1" -v "$2" 2>/dev/null || true; }
hooks() { tmux show-hooks -t "$1" 2>/dev/null | grep -c '\[4271\]' || true; }
wait_for() { # wait_for <cmd…> — up to 5 s for it to succeed
  for _ in $(seq 1 50); do "$@" && return 0; sleep 0.1; done
  return 1
}
attached() { [ "$(tmux list-clients -t one 2>/dev/null | wc -l | tr -d ' ')" -ge "$1" ]; }
gone() { [ "$(hooks one)" = 0 ]; }

# The user's server: two sessions, a global look of their own.
tmux -f /dev/null new-session -d -s one -x 120 -y 40
tmux new-window -t one
tmux new-session -d -s two
tmux set-option -g status-style 'bg=#ff0000'
tmux set-option -g pane-border-style 'fg=#00ff00'
w1="$(tmux list-windows -t one -F '#{window_id}' | head -n1)"
w2="$(tmux list-windows -t one -F '#{window_id}' | tail -n1)"
# The user's own session- and window-level values (`tmux set status-right …` typed inside a
# session sets one): the overlay must put them back, not delete them.
mine='#[fg=#123456] it'"'"'s "mine" $HOME ;'
tmux set-option -t one status-right "${mine%;}\\;" # `\;`: a literal trailing `;` for tmux
tmux set-option -t one status-style 'bg=#123456'
tmux set-option -w -t "$w2" pane-border-style 'fg=#654321'

# Two clients of session `one`: the app's and a plain terminal's.
b new-session -d -s app -x 120 -y 40 "env -u TMUX tmux attach -t one"
b new-session -d -s other -x 120 -y 40 "env -u TMUX tmux attach -t one"
wait_for attached 2 || { echo "clients did not attach" >&2; exit 1; }
app_tty="$(b display-message -p -t app '#{pane_tty}')"
echo "app client: $app_tty"

check "bad arguments exit 2" 2 "$(HOME="$W/home" /bin/sh -s -- /dev/sda hidden $C <"$SCRIPT" >/dev/null 2>&1; echo $?)"
check "a colour that is not #rrggbb exits 2" 2 "$(HOME="$W/home" /bin/sh -s -- "$app_tty" hidden red '#000000' '#000000' '#000000' '#000000' <"$SCRIPT" >/dev/null 2>&1; echo $?)"
check "tmux older than 3.2 exits 4, nothing set" 4 "$(PATH="$W/old:/usr/bin:/bin" overlay "$app_tty" hidden)"
check "no tmux exits 3" 3 "$(PATH=/nonexistent HOME="$W/home" /bin/sh -s -- "$app_tty" hidden $C <"$SCRIPT" >/dev/null 2>&1; echo $?)"
check "  ... and left session one alone" "" "$(sopt one status)"

check "hidden: exit 0" 0 "$(overlay "$app_tty" hidden)"
check "hidden: status off on the app's session" off "$(sopt one status)"
check "hidden: message line in the accent" "fg=#cba6f7,bg=#1e1e2e" "$(sopt one message-style)"
check "hidden: pane borders on every window" "fg=#6c7086" "$(wopt "$w1" pane-border-style)"
check "hidden: active border in the accent" "fg=#cba6f7" "$(wopt "$w2" pane-active-border-style)"
check "hidden: selection in the accent" "fg=#1e1e2e,bg=#cba6f7" "$(wopt "$w2" mode-style)"
check "another session untouched" "" "$(sopt two status)$(sopt two message-style)$(wopt two pane-border-style)"
check "no global option changed (status-style)" 'bg=#ff0000' "$(tmux show-options -g -v status-style)"
check "no global option changed (pane-border-style)" 'fg=#00ff00' "$(tmux show-options -gw -v pane-border-style)"
check "two hooks installed" 2 "$(hooks one)"
check "the detach hook has tmux compare the client name" 1 "$(tmux show-hooks -t one | grep -c '#{==:#{hook_client},' || true)"
check "  ... and never hands the name to the shell" 0 "$(tmux show-hooks -t one | grep -c -e '#{q:hook_client}' -e '"#{hook_client}"' || true)"
check "no hook on another session" 0 "$(hooks two)"

w3="$(tmux new-window -t one -P -F '#{window_id}')"
check "a later window gets the borders" "fg=#6c7086" "$(wopt "$w3" pane-border-style)"

check "app: exit 0" 0 "$(overlay "$app_tty" app)"
check "app: status on" on "$(sopt one status)"
check "app: status line in the theme" "bg=#1e1e2e,fg=#a6adc8" "$(sopt one status-style)"
check "app: window list only" "" "$(sopt one status-right)"
check "app: current window in the accent" "#[fg=#cba6f7,bold] #I #W " "$(wopt "$w1" window-status-current-format)"
check "app: a second run replaces the first (hooks)" 2 "$(hooks one)"

check "keep: exit 0" 0 "$(overlay "$app_tty" keep)"
check "keep: the user's status line is back" "|bg=#123456|$mine|" "$(sopt one status)|$(sopt one status-style)|$(sopt one status-right)|$(wopt "$w1" window-status-format)"
check "keep: borders still styled" "fg=#6c7086" "$(wopt "$w1" pane-border-style)"

b kill-session -t other
sleep 1
check "another client detaching restores nothing" "fg=#6c7086" "$(wopt "$w1" pane-border-style)"
check "  ... and keeps the hooks" 2 "$(hooks one)"

overlay "$app_tty" hidden >/dev/null
b kill-session -t app
if wait_for gone; then ok "the app client detaching removes the hooks"; else no "the app client detaching removes the hooks"; fi
sleep 0.5
check "  ... and puts the user's own session values back" "bg=#123456|$mine" "$(sopt one status-style)|$(sopt one status-right)"
check "  ... and the user's own window value" 'fg=#654321' "$(wopt "$w2" pane-border-style)"
check "  ... and unsets every option the user had not set" "" "$(for o in status status-left message-style message-command-style; do sopt one "$o"; done)$(for w in "$w1" "$w3"; do for o in pane-border-style pane-active-border-style mode-style window-status-format window-status-current-format; do wopt "$w" "$o"; done; done)$(wopt "$w2" mode-style)"
check "  ... and leaves no saved copy behind" 0 "$(tmux show-options -t one | grep -c @rein-saved || true)$(for w in $(tmux list-windows -t one -F '#{window_id}'); do tmux show-options -w -t "$w" | grep -c @rein-saved || true; done | tr -d '0\n')"
check "the sessions are all still there" "one two" "$(tmux list-sessions -F '#{session_name}' | sort | tr '\n' ' ' | sed 's/ $//')"

# E-56: a fresh `rein start` attaches seconds after the tab opens (two panes, each waited on
# until its shell is ready). The overlay waits for the client: here it attaches 12 s after
# the overlay starts — past the 10 s the E-51 script waited, so dc0ad9f fails this case.
b new-session -d -s late -x 120 -y 40 "sleep 12; env -u TMUX tmux attach -t one"
late_tty="$(b display-message -p -t late '#{pane_tty}')"
t0=$(date +%s)
check "slow start: exit 0 once the client attaches" 0 "$(overlay "$late_tty" hidden)"
check "slow start: it waited for the attach" yes "$([ $(($(date +%s) - t0)) -ge 11 ] && echo yes || echo no)"
check "slow start: status off on the app's session" off "$(sopt one status)"
b kill-session -t late
if wait_for gone; then ok "slow start: the app client detaching removes the hooks"; else no "slow start: the app client detaching removes the hooks"; fi
sleep 0.5
check "slow start: the user's own values are back" "|bg=#123456|$mine" "$(sopt one status)|$(sopt one status-style)|$(sopt one status-right)"

check "no client on that tty exits 5 (after the full wait)" 5 "$(overlay /dev/pts/999 hidden)"
check "the user's .tmux.conf was never loaded" no "$([ -e "$W/conf-was-read" ] && echo yes || echo no)"

if [ "$fails" -eq 0 ]; then echo "PASS: tmux overlay"; else echo "$fails failure(s)"; fi
[ "$fails" -eq 0 ]
