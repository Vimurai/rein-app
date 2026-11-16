#!/usr/bin/env bash
# E-67 Windows interop: the WSL time limit lives inside Linux (desktop.md V10 "As built and
# reviewed", D-060). The EXACT constant scripts — ADD_SCRIPT from team.rs and the migrate script
# from project_setup.rs, read from the source, not copied — run through Windows wsl.exe with the
# argv WslHost builds. `rein` is a stand-in that starts a background child and then sleeps far
# past the limit, recording both pids. Each run must return 124 before the host's limit
# (add 60 s, migrate 300 s), and both pids must be gone from `ps` when it does.
# WSL resets HOME, so the run carries ZDOTDIR (WSLENV, this run only); that .zshrc points HOME
# at the stand-in and leaves a marker showing the login shell was zsh.
set -uo pipefail
WSL=/mnt/c/Windows/System32/wsl.exe
D="$WSL_DISTRO_NAME"
REPO="$(git -C "$(dirname "$0")" rev-parse --show-toplevel)"
SRC="$REPO/crates/rein-desktop/src"
SH="$(getent passwd "$(id -un)" | cut -d: -f7)"
ADD="$(sed -n 's/^pub const ADD_SCRIPT: &str = "\(.*\)";$/\1/p' "$SRC/team.rs")"
MIG="$(sed -n '/^ *Self::MigrateTeam => {$/{n;s/^ *"\(.*\)"$/\1/p}' "$SRC/project_setup.rs")"
[ -n "$ADD" ] && [ -n "$MIG" ] || { echo "scripts not found in the source" >&2; exit 2; }
W="$(mktemp -d /tmp/reine67XXXXXX)"
cleanup() { [ -f "$W/pids" ] && kill $(cat "$W/pids") 2>/dev/null; rm -rf "$W"; }
trap cleanup EXIT
mkdir -p "$W/home/.rein/bin" "$W/proj"
cat >"$W/home/.rein/bin/rein" <<EOF
#!/bin/sh
sleep 600 &
echo "\$\$ \$!" >"$W/pids"
echo "rein stand-in \$\$ started child \$! (argv: \$*)"
sleep 600
EOF
chmod +x "$W/home/.rein/bin/rein"
printf 'echo "zsh rc read (ZSH_VERSION=$ZSH_VERSION)" >>"%s/rc.log"\nHOME="%s/home"\n' "$W" "$W" >"$W/home/.zshrc"
export WSLENV=ZDOTDIR ZDOTDIR="$W/home"
alive() { local n=0; for p in $(cat "$W/pids" 2>/dev/null); do ps -p "$p" >/dev/null 2>&1 && n=$((n + 1)); done; echo "$n"; }
case_run() { # case_run <label> <host limit s> <script> [args…]
  local label=$1 host=$2 script=$3; shift 3
  rm -f "$W/pids" "$W/rc.log"
  echo "== $label (host limit ${host} s)"
  local t0; t0=$(date +%s%N)
  "$WSL" -d "$D" --cd "$W/proj" -e "$SH" -lic "$script" "$@" </dev/null >"$W/out" 2>&1
  local code=$? ms=$((($(date +%s%N) - t0) / 1000000))
  sleep 0.5
  echo "  exit $code after $ms ms (host limit $((host * 1000)) ms: $([ "$ms" -lt $((host * 1000)) ] && echo inside || echo PAST))"
  sed 's/^/  | /' "$W/rc.log" 2>/dev/null
  tr -d '\r' <"$W/out" | sed 's/^/  > /'
  echo "  stand-in and child pids: $(cat "$W/pids" 2>/dev/null); still in ps: $(alive)"
  echo
}
echo "date: $(date -u +%FT%TZ)"
echo "wsl.exe: $("$WSL" --version 2>/dev/null | tr -d '\0\r' | head -1); distro: $D; login shell: $SH"
echo "timeout in WSL: $("$WSL" -d "$D" -e sh -c 'timeout --version | head -1' </dev/null 2>/dev/null | tr -d '\r')"
echo "WSL + login rc start-up (wsl.exe -e $SH -lic true), 5 runs:$(for i in 1 2 3 4 5; do s=$(date +%s%N); "$WSL" -d "$D" -e "$SH" -lic true </dev/null >/dev/null 2>&1; printf ' %sms' $((($(date +%s%N) - s) / 1000000)); done)"
echo "wsl.exe exit passthrough: $(for c in 96 124 137; do "$WSL" -d "$D" -e sh -c "exit $c" </dev/null >/dev/null 2>&1; printf '%s->%s ' $c $?; done)"
echo
echo "ADD_SCRIPT: $ADD"
echo "migrate --team script: $MIG"
echo
case_run "team add past the limit (timeout -k 3 55)" 60 "$ADD" rein-app dev@example.com E:1-2
case_run "state migrate --team past the limit (timeout -k 5 290)" 300 "$MIG"
