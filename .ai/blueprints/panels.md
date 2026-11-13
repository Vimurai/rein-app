# Blueprint — Side menu and panels (v1 · 2026-11-13, D-056)

> Architect-owned. Operator direction 2026-11-13: a side menu in the window with
> data-only panels; first Dashboard, Communication, Tasks and "Needs you". Later panels
> may come from the `rein-plugins` repo. Read with `desktop.md` (the window) — this file
> adds to it and changes none of its rules.

## Goal & Architecture
The operator learns what a project's Triad is doing by reading two terminal panes. The
side menu shows the same facts as plain panels next to the terminal: what is happening,
whose turn it is, which tasks are open, and what is waiting on the operator. One
sentence: **a narrow rail on the window's left edge opens one panel at a time beside the
terminal; a panel is a description (data), drawn by the app from a closed list of data
sources.**

Words. "Panel" is the unit in this file and in the UI ("Panels" in Settings). It is not
a Claude Code plugin (what `rein-plugins` ships today: agents and skills) and not a tmux
plugin (deferred in `desktop.md` V8).

## Core Concept
A **panel descriptor** names a **source** (what data) and a **view** (how it is laid
out), both from closed lists compiled into the app. A descriptor holds no code, no HTML,
no URL and no file path. The app reads the data, the app draws it. The four first panels
are descriptors compiled into the app; loading descriptors from `rein-plugins` is a later
revision that needs a contract answer (see "Deferred").

## Components
1. **Rail** — a 40 px column at the window's left edge, below the tab bar: one icon
   button per enabled panel, in descriptor order, a count badge on "Needs you", tooltip
   with the panel's title and key. Click, or the panel's action, opens that panel; the
   same click again closes it. `[panels] rail = "shown" | "hidden"` (default `shown`);
   hidden, panels still open from the menu and keys.
2. **Panel host** — one panel at a time, left of the terminal, 360 px wide (320–560 px
   by dragging its edge; width kept in `ui.json`). Opening or resizing it resizes the
   terminal through the existing fit path (one pty resize after the drag ends, not per
   pixel). Header: title, project name, "updated <age>", refresh, close. `Esc` in the
   panel returns focus to the terminal and leaves the panel open. Below 900 px window
   width the panel lies over the terminal instead of beside it. Motion follows V7 tokens
   (slide `--motion-base`, none under reduced motion); a first load longer than
   `--loader-delay` shows the V7 `Skeleton`.
3. **Descriptor registry** — the list of descriptors (schema below), each validated
   against the closed schema at build time (a test) — and, later, at load time. An
   unknown source, view, icon or key is a refusal of that descriptor, never a guess.
