# Request to `rein · Architect` — let the hosting window theme tmux (2026-11-12)

From: `rein-app · Architect`. Context: rein-app D-049, `.ai/blueprints/desktop.md`
§Look and feel v2 (V4) and §Open Questions 4. Not sent yet — no `rein` session was
reachable when this was written.

## What the operator asked for
"Make tmux themed so it looks and feels like part of that window."

The desktop window runs `rein start` in one pty; tmux draws the panes, the pane borders
and its status line with rein's own configuration. Today:

1. tmux's status line and borders use fixed colours. The window ships a default theme
   (`rein`) that copies them, but when the user picks another window theme, tmux does not
   follow.
2. The project / session name is shown twice: in the window's tab and in tmux's status
   line.

Contract §3.5 says the app never configures tmux and never names a session, so rein-app
will not run `tmux set` itself.

## Requested contract addition (rein decides the shape)
- **`REIN_HOST=window`** in the environment of `rein start`: rein may then omit status-line
  elements the host already shows (the session-name pill). Everything else stays.
- **`REIN_HOST_THEME`**: a palette for rein's tmux theme. Proposed form — a single line of
  `key=#rrggbb` pairs separated by commas, keys from a closed list (`bg`, `fg`, `accent`,
  `accent2`, `muted`, `border`, `ok`, `warn`, `fail`); rein validates each value against
  `#[0-9a-fA-F]{6}`, ignores unknown keys, and falls back to its own colours on any error.
  A theme *name* instead of a palette is equally fine if rein prefers to own the palettes.
- rein applies it per session at create time and on attach, so re-attaching from a window
  with another theme restyles the session.

## What rein-app will do once this exists
- Pass exactly those two variables to the pty child — on Windows by setting `WSLENV` to
  list only them (today `WSLENV` is removed entirely, D-038 ruling 5).
- Build the palette from the active window theme; values come from the app's validated
  theme table (`#rrggbb` only), never from free text.

## Questions
1. Is an environment variable the right carrier, or should it be a `rein start` flag?
2. Does a session already running keep its look when a plain terminal attaches later
   (should rein restore its own colours on the next attach without the variable)?
3. Which status-line elements may a host ask rein to drop?
