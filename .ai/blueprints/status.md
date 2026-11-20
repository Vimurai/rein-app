# Blueprint — Project status, notifications, readable panels and Home (v1 · 2026-11-17, D-062, D-063, D-064)

> Architect-owned. Operator direction 2026-11-17: "we are missing notifications for each
> project running so we can see when something is stuck or needs our attention, and also
> status in the tabs above"; "panels UI is not good, text cannot be read when long; Needs
> you gives a lot of things that are not important". Read with `panels.md` (the side menu)
> and `desktop.md` (the window). Where this file and `panels.md` differ, this file wins;
> the places are named under "What this changes in panels.md".

## Goal & Architecture
With several projects open, the operator has to click through tabs to learn which one is
working, which one has stopped and which one waits for them. One sentence: **the core
keeps one status per open project, computed from what the app can observe; the tab bar
shows it, a change that matters raises a notification, and the panels become a place
where long text can be read and where "Needs you" lists only what the operator can act
on.**

## Core Concept
**Project status** — one value from a closed list per open tab, plus the attention items
of that tab. It is computed in the Rust core (not in the web view) because it must keep
working while the window is hidden or minimised, where a web view's timers are slowed or
stopped. The web side draws it and decides nothing.

What the app can observe, and what it cannot:
- It **can** see: the tab's own state (starting, ended, set-up refused), which role panes
  run an agent (the 5 s status script, V9), when the project's tmux window last produced
  output, a terminal bell, and the project's two `.ai/` files (`panels.md` sources).
- It **cannot** see why an agent is quiet. A finished agent, an agent at a permission
  prompt and an agent waiting for a reply look the same. So the app says **"quiet"** and
  **"stopped"**, never "waiting for you", until rein gives a signal (`@rein_waiting`,
  asked for in `plans/rein-panels-request.md` item 2; sent 2026-11-17). The data model
  has a place for that signal; nothing reads it until rein names it in the contract.

## Components

### 1. Activity (core)
The status script (`tmux_agents.sh`, read-only, constant) also prints, for the window the
tab's client shows, tmux's `window_activity` time and the clock of the machine tmux runs
on (so the two are compared on one clock — on Windows both come from inside WSL). From
these the core keeps per tab: `quiet_s` (seconds since the last pane output) and
`worked_s` (how long output had been arriving, gaps under 15 s ignored, before it
stopped).

**The Engineer reads first, and reports before building** (a handoff, like E-69's reads):
(a) on tmux 3.2a and on the newest tmux at hand: does `window_activity` move on pane
output in the current window of an attached client, and stay still on a status-line
redraw, a client resize and a key typed into a pane that prints nothing; (b) with a real
Claude Code agent: output cadence while it works (also during a tool run of several
minutes), while it is idle at its prompt, and while it shows a permission prompt — is
"no output for 15 s" a sound line between working and not; (c) whether the bell of a
pane in a background tab reaches the app today on the app's own tmux server.
If (a) fails, the fallback is the time of the last byte the tab's pty gave the app,
valid only while the tmux status line is `hidden` (V6); with `app` / `keep` activity is
then `unknown`. If (b) fails, stop and hand back — the states below are then wrong.

**The reads, as measured (E-76, 2026-11-18; D-062 addendum 3).** (a) passes on tmux 3.2a,
3.6 and 3.8; the value is whole seconds. (b) passes: a working Claude Code agent prints
every second, also through a 3-minute tool run; it is silent at its prompt and at a
permission prompt. (c) a background tab's bell reaches the app, but Claude Code rings
none at a permission prompt. Known limits, accepted:
- A window resize makes a full-screen agent redraw, which reads as one second of output.
- A hung API call keeps the agent's spinner moving, so the tab reads `working` and is
  never `stuck`. Only rein's signal can tell; the app does not read the screen (D-062
  alternative 1).
- An agent at its prompt with a background shell still running reads `quiet`.
- The cadence is one agent version on one machine. If a later one stops printing during
  a long tool run, `quiet` and `stopped` come early; the line is then measured again.

**Where a stretch of work begins (D-062 addendum 5, E-96).** The core sees output only at
its samples, so the first output of a stretch lies somewhere after the previous sample.
E-76 started the stretch at the first sample that saw output; `worked_s` then read short
by up to one poll period (5 s shown, 15 s hidden) and a 20 s burst in a hidden window
raised no `stopped`. The rule from E-96 on: when a sample sees new output and the window
had been quiet for 15 s or more by the previous sample, the stretch begins at **the
previous sample's clock + 1 s**, and not earlier than **14 s before the output just
read**, and not later than that output. So:
- `worked_s` reads long by up to one period less a second, never short by more than 1 s.
  Work of 15 s or more always reaches the `stopped` line, at either period.
- Output seen by one sample alone reads at most 14 s, whatever the time between the two
  samples (a late poll, a machine that slept). A one-line print or a resize redraw
  still raises nothing.
- Known limit, accepted: in a hidden window a burst shorter than 15 s that crosses a
  sample can read as 15 s and raise `stopped` (2 s of output: about 1 time in 7). A
  wrong "has stopped" is cheaper than a missed permission prompt.
- The first sample of a tab, and a client that now shows another window, start at the
  output read, as before: how long it had worked is not known.
- The poll period is never over 15 s (Execution Constraints); this rule relies on it.

### 2. Status (core, a pure function with a table test)
First match wins:

