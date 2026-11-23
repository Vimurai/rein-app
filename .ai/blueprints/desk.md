# Blueprint — The Desk: one page where the operator works (v1 · 2026-11-23, D-074)

> Architect-owned. Operator, 2026-11-23, on Home and the four side panels: "I don't find
> our dashboard usable, readable, and ugly at the same time; it's not interactive, unclear
> … that goes for all modules. We need to make practical use of those modules, not just
> some text to read which no one will ever read. It should have use cases."
> Asked, the operator chose: all four use cases below; **one Desk page**; and named all
> four faults of the look (no hierarchy, too much text, unfinished, unclear words).
>
> **This file replaces** `panels.md` components 1, 2, 5, 7 (rail, panel host, views, the
> four descriptors) and `status.md` "Home" H2–H4 and "Home, second revision" R3–R5 as the
> description of what the window draws. **It keeps** every source, read rule, cap and
> security rule of `panels.md`, the status model and notifications of `status.md`, the
> waiting signal (W1–W7), `pane_focus` (R6) and `pane_send` (R7). Where files differ on
> what is drawn, this file wins; where they differ on what is read or sent, they win.

## Goal & Architecture
The operator runs an Architect and an Engineer in several projects and comes to the
window to do four things. Each is a use case with a start, an action and an end:

| # | The operator wants to | Today | On the Desk |
|---|---|---|---|
| U1 | **Answer an agent that waits** | finds the tab, finds the pane, reads the screen | a row "rein-app · Engineer asks for approval · 2 min" with **Answer**: one click puts that pane in front, enlarged; answering it clears the row |
| U2 | **Give the next order** | types into a pane | buttons on a lane and on a task: **Continue**, **Message…**, **Interrupt**, **Run the queue**, **Start**, **Review**, **Plan…** |
| U3 | **Do what only they can do** | reads handoffs to find lines addressed to them | a checklist of those lines and of the app's own facts (update, set-up, a stopped project), each with its button; **Done** ticks a line off |
| U4 | **See what changed since they looked** | reads three panels | a feed of finished tasks, failed reviews and handoffs, new ones marked, one click to the full text |

One sentence: **the Desk is the Home page rebuilt as a work surface — an inbox, the
agents with their controls, the tasks as a board and a feed of changes, for every open
project at once — and the four text panels beside the terminal become one narrow strip
of the same parts for the project in front.**

What can be built now and what waits (stated so nobody is surprised):
- U3, U4, the board, the look and the strip need nothing from outside. They are built
  first (E-106..E-109).
- U1 and U2 need the app to know that an agent waits or is at an ordinary prompt. That
  fact is rein's waiting signal (rein E-433, built on a rein branch, not on rein's main
  on 2026-11-23). Until it merges, a lane says only "running" or "ended" and offers only
  **Open**. E-81 → E-103 → E-104 stand as registered; E-110 and E-111 follow them.
- "Allow / Deny on the row itself" needs the text of the ask, which rein does not
  publish and the app does not read from the screen (D-062). Asked of rein in
  `plans/rein-desk-request.md`. Until then **Answer** takes the operator to the prompt.

## Core Concept
**Every row is a thing with one state, one sentence and one button.** A row that offers
nothing to do is not on the Desk; text longer than one line lives behind the row, in the
reader. The core decides what the rows are and which action each one offers (a closed
list); the web side draws them and sends back only a row's key and the action's name.

Four **sections**, each a list of rows from one core structure, drawn by one component
that is used twice: wide on the Desk (all projects, or one chosen by a scope chip) and
narrow in the strip (the project in front).

## Components

### 1. Desk source (core) — `desk`
One new source in the closed list of `panel_data`. It reads no new file: `state.json`
and `COMM.md` through the existing `read_project_file`, plus the tab's status. It gives:
- **`for_you`** — the inbox rows, built from the existing attention rules, with these
  changes:
  - **T1, to-do lines.** Take the newest handoff *from each sender* among the 30 read.
    In its "Next action needed": a line addressed to the operator by rule R1
    (`status.md`) is one row with the text after the address. If that line has no text
    after the address (it ends at the colon or the emphasis), every list line directly
    under it that is indented deeper is one row. Text cleaned as R2, cut at 120
    characters for the row; the whole line is in the reader. Rows with the same cleaned
    text are one row. At most 20.
  - **T2, failed review.** From `state.json` `stamps` (the newest 300): for a task, the
    newest stamp of a family (`ACCEPT`, `ARCH`, `SEC`, `TESTS`, `ALIGN`, `CLEAN`,
    `VIBE`) is `…_FAIL`, it is under 14 days old, and the task is not `DONE` with a
    later `ACCEPT_PASS`. One row per task: "<id>: <family> review failed".
  - "The last handoff mentions the operator" (the quote row) is removed: T1 replaces it.
  - Blocked tasks are no longer inbox rows (they are a board column).
  - Order: `waiting`, `ended`, `setup`, `review_failed`, `stalled`, `bell`, `todo`,
    `update`, `outdated`, `health`; inside a kind, oldest first.
