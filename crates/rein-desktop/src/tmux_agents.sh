# rein-app agent status (desktop.md §Look and feel v2 V9 component 4, D-057, E-62; status.md
# component 1, D-062, E-76). A constant script, fed on stdin:
#   sh -s -- <client tty> <app|user> [<client tty> <app|user>]...
# One pair per open tab (at most 16), so every tab is asked in one process — on Windows one
# wsl.exe for all of them. The second word of a pair is the tmux server the tab's session runs
# on (V8): `app` — rein-app's own (socket rein-app), `user` — the user's default server.
# It prints the clock of the machine tmux runs on, then for each pair, in order:
#   tab <pair index from 0> <0 listed | 5 no client on that tty, or its window is gone>
#   activity <the shown window's #{window_activity}>                       (listed only)
#   pane <the pane's @rein_role>|<pane_current_command>|<pane_dead>        (one per pane)
# Every line made from a pane starts with `pane `, so no role or command can print a header;
# the role and the command are cut to one more than the core accepts.
# and nothing else. It only reads: it never sets an option, starts a server, or kills.
# Exit: 0 answered, 2 bad arguments, 3 no tmux.

[ $# -ge 2 ] && [ $# -le 32 ] && [ $(($# % 2)) -eq 0 ] || exit 2
# Every pair is checked before anything runs.
i=0
for a in "$@"; do
  if [ $((i % 2)) -eq 0 ]; then
    case $a in
      /dev/pts/*) n=${a#/dev/pts/} ;;
      /dev/ttys*) n=${a#/dev/ttys} ;;
      *) exit 2 ;;
    esac
    case $n in '' | *[!0-9]*) exit 2 ;; esac
  else
    case $a in app | user) ;; *) exit 2 ;; esac
  fi
  i=$((i + 1))
done

command -v tmux >/dev/null 2>&1 || exit 3
# on <server> <arguments>...: one read on that server.
on() {
  if [ "$1" = app ]; then
    shift
    command tmux -L rein-app "$@"
  else
    shift
    command tmux "$@"
  fi
}

printf 'clock %s\n' "$(date +%s)"
# Each server's clients are listed once per run, whatever the number of pairs.
clients_app= clients_user= listed_app= listed_user=
i=0
while [ $# -ge 2 ]; do
  tty=$1 server=$2
  shift 2
  if [ "$server" = app ]; then
    [ -n "$listed_app" ] || clients_app=$(on app list-clients -F '#{client_tty} #{window_id}' 2>/dev/null)
    listed_app=1 clients=$clients_app
  else
    [ -n "$listed_user" ] || clients_user=$(on user list-clients -F '#{client_tty} #{window_id}' 2>/dev/null)
    listed_user=1 clients=$clients_user
  fi
  # The window the client on $tty shows, by tmux's own id (@N); never by name.
  w=$(printf '%s\n' "$clients" |
    while read -r t id; do
      if [ "$t" = "$tty" ]; then
        printf '%s\n' "$id"
        break
      fi
    done)
  case $w in '@'*) ;; *) w= ;; esac
  case ${w#?} in '' | *[!0-9]*) w= ;; esac
  # One read per tab: every pane's line, each led by the window's last output.
  panes=
  [ -n "$w" ] && panes=$(on "$server" list-panes -t "$w" -F '#{window_activity} #{=33:@rein_role}|#{=257:pane_current_command}|#{pane_dead}' 2>/dev/null)
  act=${panes%%[!0-9]*}
  if [ -n "$panes" ] && [ -n "$act" ]; then
    printf 'tab %s 0\nactivity %s\n' "$i" "$act"
    # A line not led by the window's time is the rest of a role or command with a newline
    # in it: dropped.
    printf '%s\n' "$panes" | while IFS= read -r line; do
      case $line in "$act "*) printf 'pane %s\n' "${line#* }" ;; esac
    done
  else
    printf 'tab %s 5\n' "$i"
  fi
  i=$((i + 1))
done
exit 0