| State | When | Tab mark (lucide, 12 px) | Sentence |
|---|---|---|---|
| `setup` | set-up refused, or the folder is not set up | `wrench` | Needs set-up. |
| `ended` | `rein start` ended, or an agent seen running has exited | `square` | Ended. |
| `starting` | the tab is opening (V7 phase) | the V7 spinner | Starting… |
| `needs_you` | the tab has at least one "now" item (component 4) | `bell-dot` + count over 1 | Needs you. |
| `working` | an agent runs and `quiet_s` < 15 | `circle-dot`, accent colour | Working. |
| `quiet` | an agent runs and `quiet_s` ≥ 15 | `circle`, muted | Quiet for <span>. |
| `unknown` | none of the above can be told | none | — |

Shape tells the state, not colour alone. No mark animates except the existing spinner
(D-054). The status is recomputed on every poll answer and on the app's own events (tab
phase change, bell, focus).

### 3. Tab bar and status bar (web)
- Each tab shows its mark left of the title, with an accessible name ("<project>:
  <sentence>") and a tooltip of at most three lines: the sentence; "In progress: <task
  id> — <task title>" if any; "<whose turn>'s turn · last handoff <age>" if known.
- The status bar gains one segment for the current tab, between the path and health:
  "<sentence> · <task id> · <whose turn>'s turn". It is cut from the right when narrow.
- The Dashboard panel's first row is the same status, then its present content.
- `[tabs] status = "shown" | "hidden"` (default `shown`), also in Settings.

### 4. Attention items, triaged (core; replaces the rules of `attention.ts`)
Two groups. **Now** is counted in the rail badge and makes a tab `needs_you`. **For
information** is listed collapsed under it, is not counted and never notifies.

| Item | Group | Rule |
|---|---|---|
| `waiting` | Now | rein's signal says an agent waits (not built until rein names it) |
| `bell` | Now | a bell in a tab that is not the current tab of a focused window (as today) |
| `ended` / `refused` | Now | as today |
| `stuck` | Now | an agent runs, a task is `IN_PROGRESS`, `quiet_s` ≥ `stuck_after_min` (default 10) |
| `mention` | Now | the last handoff's "Next action needed" names the operator (as today, quoted) |
| `health_fail` | Now | install health `fail` (one item, no tab) |
| `blocked` | Info | one row per project: "<n> tasks are blocked", opening to the list |
| `update` | Info | rein says the project is out of date |
| `health_warn` | Info | install health `warn` |

Removed: "no handoff for more than 30 minutes while a task is in progress". It fired for
days on a project whose agents were not running (the "29 h" row) and said nothing the
operator could act on; `stuck` replaces it and needs a running agent.

**Dismiss.** `bell`, `stuck` and `mention` rows have a Dismiss button. A dismissed item
stays hidden until its text changes (a new handoff, a new bell, quiet again after work).
Dismissals are kept in `$XDG_STATE_HOME/rein-app/dismissed.json` so a restart does not
bring them back.

**The bell is not the permission-prompt signal** (D-062 addendum 3). Claude Code rings no
bell at a permission prompt with its default settings; every `\x07` it wrote was the end
of an OSC sequence. The `bell` item stays for whatever does ring: another program, or an
agent the operator has set to ring. The app never sets that for the operator. Until
`waiting` is built, a permission prompt in a background tab is told only by the tab
turning `quiet` and by the `stopped` notification.

### 5. "Needs you" panel (web)
Items grouped by project: one header per project (name, tab mark, one "Go to tab"), its
rows below; a row is also a button that goes to the tab. Then "For information (n)",
collapsed. Empty state: "Nothing needs you." The known-gap paragraph moves behind an
"About this list" disclosure at the bottom and is shortened to two sentences.

The two sentences, from E-93 on (D-062 addendum 3; the same text in
`docs/operator-guide.md`): "The app cannot see an agent waiting at a permission prompt,
whether the window is open or closed. Until rein gives a signal, a project that goes
quiet after working is the only hint: its agent has finished or is waiting for you."

### 6. Notifications (core)
A notification is raised on a **change**, once, never repeated while the state stays.

| Event | Raised when | Fixed sentence |
|---|---|---|
| `needs_you` | a `waiting` or `bell` item appears | "<project> needs you." |
| `stopped` | `worked_s` ≥ 15, then quiet for 30 s | "<project> has stopped — it is finished or waiting." |
| `stuck` | a `stuck` item appears | "<project> has been quiet for <n> min with a task in progress." |
| `ended` | an `ended` item appears | "<project>: an agent has ended." |
| `handoff` | a `mention` item appears for a handoff newer than the last one seen | "<project>: the last handoff names you." |

Three channels, from the cheapest up:
1. **In the window** (always): the tab mark, the rail badge, and a toast (the existing
   one) when the event's tab is not the current tab.
2. **Window attention** (when the window is not focused): the task-bar flash / dock
   bounce of the Tauri core (`request_user_attention`, informational, once per event)
   and, where the Tauri core offers it on that system, the count of Now items on the
   app's icon. No new dependency. The Engineer reports which of the two each system has.
3. **System notification** (when the window is not focused, or hidden): the operating
   system's own notification. This needs a new dependency and is its own task behind
   `dependency_gate`. Clicking it brings the window forward where the system reports the
   click; going to the tab is not required.

Limits: nothing for the current tab of a focused window; nothing in the first 30 s after
a tab opens or is restored (V12 restores many at once); at most one system notification
per project per 60 s — later events in that minute become one "<project>: <n> things
need you."; `stopped` is not raised when a `needs_you` was raised for the same stop.

`stopped` needs 15 s of work, not 60 (D-062 addendum 3). It is the only notification a
permission prompt gets before `waiting` exists, and an agent often asks within its first
minute. 15 s still keeps a resize redraw or a one-line print from raising it.

### 7. Readable panels (web)
- **Type and width.** Panel body 13 px with line height 1.5; default width 400 px, range
  320 px to the smaller of 720 px and half the window.
- **Structured text.** One component turns source text into paragraphs, bullet and
  numbered lists, `code` spans, fenced code blocks (monospace, own horizontal scroll),
  **bold** and heading lines. It builds elements with text children only. It never makes
  a link, an image or HTML; a URL and a Markdown link stay literal text; an unclosed
  marker is literal. Linear in the input, which the core has already capped.
- **Task title.** A task row shows the id and a title: the description up to its first
  sentence end or line break, at most 90 characters, cut at a word. Chips: tier, owner,
  "blocked by <ids>", age. The full description is in the reader.
- **Reader.** Activating a task or a handoff opens it in a sheet over the terminal,
  beside the panel: at most 720 px wide, text column at most 72 characters, header (id
  or "From → To", title or trigger, the chips), body as structured text. `Esc` or Close
  returns focus to the row it came from; Up / Down (and `k` / `j` in `modal`) move to
  the previous / next item without closing. It lies over the terminal: no pty resize.
  It replaces the in-place task drawer.
- **Long paragraphs (D-063 addendum 1, E-82).** Tasks are written as one paragraph, so
  structure alone leaves the reader a wall of text. In the reader only: a paragraph over
  320 characters is split at sentence ends into paragraphs of at most three sentences or
  320 characters; a sentence that starts with a label and a colon ("Acceptance:",
  "Build:") starts a new paragraph with the label in bold; the body does not repeat the
  title's sentence. No character of the source is added or dropped otherwise.
- **Caps as built (E-74).** A task description is kept to 4096 characters (was 1024 in
  E-59; 64 of this repository's 82 tasks were longer). The 1 MiB reply cap is unchanged.
  Reader `Esc` returns focus to the row of the item last shown. Communication stays in
  file order, newest entry first.
- **Communication.** An entry shows who → who, age, the trigger as its title, and the
  first six lines of "Next action needed"; "Read all" opens the reader with every
  section.
- **Tasks.** A filter box over id and description (the rows already loaded; no new
  read); groups collapse; Done starts collapsed.

## Data Model
```
ProjectStatus = { tab, state: setup|ended|starting|needs_you|working|quiet|unknown,
                  quiet_s: u32 | null, in_progress: {id, title} | null,
                  turn: String | null, last_handoff_s: u32 | null,
                  now: [Item], info: [Item] }              (now + info ≤ 200 per window)
Item          = { key, kind (closed, component 4), tab | null, text (the app's sentence),
                  quote: String | null (text from a project file, ≤ 300 chars),
                  dismissable: bool }
Activity      = { last_output: time | null, worked_s: u32, waiting: bool | null }
                  (`waiting` stays null until rein's signal is in the contract)
Notification  = { event (closed, component 6), tab, at }
dismissed.json = { schema: 1, items: [{ project, key, text_hash, at }] }   (≤ 500, oldest
                  dropped; `text_hash` is a SHA-256 — no text from a project is stored)
config.toml   + [tabs] status; [notifications] enabled = true, system = true,
                  events = [the five names] (default all), stuck_after_min = 10 (3–120)
ui.json       + the same four notification settings and the tab-status switch
```
A task id is shown in a tooltip, a toast or the status bar only when it matches
`[A-Z]-[0-9]{1,6}`. Nothing is written to a project.

## API / Interface Contracts
- **Event `project_status`** (core → web): the full `ProjectStatus` of one tab, sent when
  it changes. **Command `project_status_all()`** → every open tab's, for a web view that
  has just loaded.
- **Command `attention_dismiss(tab, key)`** — the key must be one the core last sent for
  that tab and be dismissable; anything else is the fixed sentence "that item is not in
  the list".
- **Event `notify`** (core → web): `{event, tab, sentence}` for the toast.
- **Command `attention_items(tabs, health)`** → the core's `now` / `info` lists for every
  tab in one call (as built in E-75, accepted D-063 addendum 2). The web side sends closed
  booleans, a bell count and core ids by tab-bar index; the answer names tabs by index and
  the web side adds its own titles. `panel_data(tab, "attention")` still answers the facts.
  The web-side rule function is removed with its tests moved to the core's table test.
  **From E-76 on** the core holds ended / bell / agents-running for every tab itself, so
  the booleans the web side sends shrink to what only it knows (refused, update, health),
  and the quiet / stuck rule covers every open tab, not the active one only.
- The web side reports what only it knows through closed commands: `tab_current(tab)`
  and the bell, unless the core already has both (the Engineer checks and says).
- No command takes a path, a sentence or a title from the web side.

## Security
- **Text that leaves the window.** A system notification is shown by the operating
  system, possibly on a lock screen and kept in its history. It carries only the app's
  fixed sentences and the project's folder name (already printable-only, D-024) — never
  a task description, a handoff line or a quote. Toasts and tooltips may show a task id
  (pattern above) and a task title as a text node.
- **Structured text is not Markdown-to-HTML.** The component has a closed set of
  constructs and no code path that produces an anchor, an image, an `href`, a `src` or
  raw HTML; a test feeds it the hostile fixtures `panels.md` already names and asserts
  the DOM holds only the allowed element names. CSP is unchanged.
- **The status script stays read-only and constant**: it gains format fields, no new
  argument, no option set, nothing from the web side on its command line.
- **`dismissed.json`** holds project paths the app already keeps in `projects.json`, a
  key and a hash; written with temp file + rename; an unreadable file is ignored.
- **The notification dependency** (channel 3) is chosen under `dependency_gate` with
  `critic_security`: no network use, no shell, and on Windows no new process other than
  what the system's own notification API needs. If none fits, channel 3 is not built
  and channels 1–2 stand.
- No new origin, no network, no new file read in a project.

## Execution Constraints
- **Polling while hidden (changes `panels.md` "no polling when hidden").** While the
  window is hidden or minimised the status poll runs every 15 s instead of 5 s, and the
  attention stat of the two files continues at 15 s; nothing else reads. With no tab
  open there is no poll.
- On Windows one poll is one `wsl.exe` for **all** tabs on the app's server, not one per
  tab; the Engineer measures it and gives the figure (target: under 250 ms, and under 1 %
  of one core averaged over a minute with eight tabs open).
- The status function and the notification limits are pure and tested with a clock
  passed in; no test sleeps.
- The reader and the filter add no IPC call. Typing latency with the reader closed is
  unchanged (the `panel-screens.sh` figure).

## Tests
- Core: the status table (every row, and the order); `worked_s` / `quiet_s` over a
  scripted series; each attention rule; dismiss and its return on changed text; each
  event raised once on a change and not again; the 30 s start-up silence; the 60 s
  limit and the merged sentence; settings switch each event off.
- Web: structured text (each construct; link, image and HTML stay literal; unclosed
  markers); task title cut; reader focus return and Up / Down; filter; pointer and key
  parity for the new actions; tab mark names.
- Window run (`panel-screens.sh`, local): a background tab that prints then stops gets
  `working` then `quiet` then the `stopped` toast; a hidden window still raises it.
- Windows interop output for the extended status script, with timing.
- Screens gate (D-052), both themes at 1280×800: the tab bar with one tab in each state;
  "Needs you" with two projects, with only information items, and empty; the reader with
  a real long task and a real long handoff from this repository's own `.ai/`; the Tasks
  filter; a toast. The Architect looks at every one.

## Rollback Plan
`[notifications] enabled = false` and `[tabs] status = "hidden"` remove what the operator
sees without a rebuild; `system = false` removes only the system notifications. Each
task is one commit that reverts cleanly; the system-notification task is the only one
that adds a dependency and reverts with it. Deleting `dismissed.json` is safe.

## What this changes in panels.md
Component 6 (the rules — replaced by component 4 here), component 2 (width), component 5
("text nodes only" stays true; "never Markdown" becomes the closed structured text), the
task drawer (replaced by the reader), Execution Constraints (polling while hidden).

## Deferred
- **"Waiting for you" as a fact**, per role — needs rein's signal.
- Status per role in the tab ("Engineer working, Architect quiet") — tmux 3.2a gives no
  per-pane activity time; comes with rein's signal or a newer tmux floor.
- Notifications that reach a phone, sounds, quiet hours.
- Answering a prompt from the panel — no panel action types into a terminal (`panels.md`
  §Security); a change to that rule is the operator's to ask for.

## E-## Task Breakdown
1. **E-74 (Tier 2)** — readable panels: component 7.
2. **E-75 (Tier 2)** — "Needs you" triage: components 4 (without `stuck` and `waiting`)
   and 5, dismiss. Until the status task, the old quiet rule is kept only for a tab
   whose agents run.
3. **E-76 (Tier 2)** — activity and status in the core: the reads, components 1, 2 and
   the rest of 4; the events and commands; polling while hidden.
4. **E-77 (Tier 2)** — tab marks, status-bar segment, Dashboard row: component 3.
5. **E-78 (Tier 2)** — notifications, channels 1 and 2, limits and settings: component 6.
6. **E-79 (Tier 2, `dependency_gate`)** — system notifications: channel 3.
7. **E-96 (Tier 2)** — where a stretch of work begins: component 1, D-062 addendum 5.
   Before E-79.

**As built, accepted (E-78, D-062 addendum 5).** The `handoff` event is keyed by the
handoff's time; a handoff whose time is not in the written shape raises none (its
mention row still shows). Channel 2 per system: Linux — urgency hint, and the launcher
count on docks that read it; macOS — dock bounce and badge count; Windows — taskbar
flash only (no count: an overlay icon needs a drawn image per number and is not built).

## Home — every project at a glance (revision 2026-11-17, D-064)

> Operator direction 2026-11-17, on candidate 1 of `plans/desktop-feature-candidates.md`:
> "with home button at beginning of tabs on top bar; this should be the top system to
> check how everything is going".

### Goal
One place, always one click away, that answers "how is everything going" for every
project without opening any of them. **Home is a fixed first entry of the tab bar that
shows one card per project, built from the status this file already defines.** It
replaces the start screen: with no project open, Home is what the window shows.

### Components
H1. **Home button** — the first entry of the tab bar, left of every project tab: a house
   icon with the accessible name "Home"; it cannot be closed, moved or renamed, and
   project tabs cannot be dragged in front of it. It carries the count of Now items
   across all projects (the rail badge's number; nothing when zero). It is not a
   numbered tab: `tab.goto 1` is still the first project.
H2. **Home view** — fills the stage when Home is selected. The terminals of the open
   projects keep running and keep their size; no pty is resized or closed by going Home
   or coming back. Top to bottom:
   - **Summary line**: "<n> projects open · <n> working · <n> need you · <n> quiet ·
     <n> ended" (a part with zero is left out).
   - **Needs you now**: the Now items of every project (component 4), each a row that
     goes to its tab, with Dismiss where allowed. Left out when empty.
   - **Open projects**: one card per open tab (H3), ordered by state — `needs_you`,
     `ended`, `setup`, `quiet`, `working`, `starting`, `unknown` — and by tab order
     within a state. A card that changes state moves only when the pointer is not over
     the grid and no card has keyboard focus, so a card never moves under a click.
   - **Recent projects** not open now: the present start screen's rows (name, path,
     last opened, open on click), and `Open a project…`.
   - **Installation**: the health row and `Check installation`, the WSL set-up card on
     Windows when needed — the start screen's, unchanged.
H3. **Project card** — a button that goes to the project's tab. It shows: the project
   name and the tab mark with its sentence; "In progress: <task id> — <task title>" or
   "No task in progress"; "<whose turn>'s turn · last handoff <age>" and the last
   handoff's trigger as one line (cut at 120 characters); the agents by role, each
   "running" or "ended"; chips for open / in progress / blocked task counts; the number
   of Now items. A project whose files cannot be read shows its state and "Tasks not
   available" — never a guess.
H4. **Side menu on Home** — "Needs you" works as on a project. The three per-project
   panels have no project to show: their rail buttons are disabled with the tooltip
   "Open a project to see this", and an open one closes when Home is selected and
   comes back when a project tab is selected again.
H5. **Start-up** — `[window] start = "dashboard"` (the existing key and default) opens
   Home; `"last"` opens the last active project tab as V12 restores it. With no project
   to restore, Home either way.

### Data model
```
ProjectCard = ProjectStatus + { counts: {open, in_progress, blocked} | null,
                                trigger: String | null (≤ 120 chars),
                                agents: [{role, state: running|ended|unknown}] }
```
The core sends the card fields with the `project_status` event; they come from the
`summary` source and the status script the core already runs. No new file is read and
nothing is stored.

### Interface contracts
- **Action `home.show`** — selects Home. Simple style `Ctrl+Shift+H`, modal `Space h`;
  if either is taken, the Engineer proposes the nearest free binding in the handoff.
  A menu row "Home", and the button: pointer and keyboard parity as D-034.
- `tab.next` / `tab.prev` cycle through Home and the project tabs.
- No new IPC command: Home draws `project_status_all()` and the events, and opens a
  recent project through the existing `project.open`.

### Security
Every string from a project (task title, trigger, role) is a text node, already cut and
cleaned by the core. A card only switches tabs; Home runs no command, writes nothing and
types into no terminal. No new read scope, no network, CSP unchanged.

### Execution constraints
- Home adds no poll: the status poll runs for every open tab already. While Home is in
  front the summary facts of each open tab are re-read only when its two files change
  (the 5 s stat). Idle Home runs no animation and no timer of its own.
- Thirty-two open projects (V12's cap) draw without a frame over 50 ms in the window
  run; the grid is one to four columns, cards at least 280 px wide, one column below
  720 px.
- Selecting a project tab from Home shows its terminal within the time a tab switch
  takes today (the `panel-screens.sh` resume figure).

### Tests
- Web: the card order and the "does not move under the pointer" rule as a pure
  function; summary-line wording with zero parts left out; the Home button is first, not
  closable and not numbered; panels rule H4; pointer and key parity for `home.show`.
- Core: the card fields for a readable project, an unreadable one and a team-mode one.
- Window run: Home with three fixture projects in different states; a project that
  stops moves up and the Home badge counts it; back to a tab with no pty resize.
- Screens (D-052), both themes at 1280×800 and one at 700 px wide: Home with five
  projects in mixed states, Home with none open (recent projects only), Home with
  "Needs you now" rows. The Architect looks at every one.

### Rollback
The task is one commit; reverting it brings the start screen back. Nothing is stored.

### Task
7. **E-80 (Tier 2)** — Home: H1–H5. After the tab-marks and the triage tasks.

**As built, accepted (E-80, b0b0e7c; D-070).** Modal key `Space H` (`Space h` is
`health.show`). The frame figure is for **16** projects, the window's tab cap
(`MAX_TABS`); "thirty-two" above was the record file's cap and is not the limit of
what Home draws. The Home badge counts Now items (H1): a project that goes quiet moves
its card up and is not counted — the window-run line in Tests assumed it would be and
is wrong. A tab whose `rein start` has ended shows no agents line (the last reading is
stale, and a stale "running" is a guess). On Home the shortcuts act with no terminal
focused. `[window] start = "last"` with nothing restored shows Home (H5).

## Waiting signal — "waiting for you" as a fact (revision 2026-11-17, D-062 addendum 1)

> rein ruled on our request the same day: **rein D-160, task E-433 (planned, not built)**;
> normative text rein `.ai/blueprints/release-artifact.md` §Component 7 at rein 7f209e1.
> It will be published as "Pane options" in `docs/app-contract.md` with the fixture
> `tests/fixtures/release/pane-options.json`. **Nothing here is built until rein's
> Architect names the merge commit and the fixture is vendored** (as E-17 did).

### What rein gives (per agent pane, in the tmux server)
- `@rein_signals` — holds the word `waiting` while the pane's session reports. Without
  it the pane's waiting state is **unknown**, never "working" (any session started
  before `rein sync` on the rein version that ships E-433).
- `@rein_waiting` — `permission` or `question` while the agent waits; unset otherwise.
  Any non-empty value is waiting; rein may add values inside a major version.
- `@rein_waiting_since` — epoch seconds, set and unset with it.
- Bounds, not exactness: a prompt on screen for 7 s is reported; a report can outlive a
  prompt that was denied by hand or escaped, until the turn ends; a killed agent's pane
  keeps its options. No prompt text is stored. It is not a "finished" signal.

### Components
W1. **Read** — the status script's per-pane line gains the three options, read only for
   the panes of the tab's own window as today (no `-a`, no new argument).
W2. **Parse (core)** — per pane: `reports` = `@rein_signals` holds the word `waiting`;
   `waiting` = `reports` and `@rein_waiting` is non-empty **and the pane's agent is
   running** by the existing liveness rule (a dead or shell pane never waits). `kind` =
   `permission` | `question` | `other` (any other non-empty value). `since` = the number
   if it is 1–12 digits, not in the future and under 30 days old; else none. The raw
   option text is never shown, logged or sent to the web side.
W3. **Item `waiting`** (Now, one per waiting role) — "The <role> is waiting for your
   approval." / "The <role> has asked you a question." / "The <role> is waiting for
   you.", plus "for <span>" when `since` is known. `<role>` is `@rein_role` when it
   matches `[a-z][a-z0-9-]{0,31}`, else "An agent". Dismissable, because a report can
   outlive its prompt; a dismissed one returns when `since` changes.
W4. **Sentences that become exact** — the `needs_you` notification says the item's
   sentence with the project name ("<project>: the engineer is waiting for your
   approval."). `stopped` says "<project> has stopped — it is finished." only when every
   running agent pane `reports` and none waits; otherwise the D-062 sentence stays.
W5. **Per role in the window** — the tab tooltip and the Home card list each role as
   "waiting for approval" / "asked a question" / "running" / "ended"; the Home card's
   agent `state` gains `waiting`.
W6. **A project that does not report** — when a tab's agents run and no pane `reports`,
   one information item: "This project's agents do not report when one waits. Agents
   started by `rein start` on a current rein do; `rein doctor` says what is missing."
   The app runs nothing. The "About this list" text is shown only then.
   (Amended 2026-11-17 on rein D-160 addenda 1–2: only a session started through
   `rein pane <role>` reports — a `claude` typed by hand into a pane never does — and the
   signal needs tmux recorded at rein's install; so `rein sync` alone is not always the
   cure and the sentence no longer names it. The information item is per pane state, so
   a tab with one reporting role and one hand-started agent shows the reporting role's
   state and "running" for the other, with no item.)
W7. **A session that waited while the app was closed** — items still present when a
   tab's 30 s start-up silence ends raise one notification per project, once.

### Data model
`Activity.waiting: bool | null` becomes `roles: [{ role, state: running | ended |
unknown | waiting, kind: permission | question | other | null, since: u64 | null,
reports: bool }]`.

### Security
The three options are text set inside the user's own tmux server, where any program of
that user can set them: they are parsed against the closed forms above and nothing else
is taken from them. The role in a system notification is the pattern-checked role or
"An agent". No new process, argument or file.

### Tests
The parser over rein's vendored `pane-options.json` and over hostile values (long,
control characters, a `|` in a value, a future `since`); `reports` absent → unknown, no
item; waiting on a dead pane → no item; each sentence; dismiss returns on a new `since`;
W7 raised once; real-tmux run setting the three options on a fixture pane (Linux, macOS;
Windows interop output).

### Task
8. **E-81 (Tier 2)** — W1–W7. After the notifications task and Home, and only once
   rein's E-433 is merged and the Architect has named the commit.

## Home, second revision — a true "needs you", rows that fit, and the desk dashboard's facts (2026-11-20, D-071)

> Operator, 2026-11-20, on the installed Windows build with two projects open: (1) "this
> needs me is not valid" — both "Needs you now" rows were handoff lines, and rein-app's
> was addressed to the Engineer; (2) "has horizontal scroll which is ugly"; (3) "check on
> windows using zebar how we build dashboard, I want similar functionality on our home".
> The dashboard read is the operator's own `claude-desk/server.py` and
> `omarchy/dashboard.html` under `C:\Users\Emir\.glzr\zebar\`.

### Goal & architecture
Home must say "needs you" only when the operator is asked for something, must fit its
window at every width, and must show per project what the operator's desk dashboard
shows: the tasks that are active and one lane per agent. It is for the operator who
looks at Home to decide which project to go to. One sentence: **the row is true, the
row fits, and the card says what each agent and the queue are doing.**

### Core concept
Two different facts were mixed in one list. **An agent waits for the operator** is a
fact of the pane (rein's signal, E-81). **A handoff asks the operator for something** is
a line of text addressed to them. A line that only contains the word "operator" is
neither. The desk dashboard is believed because its "needs you" is a pane state, never a
text search; Home gets the same footing: the `waiting` item when E-81 lands, and a
`mention` only for a line addressed to the operator.

### What went wrong (measured from the screenshot and the code)
- `panel_attention.rs::names_operator` is true for any line of "Next action needed" that
  holds the word. The row shown was "- **Close E-99, then E-100.** After it the queue
  holds only tasks that wait: E-79 (operator), … E-6 (operator's keys)" — an order to
  the Engineer that names the operator twice in passing.
- The quote is shown as written, with its list dash and `**`.
- `home.tsx` `NeedsYouNow`: the `<ul>` is a grid whose one column is `auto`, so a long
  row makes the track as wide as its text; `truncate` on the inner span never applies
  and `#home` (only `overflow-y-auto`) scrolls sideways.

### Components
R1. **Mention rule (core, `panel_attention.rs`)** — a line of the last handoff's "Next
   action needed" is a mention only when it is **addressed to the operator**. Take the
   line; remove a leading list marker (`-`, `*`, `+`, or digits with `.` or `)`), then
   every leading `*`, `_`, `` ` `` and space; remove one leading task label (a task id
   in the `[A-Z]-[0-9]{1,6}` shape followed by `:`, with emphasis marks around it);
   remove an optional leading "the " (any case). It is a mention when the next word is
   `operator` or `operators` (any case). Examples that count: "- **Operator**: merge
   #61–#64 after #60", "Operator — sign the tag", "- **E-79:** the operator installs the
   build". Examples that do not: "E-79 (operator)", "tasks that wait on the operator's
   keys", "Close E-99 … E-6 (operator's keys)". `names_operator` is replaced, not kept
   beside it.
R2. **Mention text (core)** — the quote is the line without its list marker and with
   `**`, `__` and backticks removed (the words stay), cut at 300 characters as today.
   The sentence becomes "The last handoff asks the operator:". The dismissal hash is
   over the new text, so a row dismissed before this build may return once.
R3. **Rows that fit (web, `home.tsx`)** — Home never scrolls sideways, at any window
   width from 360 px. A "Needs you now" row is: the bell, the project name and the
   app's sentence on the first line; the quote under it, wrapped, at most three lines,
   then an ellipsis; Dismiss stays at the right, top-aligned. The whole quote is the
   row's tooltip and stays readable in the "Needs you" panel as today. Every grid and
   flex child on Home that holds project text can shrink (`min-width: 0`), and the
   single column is `minmax(0, 1fr)`. The same check covers the cards and the recent
   rows: a 200-character project name, trigger or path is cut, not scrolled.
R4. **Active tasks on the card (core + web)** — under the "In progress / No task in
   progress" line the card lists up to three tasks: `IN_PROGRESS` first, then `OPEN`,
   then `BLOCKED`, newest first inside a status (the desk dashboard's order), each
   "<id> <title cut to one line>" with a mark for its status told by shape as well as
   colour. The in-progress task is not repeated. None when `state.json` is not read.
R5. **Chips open the board (web)** — the three chips become one button, "Tasks", beside
   them: it goes to the project's tab with the Tasks panel open (the existing panel;
   no new view). The card itself still goes to the tab. Keyboard: the button is the
   card's second tab stop.
R6. **Role lanes (core + web; after E-81)** — the agents line of the card becomes one
   lane per role: role, state ("waiting for approval", "asked a question", "running",
   "ended", "unknown") and, while waiting, "for <age>" from `since`. A waiting lane has
   the `needs_you` mark. Selecting a lane goes to the tab **and makes that role's pane
   the active pane** in the app's tmux server (a closed command; see API). Lanes are in
   the order Engineer, Architect, then other roles by name.

Not in this revision, and why:
- **Continue / Esc / a typed message to an agent from Home** (the desk dashboard's
  `/send`). It types into a terminal, which Home's Security section forbids today; it
  is the operator's decision and is asked of them (D-071 r5). Not planned until yes.
- **Model, context meter, the tool running now, background shells.** The desk dashboard
  reads them from the `tmux-claude-status` plugin's pane options, which are not rein's
  contract. The app reads only what `docs/app-contract.md` §6 publishes; a request to
  rein's Architect is the way, sent when the operator confirms they want them on Home.
- **Chime.** Notifications (E-78, E-79) already cover it.
- **Start a project.** Home's recent rows and `Open a project…` already do.
- **Slot numbers 1–9 per pane.** Tab numbers and `tab.goto` exist; a lane is reached
  through its card.

### Data model
```
Attention.mentions: [String]        unchanged shape; fewer lines, cleaned text (R1, R2)
ProjectCard += active: [{ id: String (≤ 40), title: String (≤ 120, one line),
                          status: in_progress | open | blocked }]      (≤ 3; R4)
ProjectCard.agents (after E-81) = E-81's roles: [{ role, state: running | ended |
                          unknown | waiting, kind, since, reports }]    (R6)
