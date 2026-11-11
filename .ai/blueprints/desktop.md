# BLUEPRINT — Desktop window ("Modeline") · Phase 2 · v1 2026-11-11

> Architect-owned. Decision record: D-033. Builds on `.ai/architect.md` §5–6 and the library
> seams in `distribution.md` §API. Contract items stay rein's: the open questions in
> §Open Questions go to `rein · Architect` and nothing here depends on their answers.

## Goal & Architecture
Give a Rein operator one window that opens a project's Architect and Engineer panes, shows
install health, and runs install / update / rollback / profiles — without hiding the terminal.
Users are people who live in tmux and nvim. One sentence: **a window that behaves like a
terminal program — modal, keyboard-first, drawn on the character grid, configured by one
text file — wrapped around the user's own tmux session.**

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
3. **Every action is a named command.** A key, a mouse click and a `:` line all resolve to
   the same closed list of actions. There is no action reachable only by mouse.
4. **The UI teaches the CLI.** When an action runs a `rein` or `rein-app` command, the
   command line row shows the exact command; `y` in NORMAL copies it.
5. **One text file.** `config.toml` holds font, theme, keymap and window options and is
   reloaded on save. There is no settings dialog; `:config` shows and copies the file's
   path, and the operator edits it in their own editor.

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
 TERMINAL │ rein-app │  master │ ✓ install ok │ 2 profiles │ C-\ C-n   14:02   <- statusline
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
The app binds nothing in TERMINAL mode except the escape chord, so no tmux or shell binding
is ever shadowed. The chord is configurable.

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
   cwd = the project directory, a clean environment (`CLAUDECODE` unset, `TERM=xterm-256color`),
   and the pty size in cells. Streams output to the web side over a Tauri channel in chunks
   of at most 64 KiB; takes input bytes and resize events back. `detach` closes the pty
   master, which detaches the tmux client; the child is always reaped. A pty whose child
   exits shows the exit code in the stage and offers `Enter` to restart.
3. **`ProjectStore`** — the recent-projects list in the app's own state file. A directory
   is a project when it contains `.ai/`; the app reads nothing inside `.ai/`. Entries are
   canonical absolute paths; a path that no longer exists is shown dimmed, not removed.
4. **`Config`** — loads, validates and watches `config.toml`. An invalid file keeps the
   previous good config and shows the error (file, line, reason) on the command line.
   A key may be bound only to an action from the closed list; the file cannot name a
   program to run.
5. **`HealthService`** — runs `rein doctor --json` through the Phase 1 runner (same 4 MiB
   cap and strict parse; 120-second limit) when a tab opens, after update / rollback, and on
   `health.show`. It displays the document's `status` and checks; it never recomputes a
   verdict and never branches on a check id (D-029, D-032).
