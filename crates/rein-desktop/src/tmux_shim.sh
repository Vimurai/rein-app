#!/bin/sh
# rein-app tmux shim (desktop.md §Look and feel v2 V8 component 2, D-055). First on the PATH
# of a tab's process: every plain `tmux` reaches rein-app's own server — socket `rein-app`,
# the app's own config — so the user's ~/.tmux.conf and plugins never apply in the window.
# Written by the app's setup script (tmux_setup.sh) and rewritten whenever it differs.
# It reads nothing but PATH and HOME and evaluates no text; the caller's arguments are
# passed on as they are.
H=$HOME/.local/state/rein-app/tmux

# The real tmux: the first absolute PATH entry holding an executable `tmux` file that is not
# this shim (a relative entry such as `.` would run a tmux from the current directory).
real=
set -f
old_ifs=$IFS
IFS=:
for d in $PATH; do
  case $d in /*) ;; *) continue ;; esac
  c=$d/tmux
  [ -f "$c" ] && [ -x "$c" ] || continue
  [ "$c" -ef "$0" ] && continue
  [ "$c" -ef "$H/bin/tmux" ] && continue
  real=$c
  break
done
IFS=$old_ifs
set +f
[ -n "$real" ] || {
  echo "rein-app: no tmux found on PATH" >&2
  exit 127
}

# A caller that chose a server itself (-L or -S among tmux's own options, before the
# command, alone or clustered as in -uL) keeps its arguments unchanged.
skip=0
for a in "$@"; do
  if [ "$skip" = 1 ]; then
    skip=0
    continue
  fi
  case $a in
    --) break ;;
    -*) ;;
    *) break ;;
  esac
  flags=${a#-}
  while [ -n "$flags" ]; do
    f=${flags%"${flags#?}"}
    flags=${flags#?}
    case $f in
      L | S) exec "$real" "$@" ;;
      c | f | T)
        # Takes a value: the rest of this word, else the next word.
        [ -z "$flags" ] && skip=1
        break
        ;;
    esac
  done
done
exec "$real" -L rein-app -f "$H/tmux.conf" "$@"
