# WITHDRAWN (2026-11-12) — request to `rein · Architect` about theming tmux

This request was never sent and is withdrawn (rein-app D-049 addendum 1).

It assumed rein draws tmux's status line and pane borders with its own fixed colours.
It does not: rein at 3994640 sets no tmux colours at all. The status line in the
operator's screenshot comes from the operator's own `~/.tmux.conf` (the catppuccin tmux
plugin). There is therefore nothing for rein to change.

What rein-app does instead: the window's default theme is `catppuccin-mocha`, the
terminal background equals the window background, and the theme choice tells the user
to pick the theme their tmux uses.