- **`board`** — `next` (OPEN with every dependency done), `doing` (IN_PROGRESS),
  `blocked` (BLOCKED, or OPEN with an open dependency; with `waits_on`, at most three
  ids), `done` (the newest five), and the four full counts. A card is id, title (the
  existing task-title rule, 90 characters), tier, age.
- **`activity`** — events, newest first, at most 50, no older than 14 days:
  `task_done` (from `completed_at`; the first sentence of `summary`), `review_failed`
  and `accepted` (stamps `…_FAIL`, `ACCEPT_PASS`), `handoff` (from → to, the trigger).
  Each has `new` = it is later than this project's "seen" time.
- **Seen time** — one timestamp per project path in the app's own `ui.json`
  (`desk_seen`, at most 64 projects, oldest dropped). Set by `desk_seen(tab)`.
- **Done ticks** — a to-do row's key is the hash of its cleaned text; **Done** stores it
  in the existing dismissed store. A line that returns with different words is a new row.

### 2. The Desk page (web) — replaces the Home view
Selected by the Home button (now named "Desk"; icon, place, badge and `home.show` keys
unchanged). Top to bottom:
- **Header.** Up to four **tiles**, each a 20 px number over a 12 px label and a button
  that filters the page to those rows: "need you", "working", "failed", "new changes".
  A tile at zero is drawn muted and does nothing. Then **scope chips**: "All" and one
  per open project (state mark, name, its inbox count); one is selected; the choice is
  kept in `ui.json`. At the right: **Open a project…** (the existing dialog with the
  recent projects) and a menu with Check installation and Settings.
- **For you** (U1, U3) — the inbox rows. Empty: a check mark and "Nothing needs you."
- **Agents** (U1, U2) — one block per project in scope: the project name, its state in
  one word, the task in progress as "<id> <title>", then one **lane** per role (R6's
  order). A lane: role, state word, age, buttons. Before E-103 the states are "Running"
  and "Ended" and the button is **Open** (go to the tab). E-103, E-104 and E-110 add the
  waiting states, `pane_focus`, Continue / Message… / Interrupt and the orders — to
  these lanes; "Home card" in those tasks' text reads "Desk lane" (D-074).
- **Tasks** — the board: four columns "Next up", "In progress", "Blocked", "Done", each
  with its full count, at most six cards, then "Show all (<n>)" which opens the column
  alone as a filterable list. With scope "All", a card carries its project's name. A
  card opens the reader on that task. Below 900 px the columns become four tabs.
- **Changes** (U4) — the activity feed in a right column 360 px wide (under Tasks below
  1100 px). New events carry a dot; the header says "<n> new" and **Mark read**. Leaving
  the Desk after it was in front for 3 s also marks them. An event opens the reader:
  the task's summary, the stamp's clause table (the summary split at `‖` into rows), or
  the handoff.
- **No project open.** The page shows the recent projects as large rows and **Open a
  project…**; the sections are not drawn. On Windows the WSL set-up card stays first.

### 3. Row actions (core decides, web draws)
Each inbox row names exactly one **primary** action from this closed list; the web side
sends `desk_act(key, action)` and nothing else.