```
Nothing is stored; nothing is written to a project.

### API / interface contracts
- No new command for R1–R5: the card fields ride the `project_status` event; the Tasks
  button uses the existing tab switch and panel-open actions.
- **Command `pane_focus(tab, role)`** (R6) — `tab` is a core tab id; `role` must equal a
  role the core last sent for that tab. The core runs `select-pane` on the app's own
  tmux server for the pane id it holds for that role. Anything else answers the fixed
  sentence "that agent is not in the list". No pane id, path or text comes from the web
  side.

### Security
R1–R5 read no new file and add no command, process or network use. Mention lines are
project text: cleaned and cut by the core, drawn as text nodes; the cleaning removes
characters and never interprets markdown. R6 adds one tmux verb with a pane id the core
already holds, chosen by a role name compared for equality with the core's own list —
the web side cannot name a pane. Home still types into no terminal.

### Execution constraints
- R1–R2 run inside the existing 5 s facts read; at most 20 lines of 300 characters.
- R3–R5 add no poll, timer or animation. The 16-project frame figure of E-80 holds.
- `attention.rs` is at 930 of 1000 lines: its change is the one sentence. `shell.tsx`
  is at 898 of 900: R5 and R6 must not add a line to it without first moving something
  out (moves only, as E-97).

### Tests
- Core table test for R1 with every example above, plus: a numbered marker, a nested
  emphasis (`***Operator***`), "Operators:", "operator's keys" first on the line (not a
  mention: the word is `operator` followed by `'s` — count only when the word ends
  there), an empty line, a 2000-character line. R2: markers gone, words kept.
