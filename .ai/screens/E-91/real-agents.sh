# E-91 real-agents quit-and-reopen look, as run 2026-11-18 (a record, not a harness): real rein + claude
# on a PRIVATE tmux socket (bin/tmux below: -S <dir>/tmux.sock -f /dev/null), fake HOME whose rein runs
# the real one with the operator's HOME; SHELL=/bin/sh so no login shell rebuilds PATH.
# bin/tmux:
#   #!/bin/sh
#   # E-91 real-agents look: every tmux call (the app's and rein's) on a private socket, no user config.
#   exec "/home/linuxbrew/.linuxbrew/bin/tmux" -S "/tmp/e91real.TBy2/tmux.sock" -f /dev/null "$@"
# home/.rein/bin/rein:
#   #!/bin/sh
#   # The real rein, with the operator's HOME (Claude account) and the private tmux.
#   exec env HOME="/home/emir" PATH="/tmp/e91real.TBy2/bin:$PATH" "/home/emir/.rein/bin/rein" "$@"
#!/usr/bin/env bash
set -uo pipefail
R=/tmp/e91real.TBy2; REPO=/home/emir/Dev/rein-app; OUT=/home/emir/Dev/rein-app/.ai/screens/E-91
P=$R/e91check
T() { $R/bin/tmux "$@"; }
mkdir -p $R/run $R/xdg/state/rein-app; chmod 700 $R/run
cc -O2 -o $R/xresize $REPO/crates/rein-desktop/tests/e2e/xresize.c -lX11 || exit 2
cc -O2 -o $R/xinput $REPO/crates/rein-desktop/tests/e2e/xinput.c -lX11 -lXtst || exit 2
XV=""; for n in $(seq 90 130); do [ -e /tmp/.X$n-lock ] && continue; Xvfb :$n -screen 0 1280x800x24 -nolisten tcp >/dev/null 2>&1 & XV=$!; sleep 1.5; kill -0 $XV 2>/dev/null && { export DISPLAY=:$n; break; }; XV=""; done
[ -n "$XV" ] || exit 2
trap 'kill $XV 2>/dev/null; T kill-server 2>/dev/null' EXIT
printf '{"schema":1,"input_style":"simple","first_run_done":true,"theme":"catppuccin-mocha"}' >$R/xdg/state/rein-app/ui.json
X() { $R/xinput "$@"; sleep 0.8; }
shot() { ffmpeg -loglevel error -y -f x11grab -draw_mouse 0 -video_size 1280x800 -i "$DISPLAY" -frames:v 1 "$OUT/$1-1280x800.png" && echo "  $OUT/$1-1280x800.png"; }
app() {
  env -u TMUX -u TMUX_PANE -u CLAUDECODE HOME=$R/home SHELL=/bin/sh "PATH=$R/bin:$PATH" XDG_RUNTIME_DIR=$R/run XDG_STATE_HOME=$R/xdg/state XDG_CONFIG_HOME=$R/xdg/config \
    WEBKIT_DISABLE_DMABUF_RENDERER=1 $REPO/target/debug/rein-desktop "$@" >>$R/app.log 2>&1 &
  APP=$!
  for _ in $(seq 1 300); do $R/xresize rein 1280 800 >/dev/null 2>&1 && break; sleep 0.05; done
}
panes() { T list-panes -a -F '#{session_name} #{@rein_role} #{pane_current_command} #{pane_width}x#{pane_height}' | grep e91check; }
app "$P"; sleep 50
shot real-before-quit
echo "running:"; panes; echo "client: $(T list-clients -F '#{client_width}x#{client_height}')"
X key ctrl+shift+q; sleep 3
kill -0 $APP 2>/dev/null && echo "window still up" || echo "window exited"
echo "after quit: $(T list-clients | wc -l) clients; $(T list-sessions -F '#{session_name}')"
app; sleep 20
shot real-reattached
echo "reopened:"; panes; echo "client: $(T list-clients -F '#{client_width}x#{client_height}')"
X key ctrl+shift+w; sleep 3; X key Tab; X key Return; sleep 8
echo "after Close project: [$(T list-sessions -F '#{session_name}' 2>/dev/null)]"
kill $APP 2>/dev/null; wait $APP 2>/dev/null
