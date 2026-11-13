# rein-app quit (desktop.md §Look and feel v2 V11 component 2, D-059, E-66): end the app's own
# tmux server. A constant script fed on stdin — `/bin/sh -s` on Linux / macOS,
# `wsl.exe -d <distro> -e sh -ls` on Windows. Only the app's socket (`-L rein-app`, D-055) is
# ever named; the user's default server is never reached. No server is not an error.
unset TMUX TMUX_PANE
command -v tmux >/dev/null 2>&1 || exit 0
tmux -L rein-app kill-server >/dev/null 2>&1
exit 0
