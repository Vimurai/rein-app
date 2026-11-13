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
