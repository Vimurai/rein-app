# rein-app tmux home setup (desktop.md §Look and feel v2 V8 component 3a, D-055). A constant
# script fed on stdin — `/bin/sh -s` on Linux / macOS, `wsl.exe -d <distro> -e sh -s` on
# Windows — run by the app before it opens a tab's pty, only when tmux.server = "app".
# It makes, with umask 077:
#   $HOME/.local/state/rein-app/tmux/            0700
#   $HOME/.local/state/rein-app/tmux/bin/        0700
#   $HOME/.local/state/rein-app/tmux/bin/tmux    0700  the shim (tmux_shim.sh)
#   $HOME/.local/state/rein-app/tmux/tmux.conf   0600  the app's config (tmux_app.conf)
# and rewrites a file whose bytes differ from the constant (temporary name, then rename), so
# an edited shim does not survive. XDG_STATE_HOME is deliberately not read: the argv start
# script must name the same directory from another shell. The two texts below are inserted
# by the app at build time; no other value is.
# Exit: 0 ready; 20 HOME is not an absolute path of safe characters (space, quote, newline,
# glob…); 21 a directory could not be made; 22 a directory or file is a symlink, or not what
# it should be; 23 not owned by this user; 24 writable by group or others (or not checkable);
# 25 a file could not be written.

LC_ALL=C
export LC_ALL
umask 077
case $HOME in /*) ;; *) exit 20 ;; esac
case $HOME in *[!A-Za-z0-9._/-]*) exit 20 ;; esac
H=$HOME/.local/state/rein-app/tmux

# Every directory from ~/.local down: made if missing, never a symlink, owned by this user.
for d in "$HOME/.local" "$HOME/.local/state" "$HOME/.local/state/rein-app" "$H" "$H/bin"; do
  [ -L "$d" ] && exit 22
  if [ ! -e "$d" ]; then mkdir "$d" 2>/dev/null || exit 21; fi
  [ -L "$d" ] && exit 22
  [ -d "$d" ] || exit 22
  [ -O "$d" ] || exit 23
done
# The parents must not be writable by group or others (someone else could rename the home
# between setup and start); find printing nothing — an error included — refuses.
for d in "$HOME/.local" "$HOME/.local/state" "$HOME/.local/state/rein-app"; do
  [ -n "$(find "$d" -prune ! -perm -020 ! -perm -002 -print 2>/dev/null)" ] || exit 24
done
# The app's own two directories are set, not tested.
chmod 700 "$H" "$H/bin" || exit 24

# put <path> <mode>: move <path>.new into place unless it equals the file already there. A
# target that is not a regular file (a symlink, a directory) is removed first, so the rename
# cannot land through it; a directory cannot be removed this way and fails the check below.
put() {
  chmod "$2" "$1.new" || return 1
  if [ -L "$1" ] || { [ -e "$1" ] && [ ! -f "$1" ]; }; then rm -f "$1" 2>/dev/null; fi
  if [ -f "$1" ] && [ ! -L "$1" ] && [ -O "$1" ] && cmp -s "$1.new" "$1"; then
    rm -f "$1.new"
    chmod "$2" "$1" || return 1
    return 0
  fi
  mv -f "$1.new" "$1"
}

rm -f "$H/bin/tmux.new" || exit 25
cat >"$H/bin/tmux.new" <<'REIN_APP_SHIM_END' || exit 25
@@SHIM@@
REIN_APP_SHIM_END
put "$H/bin/tmux" 700 || exit 25

rm -f "$H/tmux.conf.new" || exit 25
cat >"$H/tmux.conf.new" <<'REIN_APP_CONF_END' || exit 25
@@CONF@@
REIN_APP_CONF_END
put "$H/tmux.conf" 600 || exit 25

# What the start script will use: two regular files of this user's, not symlinks (a
# directory in the shim's place would let the real tmux be found first).
for f in "$H/bin/tmux" "$H/tmux.conf"; do
  [ -L "$f" ] && exit 22
  [ -f "$f" ] || exit 22
  [ -O "$f" ] || exit 23
done

# V11 (D-059, E-66): before the app's first tab on its own server, a server left from an
# earlier run (a crash, a killed window, Windows closed under WSL) is ended, so `rein start`
# never meets a live session. Only with the word `sweep`; only the app's socket, never the
# user's default server.
if [ "${1-}" = sweep ]; then
  unset TMUX TMUX_PANE
  command -v tmux >/dev/null 2>&1 && tmux -L rein-app kill-server >/dev/null 2>&1
fi
exit 0
