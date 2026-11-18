#!/usr/bin/env bash
# tmux-server.sh — E-57 / D-055, desktop.md §Look and feel v2 V8 "Tests (V8)": the window's own
# tmux server. The setup script (src/tmux_setup.sh with the shim and the config put in, as the
# app builds it), the shim (src/tmux_shim.sh), the app config (src/tmux_app.conf) and the
# start constant (src/tmux_start.sh) — at the process boundary they are made for, Linux and
# macOS. New suite: nothing tested a tmux home or a PATH shim before; the V6 overlay keeps
# tests/e2e/tmux-overlay.sh. V12 (D-061, E-69): the setup's live lookup and the attach script
# (built from tmux_server.rs) in every shell present, on the app's socket only.
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
set -g pane-border-format "USER-FORMAT"
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
# E-58: rein's pane roles as labels, from the app's own config, never the user's format.
check "app: pane labels on top" top "$(tm -L rein-app show-options -gv pane-border-status)"
check "app: the label format is the app's" "$(sed -n 's/^set -g pane-border-format "\(.*\)"$/\1/p' "$SRC/tmux_app.conf")" "$(tm -L rein-app show-options -gv pane-border-format)"
check "app: the user's own label format is absent" 0 "$(tm -L rein-app show-options -g pane-border-format | grep -c USER-FORMAT)"
p0="$(tm -L rein-app list-panes -t demo -F '#{pane_id}' | head -n1)"
check "app: a pane without a role shows its index" " 0 " "$(tm -L rein-app display-message -p -t "$p0" '#{E:pane-border-format}')"
tm -L rein-app set-option -p -t "$p0" @rein_role engineer
tm -L rein-app set-option -p -t "$p0" @rein_account personal
check "app: a pane rein named shows its role and account" " engineer · personal " "$(tm -L rein-app display-message -p -t "$p0" '#{E:pane-border-format}')"

e2e user user
check "user: the session is on the default server" demo "$(tm list-sessions -F '#S')"
check "user: the user's option is there" yes "$(tm show-options -gv @user-marker)"
check "user: the user's binding is there" 1 "$(tm list-keys | grep -c user-binding)"
check "user: the user's own label format applies" USER-FORMAT "$(tm show-options -gv pane-border-format)"
for _ in $(seq 1 20); do [ -e "$W/plugin-ran-user" ] && break; sleep 0.1; done
check "user: the user's run-shell ran" yes "$([ -e "$W/plugin-ran-user" ] && echo yes || echo no)"

# ── V12 (D-061, E-69): the live lookup and the attach script reach the app's socket only ──
# A session `demo` on BOTH servers under a private TMUX_TMPDIR; nothing here may end either
# (the standing "never kill the user's tmux" rule; V12 kills nothing).
both() { # both <dir> → a session on the app's socket and one of the same name on the default
  mkdir -p "$1" && chmod 700 "$1"
  TMUX_TMPDIR="$1" "$REAL_TMUX" -L rein-app -f /dev/null new-session -d -s demo 2>/dev/null
  TMUX_TMPDIR="$1" "$REAL_TMUX" -f /dev/null new-session -d -s demo 2>/dev/null
}
alive() { TMUX_TMPDIR="$1" "$REAL_TMUX" ${2:+-L "$2"} has-session -t demo 2>/dev/null && echo yes || echo no; }
TAG=0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef
OTHER=fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210
look() { # look <tmpdir> <word> → "[<rows>] <exit>": each row s|N|T-or-empty|P (E-90: the lookup
  # lists every session of the app's server; the app decides in Rust); NOEND: no end line.
  local out code=0 rows
  out=$(HOME="$h" TMUX_TMPDIR="$1" TMUX=/x /bin/sh -s -- "$2" <"$SETUP" 2>/dev/null) || code=$?
  rows=$(printf '%s\n' "$out" | sed -n '/^sessions$/,/^end$/p' | grep '^s|' | sed -E "s/[$][0-9]+/N/; s/$TAG/T/; s/[|][^|]*$/|P/" | tr '\n' ';')
  printf '%s\n' "$out" | grep -qx end || [ "$code" != 0 ] || rows=NOEND
  echo "[$rows] $code"
}
h="$W/ok"
t="$W/tmp-lc1"
mkdir -p "$t" && chmod 700 "$t"
check "lookup: no server → no rows" "[] 0" "$(look "$t" "$TAG")"
both "$t"
check "lookup: the app's untagged session is listed with its path" "[s|N||P;] 0" "$(look "$t" "$TAG")"
TMUX_TMPDIR="$t" "$REAL_TMUX" set-option -t demo @rein_app_project "$TAG" # the user's server
check "lookup: the DEFAULT server is not listed (its tag is not seen)" "[s|N||P;] 0" "$(look "$t" "$TAG")"
TMUX_TMPDIR="$t" "$REAL_TMUX" -L rein-app set-option -t demo @rein_app_project "$TAG"
check "lookup: the app's session with its tag" "[s|N|T|P;] 0" "$(look "$t" "$TAG")"
check "lookup: the same rows whatever the word (the app decides)" "[s|N|T|P;] 0" "$(look "$t" "$OTHER")"
for bad in sweep "" "${TAG%?}" "${TAG}0" "$(printf %s "$TAG" | tr a-f A-F)" "${TAG%?};"; do
  check "lookup: a word that is not a tag ($(printf %.12s "$bad")) → exit 26" "[] 26" "$(look "$t" "$bad")"