- Core: `active` order and cap; none without `state.json`.
- Web: the row model (sentence, quote, tooltip); the Tasks button's action.
- Window run: Home with a 300-character quote at 1280, 700 and 360 px wide —
  `scrollWidth` equals `clientWidth` on `#home` and on the document, read and printed.
- Screens (D-052): the three widths, both themes at 1280. The Architect opens each.

### Rollback
Each task is one commit and reverts alone. Reverting R1 brings the wide rule back; no
stored state depends on it beyond dismissal hashes, which only hide rows.

### Tasks
9. **E-101 (Tier 2)** — R1, R2, R3.
10. **E-102 (Tier 2)** — R4, R5. After E-101.
11. **E-103 (Tier 2)** — R6. After E-81.

### Addendum 2026-11-20 — R7: speak to an agent from its lane (D-071 addendum 1)

> Operator, asked whether Home may type into an agent and whether to ask rein for the
> model and context facts: "just do best". Both are taken as yes. This replaces "Not in
> this revision" for the first two points above and amends Home's Security sentence:
> **Home types into a terminal only through `pane_send`, on the operator's click.**

**Core concept.** The desk dashboard sends "continue", Escape or a message to any Claude
pane it lists. The app does the same for a lane, with one rule the dashboard does not
have: **the app never presses Enter in a pane it cannot tell is at an ordinary prompt.**
Enter at a permission prompt approves it. Approving is done in the pane, by the operator.