| kind | sentence (≤ 120 characters) | primary | also |
|---|---|---|---|
| `waiting` (E-81) | "<Role> asks for approval" / "<Role> asked a question" / "<Role> waits for you" | **Answer** (E-111) | Dismiss |
| `ended` | "<project> has stopped" | **Restart** (the tab's existing start) | Open |
| `setup` | "<project> needs set-up" | **Set up** (the existing card) | Open |
| `review_failed` | "<id>: <family> review failed" | **See why** (reader, clause table) | Dismiss |
| `stalled` | "<id> in progress, no handoff for <age>" | **Open** | Dismiss |
| `bell` | "The terminal rang its bell" | **Open** | Dismiss |
| `todo` | the cleaned line | **Done** | Details (reader on its handoff), Copy |
| `update` | "rein-app <v> is available" / "Rein <v> is available" | **Update** (Updates dialog) | Later |
| `outdated` | "<project> is on an older Rein" | the existing ask | Not now |
| `health` | "The installation has a problem" | **Check** (the existing check) | — |

Only the primary button of an inbox row is filled; every other button on the page is
outlined. "Open" always means: go to that project's tab.

### 4. The project strip (web) — replaces the rail and the four panels
With a project tab in front, one panel can be opened left of the terminal: the Desk's
four sections for that project only, stacked in one column (For you; Agents; Tasks as
"Next up" with three cards and **Board**, which goes to the Desk scoped to the project;
Changes with ten events). Width, resize, overlay below 900 px, `Esc` and the pty-resize
rule are the panel host's, unchanged. The 40 px rail is removed: the strip is toggled by
a button right of the Desk button in the tab bar and by `panel.toggle` (simple
`Ctrl+Shift+1`, modal `Space P 1`). The reader (`status.md` §7) is unchanged and is the
only place long text is drawn.

### 5. The look (web) — rules every section follows
- **L1 One line.** A row is one line; a to-do row may wrap to two. Never more. The
  whole text is the row's tooltip and is in the reader.
- **L2 One shape.** Every row reads left to right: state mark · who or what · the
  sentence · age · buttons. Marks differ by shape as well as colour.
- **L3 Three sizes.** Tile numbers 20 px semibold; row text 13 px in the foreground
  colour; labels, ages and section titles 12 px muted (section titles in small capitals
  with their count). No other size on the page. Ids and commit ids are the only
  monospace.
- **L4 Five state colours**, from the theme's sixteen tokens, the same on every mark,
  tile and chip: needs you (yellow), failed (red), working (the accent), done / ok
  (green), idle (muted). Text on a surface keeps 4.5:1, a mark 3:1, in all six themes.
- **L5 Plain words.** No sentence ends in a full stop on a row. This table is the whole
  vocabulary for state:

  | was | is |
  |---|---|
  | "Quiet for 30 s." | "Idle 30 s" |
  | "Working." | "Working" |
  | "Engineer's turn" | "Next: Engineer" |
  | "Needs set-up." / "Tasks not available" | "Needs set-up" / "No task data" |
  | "Health unknown" / "Not checked yet" | "Not checked" |
  | "The last handoff mentions the operator:" + quote | the line itself, as a to-do |
  | "last handoff 1 d ago" | "Handoff 1 d ago" |
- **L6 Surfaces.** Sections sit on the window background with 24 px between them; a
  block (a project in Agents, a board column, a card) is a surface one step lighter,
  radius 8 px, padding 12 px, 8 px between rows, no border except on focus. Buttons are
  the small size, 28 px high.
- **L7 Motion.** A row that arrives fades and slides 8 px once; a tile's number counts
  once on change. CSS only, motion tokens, nothing under reduced motion, nothing loops
  (D-054 stands).
- **L8 Empty and unknown say what to do.** "Nothing needs you" · "No task data — this
  project keeps tasks in a form this version does not read" · "No changes in 14 days".
- **L9 Keys.** Sections are landmarks; Up / Down (and `j` / `k` in modal) move between
  rows across sections; `Enter` runs the row's primary action; `Tab` reaches the other
  buttons. Pointer and key parity as D-034.

### 6. Answer (web + core; after E-103) — U1
**Answer** on a `waiting` row calls `pane_focus(tab, role)` and then asks the core to
enlarge that pane (tmux's zoom, on the app's own server, the pane id the core holds),
and goes to the tab. The pane leaves zoom when the role stops waiting or the operator
presses the pane's own zoom key. The Desk presses no key in the pane (R7's rule).