done
check "lookup: setup with no word prints nothing (exit 0)" "|0" "$(out=$(HOME="$h" TMUX_TMPDIR="$t" /bin/sh -s <"$SETUP" 2>/dev/null); echo "$out|$?")"
check "lookup: both servers' sessions survive every lookup" "yes yes" "$(alive "$t" rein-app) $(alive "$t")"
check "lookup: the sweep is gone (no kill-server in the setup)" 0 "$(grep -c kill-server "$SETUP" || true)"
# No tmux: a PATH holding only the tools the setup uses (find is /usr/bin/find on macOS, and
# tmux is /usr/bin/tmux on Linux, so neither /bin nor /usr/bin alone would do).
mkdir -p "$W/notmux"
for c in mkdir find chmod rm cat cmp mv grep; do ln -sf "$(command -v "$c")" "$W/notmux/$c"; done
check "lookup: no tmux → no rows" "sessions end 0" "$(out=$(PATH="$W/notmux" HOME="$h" /bin/sh -s -- "$TAG" <"$SETUP" 2>/dev/null); echo $out $?)"

# The attach script exactly as tmux_server.rs builds it (Linux / macOS form), its last command
# replaced by an echo of the target so no terminal is needed; one real attach under `script`.
ATTACH="$(python3 - "$SRC" <<'PY'
import re, sys
src = sys.argv[1]
rs = open(src + '/tmux_server.rs').read()
tail = re.search(r'"(echo \$1 .*?)"\n', rs).group(1)
for a, b in (('{{', '{'), ('}}', '}'),
             ('{ATTACH_REFUSED}', re.search(r'ATTACH_REFUSED: i32 = (\d+)', rs).group(1)),
             ('{ATTACH_GONE}', re.search(r'ATTACH_GONE: i32 = (\d+)', rs).group(1))):
    tail = tail.replace(a, b)
print(open(src + '/tmux_start.sh').read().strip().replace('exec ~/.rein/bin/rein start', tail))
PY
)"
DRY="${ATTACH%exec tmux attach -t*}echo target \${s#x}"
sid=$(TMUX_TMPDIR="$t" "$REAL_TMUX" -L rein-app display-message -p -t demo '#{session_id}')
shells="sh -c"
command -v bash >/dev/null && shells="$shells|bash -lic"
command -v zsh >/dev/null && shells="$shells|zsh -lic"
IFS='|' read -r -a SHS <<<"$shells"
for shc in "${SHS[@]}"; do
  read -r -a sh_argv <<<"$shc"
  r=$(HOME="$h" TMUX_TMPDIR="$t" "${sh_argv[@]}" "$DRY" sh "$TAG" </dev/null 2>/dev/null | tail -n1)
  check "attach ($shc): the tagged session's id" "target $sid" "$r"
  HOME="$h" TMUX_TMPDIR="$t" "${sh_argv[@]}" "$DRY" sh "$OTHER" </dev/null >/dev/null 2>&1
  check "attach ($shc): no session with the tag → exit 76, never rein start" 76 "$?"
  for bad in sweep "${TAG%?}" "${TAG%?};"; do
    HOME="$h" TMUX_TMPDIR="$t" "${sh_argv[@]}" "$DRY" sh "$bad" </dev/null >/dev/null 2>&1
    check "attach ($shc): a word that is not a tag → exit 75" 75 "$?"
  done
  # E-90: an adoption — the session id as the second word, used as given (checked $ + digits).
  r=$(HOME="$h" TMUX_TMPDIR="$t" "${sh_argv[@]}" "$DRY" sh "$OTHER" "$sid" </dev/null 2>/dev/null | tail -n1)
  check "attach ($shc): an adoption attaches to that id" "target $sid" "$r"
  for bad in '$999' x1 '$1;x'; do
    HOME="$h" TMUX_TMPDIR="$t" "${sh_argv[@]}" "$DRY" sh "$OTHER" "$bad" </dev/null >/dev/null 2>&1
    check "attach ($shc): an id that is gone or not \$N → exit 76" 76 "$?"
  done
done
check "attach: the script names no rein" 0 "$(printf %s "$ATTACH" | grep -c 'rein start' || true)"
# util-linux and BSD `script` differ: run the attach in a detached tmux window instead (its
# PATH spelled out: a PATH with a space, as under WSL, would split the command line).
HOME="$h" TMUX_TMPDIR="$t" "$REAL_TMUX" -L host -f /dev/null new-session -d -x 80 -y 20 \
  "env -u TMUX HOME=$h TMUX_TMPDIR=$t PATH=$(dirname "$REAL_TMUX"):/usr/bin:/bin /bin/sh -c '$ATTACH' sh $TAG"
for _ in $(seq 1 50); do [ "$(TMUX_TMPDIR="$t" "$REAL_TMUX" -L rein-app list-clients -F '#{session_name}' 2>/dev/null)" = demo ] && break; sleep 0.1; done
check "attach: a real client lands on the tagged session" demo "$(TMUX_TMPDIR="$t" "$REAL_TMUX" -L rein-app list-clients -F '#{session_name}' 2>/dev/null)"
TMUX_TMPDIR="$t" "$REAL_TMUX" -L host kill-server 2>/dev/null # the test's own helper socket
check "attach: both servers' sessions are still there" "yes yes" "$(alive "$t" rein-app) $(alive "$t")"

if [ "$fails" -eq 0 ]; then echo "PASS: tmux server"; else echo "$fails failure(s)"; fi
[ "$fails" -eq 0 ]