**Components.**
R7a. **Lane actions (web)** — each lane of R6 gains `Continue`, `Message…` and
   `Interrupt`. `Message…` opens a composer under the lane: a text area (at most 4000
   characters, counted), Send (`Ctrl+Enter`) and Cancel (`Esc`, which never reaches the
   pane). `Interrupt` asks once in place ("Interrupt the <role>?" Yes / No) and then
   sends Escape. After a send the lane says "Sent." or the fixed refusal for 2 s.
R7b. **When an action is offered (core decides, web only draws)** — per role, from
   E-81's `roles`:
   | state | Continue / Message | Interrupt |
   |---|---|---|
   | `running`, `reports` true | offered | offered |
   | `waiting` (any kind) | not offered: "Answer in its pane." and the lane goes there | offered |
   | `unknown` or `reports` false | not offered: "This agent does not report its state." | not offered |
   | `ended` | not offered | not offered |
   The core checks the same table again when the command arrives, on a fresh read of
   the pane's options — the web side's picture may be five seconds old.
R7c. **Delivery (core)** — `Continue` is the literal text `continue`. Text is placed in
   a tmux buffer through the process's standard input (never in an argument or a shell
   string), pasted bracketed into the role's pane, the buffer deleted, then one Enter.
   `Interrupt` is one Escape key. All on the app's own tmux server, to the pane id the
   core holds for the role.
