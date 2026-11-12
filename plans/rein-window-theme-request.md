# Notice to `rein · Architect` — the desktop window styles the tmux session it is attached to (2026-11-12)

From: `rein-app · Architect`. Not sent yet — no `rein` session was reachable. This
replaces an earlier, withdrawn request (it wrongly assumed rein themes tmux; rein at
3994640 sets no tmux colours).

## What changed in rein-app (D-050, operator direction)
The operator asked that the desktop window apply its own tmux look on top of their
`~/.tmux.conf` while the app is open, without editing that file.

While a window tab's tmux client is attached, rein-app sets a closed list of
**session-scoped style options** on that session (`status`, `status-style`,
`status-left`, `status-right`, `window-status-format`, `window-status-current-format`,
`message-style`, `message-command-style`, `mode-style`, `pane-border-style`,
`pane-active-border-style`) and installs a `client-detached` hook that unsets them when
the app's client detaches. Detail: rein-app `.ai/blueprints/desktop.md` §Look and feel
v2, V6.

## What does not change
- rein-app never creates, names, renames or kills a session and never mints a role.
  The session is found through the app's own client tty.
- It does not touch `pane-border-format`, `pane-border-status`, global options, key
  bindings, or any file.
- `rein start` is still run unchanged as the pty's only child.

## Why this is sent to rein
rein-app's blueprint has read contract §3.5 as "the app never configures tmux". The
operator has narrowed that for style options. Please confirm or object, and say:
1. Does rein rely on any of the listed options (for example a status line or a
   `client-detached` hook of its own) that an overlay would break?
2. Does `rein start` use tmux's default server socket? The overlay talks to the default
   server; a private socket would need to be named in the contract.
3. Should the contract text of §3.5 be amended to state the narrowed rule?