### 7. Orders (web + core; after E-104) — U2
Fixed sentences sent through `pane_send`'s delivery (R7c) under R7b's table, to the role
named:
| button | where | to | text sent |
|---|---|---|---|
| **Run the queue** | a project's block in Agents | engineer | `Execute the OPEN queue.` |
| **Start** | a "Next up" card | engineer | `Execute <id>.` |
| **Review** | a `task_done` event | architect | `Review <id>.` |
| **Plan…** | the architect's lane | architect | the composer, starting with `Plan: ` |
| **Tell the Architect** | offered for 5 s after **Done** on a to-do | architect | `Operator: done — <the cleaned line>` |
`<id>` must be an id in the core's own task list for that tab and match
`[A-Z]-[0-9]{1,6}`. "Tell the Architect" is the one case where project text is sent: it
is the cleaned, cut line the operator has just read and ticked, sent only on their
click. A role that is absent, waiting or not reporting shows the button disabled with
R7b's sentence.

## Data Model
```
Desk     = { for_you: [Row] (≤ 200), lanes: [Lane], board: Board, activity: [Event] (≤ 50),
             unavailable: fixed reason | null }
Row      = { key, kind (component 3), tab, text (≤ 120), age_s: u32 | null,
             primary: answer|restart|setup|why|open|done|update|ask|check,
             more: [dismiss|open|details|copy|later], ref: Ref | null }
Ref      = { task: id } | { handoff: index } | { stamp: index }
Lane     = status.md's role entry (role, state, kind, since, reports, can)
Board    = { next, doing, blocked, done: [Card] (each ≤ 200), counts: {next, doing, blocked, done} }
Card     = { id (≤ 40), title (≤ 90), tier: 1|2|3|null, age_s, waits_on: [id] (≤ 3) }
Event    = { key, kind: task_done|review_failed|accepted|handoff, at, title (≤ 120),
             task: id | null, commit: 7 hex | null, from, to: String | null, new: bool }
Stamp    = { type, task_id, timestamp, sha, summary (≤ 2 KiB) }      newest 300 parsed
ui.json += desk_scope: "all" | project path;  desk_seen: { path: epoch seconds } (≤ 64)
```
`ui.json` keys `panels_disabled` and the last open panel id are read and ignored;
`[panels] enabled` and `[panels] rail` in `config.toml` are accepted and do nothing
(Settings shows one switch, "Project strip"). Nothing is written to a project.

## API / Interface Contracts
- `panel_data(tab, desk)` → `Desk`. The sources `tasks`, `handoffs` stay for the reader;
  a new source `stamps` gives one stamp by index for the reader. `summary` and
  `attention` stay in the core (status, badge, notifications use them) and are no longer
  asked for by the web side.
- `desk_act(key, action)` → `done` or a fixed sentence. `key` must be a row key the core
  last sent; `action` must be that row's primary or one of its `more`. `done`, `dismiss`
  and `later` write the dismissed store; the others run the existing action of that
  name. An unknown key answers "that row is no longer there".
