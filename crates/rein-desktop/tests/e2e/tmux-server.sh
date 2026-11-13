#!/usr/bin/env bash
# tmux-server.sh — E-57 / D-055, desktop.md §Look and feel v2 V8 "Tests (V8)": the window's own
# tmux server. The setup script (src/tmux_setup.sh with the shim and the config put in, as the
# app builds it), the shim (src/tmux_shim.sh), the app config (src/tmux_app.conf) and the
# start constant (src/tmux_start.sh) — at the process boundary they are made for, Linux and
# macOS. New suite: nothing tested a tmux home or a PATH shim before; the V6 overlay keeps
# tests/e2e/tmux-overlay.sh.
# Isolation: every HOME is a mktemp dir; every tmux server lives under a private
# TMUX_TMPDIR (TMUX unset, so it cannot point elsewhere); the user's own servers are never
# reached. Needs: tmux >= 3.2, sh; bash and zsh are used when present.
set -uo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
SRC="$(git -C "$HERE" rev-parse --show-toplevel)/crates/rein-desktop/src"
REAL_TMUX="$(command -v tmux)" || { echo "tmux-server: tmux not found" >&2; exit 2; }
W=""
cleanup() {
  [ -z "$W" ] && return
  for d in "$W"/tmp-*; do
    [ -d "$d" ] || continue
    TMUX_TMPDIR="$d" "$REAL_TMUX" -L rein-app kill-server 2>/dev/null || true
    TMUX_TMPDIR="$d" "$REAL_TMUX" kill-server 2>/dev/null || true
  done
  chmod -R u+rwx "$W" 2>/dev/null || true
  rm -rf "$W"
}
trap cleanup EXIT
unset TMUX TMUX_PANE XDG_STATE_HOME
W="$(mktemp -d /tmp/rein-ts.XXXXXX)"
W="$(cd "$W" && pwd -P)" # macOS: /tmp is a symlink, and the setup refuses a symlinked parent

fails=0
ok() { echo "ok   $1"; }
no() { echo "FAIL $1"; fails=$((fails + 1)); }
check() { if [ "$2" = "$3" ]; then ok "$1"; else no "$1 — expected [$2], got [$3]"; fi; }

# The setup script as the app builds it: each marker line replaced by the file's text.
SETUP="$W/setup.sh"
awk -v shim="$SRC/tmux_shim.sh" -v conf="$SRC/tmux_app.conf" '
  $0 == "@@SHIM@@" { while ((getline l < shim) > 0) print l; next }
  $0 == "@@CONF@@" { while ((getline l < conf) > 0) print l; next }
  { print }' "$SRC/tmux_setup.sh" >"$SETUP"
START="$(cat "$SRC/tmux_start.sh")"

home() { # home <name> → a fresh HOME with a stand-in rein that reports what it got
  local h="$W/$1"
  mkdir -p "$h/.rein/bin"
  cat >"$h/.rein/bin/rein" <<'EOF'
#!/bin/sh
echo "REIN $1 TMUX=[${TMUX-unset}] PATH=$PATH"
EOF
  chmod +x "$h/.rein/bin/rein"
  echo "$h"
}
setup() { HOME="$1" /bin/sh -s <"$SETUP" >/dev/null 2>&1; echo $?; }
mode() { if stat -c %a "$1" >/dev/null 2>&1; then stat -c %a "$1"; else stat -f %Lp "$1"; fi; }

echo "tmux under test: $("$REAL_TMUX" -V)"

# ── setup ─────────────────────────────────────────────────────────────────────────────
h="$(home ok)"
H="$h/.local/state/rein-app/tmux"
check "setup: exit 0 on a fresh HOME" 0 "$(setup "$h")"
check "setup: modes 700 / 700 / 700 / 600" "700 700 700 600" "$(mode "$H") $(mode "$H/bin") $(mode "$H/bin/tmux") $(mode "$H/tmux.conf")"
check "setup: the shim is the constant" yes "$(cmp -s "$H/bin/tmux" "$SRC/tmux_shim.sh" && echo yes || echo no)"
check "setup: the config is the constant" yes "$(cmp -s "$H/tmux.conf" "$SRC/tmux_app.conf" && echo yes || echo no)"
echo 'exec /bin/false' >"$H/bin/tmux"
check "setup: an edited shim is rewritten" 0 "$(setup "$h")"
check "  ... back to the constant" yes "$(cmp -s "$H/bin/tmux" "$SRC/tmux_shim.sh" && echo yes || echo no)"
check "setup: run twice, nothing else appears in the home" "bin tmux.conf" "$(ls "$H" | tr '\n' ' ' | sed 's/ $//')"