R7d. **Switch** — `[home] actions = "shown" | "hidden"` (default `shown`), also in
   Settings. Hidden: no buttons are drawn and `pane_send` refuses every call.
R8. **Model, context and running tool on the lane** — drawn only when rein publishes
   them as pane options. Requested of rein's Architect 2026-11-20; no task until rein
   rules and names the options.

**Data model.** `roles[i] += can: { send: bool, interrupt: bool, why: String | null }`
(`why` is one of the fixed sentences of R7b). Nothing is stored: no message text is
written to a file, a log or the state file.

**API.** **Command `pane_send(tab, role, action, text)`** — `action` is `continue`,
`message` or `interrupt`; `text` only with `message`. `role` is checked as in
`pane_focus`. `text`: control characters removed except newline and tab, trimmed,
1–4000 characters after that, else "that message cannot be sent". A refusal by R7b
answers its fixed sentence. Answers `sent` or a fixed sentence; never tmux's output.

**Security (Tier 3 — `security_engineer` before the commit, SEC_CLEARED required).**
- The web side names a role, never a pane; the pane id never leaves the core.
- The text is data on standard input; no shell sees it. Bracketed paste keeps a newline
  from submitting early; removing control characters keeps an escape sequence from
  ending the paste.
- R7b is the guard against approving a permission prompt from Home. A pane that does
  not report gets nothing — the conservative side of "unknown".
