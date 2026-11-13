# Request to `rein · Architect` and `rein-plugins · Architect` — data for the window's panels (2026-11-13)

From: `rein-app · Architect`. Not sent yet — no `rein` or `rein-plugins` session was
reachable when this was written. Read with `plans/rein-tmux-server-request.md` and
`plans/rein-window-theme-request.md` (also unsent).

## What the operator decided (rein-app D-056)
The desktop window gets a side menu with data-only panels: Dashboard, Communication,
Tasks and "Needs you" first. A panel is a description (a source and a view from closed
lists); the app reads the data and draws it. Detail: rein-app `.ai/blueprints/panels.md`.

## What rein-app does today, without a rein change
It reads two files of the tab's project, read-only: `.ai/state.json` (`tasks[]`,
`project.focus`) and the tail of `.ai/COMM.md` (handoff headings, `**From**: A → B`,
`**Trigger**`). Neither format is in the contract, so both parsers are provisional and
answer "not available" on anything unexpected, team-mode record files included.

## Asked of rein
1. **A stable read interface for tasks and handoffs**, named in the contract. For
   example `rein state show --json` and `rein comm list --json -n 30`, or a guaranteed
   schema for the files. It must cover team mode.
2. **An "agent is waiting on the operator" signal.** A permission prompt or a question
   in a pane is invisible to the window. For example a pane option (`@rein_waiting`) set
   and cleared by rein's hooks, or a small file per project. This is the one item the
   "Needs you" panel cannot do without rein.
3. **Hosted CI status per task** in the same interface, if rein already holds it
   (`get_ci_status`), so the Dashboard can show it.

## Also asked of rein — project lifecycle (rein-app D-057, `desktop.md` V9)
The operator wants the window to offer set-up for a folder without `.ai/`, offer
`rein sync` / the init upgrade for an out-of-date project, and close the project when
the agents have exited. The window runs only `rein init`, `rein sync` and
`rein start --kill`, each on a click.
- **a. One machine-readable project verdict** (for example `rein doctor --json --scope
  project`): not set up / needs init upgrade / needs sync / ready, each with the command
  that fixes it, and the prerequisites `rein init` does not install.
- **b. Agent liveness per role** in `rein start --status --json`, and, if you agree, the
  pane ending when its agent exits instead of falling back to a shell.
- **c. Restarting one exited agent** without killing the session: is `rein start` on a
  live session meant to do that?
- **c2. A possible bug, read in rein 0.5.0's code by rein-app's Engineer, not run:**
  `rein start` on an existing session has no early return, and its binding loop (about
  line 5509 of `bin/rein`) sends `rein pane <role>` + Enter to every role pane. A pane
  whose agent is still running would get that text typed into the running Claude after
  the 5 s shell wait. Please confirm or refute; if real, skip a pane whose agent is
  alive. (`rein start --status` is prose only: role, account, `pane_current_command`.)

## Also asked of rein — the team (rein-app D-058, `desktop.md` V10)
The window gets a Project sheet that shows `rein team show` / `rein trunk` and runs
`rein team add` and `rein state migrate --team | --legacy` on a click.
- **d. Add `team`, `state migrate`, `trunk` and `task claims` to the contract's stable
  entry points**, each with `--json`.
- **e. Commands to change and remove a developer and to change a range.** rein-app will
  not edit `.ai/team.json` itself.

## Asked of rein-plugins
4. **May a plugin carry panel descriptors** (for example `panels/<id>.json`, schema in
   rein-app `panels.md` §Data Model) next to its agents and skills? They hold no code;
   they name a source and a view the app already has. Does `claude plugin validate
   --strict` accept the extra directory, and should `check-marketplace.mjs` validate
   the descriptors?
5. **Your rule "Rein state only through the MCP servers, never by reading `.ai/`
   files"** is kept: a descriptor never reads anything; the app's sources do, and item 1
   replaces the direct reads.

## What does not change
- rein-app writes no file inside a project and carries no copy of Rein logic.
- A new *source* is always a rein-app release, never something a plugin brings.
