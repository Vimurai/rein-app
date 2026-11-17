#!/usr/bin/env bash
# E-70 Windows interop: restoring three recorded tabs (V12 components 2-5) as the window does —
# for each, the live lookup in the setup process, then (live) the attach child in a ConPTY or
# (not live) nothing: the "not running" card. Adapted from E-69's; the same transport:
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
P="$(mktemp -d /tmp/e70i.XXXX)"
export TMUX_TMPDIR="$P/t" WSLENV=TMUX_TMPDIR
mkdir -p "$TMUX_TMPDIR" "$P/one" "$P/two" "$P/three"; chmod 700 "$TMUX_TMPDIR"
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
T="$(tag "$P/proj")" O="$(tag "$P/other")"
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
echo "record: one (live), two (live), three (its session ended between runs)"
echo
for n in one two; do
  tmux -L rein-app -f /dev/null new-session -d -s "$n" -x 120 -y 40 "echo E70-$n-PANE; sleep 600"
  tmux -L rein-app set-option -t "$n" @rein_app_project "$(tag "$P/$n")"
done
W="$(wslpath -w "$P")"
for n in one two three; do
  T="$(tag "$P/$n")"
  t0=$(date +%s%N)
  ans="$("$WSL" -d "$D" -e sh -s -- "$T" <"$P/setup.sh" 2>/dev/null | tr -d '\r' | tail -n1)"
  lk=$(ms "$t0")
  if [ "$ans" = live ]; then
    powershell.exe -NoProfile -ExecutionPolicy Bypass -File "$(wslpath -w "$HERE")\\ConPtyProbe.ps1" \
      -Cmd "C:\\Windows\\System32\\wsl.exe -d $D --cd $P/$n -e $SHELL -lic \"$A\" sh $T" \
      -Out "$W\\$n" -Seconds 4 >/dev/null 2>&1
    fr=$(python3 - "$P/$n" "E70-$n-PANE" <<'PY'
import sys, re
b = open(sys.argv[1] + '.bin', 'rb').read(); log = open(sys.argv[1] + '.log').read().splitlines()
def at(off):
    for l in log:
        m = re.match(r'\s*(\d+) ms  read\s+(\d+) bytes at (\d+)', l)
        if m and off >= 0 and int(m[3]) <= off < int(m[3]) + int(m[2]): return int(m[1])
f, p = at(b.find(b'\x1b[?1049h')), at(b.find(sys.argv[2].encode()))
print(f"first tmux frame {f} ms, its pane's marker {p} ms")
PY
)
    echo "tab $n: lookup $ans in $lk ms; attach: $fr"
  else
    echo "tab $n: lookup $ans in $lk ms; no process — the not-running card"
  fi
done
echo "sessions afterwards: $(tmux -L rein-app list-sessions -F '#{session_name}' | tr '\n' ' ')(none started)"