4. **Sources** (Rust, closed list; one IPC command) — read-only, per project:
   - `tasks`: `.ai/state.json` → `tasks[]`: id, owner, status, tier, description,
     blocked_by, updated_at.
   - `handoffs`: `.ai/COMM.md` → the last 30 entries: timestamp, from, to, trigger, and
     the entry's sections as plain text.
   - `summary`: from the two above plus `state.project.focus`: counts by status, the
     task in progress, the last handoff (who → who, trigger, age), "whose turn" (the
     last handoff's `to`). Health and tmux server come from what the app already holds.
   - `attention`: see component 6.
   A file that is missing, too large, not a regular file or not in the expected shape
   gives a typed "not available" with a fixed reason — never a partial guess. A team-mode
   project (one record file per task, no `tasks[]` in `state.json`) is "not available:
   this project keeps tasks in a form this version does not read".
5. **Views** (web side, closed list): `summary` (key facts as rows and count chips),
   `list` (grouped rows with a detail drawer), `timeline` (entries newest first, each
   collapsible). All text from a source is put on the page as text nodes only.
6. **Attention ("Needs you")** — across every open tab, a list of things only the
   operator can move, each with the project and a "go to tab" action:
   - app's own state (reliable): a tab whose process ended, a tmux setup that refused,
     health `warn` / `fail`, an update available;
   - `BLOCKED` tasks, and "no handoff for more than 30 minutes while a task is in
     progress";
   - a terminal bell received in a tab that does not have focus;
   - "the last handoff mentions the operator": lines of its "Next action needed" section
     holding the word operator — shown as a quote, labelled as a mention, not a fact.
   **Known gap:** an agent waiting at a permission prompt is not visible to the app
   today. It needs a signal from rein (request in `plans/rein-panels-request.md`); until
   then the bell item is the only hint, and only if the user's Claude Code rings it.
7. **The four descriptors**: `dashboard` (source `summary`, view `summary`),
   `communication` (`handoffs`, `timeline`, "whose turn" pinned on top), `tasks`
   (`tasks`, `list`; groups In progress, Open, Blocked, Done — last 10), `attention`
   (`attention`, `list`).

## Data Model
- **Descriptor** (JSON, schema 1):
  `{schema: 1, id, title, icon, source, view, options}` — `id` `[a-z][a-z0-9-]{1,31}`;
  `title` ≤ 40 characters, shown as text; `icon` a name from the app's bundled icon list;
  `source` and `view` from the closed lists; `options` a closed set per view (group
  order, row limit ≤ 200). Nothing else is accepted.
- **Settings**: `[panels] rail`, `[panels] enabled = [ids]` (default: all four) in
  `config.toml` and Settings (one switch per panel); `ui.json`: last open panel per
  window and the panel width. Both under the existing closed key lists.
- **Source results** are typed structs with hard caps: 500 tasks, 30 handoffs, 8 KiB of
  text per handoff, 200 attention items; a field over its cap is cut with a marker.
- Nothing is written to a project. No panel data is stored between runs.

## API / Interface Contracts
- **One new IPC command**: `panel_data(tab_id, source)` → `{ok: <typed data>,
  read_at}` or `{unavailable: <fixed reason>}`. `source` is the closed enum; the project
  is the tab's own, taken from app state — the web side never sends a path.
- **Host** gains `read_project_file(project, which)` where `which` is a closed enum
  (`StateJson`, `CommMd`) mapped to the two constant relative paths. Linux / macOS: a
  direct read. Windows: through the distro's UNC path the app already derives, or
  `wsl.exe` with a constant script if the UNC read cannot meet the constraints below
  (Engineer measures and reports; no file text on argv either way — V8's rule).
- **Actions** (both input styles, menu rows, pointer parity as D-034): `panel.toggle
  <id>`, `panel.close`, `panel.refresh`, `panel.focus`. Simple keys `Ctrl+Shift+1…4`;
  modal `Space p` then the panel's digit.
- **Errors**: every refusal is one of the app's fixed sentences (V7's rule); text from
  a file never appears in an error.

## Security
- **A project's `.ai/` files are untrusted text.** Agents write them and a cloned repo
  can ship them. They are rendered as text nodes only: no HTML, no Markdown-to-HTML, no
  link that opens, no image. CSP is unchanged; no new origin, no network.
- **Read scope** is two constant relative paths under the tab's project. Before a read:
  the target must be a regular file (not a symlink, device or FIFO), at most 8 MiB for
  `state.json`; `COMM.md` is read from its last 512 KiB only. A larger or odd file is
  "not available".
- **Parsing** is bounded (caps above), never recursive on input depth beyond serde's
  default limit, and never evaluates anything. A hostile `state.json` or `COMM.md` (very
  long lines, control characters, bidi overrides, a million headings) is a test input;
  control and bidi-override characters are shown as visible placeholders.
- **Descriptors** are compiled in for this revision. No descriptor, icon or source is
  fetched. The loader for outside descriptors is not built here.
- **"Go to tab"** only switches tabs. No panel action runs a command, writes a file or
  types into the terminal.
- Tier 3 for the source task (new file-read IPC, Windows path handling) with
  `security_engineer`; Tier 2 for the rail, host and views.

## Execution Constraints
- A source is read only while its panel is open and the window is visible, plus
  `attention`, which the badge needs: every 5 s a stat of the two files per open tab;
  a read and parse only when size or mtime changed. No watcher threads, no polling when
  the window is hidden or minimised.
- A read and parse of a 2 MiB `state.json` and the 512 KiB tail of `COMM.md` finishes
  within 100 ms on Linux and 500 ms on Windows, off the UI thread; never on the pty's
  read path. Typing latency in the terminal is unchanged with a panel open (measured as
  E-53 measured the first frame).
- An idle window with a closed panel runs no animation and no timer except the 5 s
  attention stat (V7's rule, restated).

## As built — sources (E-59, e527254; accepted 2026-11-13)
- `Host::read_project_file(project, which, read)`; `read = false` is a stat only, for
  the change check (size, mtime, inode, ctime).
- Linux / macOS read through directory file descriptors only (project → `.ai` no-follow
  → the file no-follow, then `fstat` of the open file). A check-then-open by path is not
  acceptable here: `security_engineer` exploited it before the first write.
- Windows reads the distro's UNC path, no process (21–32 ms against about 155 ms per
  `wsl.exe` read). `.ai` and the file must not be reparse points (a WSL symlink, FIFO
  and socket all appear as one); path segments ending in `.` or a space are refused.
  **Accepted residual:** no final-path check on the open handle (it needs `unsafe`,
  which the workspace forbids); what is left is a same-user swap of `.ai` inside WSL.
- More than 5000 tasks → not available; the newest 500 are kept. A reply is at most
  1 MiB; `Tasks.cut` / `Handoffs.cut` mark dropped rows. The 8 KiB per handoff is
  counted in bytes, section names included, after placeholders are put in.
- **Owed with E-60:** a test that runs the Windows read on `windows-latest` (a regular
  file read; a junction in place of `.ai` refused; a trailing-dot segment refused), and
  a timing test whose bound is the blueprint's, not ten times it.

## Tests
- Descriptor schema: the four pass; unknown source / view / icon / extra key refused.
- Parsers (Rust, fixtures from this repo's own `.ai/` plus hostile inputs): tasks by
  status; team-mode shape → unavailable; handoff headings, `From → To`, sections; caps;
  control and bidi characters; a 20 MiB file refused without being read whole.
- Read scope: symlink, FIFO and directory in place of each file → unavailable.
- Web: text only (a `<script>` and a Markdown link in a handoff appear as literal text);
  attention rules as a pure function of tab states and source results; pointer parity
  and both key tables for the new actions; reduced motion.
- End to end (Linux, macOS in CI): open a panel on a fixture project, change
  `state.json`, the list updates within 6 s; hidden window → no read.
- Windows: interop output for the file read through the real `wsl.exe` path with timing
  (standing rule for anything on the `WslHost` path).
- Screens gate (D-052): each of the four panels in both themes at 1280×800, the rail
  with a badge, the narrow-window overlay, the "not available" state.

## Rollback Plan
`[panels] enabled = []` and `rail = "hidden"` remove the feature for a user without a
rebuild. Each task is one commit that reverts cleanly; the sources task adds one IPC
command and one Host method and changes no existing one. Nothing is written to a
project or to the Rein home, so there is nothing to clean up.

## Deferred
- **Descriptors from `rein-plugins`.** That repo is a Claude Code plugin marketplace with
  its own validation; whether a plugin may carry panel descriptors, how they are pinned
  and checked, and who may add a *source* are contract questions
  (`plans/rein-panels-request.md`). A new panel from outside can only combine existing
  sources and views; a new source is always an app release.
- **A stable read interface from rein** for tasks and handoffs (JSON), including team
  mode, to replace reading `.ai/state.json` and `.ai/COMM.md` directly — these formats
  are not in the contract today, so the two parsers are provisional and fail soft.
- **Permission-prompt signal**, CI status, cost and context, screens gallery, reviews,
  decisions, releases, activity log — later panels; each needs a source.

## E-## Task Breakdown
1. **E-59 (Tier 3)** — sources: `read_project_file`, the two parsers, `summary`,
   `panel_data`; all caps and refusals; `security_engineer`; Windows interop output.
2. **E-60 (Tier 2)** — rail, panel host, descriptor registry, the three views, the
   Dashboard / Communication / Tasks descriptors, settings, actions and keys; screens.
3. **E-61 (Tier 2)** — attention: the rules across tabs, the bell, the badge, the "Needs
   you" descriptor; screens.
