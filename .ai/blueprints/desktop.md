# BLUEPRINT — Desktop window ("Modeline") · Phase 2 · v1 2026-11-11

> Architect-owned. Decision record: D-033. Builds on `.ai/architect.md` §5–6 and the library
> seams in `distribution.md` §API. Contract items stay rein's: the open questions in
> §Open Questions go to `rein · Architect` and nothing here depends on their answers.

## Goal & Architecture
Give a Rein operator one window that opens a project's Architect and Engineer panes, shows
install health, and runs install / update / rollback / profiles — without hiding the terminal.
Users are people who live in tmux and nvim. One sentence: **a window that behaves like a
terminal program — drawn on the character grid, fully usable by keyboard (modal for those
who want it) and equally by mouse — wrapped around the user's own tmux session.** Users are
both people who live in tmux and nvim and people who expect to click (D-034).

Stack (fixed by `architect.md` §5): Tauri 2 (Rust core, system webview), xterm.js for the
terminal. New workspace crate `rein-desktop` (bin). The `rein-app` CLI crate gains no GUI
dependency and stays the same binary.

## Core Concept
**The window is one terminal plus three rows of chrome.** tmux stays the multiplexer: the
window owns a single pty whose child is `rein start` run in the project directory, so tmux
draws the Architect and Engineer panes itself and the operator's `tmux.conf`, prefix and
bindings work unchanged. The app never draws a pane split, never names a tmux session and
never mints a role (contract §3.5).

Everything else follows five design rules:

1. **Grid.** Every element sits on the cell grid of the configured monospace font. Borders
   are box-drawing glyphs. The window resizes in whole cells. No icon font, no sidebar, no
   menu bar, no toolbar.
2. **Modes, as in nvim.** `TERMINAL` (keys go to the pty — the default once a project is
   open), `NORMAL` (keys drive the app), `COMMAND` (the `:` line). The mode is the first
   statusline segment and sets its colour.
3. **Every action is a named command, reachable both ways (D-034).** A key, a click and a
   `:` line all resolve to the same closed list of actions. No action is reachable only by
   mouse and none only by keyboard. See "Two ways in" below.
4. **The UI teaches the CLI.** When an action runs a `rein` or `rein-app` command, the
   command line row shows the exact command; `y` in NORMAL copies it.
5. **One text file.** `config.toml` holds font, theme, keymap and window options and is
   reloaded on save; `:config` shows and copies the file's path, and the operator edits it
   in their own editor. The app never rewrites it. A small Settings overlay exists for
   people who prefer to click; it saves to the app's own `ui.json`, and the text file wins
   (D-034).

### Screen
```
 1:rein-app*  2:energy-api  3:docs                               rein 0.5.0   <- tabline
┌──────────────────────────────────┬─────────────────────────────────────────┐
│ ARCHITECT                        │ ENGINEER                                │
│                                  │                                         │  <- the pty:
│  (tmux draws all of this,        │                                         │     `rein start`,
│   the user's own tmux.conf)      │                                         │     drawn by tmux
│                                  │                                         │
└──────────────────────────────────┴─────────────────────────────────────────┘
 TERMINAL │ rein-app │ ✓ install ok │ C-\ C-n                             14:02   <- statusline
 :                                                                              <- command line
```
Overlays float centred over the stage with a rounded box border and a title in the border:
```
          ╭─ projects ──────────────────────────────╮
          │ > ene                                   │
          │   energy-api      ~/Dev/energy-api      │
          │   energy-docs     ~/Dev/energy/docs     │
          ╰──────────────────────────── 2/14 ───────╯
```
Start screen (no project open): the wordmark in block characters, the recent projects
numbered 1–9, and five key hints (`p` projects, `o` open directory, `h` checkhealth,
`u` update, `q` quit). Nothing else.

