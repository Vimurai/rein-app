#!/usr/bin/env bash
# E-90 Windows interop: the lookup that now prints every session row (V12 component 9: tag,
# id, path) and the attach child with and without an adopted session id,
# through Windows wsl.exe with the argv WslHost builds, timed. The tmux server is a private
# one: TMUX_TMPDIR reaches the distro through WSLENV — the only difference from the app, which
# removes WSLENV — so the operator's own `rein-app` server is never read or changed. The
# setup rewrites ~/.local/state/rein-app/tmux with the app's unchanged constants (a no-op).
# The attach runs in a ConPTY (ConPtyProbe.ps1, from E-53) as the window starts it; the
# session's pane prints a marker, so the read log shows the tab reached that pane.
set -uo pipefail
unset TMUX TMUX_PANE
REPO="$(git -C "$(dirname "$0")" rev-parse --show-toplevel)"
HERE="$(cd "$(dirname "$0")" && pwd)"
WSL=/mnt/c/Windows/System32/wsl.exe
D="$WSL_DISTRO_NAME"
SRC="$REPO/crates/rein-desktop/src"
P="$(mktemp -d /tmp/e90i.XXXX)"
export TMUX_TMPDIR="$P/t" DISABLE_AUTO_UPDATE=true WSLENV=TMUX_TMPDIR:DISABLE_AUTO_UPDATE # the second: no oh-my-zsh update prompt in the probe
mkdir -p "$TMUX_TMPDIR" "$P/proj" "$P/other" "$P/adopt" "$P/twice"; chmod 700 "$TMUX_TMPDIR"
cleanup() { tmux -L rein-app kill-server 2>/dev/null; rm -rf "$P"; }  # the private socket only
trap cleanup EXIT
# The two scripts exactly as the app builds them (tmux_server.rs).
python3 - "$SRC" "$P" <<'PY'
import re, sys
src, p = sys.argv[1], sys.argv[2]
t = open(src + '/tmux_setup.sh').read().replace('@@SHIM@@\n', open(src + '/tmux_shim.sh').read()).replace('@@CONF@@\n', open(src + '/tmux_app.conf').read())
open(p + '/setup.sh', 'w').write(t)
rs = open(src + '/tmux_server.rs').read()
tail = re.search(r'"(echo \$1 .*?)"\n', rs).group(1)
for a, b in (('{{', '{'), ('}}', '}'), ('{ATTACH_REFUSED}', re.search(r'ATTACH_REFUSED: i32 = (\d+)', rs).group(1)), ('{ATTACH_GONE}', re.search(r'ATTACH_GONE: i32 = (\d+)', rs).group(1))):
    tail = tail.replace(a, b)
start = open(src + '/tmux_start.sh').read().strip()
open(p + '/attach-wsl.txt', 'w').write(start.replace('exec ~/.rein/bin/rein start', 'tty; ' + tail))
PY
A="$(cat "$P/attach-wsl.txt")"
tag() { printf '%s\0%s' "$D" "$1" | sha256sum | cut -d' ' -f1; }
T="$(tag "$P/proj")" O="$(tag "$P/other")" AD="$(tag "$P/adopt")"
ms() { echo $((($(date +%s%N) - $1) / 1000000)); }
lookup() { # lookup <label> <tag>
  local t0 out code
  echo "== lookup: $1"
  echo "\$ wsl.exe -d $D -e sh -s -- ${2:0:12}… < setup script"
  t0=$(date +%s%N)
  out="$("$WSL" -d "$D" -e sh -s -- "$2" <"$P/setup.sh" 2>&1 | tr -d '\r')"
  code=${PIPESTATUS[0]}
  echo "  | $out"
  echo "  exit $code in $(ms "$t0") ms"
  echo
}
echo "date: $(date -u +%FT%TZ)"
echo "wsl.exe: $("$WSL" --version 2>/dev/null | tr -d '\0\r' | head -1); distro: $D; shell: $SHELL; tmux: $(tmux -V)"
echo "attach script: $(printf %s "$A" | wc -c) bytes, quote characters: $(printf %s "$A" | tr -cd "'\"" | wc -c)"
echo
lookup "no server yet → no rows" "$T"
tmux -L rein-app -f /dev/null new-session -d -s proj -c "$P/proj" -x 120 -y 40 "echo E90-TAGGED; sleep 600"
tmux -L rein-app set-option -t proj @rein_app_project "$T"
tmux -L rein-app new-session -d -s old-adopt -c "$P/adopt" -x 120 -y 40 "echo E90-AGENT-PANE; sleep 600"
tmux -L rein-app new-session -d -s old-two-a -c "$P/twice" "sleep 600"
tmux -L rein-app new-session -d -s old-two-b -c "$P/twice" "sleep 600"
lookup "a tagged, an untagged and two untagged of one folder → every row" "$T"
lookup "a word that is not a tag → exit 26" "sweep"
ID="$(tmux -L rein-app display-message -p -t old-adopt '#{session_id}')"
echo "== attach: an adopted id that is gone since the lookup → exit 76, no rein start"
t0=$(date +%s%N)
out="$("$WSL" -d "$D" --cd "$P/adopt" -e "$SHELL" -lic "$A" sh "$AD" '$999' </dev/null 2>&1 | tr -d '\r')"
code=${PIPESTATUS[0]}
printf '%s\n' "$out" | sed 's/^/  | /'
echo "  exit $code in $(ms "$t0") ms"
echo
W="$(wslpath -w "$P")"
for r in 1 2 3; do
  powershell.exe -NoProfile -ExecutionPolicy Bypass -File "$(wslpath -w "$REPO/.ai/screens/E-69")\\ConPtyProbe.ps1" \
    -Cmd "C:\\Windows\\System32\\wsl.exe -d $D --cd $P/adopt -e $SHELL -lic \"$A\" sh $AD $ID" \
    -Out "$W\\attach$r" -Seconds 5 >/dev/null 2>&1
  python3 - "$P/attach$r" "$r" <<'PY2'
import sys, re
b = open(sys.argv[1] + '.bin', 'rb').read(); log = open(sys.argv[1] + '.log').read().splitlines()
def at(off):
    for l in log:
        m = re.match(r'\s*(\d+) ms  read\s+(\d+) bytes at (\d+)', l)
        if m and off >= 0 and int(m[3]) <= off < int(m[3]) + int(m[2]): return int(m[1])
tty = re.search(rb'/dev/pts/\d+', b)
print(f"adopt attach run {sys.argv[2]}: tty line {tty.group().decode() if tty else None}; first tmux frame (1049h) at {at(b.find(b'\x1b[?1049h'))} ms; adopted pane marker at {at(b.find(b'E90-AGENT-PANE'))} ms")
PY2
done
echo "clients now: $(tmux -L rein-app list-clients 2>/dev/null | wc -l) (the probe ended each attach); sessions: $(tmux -L rein-app list-sessions -F '#{session_name}' | tr '\n' ' ')"
echo "rein start run: never (no rein call in the attach script)"
