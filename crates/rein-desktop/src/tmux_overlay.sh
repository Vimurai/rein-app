# rein-app tmux overlay (desktop.md §Look and feel v2 V6, D-050). A constant script, fed on
# stdin:   sh -s -- <client tty> <hidden|app|keep|off> <bg> <fg> <muted> <border> <accent>
# It styles the session the client on <client tty> is attached to with a closed list of
# session options (and window options per window of that session), and installs a
# `client-detached` hook that puts the user's own values back when that client detaches.
# `off` only removes an earlier overlay. Running it again replaces the previous overlay.
#
# Each option it sets first saves the session's (or window's) own value in a user option
# `@rein-saved-<option>`: `1<value>` when set there, `0` when not. Restoring sets the value
# back or unsets the option, then drops the saved copy; an option it never set is left alone.
# It never reads a file, sets a global option, binds a key, or creates, renames or kills a
# session. Exit: 0 done, 2 bad arguments, 3 no tmux, 4 tmux older than 3.2, 5 no client on
# that tty within 60 s, 6 tmux refused an option.

[ $# -eq 7 ] || exit 2
tty=$1 mode=$2 bg=$3 fg=$4 muted=$5 border=$6 accent=$7
case $tty in
  /dev/pts/*) n=${tty#/dev/pts/} ;;
  /dev/ttys*) n=${tty#/dev/ttys} ;;
  *) exit 2 ;;
esac
case $n in '' | *[!0-9]*) exit 2 ;; esac
case $mode in hidden | app | keep | off) ;; *) exit 2 ;; esac
for c in "$bg" "$fg" "$muted" "$border" "$accent"; do
  case $c in \#[0-9a-f][0-9a-f][0-9a-f][0-9a-f][0-9a-f][0-9a-f]) ;; *) exit 2 ;; esac
done

command -v tmux >/dev/null 2>&1 || exit 3
v=$(tmux -V 2>/dev/null) || exit 3
v=${v#tmux }
v=${v#next-}
major=${v%%.*}
minor=${v#*.}
minor=${minor%%[!0-9]*}
case $major in '' | *[!0-9]*) exit 4 ;; esac
case $minor in '' | *[!0-9]*) exit 4 ;; esac
[ "$major" -gt 3 ] || { [ "$major" -eq 3 ] && [ "$minor" -ge 2 ]; } || exit 4

# The session of the client on $tty, by tmux's own id ($N); never by name.
session_of() {
  tmux list-clients -F '#{client_tty} #{session_id}' 2>/dev/null |
    while read -r t s; do
      if [ "$t" = "$tty" ]; then
        printf '%s\n' "$s"
        break
      fi
    done
}
# A fresh `rein start` attaches only once it has built the layout and each pane's shell is
# ready — seconds after the tab opened (E-56). Wait up to 60 s (300 x 0.2 s); the app kills
# this run at once if the tab's terminal ends first.
sid=
i=0
while [ $i -lt 300 ]; do
  sid=$(session_of)
  [ -n "$sid" ] && break
  i=$((i + 1))
  sleep 0.2
done
case $sid in '$'*) s=${sid#?} ;; *) exit 5 ;; esac
case $s in '' | *[!0-9]*) exit 5 ;; esac

windows() {
  tmux list-windows -t "$sid" -F '#{window_id}' 2>/dev/null |
    while read -r w; do
      case $w in '@'*) ;; *) continue ;; esac
      case ${w#?} in '' | *[!0-9]*) continue ;; esac
      printf '%s\n' "$w"
    done
}

WOPTS='pane-border-style pane-active-border-style mode-style window-status-format window-status-current-format'
SOPTS='status status-style status-left status-right message-style message-command-style'
HOOKS='after-new-window[4271] client-detached[4271]'

# A tmux argument ending in `;` ends the command; `\;` is that character itself.
esc() {
  case $1 in
    *\;) printf '%s\\;' "${1%;}" ;;
    *) printf '%s' "$1" ;;
  esac
}

# The restore, as shell text run by tmux's `run-shell` — by the detach hook and by a re-run
# of this script alike. `run-shell` expands formats first: `##` is a literal `#`, and `\$N`
# is the session id once the shell reads it. No single quote: it sits inside tmux's.
RESTORE=$(printf '{ for o in %s; do v=$(tmux show-options -qv -t \\$%s "@rein-saved-$o"); case $v in 1*) v=${v##1}; case $v in *\\;) v="${v%%;}\\;";; esac; tmux set-option -t \\$%s "$o" "$v";; 0) tmux set-option -u -t \\$%s "$o";; esac; tmux set-option -u -t \\$%s "@rein-saved-$o"; done; for w in $(tmux list-windows -t \\$%s -F "##{window_id}"); do for o in %s; do v=$(tmux show-options -wqv -t "$w" "@rein-saved-$o"); case $v in 1*) v=${v##1}; case $v in *\\;) v="${v%%;}\\;";; esac; tmux set-option -w -t "$w" "$o" "$v";; 0) tmux set-option -wu -t "$w" "$o";; esac; tmux set-option -wu -t "$w" "@rein-saved-$o"; done; done; for h in %s; do tmux set-hook -u -t \\$%s "$h"; done; } >/dev/null 2>&1' \
  "$SOPTS" "$s" "$s" "$s" "$s" "$s" "$WOPTS" "$HOOKS" "$s")

tmux run-shell "$RESTORE" >/dev/null 2>&1
[ "$mode" = off ] && exit 0

fail=0
t() { tmux "$@" 2>/dev/null || fail=1; }

# put <-s|-w> <target> <option> <value>: save the target's own value, then set ours.
put() {
  f=
  [ "$1" = -w ] && f=w
  if [ -n "$(tmux show-options -"${f}"q -t "$2" "$3" 2>/dev/null)" ]; then
    saved=1$(tmux show-options -"${f}"qv -t "$2" "$3" 2>/dev/null)
  else
    saved=0
  fi
  t set-option ${f:+-w} -t "$2" "@rein-saved-$3" "$(esc "$saved")"
  t set-option ${f:+-w} -t "$2" "$3" "$4"
}

# Window options: per window now, and for later windows through the after-new-window hook
# (whose target is the new window, which has no value of its own to save).
WSF=' #I #W '
WSCF="#[fg=$accent,bold] #I #W "
for w in $(windows); do
  put -w "$w" pane-border-style "fg=$border"
  put -w "$w" pane-active-border-style "fg=$accent"
  put -w "$w" mode-style "fg=$bg,bg=$accent"
  if [ "$mode" = app ]; then
    put -w "$w" window-status-format "$WSF"
    put -w "$w" window-status-current-format "$WSCF"
  fi
done
anw() { printf "set-option -w @rein-saved-%s 0 ; set-option -w %s '%s'" "$1" "$1" "$2"; }
ANW="$(anw pane-border-style "fg=$border") ; $(anw pane-active-border-style "fg=$accent") ; $(anw mode-style "fg=$bg,bg=$accent")"
[ "$mode" = app ] &&
  ANW="$ANW ; $(anw window-status-format "$WSF") ; $(anw window-status-current-format "$WSCF")"

put -s "$sid" message-style "fg=$accent,bg=$bg"
put -s "$sid" message-command-style "fg=$accent,bg=$bg"
case $mode in
  hidden) put -s "$sid" status off ;;
  app)
    put -s "$sid" status on
    put -s "$sid" status-style "bg=$bg,fg=$muted"
    put -s "$sid" status-left ''
    put -s "$sid" status-right ''
    ;;
esac

t set-hook -t "$sid" 'after-new-window[4271]' "$ANW"
# Only the app's client restores. tmux compares the client name itself, so the shell sees
# only `1` or `0` — never a name a client chose (security review of E-51, F1). `$tty` holds
# no `,` or `}`: it was checked as /dev/pts/N or /dev/ttysN.
t set-hook -t "$sid" 'client-detached[4271]' \
  "run-shell -b '[ #{==:#{hook_client},$tty} = 1 ] || exit 0; $RESTORE'"

# The client may have gone while the overlay was being set: then nothing would undo it.
[ "$(session_of)" = "$sid" ] || {
  tmux run-shell "$RESTORE" >/dev/null 2>&1
  exit 5
}
[ $fail -eq 0 ] || exit 6
exit 0
