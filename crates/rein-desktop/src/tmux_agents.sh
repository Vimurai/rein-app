# rein-app agent status (desktop.md §Look and feel v2 V9 component 4, D-057, E-62). A
# constant script, fed on stdin:   sh -s -- <client tty> <app|user>
# The last word is the tmux server the tab's session runs on (V8): `app` — rein-app's own
# (socket rein-app), `user` — the user's default server.
# It prints one line per pane of the window the tab's tmux client shows:
#   <the pane's @rein_role>|<pane_current_command>|<pane_dead>
# and nothing else. It only reads: it never sets an option, starts a server, or kills.
# Exit: 0 listed, 2 bad arguments, 3 no tmux, 5 no client on that tty (or its window is gone).

[ $# -eq 2 ] || exit 2
tty=$1 server=$2
case $server in app | user) ;; *) exit 2 ;; esac
case $tty in
  /dev/pts/*) n=${tty#/dev/pts/} ;;
  /dev/ttys*) n=${tty#/dev/ttys} ;;
  *) exit 2 ;;
esac
case $n in '' | *[!0-9]*) exit 2 ;; esac

command -v tmux >/dev/null 2>&1 || exit 3
if [ "$server" = app ]; then
  tmux() { command tmux -L rein-app "$@"; }
fi

# The window the client on $tty shows, by tmux's own id (@N); never by name.
w=$(tmux list-clients -F '#{client_tty} #{window_id}' 2>/dev/null |
  while read -r t i; do
    if [ "$t" = "$tty" ]; then
      printf '%s\n' "$i"
      break
    fi
  done)
case $w in '@'*) ;; *) exit 5 ;; esac
case ${w#?} in '' | *[!0-9]*) exit 5 ;; esac

tmux list-panes -t "$w" -F '#{@rein_role}|#{pane_current_command}|#{pane_dead}' 2>/dev/null || exit 5