6. **Web UI (TypeScript, no framework)** — four modules: `grid` (cell metrics, box
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
Nothing new is written under `$HOME/.rein` or inside a project.

## API / Interface Contracts
IPC commands (web → core). The table is closed; Tauri capabilities grant these and nothing
else (no `shell`, `fs`, `http` or `opener` plugin).
| Command | Input | Output | Errors |
|---|---|---|---|
| `tab_open` | project path | `Tab` + output channel | not a directory · no `.ai/` · rein not installed (offers `release.update`) · unsafe path |
| `pty_write` | pty id, bytes (≤ 64 KiB) | — | unknown pty |
| `pty_resize` | pty id, cols 2–500, rows 2–200 | — | out of range |
| `tab_detach` | tab id | — | — |
| `projects_list` / `projects_forget` | — / path | `ProjectsFile.recent` | — |
| `config_get` | — | `ConfigFile` + source path + last error | — |
| `health_get` | refresh: bool | `HealthView` | — (failure is `status: unknown`) |
| `release_check` | — | installed version, newest version, notes | offline |
| `release_apply` | `update` \| `rollback` | progress lines (channel), then the Phase 1 exit code | lock held · every Phase 1 refusal, text unchanged |
| `profiles_plan` / `profiles_apply` | profile names | per-profile diff of the three allowed keys / result | Phase 1 errors, text unchanged |
| `clipboard_write` | text (≤ 64 KiB) | — | — |
Events (core → web): `config_changed`, `health_changed`, `pty_exit { id, code }`.

CLI: `rein-app window [DIR]` starts `rein-desktop` (found beside the `rein-app` binary,
then on `PATH`) with DIR as the first tab; exit 1 with one line when it is not installed.
`rein-desktop [DIR]` may be started directly. A second start hands its DIR to the running
window as a new tab (single instance) and exits 0.

Command line (`:`): `:open DIR`, `:projects`, `:checkhealth`, `:update`, `:rollback`,
`:profiles`, `:config`, `:theme NAME`, `:q`, `:qa`, `:help`. Tab completes command names,
directories and theme names. An unknown command prints `E492: Not a command: <name>`.

## Security
Trust boundaries added by the window:
1. **Web view → core.** The web view loads only the bundled UI (`tauri://`), with a CSP of
   `default-src 'self'`, no remote origin, no `eval`, no inline script. Navigation and new
   windows are refused. Every IPC input is validated in Rust against the types above; the
   web side is treated as untrusted even though we ship it.
2. **Core → processes.** The only programs the window starts are `rein` from the installed
   tree (argv fixed per action: `start`, `doctor --json`, and what the Phase 1 library
   already runs). The app does **not** start `$EDITOR`: `:config` shows the file's path on
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
5. **Settings boundary (Tier 3).** `profiles_apply` reaches `rein_profiles::apply` and
   nothing else, so the three-key allow-list and the compile-time ban on `hooks`,
   `permissions`, `autoMode`, `defaultMode` hold unchanged. `release_apply` takes the same
   install lock and owner guard as the CLI.
No secret is read, stored or shown. The window makes no network call of its own; only
`release_check` / `release_apply` reach the network, through the Phase 1 verifier.

## Execution Constraints
- First paint ≤ 400 ms on a warm start; a key in TERMINAL mode reaches the pty in ≤ 5 ms of
  app time; pty output is batched per animation frame, with back-pressure — when the web
  side is more than 4 MiB behind, the core stops reading the pty until it catches up.
- At most 16 tabs; scrollback is tmux's (xterm.js keeps 1 000 lines).
- One window per user session (single instance lock in `$XDG_RUNTIME_DIR`, falling back to
  the state directory).
- Platforms for this revision: Linux (X11 and Wayland, WSLg included) and macOS. The Windows
  window (pty child `wsl.exe`), the bundled Node, and notarization / code signing are later
  revisions of this file and are not started by any task below.
- Dependencies each pass `skill: dependency_gate`: `tauri` 2, `portable-pty`, `toml`,
  `notify`; web: `@xterm/xterm`, `@xterm/addon-fit`, `@xterm/addon-webgl`, `esbuild`,
  `typescript`. No UI framework, no CSS framework, no icon set. The web bundle is built in
  CI from a committed lockfile.
- Hosted CI builds and tests `rein-desktop` on Linux and macOS; the Windows job keeps
  building the CLI crates only.

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
  path with a control character, path without `.ai/`, unknown pty).
- **Config:** defaults file parses to the built-in defaults; unknown key, bad colour,
  binding to an unknown action and an oversized file are each refused with file and line,
  and the previous config stays active; a save triggers `config_changed`.
- **Health / release / profiles:** stubbed `Process` — `fail`, `warn`, `ok`, garbage →
  `unknown`; `release_apply` returns the Phase 1 exit codes unchanged; `profiles_apply`
  cannot be handed a key name (it takes profile names only).
- **Capabilities:** a test reads the Tauri capability file and fails if any plugin
  permission beyond the IPC table and window basics is present; the CSP string is asserted.
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

## E-## Task Breakdown (registered IDs)
| ID | Scope | Tier | Depends on |
|---|---|---|---|
| E-34 | `rein-desktop` crate: Tauri 2 window, closed capability file, CSP, single instance, `rein-app window`, web build (esbuild, no framework), the grid shell with tabline / stage / statusline / command line drawn from static data; CI on Linux + macOS | 2 | — |
| E-35 | `PtyHost` + `tab_open` / `pty_write` / `pty_resize` / `tab_detach` + xterm.js stage: `rein start` in the project directory, back-pressure, reaping, exit display | 3 | E-34 |
| E-36 | `modes` state machine, default keymap, leader + which-key overlay, command line with completion and `E492`, `cmd.yank` | 2 | E-34 |
| E-37 | `Config`: `config.toml` load / validate / watch, six themes shared with xterm.js, `decorations`, `opacity`, window class | 2 | E-36 |
| E-38 | `ProjectStore`, start screen, project picker overlay, tabs (`gt`, `gT`, `1`–`9`, `ZZ`, `:q`, `:qa`) | 2 | E-35, E-36 |
| E-39 | `HealthService` + `:checkhealth` overlay + statusline segments; `release_check` / `release_apply` with `y/N` confirm; `profiles_plan` / `profiles_apply` overlay | 3 | E-38 |
Tier 3 tasks run `security_engineer` before commit. E-39 closes with a `ux_reviewer` pass.