### Default keymap
| Mode | Keys | Action |
|---|---|---|
| TERMINAL | `C-\ C-n` (nvim's terminal escape) | `mode.normal` |
| NORMAL | `i`, `a`, `Enter` | `mode.terminal` |
| NORMAL | `:` | `mode.command` |
| NORMAL | `Space` | leader — after 400 ms a which-key overlay lists what can follow |
| NORMAL | `Space p` / `Space o` | `project.pick` / `project.open` |
| NORMAL | `gt` `gT` `1`–`9` | `tab.next` `tab.prev` `tab.goto N` |
| NORMAL | `Space h` | `health.show` (also `:checkhealth`) |
| NORMAL | `Space u` / `Space r` | `release.update` / `release.rollback` (each asks `y/N` on the command line) |
| NORMAL | `Space s` | `profiles.show` |
| NORMAL | `y` | `cmd.yank` (copy the last shown command) |
| NORMAL | `ZZ`, `:q` | `tab.detach` — closes the tab; the tmux session keeps running |
| NORMAL | `:qa` | `app.quit` — detaches every tab |
| overlay | `j` `k` `C-n` `C-p` `Enter` `Esc` | move, accept, close |
| NORMAL | `Space S` `Space c` `Space R` `Space t` `Space ,` `Space i` `Space Space` | `profiles.apply` · `config.open` · `config.reload` · `theme.set` (opens the menu at "Theme") · `settings.show` · `input.style` (toggle) · `menu.show` (D-047) |
`project.open` with no directory opens the system folder dialog in both styles; `:open DIR`
still takes a typed path.
The app binds nothing in TERMINAL mode except the escape chord, so no tmux or shell binding
is ever shadowed. The chord is configurable.

### Two ways in: click-first and modal (D-034)
The same window serves someone who has never used vim. Nothing is hidden behind a key.

**Input style** — `simple` (the default) or `modal`, chosen on the first-run screen ("How do
you like to work?" — two large choices, each with a three-line preview) and changeable any
time in Settings.
- `simple`: there are no modes to learn. Keys always go to the terminal. App shortcuts are
  the ones common terminals use: `Ctrl+Shift+P` menu, `Ctrl+Shift+O` open project,
  `Ctrl+Shift+T` project picker, `Ctrl+Tab` / `Ctrl+Shift+Tab` and `Alt+1`–`9` tabs,
  `Ctrl+Shift+W` close tab, `Ctrl+Shift+C` / `Ctrl+Shift+V` copy / paste, `Ctrl+=` / `Ctrl+-`
  / `Ctrl+0` font size, `F1` help, `Ctrl+,` settings, `Ctrl+Shift+Q` quit (D-047). The
  statusline shows no mode segment.
- `modal`: the keymap table above. The `simple` shortcuts keep working in every mode except
  where the user rebinds them.
Both are tables for the one `modes` function; switching style swaps the table.

**Everything in the chrome is a button.** Still drawn on the grid, with a hover highlight,
a pointer cursor and a tooltip naming the action and its shortcut in the current style:
```
 1:rein-app* ×  2:energy-api ×  3:docs ×   [+]                 rein 0.5.0  [≡]
```
- Tabline: click a tab to switch, `×` to close (detach), `[+]` to open a project through
  the system folder dialog, the version to open Update, `[≡]` to open the **menu**.
- **Menu** (`[≡]`, `Ctrl+Shift+P`, `:` in modal): one searchable list of every action in
  plain words ("Open a project…", "Check installation health", "Update Rein", "Undo last
  update", "Profiles", "Settings", "Help") with its shortcut at the right. Click or Enter
  runs it. This is the only place a newcomer needs to know.
- Statusline: each segment is a button — the project segment opens the project picker,
  health opens the health view. There is **no branch and no profiles segment** (D-047):
  a branch needs git or a read inside the project, a profile count needs a CLI run at
  every start (a `wsl.exe` call on Windows); tmux's own status line shows the branch, and
  Profiles is in the menu.
- Overlays: rows are clickable, the wheel scrolls, a click outside or `×` closes. Anything
  that changes the machine (update, rollback, apply profiles) ends in two labelled buttons,
  the safe one focused — `[ Cancel ]  [ Update to 0.5.1 ]` — and `y` / `n` still answer.
- Start screen: a large `[ Open a project… ]` button, the recent projects as a clickable
  list, and a `[ Check installation ]` button; the key hints stay beside them.
- Terminal: drag selects, right-click opens Copy / Paste / Select all, `Ctrl`+wheel changes
  the font size. The mouse is passed to tmux when tmux has mouse mode on (Shift bypasses,
  as in every terminal).
- **Plain words first.** Labels and messages say what happens ("Undo last update"), with
  the command shown beneath in dim text — rule 4 becomes how a clicker learns the CLI.
  Error lines keep rein's own text unchanged.

**Settings** (menu → Settings, `:settings`): a small overlay with input style, theme (live
preview), font size, title bar on/off, and a `[ Open config file location ]` button. Choices
made here are saved in the app's own `ui.json`; `config.toml` stays the operator's and is
never rewritten. A key set in `config.toml` wins, and Settings shows that row as "set in
config.toml" and read-only.

### For tiling window managers
`decorations = false` removes the title bar; the window class / app id is `rein-app` so
i3, sway and Hyprland rules can match it; `opacity` (0.6–1.0) is honoured where the
compositor supports it. Six built-in themes, each sixteen colours plus foreground,
background and accent: `gruvbox-dark` (default), `tokyonight`, `catppuccin-mocha`, `nord`,
`solarized-dark`, `github-light`. A `[theme.custom]` table overrides any colour. The same
sixteen colours are given to xterm.js, so the chrome and the terminal always match.

## Components
1. **`rein-desktop` (Rust, Tauri core)** — owns the window, the IPC command table, and the
   three services below. Calls `rein_release` and `rein_profiles` through the existing
   library seams; adds no second implementation of anything in Phase 1.
2. **`PtyHost`** — one pty per tab. Spawns exactly `<home>/.rein/bin/rein start` with
   cwd = the project directory, a clean environment (`CLAUDECODE`, `TMUX` and `TMUX_PANE`
   unset — with `TMUX` set `rein start` switches the outer tmux client instead of attaching
   in our pty; `TERM=xterm-256color`),
   and the pty size in cells. Streams output to the web side over a Tauri channel in chunks
   of at most 64 KiB; takes input bytes and resize events back. `detach` closes the pty
   master, which detaches the tmux client; the child is always reaped. A pty whose child
   exits shows the exit code in the stage and offers `Enter` to restart.
3. **`ProjectStore`** — the recent-projects list in the app's own state file. A directory
   is a project when it contains `.ai/`; the app reads nothing inside `.ai/`. Entries are
   canonical absolute paths; a path that no longer exists is shown dimmed, not removed.
4. **`Config`** — loads, validates and re-reads `config.toml` when its modification time
   changes (polled every 500 ms; no watcher dependency — D-044). An invalid file keeps the
   previous good config and shows the error (file, line, reason) on the command line.
   A key may be bound only to an action from the closed list; the file cannot name a
   program to run.
5. **`HealthService`** — runs `rein doctor --scope install --json` with cwd = the home
   directory (D-045 addendum 1: the install-scope document the CLI's own verdict reads,
   D-029; a full run judges the cwd as a project and fails on a healthy install) through
   the Phase 1 runner (same 4 MiB cap and strict parse; 120-second limit) when a tab
   opens, after update / rollback, and on `health.show`; never while a release or profile
   run is in progress. The segment says "install", and that is all it claims. Per-project
   health (full scope, cwd = the tab's project) is not in this revision. It displays the document's `status` and checks; it never recomputes a
   verdict and never branches on a check id (D-029, D-032). The strict doctor parser
   (`parse`, `Document`, `Status`, `check_lines`) moves from the `rein-app` bin crate into
   `rein-platform` — moved, not copied — and both use it.
   **Release and profile actions run the `rein-app` CLI (D-045)**, not an in-process
   library: one `Host` method `run_cli(action)` with a constant argv per action —
   `update --check`, `update --yes`, `rollback`, `profiles list`,
   `profiles apply [--dry-run] --profile N…`. `LocalHost` starts the `rein-app` binary
   found beside `rein-desktop` (else on `PATH`; resolved to an absolute path once at
   start), stdin closed, `CLAUDECODE` unset; `WslHost` runs the same argv inside WSL (W4).
   Lines are streamed to the overlay escaped and unchanged; the exit code is returned and
   is the only thing the window decides on. The lock, owner guard, snapshot swap, verdict
   and refusal texts are therefore the CLI's own, by construction. `update --check` prints
   `installed: <version>` and `newest: <version>` as its first two lines, then the notes;
   the window may display those values and nothing else is read from the text. One CLI
   run at a time per window. If the binary is not found: "rein-app is not installed
   beside the window" and health keeps working.
6. **Web UI (TypeScript + React; shadcn/ui for the click layer — D-037)** — shadcn/ui
   components (Radix primitives + Tailwind, copied into `crates/rein-desktop/ui/components/`
   and owned by us) are used for the menu (its Command component), dialogs and confirm
   buttons, tooltips, the context menu, the Settings overlay and the first-run screen. They
   are restyled to the grid: the configured monospace font, cell-multiple sizes and
   spacing, square or box-drawing borders, no shadows, no animation longer than 80 ms,
   colours only from the theme's sixteen-colour tokens (exposed as CSS variables, so a
   theme change restyles them live). shadcn's default look is not shipped. The tabline,
   statusline, command line and terminal stage are our own React components on the grid,
   not shadcn. The `modes` function stays pure TypeScript with no React import. Four modules: `grid` (cell metrics, box
   drawing, layout of the three rows and overlays), `modes` (a pure state machine:
   `(mode, pending keys, key) → (mode, pending keys, action?)`), `overlays` (picker,
   which-key, health, profiles, confirm), `term` (xterm.js instance per tab). The web side
   holds no state that is not derived from the core, apart from the mode and pending keys.

## Data Model
```
Action        = closed enum (Rust and TS generated from one list):
                mode.normal | mode.terminal | mode.command | project.pick | project.open
                | tab.next | tab.prev | tab.goto(n) | tab.detach | app.quit | health.show
                | release.update | release.rollback | profiles.show | profiles.apply
                | cmd.yank | config.open | config.reload | theme.set(name)
                | menu.show | settings.show | help.show | input.style(simple | modal)
                | font.bigger | font.smaller | font.reset | term.copy | term.paste
Tab           = { id: u32, project: ProjectPath, pty: PtyId | none, title: String }
ProjectPath   = canonical absolute path; printable, no control or bidi characters (D-024 rule)
ProjectsFile  = { schema: 1, recent: [{ path, last_opened }] }      (at most 64 entries)
ConfigFile    = { font: { family, size }, theme: { name, custom? },
                  window: { decorations: bool, opacity: f32, start: "dashboard" | "last" },
                  keys: { escape: [Chord], leader: Chord, normal: { "<keys>": ActionName } } }
HealthView    = { status: ok | warn | fail | unknown, checks: [{ id, status, line, fix? }],
                  taken_at }                      (`unknown` = no valid document, never a pass)
```
Files, by the XDG base directories (macOS uses the same paths under `~/Library/Application
Support/rein-app` only when the XDG variables are unset):
- `$XDG_CONFIG_HOME/rein-app/config.toml` — the operator's; created with commented defaults
  on first run, never rewritten by the app afterwards.
- `$XDG_STATE_HOME/rein-app/projects.json` — the app's; temp file + rename.
- `$XDG_STATE_HOME/rein-app/ui.json` — the app's (D-034):
  `{ schema: 1, input_style: "simple" | "modal", theme?, font_size?, decorations?, first_run_done }`;
  temp file + rename; an unreadable file is ignored and the defaults apply.
  Effective setting = `config.toml` value if set, else `ui.json`, else the default.
Nothing new is written under `$HOME/.rein` or inside a project.

## API / Interface Contracts
IPC commands (web → core). The table is closed; Tauri capabilities grant these and nothing
else (no `shell`, `fs`, `http` or `opener` plugin).
| Command | Input | Output | Errors |
|---|---|---|---|
| `tab_open` | project path | `Tab` + output channel | not a directory · no `.ai/` · rein not installed (offers `release.update`) · unsafe path |
| `pty_write` | pty id, bytes (≤ 64 KiB) | — | unknown pty |
| `pty_resize` | pty id, cols 2–500, rows 2–200 | — | out of range |
| `pty_ack` | pty id, bytes (u32) the web side has written into xterm.js since its last ack | — | unknown pty · more bytes than are outstanding (refused, the counter is unchanged) |
| `tab_detach` | tab id | — | — |
| `projects_list` / `projects_forget` | — / path | `ProjectsFile.recent` | — |
| `config_get` | — | `ConfigFile` + source path + last error | — |
| `health_get` | refresh: bool | `HealthView` | — (failure is `status: unknown`) |
| `release_check` | — | the lines of `rein-app update --check` + its exit code (0 newer · 10 current · 11 offline) | CLI not found |
| `release_apply` | `update` \| `rollback` | the CLI's lines (channel), then its exit code | a run is already in progress · CLI not found · every Phase 1 refusal, text unchanged |
| `profiles_plan` / `profiles_apply` | profile names (`[A-Za-z0-9][A-Za-z0-9._-]{0,63}` — never a leading `-`, which could be read as a flag; each present as a table row of `rein-app profiles list`) | the lines of `profiles apply --dry-run` / `profiles apply` + exit code | unknown or unsafe name · Phase 1 errors, text unchanged |
| `clipboard_write` | text (≤ 64 KiB); text only, through `arboard` (D-042) | — | — |
| `dir_complete` | absolute path prefix that passes the project-path check | at most 50 directory paths (directories only; hidden ones only when the typed stem starts with `.`; names failing the display-text rule dropped), through the `Host` seam | unsafe or relative prefix → empty list |
| `clipboard_read` | — (only on a paste action started by the user) | text (≤ 1 MiB) | — |
| `project_choose` | — | a directory picked in the system folder dialog, then validated exactly as `tab_open` validates | cancelled · same as `tab_open` |
| `ui_get` / `ui_set` | — / one field of `ui.json`, value checked against its type | effective settings, each marked `config` \| `ui` \| `default` | field set in `config.toml` (read-only) |
Events (core → web): `config_changed`, `health_changed`, `pty_exit { id, code }`.

CLI: `rein-app window [DIR]` starts `rein-desktop` (found beside the `rein-app` binary,
then on `PATH`) with DIR as the first tab; exit 1 with one line when it is not installed.
`rein-desktop [DIR]` may be started directly. A second start hands its DIR to the running
window as a new tab (single instance) and exits 0.

Command line (`:`): `:open DIR`, `:projects`, `:checkhealth`, `:update`, `:rollback`,
`:profiles`, `:config`, `:settings`, `:theme NAME`, `:q`, `:qa`, `:help`. Tab completes command names,
directories and theme names. An unknown command prints `E492: Not a command: <name>`.

## Security
Trust boundaries added by the window:
1. **Web view → core.** The web view loads only the bundled UI (`tauri://`), with a CSP of
   `default-src 'self'`, no remote origin, no `eval`, no inline script. Navigation and new
   windows are refused. Every IPC input is validated in Rust against the types above; the
   web side is treated as untrusted even though we ship it. `Object.prototype` is frozen
   by the UI itself right after its bundled modules load and before any pty output, IPC
   reply or event is handled (D-040, ratified D-044) — Tauri's `freezePrototype` stays off
   because xterm.js assigns over a prototype property while loading.
2. **Core → processes.** The only programs the window starts are `rein` from the installed
   tree (argv fixed per action: `start`, `doctor --scope install --json`) and the
   `rein-app` CLI (§Components 5, D-045). The app does **not** start `$EDITOR`: `:config` shows the file's path on
   the command line and copies it, and the operator opens it in their own editor; the
   watcher picks up the save. No IPC command takes a program name or an argv.
   The project path is passed as cwd, never interpolated into a command line.
3. **Terminal output → UI.** Pty bytes are untrusted. They go only to xterm.js. OSC 52
   (clipboard write from the terminal) is off by default and a config switch; hyperlinks
   (OSC 8 and detected URLs) are shown but not opened by the app. Tab titles and every
   string taken from a path, the doctor document or a manifest pass the Phase 1 escaping
   before display in the chrome.
4. **Config file.** Read-only input: bounded to 256 KiB, unknown keys are an error, key
   bindings resolve only to `Action` names, colours only to `#rrggbb`. It cannot start a
   process, load a file or change a URL.
5. **Settings boundary (Tier 3).** `profiles_apply` and `release_apply` run the `rein-app`
   CLI with constant argv (D-045); the window itself never writes a settings file or a
   Rein tree. So the three-key allow-list, the compile-time ban on `hooks`, `permissions`,
   `autoMode`, `defaultMode`, the install lock and the owner guard are the CLI's,
   unchanged. The only variable argv elements are validated profile names; no key name,
   flag or version string from the web side ever reaches an argv. The programs the window
   starts are now `rein` (installed tree) and `rein-app` (beside the window, else `PATH`).
6. **Pointer layer (D-034).** A click resolves to an `Action` through the same table as a
   key, so it gains no new power. The folder dialog is opened by the core itself with
   `rfd` (D-046, ratified D-047): **no Tauri plugin and no capability permission at all**
   — stricter than first planned (the dialog plugin would also bring the `fs` plugin).
   It is called from the IPC worker thread on Linux and Windows and from the main thread
   only on macOS (on GTK a main-thread call deadlocks the window). Its result is a path
   and passes the `tab_open` validation. Paste
   is sent to the pty only from a user gesture, and a paste containing a newline asks once
   ("Paste 3 lines?") before it is sent. `ui_set` accepts only the listed fields.
No secret is read, stored or shown. The window makes no network call of its own; only
`release_check` / `release_apply` reach the network, through the Phase 1 verifier.

## Execution Constraints
- First paint ≤ 400 ms on a warm start; a key in TERMINAL mode reaches the pty in ≤ 5 ms of
  app time; pty output is batched per animation frame, with back-pressure — when the web
  side is more than 4 MiB behind, the core stops reading the pty until it catches up.
  "Behind" = bytes sent on the channel minus bytes acknowledged through `pty_ack`; the core
  keeps the counter and never trusts an ack above what is outstanding (E-35, accepted
  2026-11-11).
- At most 16 tabs; scrollback is tmux's (xterm.js keeps 1 000 lines).
- One window per user session (single instance lock in `$XDG_RUNTIME_DIR`, falling back to
  the state directory).
- Platforms: Linux (X11 and Wayland) and macOS, and Windows 10/11 x64 as a native WebView2
  window over WSL2 (§Windows window, D-038). WSLg is **not** a supported way to run the
  Linux build (WebKitGTK paints nothing there — Engineer finding 2026-11-11). The bundled
  Node and notarization / code signing are later revisions and are not started by any task
  below.
- Dependencies each pass `skill: dependency_gate`: `tauri` 2, `portable-pty`, `toml`,
  `arboard` (text clipboard, D-042; `notify` is not used — D-044), `rfd` (folder dialog,
  D-046); web: `@xterm/xterm`, `@xterm/addon-fit`, `@xterm/addon-webgl`, `esbuild`,
  `typescript`, and (D-037) `react`, `react-dom`, `tailwindcss` (compiled at build time to
  one static stylesheet), the `@radix-ui/*` primitives of the components actually used,
  `cmdk`, `class-variance-authority`, `clsx`, `tailwind-merge`. Only the shadcn components
  named in §Components 6 are copied in; no icon set (glyphs are text), no other component
  library. The CSP is unchanged: a component that needs an inline `<style>` or script
  element is not used. One narrowing (D-044): Radix Dialog / Context Menu / Select try to
  insert a `<style>` element at runtime (scroll lock); the CSP refuses it and that is
  accepted **as long as nothing the user sees or does depends on the refused style** —
  the one rule that matters is shipped statically. A component that misbehaves without it
  is dropped, as before. Our own class names must not collide with a Tailwind utility
  (`border` did; a test fails on any class defined by both). Web bundle ≤ 1.5 MiB uncompressed, xterm.js included. The web
  bundle is built in CI from a committed lockfile.
- Hosted CI builds and tests `rein-desktop` on Linux, macOS and (from E-43) Windows.

## Look and feel v2 — modern chrome (revision 2026-11-12, D-049)
Operator feedback on the first Windows build: the grid-drawn chrome looks unpolished, a
console window opens beside the app, Nerd Font icons in tmux show as empty boxes, and
tmux's colours clash with the window. **This section supersedes design rule 1 ("Grid"),
the "Screen" drawings, the chrome parts of "Two ways in", and D-037 ruling 3** wherever
they describe how the chrome looks. Behaviour is unchanged: the `Action` list, both key
tables, the IPC table, `modes`, the Host seam and every security boundary stay as they are.

**Core concept: a modern desktop app around a terminal.** The chrome is ordinary
shadcn/ui (new-york style) in a proportional UI font with icons, rounded corners and
shadows; only the terminal stage is monospace. The terminal's background is the window's
background, so tmux and the chrome read as one surface.

### V1. Design tokens
- **UI font:** `"Segoe UI Variable Text", "Segoe UI", system-ui, -apple-system, sans-serif`,
  13 px (status bar 12 px). **Monospace** only inside the terminal and for commands/paths.
- **Shape:** radius 8 px (controls 6 px), 1 px borders in the `border` token, shadow on
  popovers, menus and dialogs, transitions 120–150 ms (none under
  `prefers-reduced-motion`). Spacing on a 4 px scale. No box-drawing glyphs in the chrome.
- **Colour:** each theme maps to the shadcn variables (`background`, `foreground`, `card`,
  `popover`, `muted`, `muted-foreground`, `primary`, `primary-foreground`, `border`,
  `ring`, `destructive`) by one pure function: `background` = the theme background,
  `card` / `popover` = one step lighter (dark themes) or darker (light), `primary` /
  `ring` = the accent, `destructive` = the theme's red. A test asserts ≥ 4.5:1 for text
  pairs in all themes.
- **Icons:** `lucide-react`, importing only the icons used (D-037 "no icon set" is
  withdrawn). Every icon-only button keeps its tooltip and accessible name.

### V2. Layout
```
┌────────────────────────────────────────────────────────────────────────────┐
│ ◆  [ schraubr_laravel × ] [ energy-api ] [ + ]      drag     0.5.0  ☰  ─ □ ✕ │ title bar 40 px
├────────────────────────────────────────────────────────────────────────────┤
│                                                                            │
│   terminal (tmux) — same background, 8 px side padding, no border          │
│                                                                            │
├────────────────────────────────────────────────────────────────────────────┤
│ ~/Dev/schraubr_laravel   ✓ Install OK              ⧉ rein start            │ status bar 26 px
└────────────────────────────────────────────────────────────────────────────┘
```
- **Title bar = tab bar.** Tabs are rounded pills: active tab on `card` with the accent
  as a 2 px underline, close icon on hover, middle-click closes, a dot when the pty has
  exited. Then a `+` icon button, a drag region, the version chip (opens Update; shows a
  dot when a newer version is known), the menu icon button.
  - **Windows:** `decorations: false`, `shadow: true` (keeps Windows 11's rounded corners
    and shadow); our own minimise / maximise / close buttons at the right (46 × 40 px,
    close turns red on hover); double-click on the drag region maximises. The capability
    file gains exactly `core:window:allow-start-dragging`, `allow-minimize`,
    `allow-toggle-maximize`, `allow-close`; the capability test lists them.
  - **macOS:** overlay title bar with the native traffic lights; the tab row starts after
    an 80 px inset.
  - **Linux:** native decorations by default with the same row beneath as a tab bar;
    `decorations = false` gives the row without window buttons (tiling WMs).
- **Stage:** the terminal fills the rest; background = `background`; 8 px left/right and
  4 px top/bottom padding. The window no longer snaps to whole cells.
- **Status bar:** left — the project path (dim, click opens the picker) and the health
  badge with an icon (check / warning / cross / question mark for `unknown`); right — the
  last command as a chip with a copy icon (rule 4, "the UI teaches the CLI"), and in
  `modal` style the mode badge. **No clock** (tmux shows one).
- **No permanent command-line row.** Messages (errors, `E492`, "copied") appear as a
  toast at the bottom right for 4 s (`aria-live="polite"`, our own small component). In
  `modal` style `:` opens the command palette anchored at the bottom with the same
  completion.
- **Overlays:** shadcn Dialog, Command, DropdownMenu, ContextMenu, Tooltip, Select,
  Switch — default new-york look driven only by the tokens. Confirm dialogs keep two
  labelled buttons with the safe one focused.
- **Start screen:** a centred column — wordmark, a primary `Open a project…` button, the
  recent projects as rows (folder icon, name, dim path, last opened), a secondary
  `Check installation` button, and the WSL setup card on Windows when needed.
- **Settings / first run:** the same content as today in shadcn form controls; the
  first-run choice shows two cards.

### V3. Fonts in the terminal (the empty boxes)
The app bundles, as web fonts served from the app itself (`font-src` is covered by
`default-src 'self'`):
- **JetBrains Mono** (regular, bold, italic, bold italic; OFL-1.1) — the default terminal
  font on every platform, so the default no longer depends on what is installed.
- **Symbols Nerd Font Mono** (MIT plus the icon sets' licences) — **always appended** to
  the terminal font stack as a fallback, so Powerline and Nerd Font glyphs from tmux, the
  shell prompt or Claude Code render whatever `font.family` the user sets.
Font files are vendored under `crates/rein-desktop/ui/fonts/` with their licence texts and
SHA-256 sums in a manifest that a test checks; they pass `skill: dependency_gate`. xterm.js
is opened only after `document.fonts.load` has resolved for the stack, and the glyph atlas
is rebuilt when fonts finish loading. Budget: fonts ≤ 3 MiB in total; the JS + CSS bundle
limit rises from 1.5 to 2 MiB.

### V4. Theme and tmux
- **Corrected 2026-11-12 (D-049 addendum 1):** rein ships **no** tmux colours (searched at
  rein 3994640). The status line in the operator's screenshot is the operator's own
  `~/.tmux.conf` (the catppuccin plugin, flavour mocha). So there is no `rein` theme.
- **The default theme becomes `catppuccin-mocha`** (already built in; it replaces
  `gruvbox-dark` as the default). The terminal background equals the window background
  in every theme. The six themes are unchanged.
- **tmux looks like part of the window when the window theme matches the user's tmux
  theme.** tmux's status line and borders are the user's own configuration; the app never
  configures tmux (contract §3.5) and rein does not either, so there is nothing to ask
  rein for. **Superseded by V6 (D-050):** the app now applies its own tmux overlay while
  attached, so no "pick the theme your tmux uses" line is shown.

### V6. tmux overlay — the app's own tmux look while it is attached (D-050)
Operator direction: "I don't want to touch my existing tmux config, but on top of it
rein-app should have its own setup that overrides while the app is open, so the app
feels natural." This replaces the last bullet of V4 ("pick the theme your tmux uses").

**Core concept.** When a tab's tmux client is attached, the app sets a closed list of
**session-scoped style options** on that session, coloured from the active window theme.
tmux itself undoes them the moment the app's client detaches. No file of the user's is
read or written, no global option is changed, no session is created, renamed or killed,
and the session is found through the app's own client tty — never by name.

- **Overlay script.** One compile-time-constant POSIX `sh` script, fed on **stdin** to
  `sh -s -- <client tty> <status mode> <colour>…` (so nothing passes through a shell
  command line or `wsl.exe`'s re-parsing except simple validated words). It:
  1. finds `tmux` on the login `PATH`; exits quietly if tmux is older than 3.2;
  2. waits up to 60 s for a client whose `client_tty` equals `$1`, and takes its
     `session_id`; no such client → exits quietly (nothing is styled). The wait (and the
     whole run, limit 75 s) ends at once when the tab closes or its terminal ends. (E-56,
     2026-11-12: the wait was 10 s; a fresh `rein start` builds the layout and waits for
     each pane's prompt before attaching, which can take longer, and the user's status
     line then stayed. Overlay runs still go one at a time, so a second tab's overlay can
     wait behind the first's — known limit; its output is never delayed.)
     **A failed overlay is told to the user:** when a tab's overlay ends without being
     applied (any reason other than the tab having closed), the window shows the toast
     once for that tab with the health row's text. The health badge is not changed by it
     (the badge is the install verdict, D-029);
  3. applies, with `set-option -t <session_id>` (and `-w` per window of that session, plus
     a session `after-new-window` hook for later windows), only these options:
     `status`, `status-style`, `status-left`, `status-right`, `window-status-format`,
     `window-status-current-format`, `message-style`, `message-command-style`,
     `mode-style`, `pane-border-style`, `pane-active-border-style`;
  4. installs a session `client-detached` hook that, when the detaching client is `$1`
     (`#{hook_client}`), unsets every option and hook it set — so a closed tab, a quit
     and a crash all restore the user's look without the app doing anything;
  5. is idempotent: running it again (theme change) replaces the previous overlay.
- **What it looks like.** Pane borders in the theme's `border` colour, the active pane's
  border in the accent; copy-mode selection and the tmux message line in the accent;
  status line per the setting below. `pane-border-format` and everything else rein or the
  user draws inside the borders is left alone.
- **Settings** (`config.toml` `[tmux]`, `ui.json`, the Settings overlay; `config.toml`
  wins as always): `overlay = true | false` (default `true`) and
  `status = "hidden" | "app" | "keep"` (default `hidden` — the window's tab bar and status
  bar already show the project; `app` = a minimal status line in the theme's colours
  showing only the window list; `keep` = the user's own status line untouched).
- **Client tty.** Linux / macOS: the pty's slave name from the pty layer. Windows: the
  start script becomes `tty; exec ~/.rein/bin/rein start`; the host reads the first 4 KiB
  of pty output for a line that is exactly a tty path and removes that line from what the
  terminal shows.
- **Where it runs.** `LocalHost`: `/bin/sh -s -- …`. `WslHost`:
  `wsl.exe -d D -e sh -ls -- …` with the same script on stdin. 15-second limit, output
  discarded except the exit code; any failure leaves tmux as the user has it and shows
  nothing but a line in the health view ("tmux overlay not applied: <reason>").
- **Validation before any process starts:** tty `^/dev/(pts/[0-9]{1,5}|ttys[0-9]{1,4})$`;
  status mode one of three words; colours `^#[0-9a-f]{6}$` from the app's theme table.
  Session and window ids used inside the script come from tmux's own output and are
  checked against `^[$@][0-9]+$` by the script.
- **As built (E-51, 0aa9206; D-050 addendum 1 — these replace the wording above where
  they differ):**
  1. IPC command `tab_tmux(tab, colours)` with the permission `allow-tab-tmux`. The web
     side sends only the five theme colours; the status mode comes from the settings. One
     script runs at a time; per tab only the newest request runs.
  2. The script takes a fourth, internal mode word `off`, sent when `overlay = false`, so
     switching the overlay off removes it from attached tabs at once.
  3. **The user's values are put back, not unset.** Each option's own session- or
     window-level value is saved first in `@rein-saved-<option>` (`1<value>` or `0` for
     "not set") and restored exactly on detach, on `off`, and before a re-run.
  4. `mode-style`, `window-status-format` and `window-status-current-format` are window
     options and are set per window, like the border styles.
  5. The detach hook compares the client inside tmux
     (`#{==:#{hook_client},<tty>}`); the client name never reaches a shell.
  6. Both hooks use the fixed index `[4271]`, which is what makes a re-run replace the
     earlier overlay and leaves the user's own hooks alone.
- **Known limits (accepted):** if the app's client switches session (`switch-client`,
  prefix-s) the overlay stays on the session it left until that session's next app
  detach or re-run — tmux looks `client-session-changed` up on the new session, so the old
  one never hears of it (security review F3, P2). On timeout only the direct child is
  killed (F5). The tty number can be reused during the 60 s wait (F7). Also: a plain terminal attached to the same session at the same
  time sees the overlay until the app detaches; the user's look flashes for a moment
  before the overlay lands; a status line built by a plugin with its own colours is
  replaced or hidden, not recoloured.
- **Security.** The process list gains `sh` (absolute `/bin/sh` locally) running one
  constant script; that script runs `tmux` with a closed option list. On Windows the tty
  comes from pty output, which is untrusted: the worst a forged line can do is style
  another tmux session of the same user. No command, format or option name comes from
  the web view, the config file or the terminal.
- **Contract note.** Our blueprint has said "the app never configures tmux" (our reading
  of rein contract §3.5). The operator, who owns both projects, directs this narrower
  rule: the app never creates, names, renames or kills a session and never mints a role;
  it may set the listed style options on the session its own client is attached to.
  `plans/rein-window-theme-request.md` carries the notice for `rein · Architect`.
- **Tests.** The script against a real tmux server on a private socket (`tmux -L`) in CI
  on Linux and macOS: options applied to the right session only; a second session
  untouched; detach of the app client restores every option and removes the hooks; detach
  of another client does not; second run replaces the first; tmux < 3.2 and "no such
  client" exit quietly. Unit tests: argument validation; the Windows tty line is consumed
  and a non-matching first line is passed through untouched. The user's `~/.tmux.conf`
  is never opened (asserted by running with `HOME` pointing at a directory whose
  `.tmux.conf` is unreadable).

### V5. Windows: no console window
`rein-desktop.exe` is built for the Windows GUI subsystem (release builds), so no console
window opens with it and none can close it. Consequences: a start-up failure with no
window (and a refused second start) is shown in a native message box instead of stderr;
every helper process (`wsl.exe`, `rein-app`) is started with `CREATE_NO_WINDOW` — a test
enumerates the spawn sites; the ConPTY child is unaffected.

### V7. Motion and loading states (revision 2026-11-12, D-054)
Operator direction: "add some loader animations; everything should be cool, minimal,
modern." Today every wait is silent — a new tab shows one line and a bare cursor for a few
seconds before tmux draws (E-53), the health check and the update dialogs give no sign
they are running, and the window opens with a flash.

**Core concept: every wait is shown, nothing else moves for its own sake.** Motion is
small (≤ 8 px, ≤ 220 ms), on `transform` and `opacity` only, and never delays an action:
the action starts at once and the animation runs beside it. The terminal's own content is
never animated, dimmed or covered — real output (an error from `rein start`) must stay
readable under any loader.

**Motion tokens** (added to V1's tokens, one place in the stylesheet):
| Token | Value | Used for |
|---|---|---|
| `--motion-fast` | 120 ms | hover, press, icon swap |
| `--motion-base` | 150 ms | tabs, toast, menus, tooltips |
| `--motion-slow` | 220 ms | dialogs, first paint of the window |
| `--motion-ease` | `cubic-bezier(0.2, 0, 0, 1)` | everything that enters or moves |
| `--motion-spin` | 800 ms linear, infinite | spinners |
| `--motion-sweep` | 1100 ms, infinite | the `ProgressLine` sweep (as built, E-54) |
| `--loader-delay` | 150 ms | a loader is not shown for a wait shorter than this |

**Components**
1. **Loading primitives** (one file, three small components, CSS only):
   - `Spinner` — the lucide `loader-circle` icon turning at `--motion-spin`; sizes 12 / 14 /
     16 px; takes the colour of its text.
   - `ProgressLine` — a 2 px indeterminate line in the accent colour that sweeps left to
     right across its container.
   - `Skeleton` — a rounded `muted` block with a slow opacity pulse (1.2 s), for rows whose
     content has not arrived.
   Each appears only after `--loader-delay`, so a fast operation shows no flicker.
2. **Tab start state** (the E-53 wait). A tab is `starting` from `tab_open` until the first
   of: the terminal switches to the alternate screen (tmux has drawn), the tab has received
   4 KiB of pty output in total, the pty exits, the user types into the tab, or 20 s pass.
   (The 4 KiB rule is the fallback for older ConPTY builds that draw the alternate screen
   themselves and never forward `ESC[?1049h` — E-53 measured `rein start`'s own lines at
   under 1 KiB and tmux's first frame at about 5 KiB.) While `starting`: a `ProgressLine` runs along the
   top edge of the stage directly under the title bar, the tab pill shows a `Spinner` in
   place of its close icon, and the status bar shows `Starting rein…` where the last
   command chip sits. The terminal stays fully visible beneath. A pure function
   `tabPhase(events) → starting | running | exited` holds this rule; the UI only renders it.
3. **Busy states of existing controls.**
   - Health badge: `Spinner` + `Checking…` while a health run is in flight; the result
     icon and colour cross-fade in (`--motion-base`).
   - Update / rollback / profiles dialogs: after the confirm button is pressed it shows a
     `Spinner` and its progressive label (`Updating…`, `Rolling back…`, `Applying…`) and is
     disabled; the other button stays usable where cancelling is possible today.
   - Start screen: recent projects show three `Skeleton` rows until the list arrives; the
     WSL card shows `Spinner` + `Checking WSL…` until its check returns.
   - Project picker (`dir_complete`): a `Spinner` at the right of the input while a
     completion is pending.
   - Copy chip: the copy icon swaps to a check for 1.2 s after a copy.
4. **Transitions.**
   - Window: created with the theme's background colour on the Rust side (no white
     flash); the UI root fades in over `--motion-slow` on first paint.
   - Tabs: a new pill grows in and a closed one shrinks out (`--motion-base`); the accent
     underline slides between pills.
   - Dialogs, command palette: fade + scale 0.98 → 1 (`--motion-slow` in, `--motion-fast`
     out). Menus, tooltips, context menu: fade + 4 px slide (`--motion-base`).
   - Toast: slides up 8 px and fades in; fades out.
   - Buttons and rows: background / border colour over `--motion-fast`; pressed buttons
     scale to 0.98.

**Data model.** No stored data. One derived UI value per tab (`phase`) and one boolean per
running operation (`busy`), both in memory. No `config.toml` or `ui.json` key is added:
reduced motion follows the operating system setting only.

**Interface contracts.** No IPC command is added; the capability file is unchanged. One
answer changed (E-54, for V6's failed-overlay toast): `tab_tmux` returns `string | null` —
the reason when the overlay was not applied, `null` when it was or the tab closed. The
reason is one of the app's own fixed sentences chosen by the script's exit code, never
text from tmux, the pty or a path. As built: the completion spinner sits on the `:open`
palette (the only caller of `dir_complete`), not the picker; debug builds honour
`REIN_DESKTOP_SLOW_LIST_MS` (≤ 30 s) to hold the recent list back for screenshots, release
builds ignore it. `tabPhase` reads what the UI already has: the `tab_open` call, the terminal's
buffer-change event, pty exit, key input. The window background colour is set in the
existing window builder from the theme already loaded at start.

**Security.** Presentation only. The CSP stays as it is: every animation is a class and a
keyframe in the bundled stylesheet — no inline `style`, no runtime-injected style, no
animation library, no new dependency. Loader text is fixed strings; nothing from the pty,
a path or rein is put into an animated label.

**Execution constraints.**
- Animate `transform` and `opacity` only (the tab pill's width is the one exception). No
  animation on or over the terminal canvas; the `ProgressLine` sits in its own 2 px strip.
- No animation may run while nothing is loading: an idle window has zero running
  animations (a test counts them).
- `prefers-reduced-motion: reduce`: no movement, fades become instant; `Spinner` is the
  still icon, `ProgressLine` a still 2 px accent line, `Skeleton` does not pulse. The
  text labels (`Starting rein…`, `Checking…`) carry the meaning, with `aria-busy` on the
  region and `role="status"` on the label.
- Bundle limit of E-49 (JS + CSS ≤ 2 MiB) still holds.

**Tests (V7).** `tabPhase` unit test for each exit (alternate screen, pty exit, key, 20 s)
and for output that arrives before the alternate screen; loader-delay test (an operation
that ends in 100 ms shows no loader); a stylesheet test that every `animation` and
`transition` is switched off under reduced motion and that an idle start screen and an idle
project tab have no running animation; pointer-parity, newcomer-path, accessibility-name,
IPC and capability tests unchanged; screens gate (D-052) with the loaders frozen mid-run.

**Rollback (V7).** Presentation only: revert the task's UI commit; the tokens and the three
primitives have no other users. The window background colour can stay.

### V8. The app's own tmux server (revision 2026-11-12, D-055)
Operator report on the E-54 build: the tab still picks up the tmux **plugins** from their
own config. Direction: inside rein-app, tmux is in rein-app's scope — none of the user's
tmux config or plugins applies. (Later, not in this revision: plugins from rein's own
list, switched on and off in Settings.)

**Why the overlay (V6) cannot do this.** tmux reads its config and starts plugins once,
when the *server* starts, for every session on it. Session options can hide what a plugin
draws; they cannot stop the plugin's scripts, key bindings and hooks.

**Core concept: sessions opened in the window live on a separate tmux server that has
never read the user's config.** The server is the tmux socket named `rein-app`, started
with rein-app's own config file. The user's own tmux server, `~/.tmux.conf`, plugins and
sessions are not touched, and `rein start` in an ordinary terminal behaves as before.

**How, without changing rein.** `rein start` runs plain `tmux` (no socket or config
option; checked in rein 0.5.0). tmux offers no environment variable for the config file,
so the window puts a small **shim named `tmux`** first on the `PATH` of the tab's process.
Every `tmux` that `rein start` — and anything in the panes — runs goes through it and
reaches the app's server. This is the same technique the E-53 / E-56 harness already uses
with the real `rein start`. A native option in rein (a socket and config named through
the contract) is requested in `plans/rein-tmux-server-request.md`; when rein ships it the
shim is dropped.

**Components**
1. **Tmux home** — the directory `$HOME/.local/state/rein-app/tmux/` (inside WSL on
   Windows; `XDG_STATE_HOME` is deliberately not read — the setup script and the start
   script run in different shells and must name the same directory), mode 0700, holding exactly two files written by the app:
   `bin/tmux` (the shim, 0700) and `tmux.conf` (0600). Both have constant content
   compiled into the app; they are rewritten (write to a temporary name, rename) whenever
   they differ. Nothing else is ever read from or written to this directory.
2. **Shim** (`bin/tmux`, POSIX `sh`, constant). It finds the real `tmux` as the first
   `tmux` on `PATH` that is not itself, then `exec`s it with `-L rein-app -f <tmux.conf>`
   before the caller's arguments. If the caller already passed `-L` or `-S`, the arguments
   go through unchanged. No real tmux → exit 127 with one line on stderr. It reads no
   other input and never evaluates text.
3. **Two constant scripts, split by transport** (amended 2026-11-13 on the Engineer's
   report: file text cannot safely pass through a `wsl.exe` argument). The same two on
   Linux, macOS and WSL.
   - **3a. Setup script — fed on stdin** (V6's transport: `/bin/sh -s`; on Windows
     `wsl.exe -d D -e sh -s`), run by the app before it opens the tab's pty, only when
     `tmux.server = "app"`. `umask 077`; refuses (own exit code each, mapped to fixed
     sentences as V6's are) a `HOME` with a newline, quote, space or glob character
     (`*`, `?`, `[`), a parent directory (`.local`, `.local/state`, `rein-app`) that is
     not the user's, is a symlink or is writable by group or others, a symlinked home or
     `bin`, a home not owned by the user or writable by group or others; creates
     0700 / 0700 / 0600; compares both files with the constants and rewrites those that
     differ. On failure the pty is not opened; the tab shows the fixed sentence and names
     the `tmux server` setting. The tab and its V7 start loader appear before setup runs,
     not after.
   - **3b. Start script — argv, short, no quote character** (replaces V6's
     `tty; exec ~/.rein/bin/rein start`):
     `set -o noglob; H=$HOME/.local/state/rein-app/tmux; for p in $HOME/.local $HOME/.local/state $HOME/.local/state/rein-app $H $H/bin; do [ -O $p ] && [ -d $p ] && [ ! -L $p ] || exit 126; done; for p in $H/bin/tmux $H/tmux.conf; do [ -O $p ] && [ -f $p ] && [ ! -L $p ] || exit 126; done; unset TMUX TMUX_PANE; PATH=$H/bin:$PATH; tty; exec ~/.rein/bin/rein start`
     (`set -o noglob` first: without it a glob character in `HOME` is expanded by
     `[ -O ]` and the check fails open. Not `set -f`: in zsh that letter is NO_RCS, not
     "no globbing" — `security_engineer`, 2026-11-13).
     A tab whose process exits 126 before any output shows a fixed sentence, not a bare
     closed tab.
   - **Rule for every argv script, now and later:** no quote character, no file text;
     anything longer or carrying content goes on stdin.
4. **App config** (`tmux.conf`, a closed list — no `run-shell`, `source-file`, `if-shell`,
   hook or plugin line): `default-terminal "tmux-256color"`, RGB `terminal-features`,
   `mouse on`, `escape-time 10`, `focus-events on`, `history-limit 50000`,
   `set-clipboard external` (tmux's default; `on` would let any program in a pane write
   the host clipboard through OSC 52 — `security_engineer`, 2026-11-13), `status off`. Everything else is tmux's default (prefix `C-b`).
   **Pane role labels (E-58, amended 2026-11-13).** `rein start` sets no pane-border
   option (Engineer's read; my earlier sentence here was wrong) — the labels the
   operator saw came from their own config. The app config therefore also sets
   `pane-border-status top` and a `pane-border-format` built only from `#{...}`
   variables, reading whatever rein uses to name a pane's role. `#(` (a command) is
   refused anywhere in the file by the closed-list test. If rein names the role nowhere
   tmux can read, no format is invented; the question goes to rein.
5. **Overlay and every other tmux call of the app** (V6's script, the detach hook) use the
   same server: the real tmux with `-L rein-app`. V6's rules are otherwise unchanged; the
   "put the user's values back" step now restores the app config's values.
6. **Setting** `[tmux] server = "app" | "user"` in `config.toml` and Settings ("tmux
   server: rein-app's own / my own"), default `app`. `user` is today's behaviour: no shim,
   the user's default server, V6 overlay on top. A change applies to tabs opened after it.

**Data model.** One new enum value pair in `config.toml` / `ui.json` (`tmux.server`). Two
constant files in the tmux home. No session name, socket path or plugin list is stored.

**Interface contracts.** No IPC command is added. `settings_get` / `settings_set` carry
the new key under the existing closed key list (the key-list test gains it). The start
script and shim are constants; the only values put into them are the tmux home path and,
on Windows, nothing new. From an ordinary terminal the app's sessions are reached with
`tmux -L rein-app attach` (operator guide).

**Security (Tier 3).** A directory is put first on `PATH` and a script in it runs for
every `tmux` call, so: the directory and files are created 0700 / 0700 / 0600 with
`umask 077`; the start script refuses a tmux home it does not own or that others can
write, and refuses a symlinked home or `bin`; file contents are compiled constants,
compared and rewritten on every start, so an edited shim does not survive; the shim takes
nothing from the environment except `PATH` and `HOME`; a path with a newline, quote or
space in it is refused rather than escaped. Between setup and start only the same user
can change the files; the start script re-checks owner and symlinks just before use. The app config contains no
command-running line, and the test that asserts this reads the file itself. On Windows
the process list is still only `wsl.exe` with constant scripts. No network. `security_engineer`
review required.

**Execution constraints.** Setup is one extra short process per tab open: negligible on
Linux and macOS, about 0.4 s on Windows (one `wsl.exe` start, by E-53's numbers; to be
measured in the task's interop dump). It runs off the UI thread with a 15 s limit. The app's server
keeps running after the window closes, like the user's did — sessions survive. The app
never kills the server.

**Known limits.**
- A project that already has a session on the user's own tmux server (started before
  this change, or by `rein start` in a terminal) is **not** found by the window any
  more: opening it in the window starts a second session, with a second Architect and
  Engineer, on the app's server. The window cannot detect this without rein's session
  rules. The operator guide says so; the request to rein asks for a way to detect it.
- `tmux ls` in an ordinary terminal does not list the window's sessions.
- The user's prefix key, bindings and mouse settings do not apply in the window.
- The shim covers programs started from the tab. A program that calls tmux by absolute
  path bypasses it and reaches the user's server. rein 0.5.0 does not (Engineer's read,
  2026-11-13: every call is plain `tmux`), and inside a pane `$TMUX` already points at
  the app's server.
- A `HOME` with a space in it is refused; the user sets `tmux.server = "user"`.

**Tests (V8).** Shim: real-tmux lookup skips itself; `-L` / `-S` pass through; exit 127
without tmux; config has no command-running line. Setup script: refuses a home that is
group-writable, not owned, or a symlink, and a `HOME` with a space; rewrites an edited
shim. Start script: exits 126 when the shim is missing, a symlink, or `XDG_STATE_HOME`
is set to another place (it must still use the `HOME` path); the constant contains no
quote character. End to end
(`tests/e2e/`, Linux + macOS in CI): with a user `~/.tmux.conf` that sets a marker option,
binds a key and has a `run-shell` plugin line that creates a file — after a tab starts,
the session is on the `rein-app` socket, the marker and binding are absent, the file was
not created, and no session was added to the default server; with `server = "user"` all
three are present. The V6 overlay cases run against the app's server. Windows: the E-53
harness through `wsl.exe` with the operator's own config, fresh start, dump under the
task's screens folder. Screens gate (D-052) with the operator's config in place.

**Rollback (V8).** Set `tmux.server = "user"` (per user, no rebuild), or revert the task's
commit; the tmux home can be deleted at any time and is recreated. Sessions on the app's
server keep running and are reachable with `tmux -L rein-app`.

**Deferred: rein's plugins.** A later revision adds tmux plugins from a fixed list
published by the rein project (pinned commits, checked before use, on/off in Settings,
loaded only into the app's server). Nothing in V8 downloads or loads a plugin.

### V9. Project lifecycle: set up, out of date, agent exited (revision 2026-11-13, D-057)
Operator reports on the E-58 build: (1) `/exit` in a Claude pane leaves a bare shell in
the tmux pane and the project stays open; the window "should be smart enough to close
the whole project". (2) Opening a folder with no `.ai/` should say what is missing and
offer to set it up. (3) A project whose `.ai/` is from an older Rein should offer to
bring it up to date.

**Core concept: the window notices three project states and offers the one rein command
that fixes each; it never fixes anything silently.** The rule that bounds all three:
**the window runs only rein's own entry points (`rein init`, `rein sync`,
`rein start --kill`), each after the operator's click, as constant commands.** Anything
else a project needs (a git repository, Node, a `package.json` if rein asks for one) is
listed with the command to copy, and not run by the window: rein-app writes no file
inside a project (contract §3.3) and carries no copy of Rein logic. What is missing or
out of date is **rein's verdict** (`rein init --check`, `rein doctor --json`,
`rein start --status`), never a comparison made by the app.

**Components**
1. **Project check** — when a tab opens, before the pty: is there a `.ai/` directory in
   the project? On Windows this rides in the process that already runs before the pty
   (V8's setup) when there is one; it never adds a second process to a normal tab open.
   Present → the tab starts as today. Absent → component 2.
2. **Set-up card** (in the tab, in place of the terminal; shadcn `Card`): "This folder
   is not set up for Rein." Then the list of what set-up will do and what is missing, as
   rein reports it (`rein init --check`; prerequisites rein names), each as a plain row.
   Buttons: **Set up this project** (primary), **Open another folder**, **Close tab**.
   Set up runs `rein init` **in the tab's own terminal**, so its output and any question
   it asks are visible and answerable; on exit 0 the tab goes on to `rein start` in the
   same terminal; on any other exit the terminal stays with the output and a fixed line
   above it, and nothing is retried. A prerequisite rein does not install is shown with
   its command and a copy button; Set up stays disabled until a re-check finds it.
3. **Out-of-date bar** — after the tab has started (never delaying it), one background
   run of rein's own check for the project. If rein says the project needs `rein init`
   (upgrade) or `rein sync`, a one-line bar above the terminal names what rein reported
   and offers the command rein itself names as the fix: **Run rein sync** / **Update
   project**, **Not now**, **Don't ask for this project**. The command runs through the
   existing CLI runner with its output in a sheet (as update / rollback do), not typed
   into a pane. Whether the agents must be restarted afterwards is said as rein says it.
   A fix that is not one of the two commands is shown as text only.
4. **Agent-exit bar** — while a tab is the active one and the window is visible, the
   window asks rein (or tmux on the tab's own server, see "Open to the Engineer") which
   role panes still run their agent. When a role's agent has ended:
   - one of several ended: a bar "The Engineer has exited." with **Close project** and
     **Keep open**. Nothing else happens by itself.
   - **all** role agents ended: the project is closed without asking when
     `[project] close_when_agents_exit = true` (the default); otherwise the same bar.
   **Close project** = `rein start --kill --yes` for that project on the tab's tmux
   server, then the tab closes. With an agent still running, Close project first asks
   "The Architect is still running. Close it too?" (confirm dialog, default No).
   Closing a tab the ordinary way is unchanged: the session keeps running (D-034).
5. **Setting** `[project] close_when_agents_exit = true | false` and the per-project
   "don't ask" list (`ui.json`, project path keys as the recent list already keeps).

**Open to the Engineer — read rein 0.5.0, report, then build** (no guessing):
- what `rein init` needs beforehand (git? Node? a `package.json`?), whether it asks
  questions, and what `rein init --check` prints for a folder with no `.ai/`;
- which of rein's outputs says "needs sync" / "needs init upgrade" for an existing
  project, and whether it names its own fix;
- whether `rein start --status` reports per-role agent liveness in a form that can be
  read without parsing prose; if not, the fallback is one `tmux list-panes -F` with a
  constant format (`@rein_role`, `pane_current_command`, `pane_dead`) on the tab's
  server through the overlay's stdin transport — and the gap goes into the rein request;
- whether `rein start` on a session with one exited agent restarts that agent. If it
  does, the bar gains **Restart the Engineer**; if not, restart stays out of this
  revision and is asked of rein.

**Data model.** One config key, one `ui.json` list. No project state is stored; each
verdict is asked again on the next open.

**Interface contracts.** New IPC, all with the project taken from the tab's state and
no path or command text from the web side: `project_check(tab_id)` → `ready |
not_set_up {rows} | unavailable`; `project_setup(tab_id)` (starts `rein init` in the
tab's pty); `project_fix(tab_id, which)` with `which` ∈ {`sync`, `init`};
`project_agents(tab_id)` → per role `running | ended | unknown`;
`project_close(tab_id, confirm_running)`. Rows and reasons that come from rein's output
are escaped lines, shown as text (the rule the CLI runner already follows).

**Security.** Every command is a constant argv of a rein entry point (V8's argv rule: no
quote character, no file text). A click is required for each run; nothing in a project's
files can trigger one. `rein start --kill` is destructive for a running agent, hence the
confirm. Rein's output is untrusted text: text nodes only, capped. A folder that is not
a directory the user picked or typed is never offered set-up. Changes on the `WslHost`
path need interop output (standing rule).

**Execution constraints.** A normal tab open gains no process and no measurable delay.
The out-of-date check runs once per tab open, off the UI thread, limit 30 s, result
dropped if the tab closed. The agent poll: active tab only, window visible, at most one
short process per 5 s, stopped the moment the tab's process ends.

**Tests.** Pure: the bar and card states from each verdict; "all ended" vs "one ended";
the confirm rule. Rust: the constants are argv-safe; every verdict maps to a fixed
sentence or an escaped row. End to end (Linux, macOS; a stand-in `rein` as E-57 used):
a folder without `.ai/` shows the card, Set up runs init then start in the same
terminal, a failing init stays visible; a stale project shows the bar and runs the named
command; an exited agent shows the bar; all exited closes the project and kills the
session on the tab's server only — a session of the same name on the default server is
untouched. Screens gate (D-052): the card, both bars, the confirm, both themes.

**Rollback.** `close_when_agents_exit = false` and "Don't ask for this project" switch
the prompts off per user; each task reverts as one commit. No file of the app's is left
in a project.

**Asked of rein** (added to `plans/rein-panels-request.md`): `rein pane` should end the
pane (or say so in `--status`) when its agent exits; `rein init` should cover every
prerequisite it can, and name the rest in `--check`; a machine-readable project verdict.

**Ruling for components 2–3 after the read of rein 0.5.0 (2026-11-13, E-63).**
- The out-of-date verdict is `rein init --check`'s exit code, run once per tab open in
  the background, after the tab has started: 0 → nothing; 1 → the bar; anything else or
  a timeout → no bar and one fixed health line. The app compares nothing itself.
- The bar has one fix: **Update project** = `rein init`. There is no "Run rein sync"
  button: rein gives no machine verdict for sync, and `rein init` covers what sync
  refreshes. Component 3's "Run rein sync" is withdrawn until rein has `sync --check`.
- The rows are `--check`'s own lines as text nodes: at most 40 rows, 200 characters
  each, control and bidi characters as placeholders, a "… and N more" row past the cap.
- **Update project** runs through the CLI runner with output in a sheet, with a limit
  long enough for a cold `npm ci` (10 minutes) and a visible Cancel. After exit 0 the
  check runs once more: 0 → the bar goes and one fixed line says running agents keep
  their old settings until the project is closed and opened again; 1 → one fixed line,
  no second offer in this tab. No loop.
- **Not now** hides the bar for this tab. **Don't ask for this project** is kept
  (`ui.json`).
- The set-up card shows rein's one line and nothing the app knows about rein: no git or
  Node notes. Its own fixed text says only what the click does: it runs `rein init` in
  this terminal, then starts the project. Set up is never disabled, since rein names no
  blocking prerequisite.
- Provisional: the exit-code meaning of `init --check` is not in the contract (asked of
  rein). If rein changes it, the bar fails to silence, not to a wrong offer.

**Rework ruled on review of E-63 (2026-11-13) — built in d03b0b2; E-63 accepted at 1f5a3a8.** Windows: the parent search is exit 6 of the one validation call. The blank failed-set-up terminal was the window (text written, not drawn); a repaint ends it and the screens script now fails on a blank shot, but the cause inside xterm is not known.
- **Update project asks before it runs.** `rein init` replaces `ENGINEER.md`,
  `ARCHITECT.md`, `CLAUDE.md`, `.claude/agents` and skills with no backup (read in rein
  0.5.0). The click on the bar opens the sheet with `--check`'s rows, the fixed line
  "rein init replaces the files it lists as refresh. Edits you made in those files are
  lost — commit them first." and **Run rein init** / **Cancel**, Cancel focused.
  Nothing runs before that click. The app names no file itself and parses no row.
- **A folder inside a Rein project gets no set-up offer.** The existing validation call
  also looks for `.ai/` in the parents (a constant loop, no new process). Found → the
  card says "This folder is inside a Rein project." with the parent's path, **Open that
  project** as the primary button, and no Set up button.
- **A failed set-up must show rein's output and the fixed line** in the terminal; the
  E-63 screens show an empty terminal.
- Accepted as built: `{setup}` from `tab_open`, the same tab id through set-up, exit
  119, at most 2 checks per tab and stage, `project_fix_cancel`, `update_dont_ask`
  (≤ 500), one validation call per open on Windows.

**As built — agent exit (E-62, b8fc0d6 + 81f141a; accepted 2026-11-13).**
- Status is the fallback: a constant read-only script on stdin (`sh -s -- <client tty>
  <app|user>`) lists the panes of the window the tab's own client shows, by window id,
  with `#{@rein_role}|#{pane_current_command}|#{pane_dead}`. At most one such process
  per 5 s for the whole window, enforced in Rust.
- A role is `ended` only after it was seen `running` in this tab. Running = the pane is
  alive and its command is not a shell from a fixed list. (Checked on a live machine:
  real agent panes report `claude`.) If an agent's command were ever a shell name the
  bar would never show — the failure is silence, never a wrong close.
- `project_close(tab, confirm_running)` answers `{closed}` or `{confirm: [roles]}`; a
  status that is not known also asks. The close the app starts by itself (all ended)
  never asks and never kills a role that is running again — it shows the bar instead.
- The close is `rein start --kill --yes` in the tab's project through the tab's own
  start path (the V8 start constant plus ` --kill --yes` on the app's server).
- "Keep open" hides the bar until the set of ended roles changes.
- No Restart (rein has no safe single-agent restart). No per-project "don't ask" for
  this bar; the setting is per user.
- Known and accepted: `rein start --kill` itself rewrites missing
  `.claude/settings.<role>.json` in the project before it kills (rein's write, not the
  app's; reported to rein).

### V10. Project sheet: the team (revision 2026-11-13, D-058)
Operator direction: rein's team commands (who is on the project, their id ranges, team
mode) should be usable from the window, "settings or something".

**Where it lives.** Not in Settings: Settings is the app's own, per user; the team is
one project's, kept by rein in that project (`.ai/team.json`). Not a side-menu panel:
panels never run a command (`panels.md`). It is a **Project sheet** — the same `Sheet`
the update and profiles views use — opened for the active tab's project from the menu
("Project: team…"), a key in both styles, and a "Project" button at the foot of the
side-menu rail once the rail exists. V10 gives it one section, Team; later
project-scoped things (accounts bound to the project, gate status) go here too.

**Core concept: the sheet is a form over rein's own team commands.** It shows what
`rein team show` and `rein trunk` say, and its buttons run `rein team add` and
`rein state migrate --team | --legacy`. rein-app never reads or writes `.ai/team.json`
itself and keeps no copy of the team.

**Components**
1. **Team view** — on open, one run each of `rein team show` and `rein trunk`: the
   developers with their id ranges, the trunk, and whether the project is in team mode.
   If rein's output cannot be read as rows without parsing prose, it is shown as rein
   printed it (escaped text, monospaced) and the form below still works.
2. **Add a developer** — two fields: e-mail, and id range as prefix + from + to. Checked
   in the web side for help text and **again in Rust, which is the only check that
   counts**: e-mail `[A-Za-z0-9._%+-]{1,64}@[A-Za-z0-9.-]{1,190}`, no leading `-`;
   prefix one of rein's task prefixes; `from` ≤ `to`, both 1–99999. Runs
   `rein team add <email> --range <P>:<from>-<to>`; rein's answer (an overlapping range,
   an unknown prefix) is shown under the form as rein printed it.
3. **Team mode switch** — "Turn on team mode" / "Go back to single-developer mode" =
   `rein state migrate --team [--trunk <name>]` / `--legacy`. A confirm dialog says in
   plain words what rein will do to the project's files (from rein's own description),
   that the change must then be committed, and that the side-menu Tasks panel does not
   read team-mode projects yet. Trunk, when asked for, is picked from the branch names
   rein reports, never typed freely.
4. **Not offered, because rein has no command for it:** changing or removing a
   developer, changing a range. The sheet says so in one line and names the file; the
   request to rein asks for the commands.

**Data model.** None in the app. No setting. The two value shapes above are the only
user text that ever becomes a command word.

**Interface contracts.** IPC, project always from the tab's state: `team_get(tab_id)` →
`{rows | text, trunk, mode}` or a fixed "not available"; `team_add(tab_id, email,
prefix, from, to)`; `team_mode(tab_id, mode, trunk?)`. Each returns rein's exit code and
escaped output lines through the existing CLI runner's event stream.

**Security (Tier 3).** This is the first place user-typed text becomes part of a command.
- Each value is validated in Rust against the closed patterns above and passed as **its
  own argv word**, never placed inside a script string. Where the transport cannot pass
  a separate word safely (the Windows `wsl.exe` path — Engineer shows which), the values
  travel on **stdin** to a constant script that re-validates them with the same patterns
  before use. No quote character can appear in an accepted value, so V8's argv rule holds.
- A value starting with `-`, or holding whitespace, a quote, `$`, a backtick, `;`, `|`,
  `&`, a newline or a non-ASCII character, is refused with a fixed sentence.
- Every run needs a click; the migrate needs the confirm. rein's output is untrusted
  text: text nodes only, capped at 200 lines.
- `security_engineer` review; Windows interop output with hostile values (standing rule).

**Execution constraints.** Commands run off the UI thread through the CLI runner, one at
a time per project, limit 60 s (migrate 5 min). Nothing runs until the sheet is opened.

**Tests.** Validation table in Rust (accepted and refused values, including every
character above, a 300-character e-mail, `--range` as an e-mail, a range of `0-0` and
`5-1`); the argv built for each transport holds the value as one word and no quote; end
to end with a stand-in `rein` that records its argv: add, an overlap error shown,
migrate with and without the confirm; the sheet on a project without `.ai/` says "not
set up" and offers V9's card. Screens gate (D-052): the sheet with a team, the add form
with an error, the confirm, both themes.

**Rollback.** The sheet only calls rein; reverting the task's commit removes it. A
migrate is undone by rein's own `--legacy` / `--team`.

**Contract note.** `rein team`, `rein state migrate` and `rein trunk` are not in the
contract's list of stable entry points (§3.6). The sheet is built against rein 0.5.0 and
fails soft on a different answer; the request to rein asks to add them, with `--json`.

### Tests (v2)
- Token function: contrast ≥ 4.5:1 for every text pair in every theme; the default theme is
  `catppuccin-mocha`.
- Capability test: exactly the four window permissions added, nothing else.
- Pointer parity, newcomer path and the IPC tests still pass unchanged.
- Font manifest sums; the terminal font stack always ends with the symbols font; a
  rendering check (`ux_reviewer`, Xvfb) that a Nerd Font glyph (U+E0B0, U+F07B) is drawn
  from the bundled font and is not a missing-glyph box.
- Windows: the release binary's PE subsystem is GUI (checked in the installer workflow).
- `ux_reviewer` screenshots of the start screen, a project tab, menu, Settings, health and
  a confirm dialog in `catppuccin-mocha` and `github-light`; **the screenshots are attached to the
  handoff for the operator** before the task is closed.

### Rollback (v2)
Presentation only: revert the UI commits to return to the grid chrome; the default-theme change
and bundled fonts can stay. The GUI-subsystem attribute is one line.

## Windows window (revision 2026-11-11, D-038)
**Core concept: Rein stays inside WSL; only the window is Windows.** `rein-desktop.exe` is
the same crate built for Windows (Tauri 2 on WebView2). It holds no Rein tree, no profile
file and no project file on the Windows side. Every process it starts is `wsl.exe` with a
fixed argv shape, and everything it knows about a project it learns by asking inside WSL.
The web UI, the IPC table, the `Action` list and the pty flow control are unchanged.

### Components (Windows)
W1. **`Host` seam** — one trait in `rein-desktop` behind which sit all the things that
   differ by platform: validate a project, build the pty child, run doctor, run the
   release / profile actions, name the app's directories. `LocalHost` is today's behaviour
   (Linux, macOS) moved behind it unchanged. `WslHost` is the Windows one. Platform `cfg`
   is confined to the two implementations, `dirs`, `single_instance`, `pty_sys` (pty
   writer / hangup / kill) and the one `platform_host()` constructor in `main.rs`. Process
   running and WSL detect / decode live in the shared crate `rein-platform`, used by both
   `rein-app` and `rein-desktop` (accepted from E-43).
W2. **`WslHost`** — owns the distro and every `wsl.exe` call.
   - Program: `%SystemRoot%\System32\wsl.exe` by absolute path, never a `PATH` search.
   - **State probe** at start and on `health.show`: `wsl.exe --status`, then
     `wsl.exe --list --quiet` (UTF-16LE decode — reuse the Phase 1 `wsl.rs` logic by moving
     `decode` / `detect` to a place both crates can use; do not copy it). Result:
     `Absent` | `NoDistro` | `Ready`.
   - **Distro**: `wsl.distro` from `config.toml` if set (must be in the list), else WSL's
     default. Its name, the Linux home and the login shell are learned once by
     `wsl.exe [-d D] -e sh -c 'printf %s\\n $WSL_DISTRO_NAME $HOME $SHELL'` and validated
     (name `[A-Za-z0-9._-]{1,64}`; home and shell absolute POSIX paths, printable ASCII, no
     space; shell falls back to `/bin/sh`). From then on every call passes `-d <name>`.
     One distro per window in this revision.
   - **Project validation** (replaces the local `.ai/` check): one call
     `wsl.exe -d D --cd <linux path> -e sh -c 'case $PWD in /) exit 5;; esac; [ -d .ai ] || exit 3; [ -x ~/.rein/bin/rein ] || exit 4; pwd -P'`.
     Exit 0 → stdout is the canonical path (re-validated as a `ProjectPath`); 3 → no `.ai/`;
     4 → rein not installed in WSL; 5 or anything else → not a directory. 30-second limit,
     64 KiB output cap.
     **`--cd` does not fail** (E-43 smoke, 2026-11-11): for a missing directory, a file or
     an unreadable directory `wsl.exe` prints `chdir(...) failed`, runs the command in `/`
     and exits 0. Hence the `case` guard (no quotes needed — a `case` word is not split),
     and the path `/` itself is refused as a project before any call. The pty child keeps
     its script unchanged (it runs in the user's shell, which may be fish — no `case`): if
     the directory vanishes between validation and spawn, `rein start` runs in `/` and
     fails with its own message, which the stage shows with the exit code as for any exit.
   - **Pty child** (ConPTY through `portable-pty`): exactly
     `wsl.exe -d D --cd <linux path> -e <shell> -lic 'exec ~/.rein/bin/rein start'`.
     The user's own shell, login and interactive, so `PATH` is what their terminal has
     (nvm, Homebrew). The script is one constant; the project path appears only as the
     `--cd` argument.
   - **Environment**: the child gets the Windows environment with `WSLENV` removed, so no
     Windows variable (`CLAUDECODE`, `TMUX`, anything else) crosses into WSL. `TERM` is not
     forwarded; WSL sets `xterm-256color` itself — a smoke check asserts that.
   - **Detach**: closing the ConPTY ends `wsl.exe`, which hangs up the tmux client; the
     session keeps running. `wsl.exe` is always reaped.
W3. **Windows paths → `ProjectPath`** — a pure function. A `ProjectPath` on Windows is the
   **Linux** absolute path inside the distro. Accepted inputs (argv, `:open`, folder
   dialog, single-instance handoff):
   - a POSIX absolute path (`/home/me/proj`) — used as is;
   - `\\wsl.localhost\<distro>\…` or `\\wsl$\<distro>\…` — mapped to `/…`; the distro must
     equal the window's distro (case-insensitive), else refused naming both.
   - Anything else (a drive path such as `C:\…`, a relative path, another UNC host) is
     refused with: "Rein projects live inside WSL — choose a folder under
     \\wsl.localhost\<distro>". `/mnt/c` projects are not supported in this revision.
   On `WslHost` a path containing `"`, `\`, a control or bidi character is refused before
   any call (`wsl.exe` re-parses its command line; we do not rely on escaping).
   The folder dialog opens at `\\wsl.localhost\<distro>\home\<user>`.
W4. **Health, release, profiles on Windows** — never the Windows binary's own
   `rein_release` / `rein_profiles` against a Windows home. All run inside WSL through the
   login shell with a constant script per action:
   | Action | Script (after `wsl.exe -d D -e <shell> -lc`) | Shown as |
   |---|---|---|
   | `health_get` | `cd; exec ~/.rein/bin/rein doctor --scope install --json` | parsed on the Windows side by the Phase 1 parser (4 MiB, strict, 120 s) |
   | `release_check` | `exec rein-app update --check` | the CLI's lines, escaped, unchanged |
   | `release_apply update` / `rollback` | `exec rein-app update --yes` / `exec rein-app rollback` | lines streamed; the CLI's exit code |
   | `profiles_plan` / `profiles_apply` | `exec rein-app profiles apply --dry-run --profile N…` / without `--dry-run` | the CLI's lines; exit code |
   **Directory completion on Windows (`dir_complete`, part of E-45):** one more fixed call
   shape, 5-second limit, 64 KiB cap — `wsl.exe -d D --cd <parent> -e sh -c 'case $PWD in /) exit 5;; esac; ls -1Ap'`,
   and for the parent `/` itself the same call with the script `ls -1Ap` (the `--cd`
   fallback cannot mislead there). Only lines ending in `/` are kept, then the same
   filter and 50-entry cut as on Linux. Any failure → an empty list, never an error line.
   Until E-45 `WslHost` returns none.
   Profile names are checked against `[A-Za-z0-9._-]{1,64}` and against the list from
   `rein-app profiles list` before they are placed in an argv. If `rein-app` is missing
   inside WSL (exit 127), the overlay says "rein-app is not installed inside WSL — install
   the Linux package there" and health still works. The CLI gains one additive flag,
   `rein-app update --check`: print the installed version, the newest version and its
   notes; install nothing; exit 0 newer available · 10 current · 11 offline.
W5. **Single instance (Windows)** — no named pipe. A lock file
   `%LOCALAPPDATA%\rein-app\instance.lock` held open without sharing for the window's
   life. A second start that cannot take it writes its DIR to
   `%LOCALAPPDATA%\rein-app\handoff\<random>.txt` (temp name + rename, ≤ 4 KiB) and waits
   up to 5 s for the window to delete it — deleted → exit 0; not → exit 1 with one line.
   The window polls the directory every 200 ms (accepted as the design — no watcher
   needed for one small private directory), validates the text as in W3 and opens a tab. The directory is private to the user by profile ACL; no listener
   is exposed to other users or the network.
W6. **Directories (Windows)** — config: `%APPDATA%\rein-app\config.toml`; state
   (`projects.json`, `ui.json`, lock, handoff): `%LOCALAPPDATA%\rein-app\`. XDG variables
   are ignored on Windows. Default font family on Windows: `Cascadia Mono, Consolas,
   monospace`.
W7. **Installer build** — a separate hosted workflow (`desktop-windows.yml`) on
   `windows-latest`: `workflow_dispatch` and pushes to `master` that touch
   `crates/rein-desktop/**`, `crates/rein-platform/**`, `Cargo.lock` or the workflow. `permissions: contents: read`,
   no secret, actions pinned as in `ci.yml`. It builds the UI from the lockfile, runs
   `cargo test -p rein-desktop`, builds the Tauri **NSIS** bundle only (per-user install,
   WebView2 bootstrapper, no MSI — the CLI owns the `.msi`), and uploads it as workflow
   artifact `rein-desktop-windows-x64-setup` (14-day retention). It is **not** a release
   asset: `release.yml` and the dist configuration are untouched, the installer is
   unsigned (SmartScreen will warn) and is a test build for the operator. Passes
   `skill: ci_gate`.

### Data model changes (Windows only)
```
WslState     = Absent | NoDistro | Ready { distro, home, shell }
ProjectsFile.recent[] gains  distro?: String      (written on Windows; ignored elsewhere)
ConfigFile gains             wsl?: { distro?: String }   (accepted everywhere, used on Windows)
```
On Windows a recent project is not probed for existence when listed (no `wsl.exe` call per
row); a missing one fails when opened.

### Interface changes
- `tab_open` / `project_choose` errors gain: WSL not installed · no distro · project is in
  another distro · not a WSL path. The first two are shown on the start screen as one
  message with the command to run (`wsl --install`, `wsl --install -d Ubuntu`) and a
  `[ Check again ]` button. The window never installs WSL and never asks for elevation.
- `rein-desktop.exe [DIR]`: DIR in any form W3 accepts.
- `rein-app window` is unchanged on Linux/macOS; on Windows it starts `rein-desktop.exe`
  found beside it or on `PATH`.

### Security (Windows)
- The process list stays closed: `wsl.exe` only, by absolute path, with the argv shapes in
  W2 and W4. Scripts are compile-time constants with one exception (D-047): the profile
  script `exec rein-app profiles apply … --profile N…` has the profile names placed in
  it, because the login shell takes one script string. Every word of that script is
  re-checked against `[A-Za-z0-9._-]` and every name against the profile-name rule before
  it is built, so no shell metacharacter can be present. Nothing else is ever placed in
  a script. The other variable argv elements are the validated distro name, the validated
  shell path and the validated project path (as `--cd`). No IPC command carries a program,
  a script or a distro name.
- Nothing crosses the environment boundary (`WSLENV` removed).
- The Windows binary never writes a Claude settings file or a Rein tree: the Tier 3
  settings boundary stays inside the Linux `rein-app`, with its lock and allow-list.
- CLI text returned from WSL is untrusted display text: capped, escaped by the Phase 1
  rule, never parsed for a decision except the doctor document and exit codes.
- The handoff directory accepts only a path string; the window validates it like any other.
- `%SystemRoot%` is used only when drive-absolute, else `C:\Windows` (a relative value
  would make the pty layer search `PATH` for `wsl.exe`). A UNC `%LOCALAPPDATA%` is refused:
  the lock and handoff directory must sit under the profile ACL. The handoff read is
  bounded. (E-43 `security_engineer`, PASS.)
- **Accepted residual risks (E-43 review):**
  1. The pty start script has no `/` guard (it runs in the user's shell, which may be
     fish). A project directory removed between validation and spawn makes `rein start`
     run in `/`, where it exits 2 ("no .ai"). It would open a session only if `/.ai`
     existed, which needs root inside the distro — someone who already owns the distro.
  2. "Nothing crosses" covers variables only. WSL's default `appendWindowsPath=true` puts
     Windows directories on the login shell's Linux `PATH`, so a Windows-side program (an
     npm shim, say) can be found by name inside WSL. This is WSL's behaviour for every
     terminal, not something the window adds; the operator can turn it off with
     `[interop] appendWindowsPath=false` in `/etc/wsl.conf`. The Windows operator docs
     must say so when they are written.
- The unsigned installer is a known gap until the signing revision; it is published only
  as a workflow artifact, never on a release.

### Limits
Every non-pty `wsl.exe` call: 30-second limit (a cold WSL VM takes several seconds), 64 KiB
output cap, except doctor (120 s, 4 MiB) and release / profile runs (streamed, the Phase 1
limits apply inside WSL). The state probe runs off the UI thread; first paint does not wait
for WSL.

### Tests (Windows revision)
- `WslHost` against a stubbed process runner: the exact argv of every call (program path,
  `-d`, `--cd`, constant script); `Absent` / `NoDistro` / `Ready`; UTF-16 list output; a
  bad distro name, home or shell from the probe is refused; validation exits 0 / 3 / 4 /
  5 / other map to the errors (5 and other → not a directory); `/` is refused before any
  call.
- Path mapping, table-driven: POSIX path, both UNC forms, wrong distro, drive path,
  relative path, a path with `"` or a control character.
- The pty child spec on Windows has no `WSLENV` and the constant script.
- Single instance: second start hands over and exits 0; no window → takes the lock; a
  stale handoff file is ignored; an oversized or unsafe handoff is dropped.
- Windows CI job compiles and runs these (none needs a distro).
- Engineer smoke from inside WSL through interop (`/mnt/c/Windows/System32/wsl.exe` with
  the same argv): the probe, the validation call and `-lic 'exec ~/.rein/bin/rein start'`
  behave as specified. Reported in the handoff; the operator's install is the final check.

### Rollback (Windows)
Delete `desktop-windows.yml` and `WslHost`; `LocalHost` behind the seam is the previous
behaviour. The operator uninstalls from Windows Settings and deletes
`%APPDATA%\rein-app` and `%LOCALAPPDATA%\rein-app`. Nothing inside WSL was changed by the
window; tmux sessions are unaffected.

## Rollback Plan
The window is additive: a separate crate and binary, no change to the CLI's behaviour, to
`$HOME/.rein` or to any profile file beyond what Phase 1 already does. To undo, remove
`crates/rein-desktop` from the workspace and the `window` subcommand; the operator deletes
`$XDG_CONFIG_HOME/rein-app` and `$XDG_STATE_HOME/rein-app`. A running tmux session is
unaffected at every point — closing or killing the window only detaches a client. No
release artifact carries the window until a task adds it to the dist configuration.

## Test Plan (acceptance for Engineer)
- **Modes:** table-driven tests of the pure `modes` function — every row of the default
  keymap, the leader timeout, an unbound key in NORMAL (ignored), and that TERMINAL mode
  forwards every key except the escape chord (a property test over random key sequences).
- **Pty:** with a stub child (`cat`, `sh -c 'exit 3'`): bytes round-trip, resize reaches
  the child, detach reaps it, exit code is reported, back-pressure pauses reading. A test
  asserts the argv is exactly `[rein, start]` and the cwd is the project.
- **IPC validation:** one refused case per command (oversized write, resize out of range,
  path with a control character, path without `.ai/`, unknown pty). `dir_complete`: a
  relative prefix and a prefix with a control character return nothing; more than 50
  matches are cut to 50; a file is never returned.
- **Config:** defaults file parses to the built-in defaults; unknown key, bad colour,
  binding to an unknown action and an oversized file are each refused with file and line,
  and the previous config stays active; a save triggers `config_changed`.
- **Health / release / profiles:** stubbed process runner — `fail`, `warn`, `ok`, garbage →
  `unknown`; the exact argv of every `run_cli` action; `release_apply` returns the CLI's
  exit code unchanged; a profile name outside the pattern or absent from `profiles list`
  is refused before any process starts; a second run while one is in progress is refused;
  CLI not found is reported and health still works. `rein-app update --check`: 0 / 10 /
  11, the two leading lines, and that it installs nothing (CLI crate test).
- **Capabilities:** a test reads the Tauri capability file and fails if any plugin
  permission beyond the IPC table and window basics is present; the CSP string is asserted.
- **Pointer parity (D-034):** a test walks the `Action` list and fails if an action has no
  menu entry, or no binding in either the `simple` or the `modal` table; `simple` style
  forwards every key that is not one of its shortcuts; every chrome button has a tooltip
  and an accessible name; multi-line paste asks first; a field set in `config.toml` is
  refused by `ui_set`; an unreadable `ui.json` yields the defaults.
- **Newcomer path:** with a fresh state directory, opening a project, checking health and
  starting an update can each be done with the mouse alone, and with the keyboard alone.
- **Screens:** `ux_reviewer` on the start screen, a project tab, each overlay, in
  `gruvbox-dark` and `github-light`, at 80×24 and 200×60 cells — contrast ≥ 4.5:1 for text.

## Open Questions (to `rein · Architect` — none blocks a task below)
1. Is there, or can there be, a machine-readable way to learn whether a project's session
   is already running (for a marker in the tabline)? Until answered the tabline shows only
   tabs this window has attached.
2. Does `rein start` in a pty with no outer tmux always create-or-attach, and is its exit
   code on "no `.ai/` here" stable? The window shows whatever it prints either way.
3. May an app display the latest handoff line of a project (a one-line status source such
   as `rein status --json`)? The app will not read `.ai/` files itself.

4. (D-049) Withdrawn 2026-11-12 — it assumed rein themes tmux; it does not (addendum 1).

## E-## Task Breakdown (registered IDs)
| ID | Scope | Tier | Depends on |
|---|---|---|---|
| E-34 | `rein-desktop` crate: Tauri 2 window, closed capability file, CSP, single instance, `rein-app window`, web build (esbuild, no framework), the grid shell with tabline / stage / statusline / command line drawn from static data; CI on Linux + macOS | 2 | — |
| E-35 | `PtyHost` + `tab_open` / `pty_write` / `pty_resize` / `tab_detach` + xterm.js stage: `rein start` in the project directory, back-pressure, reaping, exit display | 3 | E-34 |
| E-36 | `modes` state machine, default keymap, leader + which-key overlay, command line with completion and `E492`, `cmd.yank` | 2 | E-34 |
| E-37 | `Config`: `config.toml` load / validate / watch, six themes shared with xterm.js, `decorations`, `opacity`, window class | 2 | E-36 |
| E-38 | `ProjectStore`, start screen, project picker overlay, tabs (`gt`, `gT`, `1`–`9`, `ZZ`, `:q`, `:qa`) | 2 | E-35, E-36 |
| E-39 | `HealthService` + `:checkhealth` overlay + statusline segments; `release_check` / `release_apply` with `y/N` confirm; `profiles_plan` / `profiles_apply` overlay | 3 | E-38 |
| E-40 | Click-first layer: `simple` input style and its shortcut table, first-run choice, clickable tabline / statusline / overlays / start screen, the `[≡]` menu, labelled confirm buttons, tooltips, terminal context menu and paste guard, `project_choose` | 2 | E-38 |
| E-41 | Settings overlay + `ui.json` (`ui_get` / `ui_set`, `config.toml` wins), font zoom, pointer-parity tests, final `ux_reviewer` pass | 2 | E-37, E-39, E-40 |
| E-42 | Move the E-34 web shell to React + Tailwind and add the restyled shadcn/ui base (theme tokens as CSS variables, the components named in §Components 6); do it before E-36 | 2 | — |
| E-43 | Windows window core: `Host` seam (`LocalHost` unchanged), `WslHost` (state probe, distro, project validation, ConPTY child), Windows path mapping, Windows directories, lock-file single instance; `rein-desktop` builds and tests on the Windows CI job | 3 | E-35 |
| E-44 | `desktop-windows.yml`: NSIS installer as a workflow artifact (`ci_gate`) | 2 | E-43 |
| E-45 | Windows health / release / profiles through WSL (W4), `rein-app update --check`, WSL-absent start screen, folder dialog start directory, `wsl.distro` config key | 3 | E-39, E-43 |
E-43 and E-44 depend only on E-35: do them **before E-42** so the operator has an
installable window (it opens the DIR given on the command line until E-38 lands). E-38,
E-40 and E-41 reach projects only through the `Host` seam, so they work on Windows without
their own Windows code.
E-36..E-41 build on E-42: overlays, menu, tooltips, dialogs and Settings use the shadcn
components; "no framework" in the E-34 and earlier task texts is superseded by D-037.
Tier 3 tasks run `security_engineer` before commit. The `ux_reviewer` pass named in E-39
is repeated at the end of E-41, which is the one that counts.
E-34..E-39 as registered stay valid: E-34's capability test allows no plugin until E-40
adds the folder dialog and updates that test; E-36 builds `modes` so a second key table can
be supplied.