- `desk_seen(tab | all)` → sets the seen time to now.
- `desk_all()` → the `Desk` of every open tab in one reply (the page's first draw); after
  it the existing `project_status` event carries a change mark and the page asks again
  for that tab only.
- Actions and keys: `home.show` (kept), `panel.toggle` (now the strip), `panel.close`,
  `panel.refresh`, `panel.focus`, new `desk.scope.next` / `desk.scope.prev`
  (simple `Ctrl+Shift+]` / `[`, modal `Space ]` / `[`; if taken, the Engineer proposes
  the nearest free pair in the handoff). `Ctrl+Shift+2…4` and `Space P 2…4` are freed.
- E-111: `pane_zoom(tab, role)`, checked as `pane_focus`. E-110: `pane_send` gains
  `action: order` with `order: run_queue | start | review | told` and `task: id | null`.

## Security
- No new file, process, network use or origin for components 1–5. `stamps` is a new
  parse of text the app already reads: bounded (300 stamps, 2 KiB each), same
  placeholders for control and bidi characters, a hostile `stamps` array is a test input.
- All project text (to-do lines, titles, summaries, triggers) is cleaned and cut by the
  core and drawn as text nodes; the reader's closed structured text is the only
  formatter. No link opens.
- The web side never names a path, a pane, a task or an action freely: it returns a row
  key the core issued and an action that row offered; the core checks both.
- `desk_act` runs only actions the window already has. **Restart** and **Set up** are
  the tab's existing start and set-up, which already ask where they asked before.
- A to-do line is a line an agent wrote. It is shown as a claim, never as a fact of the
  app: its row has the to-do mark and its handoff one click away, and **Done** only
  hides it. A project can therefore put text in front of the operator — as it can today
  in the Communication panel — and can do nothing else.
- Components 6 and 7 extend commands that type into or resize a pane: Tier 3 for E-110
  (`security_engineer`, SEC_CLEARED; the threat to write down: project text reaching a
  pane — only through "Tell the Architect", on a click, cleaned and cut, under R7b);
  Tier 2 for E-111 (one tmux verb, pane id held by the core).

## Execution Constraints
- No new poll: the 5 s stat of the two files per open tab drives everything; the desk
  source is computed only when a file changed or the tab's status changed.
- `desk_all()` for 16 projects, each with 500 tasks, 300 stamps and 30 handoffs, answers
  within 150 ms on Linux and 600 ms on Windows in an optimised build, off the UI thread.
- The Desk draws 16 projects with no frame over 50 ms; a list longer than its cap is
  never drawn whole (six cards, "Show all" virtualises nothing but shows one column).
- The Desk never scrolls sideways from 480 px wide (D-072's rule).
- Idle, the Desk runs no animation and no timer of its own.
- `shell.tsx` is at 898 of 900 lines: the Desk and the strip live in new files
  (`desk.tsx`, `desk-rows.tsx`, `desk-board.tsx`, `desk-strip.tsx`), each under 400
  lines; `home.tsx`, `side-menu.tsx`, `panel-views.tsx` and the descriptor registry are
  deleted by the task that replaces them, not left beside the new code.

## Tests
- Core: T1 over fixtures (an address with text; an address with a nested list; two
  senders; the same line from both → one row; "E-79 (operator)" → none); T2 (FAIL then
  PASS → none; FAIL on a DONE task with a later ACCEPT_PASS → none; older than 14 days →
  none); board placement including an OPEN task with an open dependency; activity order,
  cap and `new` against a seen time; hostile `stamps`; `desk_act` with an unknown key and
  with an action the row did not offer.
- Web: the row model for every kind (sentence, primary, more); L5's table as a test over
  every state sentence; exactly one filled button per inbox row; tiles filter; scope
  kept; key movement L9 and pointer parity.
- Window run (`tests/e2e/home-screens.sh`, extended, not a new suite): the Desk with
  three fixture projects; ticking **Done** removes the row and it stays gone after a
  restart; a new handoff appears in Changes within 6 s with its dot; `scrollWidth`
  equals `clientWidth` at 1280, 700 and 480 px; the strip opens beside the terminal
  with one pty resize.
- Screens (D-052), both themes at 1280×800: the Desk with five projects in mixed states;
  the Desk scoped to one project; no project open; the board's "Show all"; the reader on
  a failed review; the strip. One theme at 700 and 480 px. The Architect opens every
  one, and the operator is shown the 1280 pair before E-108 starts.

## Rollback Plan
Each task is one commit. E-107 and E-109 delete what they replace, so reverting them
brings Home and the panels back whole. `desk_seen`, `desk_scope` and the done ticks are
the app's own state and are ignored by an older build. Nothing is written to a project
or to the Rein home.

## Deferred
- **Allow / Deny on the row** and the text of what an agent asks — needs rein
  (`plans/rein-desk-request.md` ask 2).
- **A to-do record instead of handoff lines** — the same file, ask 3. T1 is provisional
  and fails soft, like the two parsers it sits on.
- **The diff and the screenshots of a finished task** on its event — a git read and an
  image read are new read scopes; a later revision, Tier 3.
- Model, context and running tool on a lane (R8) — still with rein.
- Dragging a card between columns: the app writes nothing to a project.

## E-## Task Breakdown
1. **E-106 (Tier 2)** — component 1 and the API of `desk`, `stamps`, `desk_act`,
   `desk_seen`, `desk_all`.
2. **E-107 (Tier 2)** — component 2's header, For you and Agents, component 3, the look
   L1–L9; Home's view is deleted. After E-106. **The operator sees its screens.**
3. **E-108 (Tier 2)** — component 2's Tasks board and Changes feed. After E-107.
4. **E-109 (Tier 2)** — component 4: the strip; the rail, the four panels and their
   settings go. After E-108.
5. **E-111 (Tier 2)** — component 6, Answer. After E-103 (which waits on E-81).
6. **E-110 (Tier 3)** — component 7, the orders. After E-104.
