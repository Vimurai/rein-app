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