- The command is reachable only from the app's own window (the existing IPC scope); no
  listener, port or token is added, unlike the desk dashboard's HTTP server.
- Threats to write down in the review: text from a project reaching `pane_send` (none
  may: only the composer's own text and the literal `continue`), a stale lane sending
  to a pane whose agent changed (the fresh read of R7b), a paste into a pane whose
  process is no longer the agent (state `ended` → refused).

**Execution constraints.** One send is at most four tmux calls and answers within the
time one status read takes; a second send to the same pane waits for the first. No
retry. No animation beyond the 2 s text.

**Tests.** Core: the R7b table for every state; hostile text (4001 characters, only
control characters, an ESC `[201~` inside the text, a NUL); the argv of every tmux call
is constant apart from the pane id; `hidden` refuses. Real tmux on Linux: a fixture pane
running `cat` with rein's options set to reporting receives a two-line message as one
paste and one Enter (the captured pane shows both lines, in order, once); the same pane
set to `waiting` receives nothing; Interrupt delivers one Escape. Web: composer limits,
`Ctrl+Enter`, `Esc` stays in the composer, pointer and key parity. Screens, both themes.

**Rollback.** One commit. `[home] actions = "hidden"` turns it off without a build.

**Task.** 12. **E-104 (Tier 3)** — R7a–R7d. After E-103.
