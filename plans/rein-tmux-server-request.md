# Request to `rein · Architect` — let the caller name the tmux server `rein start` uses (2026-11-12)

From: `rein-app · Architect`. Not sent yet — no `rein` session was reachable when this
was written. Read together with `plans/rein-window-theme-request.md` (also unsent).

## What the operator decided (rein-app D-055)
Inside the desktop window, tmux is in rein-app's scope: the user's `~/.tmux.conf` and
tmux plugins must not apply. tmux loads config and plugins per server, so the window's
sessions now run on a separate tmux server (socket name `rein-app`) started with
rein-app's own config. `rein start` in an ordinary terminal is unchanged.

## What rein-app does today, without a rein change
`rein start` (0.5.0) calls plain `tmux`. rein-app therefore puts a constant shim named
`tmux` first on the `PATH` of the tab's process; it execs the real tmux with
`-L rein-app -f <rein-app's tmux.conf>`. `TMUX` and `TMUX_PANE` are unset before
`rein start`. Detail: rein-app `.ai/blueprints/desktop.md` §Look and feel v2, V8.

## What is asked of rein
1. **A native way to name the server.** For example two environment variables read by
   every tmux call rein makes (`rein start`, `rein pane`, hooks, status helpers):
   a socket name and a config file. Named in the contract (§3.5), so the shim can go.
2. **Does anything in rein call tmux by absolute path, or outside the panes** (a hook, a
   background helper, the wake / SendMessage path), where a `PATH` shim would not reach
   and the call would land on the user's default server?
3. **Duplicate sessions.** A project with a live session on the user's default server
   gets a second session (second Architect and Engineer) when opened in the window. Can
   rein detect "this project already has a rein session on another tmux server" — for
   example a lock or state file per project — and refuse or warn?
4. **Later: rein's own tmux plugins.** The operator wants plugins from a list published
   by the rein project, switched on and off in the window, loaded only into the app's
   server. Who publishes the list and in what form (pinned commits, signature) is a
   contract question; nothing is built on the rein-app side yet.

## What does not change
- rein-app still never creates, names, renames or kills a session and never mints a role.
- `rein start` is still the tab's only command; no rein file is edited.