h="$(home dirshim)"
mkdir -p "$h/.local/state/rein-app/tmux/bin/tmux"
check "setup: a directory in the shim's place is refused (P1)" 22 "$(setup "$h")"
h="$(home linkbin)"
mkdir -p "$h/.local/state/rein-app/tmux" "$W/elsewhere"
ln -s "$W/elsewhere" "$h/.local/state/rein-app/tmux/bin"
check "setup: a symlinked bin is refused" 22 "$(setup "$h")"
h="$(home linkparent)"
mkdir -p "$W/elsewhere2" "$h"
ln -s "$W/elsewhere2" "$h/.local"
check "setup: a symlinked ~/.local is refused" 22 "$(setup "$h")"
h="$(home linkshimdir)"
mkdir -p "$h/.local/state/rein-app/tmux/bin" "$W/decoy"
ln -s "$W/decoy" "$h/.local/state/rein-app/tmux/bin/tmux"
check "setup: a symlink to a directory in the shim's place is replaced, not written through" "0 0" "$(setup "$h") $(ls -A "$W/decoy" | wc -l | tr -d ' ')"
check "  ... and the shim is a regular file again" yes "$([ -f "$h/.local/state/rein-app/tmux/bin/tmux" ] && [ ! -L "$h/.local/state/rein-app/tmux/bin/tmux" ] && echo yes || echo no)"
h="$(home gw)"
mkdir -p "$h/.local/state"
chmod 775 "$h/.local/state"
check "setup: a parent writable by the group is refused" 24 "$(setup "$h")"
mkdir -p "$W/sp ace"
check "setup: a HOME with a space is refused" 20 "$(setup "$W/sp ace")"
for g in 'st*r' 'qu?st' 'br[ack'; do
  mkdir -p "$W/$g"
  check "setup: a HOME with a glob character is refused ($g)" 20 "$(setup "$W/$g")"
done
check "setup: a relative HOME is refused" 20 "$(setup "relative/home")"

# ── shim ──────────────────────────────────────────────────────────────────────────────
h="$W/ok"
H="$h/.local/state/rein-app/tmux"
mkdir -p "$W/fake" "$W/dot"
printf '#!/bin/sh\necho "REAL $*"\n' >"$W/fake/tmux"
printf '#!/bin/sh\necho "DOT"\n' >"$W/dot/tmux"
chmod +x "$W/fake/tmux" "$W/dot/tmux"
shim() { (cd "$W/dot" && HOME="$h" PATH="$1" "$H/bin/tmux" "${@:2}" 2>&1); }
check "shim: adds the app's server and config" "REAL -L rein-app -f $H/tmux.conf new -s x" "$(shim "$H/bin:$W/fake" new -s x)"
check "shim: skips itself, also listed twice" "REAL -L rein-app -f $H/tmux.conf ls" "$(shim "$H/bin:$H/bin:$W/fake" ls)"
check "shim: never a relative PATH entry (.)" "REAL -L rein-app -f $H/tmux.conf ls" "$(shim ".:$H/bin:$W/fake" ls)"
check "shim: -L passes through" "REAL -L mine ls" "$(shim "$H/bin:$W/fake" -L mine ls)"
check "shim: -S passes through" "REAL -S /s ls" "$(shim "$H/bin:$W/fake" -S /s ls)"
check "shim: clustered -uL passes through" "REAL -uLmine ls" "$(shim "$H/bin:$W/fake" -uLmine ls)"
check "shim: -f VALUE is a value, not a choice of server" "REAL -L rein-app -f $H/tmux.conf -f L ls" "$(shim "$H/bin:$W/fake" -f L ls)"
check "shim: -L after the command is the command's" "REAL -L rein-app -f $H/tmux.conf select-pane -L" "$(shim "$H/bin:$W/fake" select-pane -L)"
check "shim: no tmux → 127" 127 "$( (HOME="$h" PATH="$H/bin" "$H/bin/tmux" ls >/dev/null 2>&1); echo $?)"

