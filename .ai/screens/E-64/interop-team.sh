#!/usr/bin/env bash
# E-64 Windows interop: `rein team add` from the window on Windows, through Windows wsl.exe with
# exactly the argv WslHost::rein_team_add builds:
#   wsl.exe -d <distro> --cd <project> -e <login shell> -lic <ADD_SCRIPT> rein-app <email> <range>
# ADD_SCRIPT is read from team.rs (the constant itself, not a copy). `rein` is a stand-in in a
# throwaway HOME that records its argv one word per line. WSL resets HOME, so the run carries
# ZDOTDIR (WSLENV, this run only); that .zshrc points HOME at the stand-in, is hostile (IFS,
# word splitting, glob substitution) and leaves a marker showing the login shell was zsh.
# Part 1: values the Rust check accepts arrive as one word each.
# Part 2: values the Rust check refuses (no process is started for them in the app — see the
#         validation-table test below) sent straight to the script anyway: its own re-check
#         refuses each one before `rein` runs.
set -uo pipefail
WSL=/mnt/c/Windows/System32/wsl.exe
D="$WSL_DISTRO_NAME"
REPO="$(git -C "$(dirname "$0")" rev-parse --show-toplevel)"
SH="$(getent passwd "$(id -un)" | cut -d: -f7)"
SCRIPT="$(sed -n 's/^pub const ADD_SCRIPT: &str = "\(.*\)";$/\1/p' "$REPO/crates/rein-desktop/src/team.rs")"
[ -n "$SCRIPT" ] || { echo "ADD_SCRIPT not found in team.rs" >&2; exit 2; }
W="$(mktemp -d /tmp/reine64XXXXXX)"
trap 'rm -rf "$W"' EXIT
mkdir -p "$W/home/.rein/bin" "$W/proj"
cat >"$W/home/.rein/bin/rein" <<EOF
#!/bin/sh
{ echo "argc=\$#"; for a in "\$@"; do printf '  [%s]\n' "\$a"; done; } >>"$W/argv.log"
EOF
chmod +x "$W/home/.rein/bin/rein"
cat >"$W/home/.zshrc" <<EOF
echo "zsh rc read (ZSH_VERSION=\$ZSH_VERSION)" >>"$W/argv.log"
HOME="$W/home"
IFS=.@+:-
setopt shwordsplit globsubst
EOF
export WSLENV=ZDOTDIR ZDOTDIR="$W/home"
run() { # run <email> <range>
  : >"$W/argv.log"
  "$WSL" -d "$D" --cd "$W/proj" -e "$SH" -lic "$SCRIPT" rein-app "$1" "$2" </dev/null >/dev/null 2>&1
  local code=$?
  echo "  exit $code"
  sed 's/^/  | /' "$W/argv.log"
}
echo "date: $(date -u +%FT%TZ)"
echo "wsl.exe: $("$WSL" --version 2>/dev/null | tr -d '\0\r' | head -1); distro: $D; login shell: $SH"
echo "ADD_SCRIPT: $SCRIPT"
echo
echo "== 1. accepted values: one argv word each"
for v in "a.b+c@x-y.io P:425-524" "dev_1@example.com E:1-99999"; do
  set -- $v
  echo "\$ ... rein-app '$1' '$2'"
  run "$1" "$2"
done
echo
echo "== 2. refused values sent straight to the script: rein never runs"
while IFS='|' read -r e r why; do
  echo "\$ ... rein-app '$e' '$r'   # $why"
  run "$e" "$r"
done <<'EOF'
-x@example.com|E:1-2|leading dash (an option)
a b@example.com|E:1-2|space
a"b@example.com|E:1-2|double quote
a$(id)@example.com|E:1-2|command substitution
a;id@example.com|E:1-2|semicolon
a*@example.com|E:1-2|glob
a@b@example.com|E:1-2|two @
a@example.com|E:1-2 --planner|space in the range
a@example.com|X:1-2|prefix not E or P
a@example.com|E:1-2-3|two dashes
a@example.com|E:1-2E|letter after the range (security review finding 1)
EOF
echo
echo "== Rust: the same values are refused before any process (team::tests::the_validation_table)"
(cd "$REPO" && cargo test -p rein-desktop team::tests::the_validation_table 2>&1 | grep -E "^test result|running")