# ── start constant ────────────────────────────────────────────────────────────────────
check "start: no quote character" 0 "$(printf '%s' "$START" | tr -cd "'\"" | wc -c | tr -d ' ')"
check "start: one line" 1 "$(wc -l <"$SRC/tmux_start.sh" | tr -d ' ')"
for sh in sh bash zsh; do
  command -v "$sh" >/dev/null 2>&1 || { echo "skip $sh (not installed)"; continue; }
  out="$(cd "$W" && env -i HOME="$h" PATH="/usr/bin:/bin" TMUX=/x TMUX_PANE=%1 XDG_STATE_HOME="$W/xdg" "$sh" -c "$START" 2>&1)"
  check "start ($sh): runs rein start with ~ expanded under set -o noglob" "REIN start" "$(echo "$out" | cut -d' ' -f1-2)"
  check "start ($sh): the shim first on PATH, TMUX unset, XDG_STATE_HOME ignored" "TMUX=[unset] PATH=$H/bin:/usr/bin:/bin" "$(echo "$out" | cut -d' ' -f3-)"
  # A HOME holding a glob character that would match a ready home next to it.
  mkdir -p "$W/gl" && cp -R "$h" "$W/gl/hax" && g="$W/gl/h?x"
  check "start ($sh): a glob in HOME is not expanded (exit 126)" 126 "$( (env -i HOME="$g" PATH=/usr/bin:/bin "$sh" -c "$START" >/dev/null 2>&1); echo $?)"
  m="$(home "missing-$sh")"
  check "start ($sh): no shim → exit 126" 126 "$( (env -i HOME="$m" PATH=/usr/bin:/bin "$sh" -c "$START" >/dev/null 2>&1); echo $?)"
done
# zsh with GLOB_SUBST and SH_WORD_SPLIT (a user rc can set them): `set -f` would be NO_RCS
# there and the glob HOME would pass the checks; `set -o noglob` refuses it.
if command -v zsh >/dev/null 2>&1; then
  check "start (zsh GLOB_SUBST SH_WORD_SPLIT): a glob in HOME is not expanded (exit 126)" 126 "$( (env -i HOME="$W/gl/h?x" PATH=/usr/bin:/bin zsh -c "setopt GLOB_SUBST SH_WORD_SPLIT; $START" >/dev/null 2>&1); echo $?)"
fi
h2="$(home linkshim)"
setup "$h2" >/dev/null
H2="$h2/.local/state/rein-app/tmux"
mv "$H2/bin/tmux" "$H2/bin/real" && ln -s real "$H2/bin/tmux"
check "start: a symlinked shim → exit 126" 126 "$( (env -i HOME="$h2" PATH=/usr/bin:/bin sh -c "$START" >/dev/null 2>&1); echo $?)"

# ── end to end: the user's config does not reach the app's server ─────────────────────
e2e() { # e2e <label> <app|user> → the stand-in rein starts a session through plain `tmux`
  local h t
  h="$(home "e2e-$1")"
  t="$W/tmp-$1"
  mkdir -p "$t"
  chmod 700 "$t"
  # The user's config: a marker option, a key binding and a plugin-style run-shell line.
  cat >"$h/.tmux.conf" <<EOF
set -g @user-marker yes
bind-key M-y display-message user-binding
run-shell "touch $W/plugin-ran-$1"
EOF
  cat >"$h/.rein/bin/rein" <<'EOF'
#!/bin/sh
tmux new-session -d -s demo -x 80 -y 24 && echo started
EOF
  chmod +x "$h/.rein/bin/rein"
  local script="$START"
  [ "$2" = user ] && script='unset TMUX TMUX_PANE; exec ~/.rein/bin/rein start'
  [ "$2" = app ] && { setup "$h" >/dev/null || echo "setup failed" >&2; }
  (cd "$W" && env -i HOME="$h" PATH="$(dirname "$REAL_TMUX"):/usr/bin:/bin" TMUX_TMPDIR="$t" sh -c "$script" >/dev/null 2>&1)
  E2E_T="$t" E2E_H="$h"
}
tm() { TMUX_TMPDIR="$E2E_T" HOME="$E2E_H" "$REAL_TMUX" "$@" 2>/dev/null; }

e2e app app
check "app: the session is on the rein-app socket" demo "$(tm -L rein-app list-sessions -F '#S')"
check "app: no session on the default server" "" "$(tm list-sessions -F '#S')"
check "app: the user's option is absent" "" "$(tm -L rein-app show-options -gv @user-marker)"
check "app: the user's binding is absent" 0 "$(tm -L rein-app list-keys | grep -c user-binding)"
check "app: the user's run-shell did not run" no "$([ -e "$W/plugin-ran-app" ] && echo yes || echo no)"
check "app: the app's config is in force (status off)" off "$(tm -L rein-app show-options -gv status)"

e2e user user
check "user: the session is on the default server" demo "$(tm list-sessions -F '#S')"
check "user: the user's option is there" yes "$(tm show-options -gv @user-marker)"
check "user: the user's binding is there" 1 "$(tm list-keys | grep -c user-binding)"
for _ in $(seq 1 20); do [ -e "$W/plugin-ran-user" ] && break; sleep 0.1; done
check "user: the user's run-shell ran" yes "$([ -e "$W/plugin-ran-user" ] && echo yes || echo no)"

if [ "$fails" -eq 0 ]; then echo "PASS: tmux server"; else echo "$fails failure(s)"; fi
[ "$fails" -eq 0 ]
