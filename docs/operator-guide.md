# rein-app operator guide

This guide describes what `rein-app` does to your machine. It does not restate Rein's own
contract with the app: that contract is `docs/app-contract.md` in the
[rein repository](https://github.com/Vimurai/rein), which rein publishes with its first
release.

> No `rein` release has been published yet. Everything below describes the behaviour of
> the current code; `install`, `update` and `rollback` cannot complete until a signed
> release exists.

## Commands

| Command | What it does | Exit codes |
|---|---|---|
| `rein-app install [--version X.Y.Z]` | Fetches and verifies the release (the newest if `--version` is omitted), installs it into `$HOME/.rein`, then runs `rein install` and `rein doctor --scope install` from it. | 0 ok (warnings, and any host or project check, are printed but do not fail it) · 2 verification failed · 3 the install check failed · 4 a required runtime is missing · 13 `rein install` failed (rein's own exit code is printed) · 14 the install check could not run, did not finish within 120 s, or printed no valid result |
| `rein-app update [--yes]` | Shows the newest available version and its notes, and installs it after you confirm (`--yes` skips the question). | as `install` · 10 already current · 11 release server unreachable |
| `rein-app rollback` | Restores the previous version's **code** from its snapshot, then runs `rein install` and `rein doctor --scope install`. Your data is untouched. | 0 · 3 · 13 · 14 as `install` · 12 no snapshot to roll back to |
| `rein-app doctor [--json]` | Runs `rein doctor` (with `--json` if given) from the installed tree and passes its output through untouched. | rein's own exit code (with `--json`, 0 or 1 means stdout is the JSON document) · 14 `rein doctor` could not be started or ended without an exit code |
| `rein-app profiles list` | Lists the Claude Code profiles found on this machine. | 0 |
| `rein-app profiles apply [--profile NAME]… [--enable LIST\|all] [--disable LIST\|all] [--undo] [--dry-run]` | Shows, per profile, how the Rein-owned settings would change, then writes them. `--profile` can be given more than once (default: every profile). `--enable`/`--disable` install or disable Rein plugins through `claude plugin …`. `--dry-run` prints the diff and the commands it would run, writes nothing and runs no `claude` command. | 0 · 4 `claude` not found · 5 a `settings.json` could not be parsed · 7 `claude plugin` failed |
| `rein-app launch [DIR]` (Windows; `DIR` defaults to `.`) | Opens Windows Terminal into the project's Rein session inside WSL2. | 0 · 4 no `rein-app` installed inside the WSL distro · 20 WSL is absent and you declined to install it |

**On every command:**
- `--home DIR` operates on another home directory instead of `$HOME`. It must be absolute.
- Exit **1** means the command refused before writing anything, or hit another error; the
  message names the path or the cause (see [Refusals](#refusals-you-can-meet)). For `doctor`,
  1 is rein's own code, never the app's.
- Exit **64** is a usage error: an unknown command or flag, or a bad value. Exit 2 always
  means a release failed verification.

**The install verdict** (exit 0, 3 or 14) is the overall status of `rein doctor --scope
install --json`, which checks only the installed tree: `fail` is exit 3, `warn` or `ok` is
exit 0. Every check is printed. Checks of your machine (Claude, browsers, gh) and of the
directory you ran rein-app from belong to `rein-app doctor`, which runs the full `rein
doctor` and never fails an install. Exit 14 means the check could not start, did not
finish within 120 seconds (it is then stopped and the install lock released), or printed
something that is not a valid result; it is never treated as a pass. `rein install` itself
has no time limit.

`rein install` exit codes, printed after exit 13: **3** means the installed tree does not
match the release manifest, or a gate dependency is off its pin; **1** means Node is below
the required version or `/bin/bash -p` is missing. rein-app never retries or repairs either.
Re-run `rein-app install` after fixing the cause.

## What an install touches

A release is verified completely (signature, then every file's hash) outside the Rein home
before anything reaches it. Then, under `$HOME/.rein`, rein-app opens only:

- the **mirrored directories** the release lists (today `bin hooks mcp shared claude
  contracts templates copilot scripts config`), whose content is replaced by the release.
  A file the release does not ship is deleted, unless it is operator-owned;
- `manifest.json` at the top of the home, a copy of the release manifest;
- `VERSION`, written last, so a failed install never advertises the new version.

**Everything else in `$HOME/.rein` is never read, copied, written or deleted.** That covers
your accounts, secrets, databases, logs and anything a later Rein adds. Inside the mirrored
directories, operator-owned paths (the release manifest lists them, e.g.
`mcp/*/node_modules/`) are never deleted. A path the release ships is always written, even
when it matches an operator-owned pattern.

Each file is written to a temp file beside it and renamed into place, so a running Rein sees
either the old file or the new one, never half of one. `hooks` is the last mirrored directory written, so a hook never lands
before the code it calls.

## Snapshots and rollback

Before an update, rein-app copies the current release **code** (not your data, about 9 MB)
to `$HOME/.rein.prev-<version>/`. The two newest snapshots are kept.

If an install fails partway, rein-app restores from that snapshot and `VERSION` stays at the
old version.

`rein-app rollback` restores the newest snapshot, then runs `rein install` and `rein
doctor`. Before restoring, it snapshots the current code, so a second `rollback` returns you
to the newer version. Rollback restores **code only**; your data is never rolled back.

A directory named `.rein.prev-*` without a valid snapshot inside it is treated as yours. It
is never pruned or restored.

If an install with no valid `VERSION` in the home runs, there is nothing to snapshot. rein-app
says so in one line, and rollback is unavailable for that update.

## Profile settings (`profiles apply`)

rein-app finds Claude Code profiles in this order:

1. `default`: the default config directory, `~/.claude`.
2. Every account in `$HOME/.rein/accounts.json`, under its name, at its `config_dir`.
   rein-app reads that file but never writes it; `rein account add` and
   `rein account remove` change it.
3. `personal`: `~/.claude-personal`, if it exists and no usable account is named
   `personal`.

A directory is listed once, under the first name that reaches it. `default` is reserved for
`~/.claude`: an account with that name pointing elsewhere is skipped with a warning. An
account whose name is not printable ASCII (or is longer than 64 bytes), or whose
`config_dir` is missing, not absolute, or contains a control or bidirectional character, is
skipped with a warning. So is an account whose directory is already listed under another
name; the warning names that profile. An account that is not listed never hides
`~/.claude-personal`.

rein-app writes the three keys below wherever `accounts.json` points, including a
`config_dir` that is a symlink or lies outside your home. `apply` prints a note for a
profile whose directory is not named `.claude*`.

In each profile's `settings.json` it manages exactly **three keys**: `enabledPlugins`,
`extraKnownMarketplaces` and `attribution`.

- It **never** reads, copies, writes or removes `hooks`. Rein's gate hooks live in each
  project's `.claude/settings.json`, written by `rein init` / `rein sync`, not by rein-app.
- It can never write `permissions`, `defaultMode` or `autoMode`.
- Every other key you have is kept with the same value and in the same order. The file is
  re-serialised, so whitespace may change; values, number spelling and key order do not.
- The first time it writes a profile it saves `settings.json.rein-bak`, your pre-Rein
  settings. That backup is written once and never overwritten.
- `--undo` restores only the three Rein-owned keys from that backup, by value. Keys the
  backup lacked are removed, and every other key keeps its current value. It never
  restores the whole file.
- A `settings.json` or config directory that is a symlink pointing outside its config
  directory is refused.

## Refusals you can meet

The first four rows exit **1** and write nothing. The message names the path involved.

| Message is about | What happened | What to do |
|---|---|---|
| `$HOME/.rein.lock` exists | Another `install`/`update`/`rollback` is running, or one was killed. The lock file holds that run's PID. | Wait for it. If no process with that PID is alive, remove the lock file and re-run. |
| owner guard | rein-app is running as a different user than the one who owns the home (typically `sudo rein-app …`). | Run it as the owner of the home, without `sudo`. |
| a symlinked Rein home or mirrored directory | `$HOME/.rein`, a mirrored directory, or a directory on the way to a release file is a symlink. rein-app never writes through a symlink. | Replace the symlink with a real directory, then re-run. |
| a directory where a file goes (or the reverse) | Something in the home has the wrong type for a release path, e.g. a directory named `VERSION`. | Move it aside, then re-run. |
| exit 13 | `rein install` failed after the files were written. Its own message and exit code are printed. | Fix what rein reports, then re-run `rein-app install`. If rein reports a tree mismatch, `rein-app install --version <the installed version>` rewrites every shipped file. |
| exit 2 | The release failed verification. Nothing was written to the home. | Do not work around it. A message ending "install a newer rein-app" means the release uses a signing key or manifest schema this rein-app does not know: update rein-app first. |
| exit 2, `case collision: 'A' and 'B'` or a path "outside printable ASCII" | The release lists two paths that differ only by upper/lower case (a directory counts, so `shared/Lib` collides with `shared/lib/x.mjs`), or a path with a byte outside printable ASCII. On a case-insensitive filesystem one file would overwrite the other, so the release is refused on every OS. Nothing was written to the home. | rein's release tool refuses to build such a release, so report it to rein. Do not work around it. |
| exit 2, `'P' is a case variant of operator_owned pattern 'Q'`, or `directory 'D' is a case variant of the start of operator_owned pattern 'Q'` | The release ships a path, or uses a directory, that is one of your operator-owned paths in another upper/lower case (e.g. `shared/Toolchain.sh` when `shared/toolchain.sh` is yours, or a `Shared` directory). On a case-insensitive filesystem it would overwrite your file. Nothing was written to the home. | Report it to rein. Do not work around it. |
| exit 2, `… over the limit of N` | The release's manifest is larger than rein-app reads: more than 128 mirrored directories, 256 operator-owned patterns or 100000 files, or a path or pattern longer than 1024 bytes or deeper than 64 directories. Each limit is at least 10 times what rein ships today. Nothing was written to the home. | Update rein-app; if the newest rein-app still refuses it, report it to rein. |

## Desktop window

> **Status: a test build.** The window (`rein-desktop`) is not part of any release, and
> its Windows installer is unsigned. It is built only by this repository's `desktop-windows`
> workflow (Windows) or from source (Linux, macOS).

The window draws Rein sessions in tabs: each tab runs `rein start` in a project
directory, with the tabs in the title bar on top and a status bar at the bottom. Closing the window
detaches the tabs; their tmux sessions keep running.

### Windows

Rein and your projects stay inside WSL2. The window is a native Windows program, and
every process it starts runs through `%SystemRoot%\System32\wsl.exe`. It never installs
WSL and never asks for administrator rights.

**Install.** Download the installer from the newest successful
[`desktop-windows`](../.github/workflows/desktop-windows.yml) run on `master`. The workflow
keeps it for 14 days.

```sh
gh run list --workflow desktop-windows.yml --branch master --status success --limit 1
gh run download <run-id> -n rein-desktop-windows-x64-setup
```

Run `rein-app_<version>_x64-setup.exe`. It installs for the current user only, so it
needs no administrator rights, and it downloads WebView2 if it is missing. Because the
installer is unsigned, SmartScreen warns "Windows protected your PC": choose **More info →
Run anyway**. Start the window from the Start menu: **rein-app**.

**Before you start.**
- WSL2 with a Linux distribution. If it is missing, a card on the start screen says so and
  gives the command to run (`wsl --install`, or `wsl --install -d Ubuntu`); **Check again**
  (or `c`) checks again.
- Rein installed inside that distribution (`rein` on the login shell's `PATH`).
- For **update, rollback and profiles**: the Linux `rein-app` installed *inside* WSL. The
  window runs `rein-app` there; if it is not found (exit 127), the window says "rein-app
  is not installed inside WSL, or not on its login shell's PATH". The health check needs
  only `rein`.

**Project paths.** A project is a Linux path inside the window's distribution:
- `/home/you/project`: a POSIX absolute path, used as is;
- `\\wsl.localhost\<distro>\home\you\project` (or `\\wsl$\<distro>\…`): mapped to
  `/home/you/project`. The distribution must be the window's own.

A drive path (`C:\…`), a relative path or another network host is refused with "Rein
projects live inside WSL — choose a folder under `\\wsl.localhost\<distro>`". The folder
dialog opens in `\\wsl.localhost\<distro>\home\<user>`.

**Which distribution.** WSL's default, unless `config.toml` names one:

```toml
[wsl]
distro = "Ubuntu-24.04"
```

**Windows programs on the Linux PATH.** By default WSL appends the Windows `PATH` to the
Linux one (`appendWindowsPath=true`), so a Windows program, such as an npm shim, can be
found by name inside WSL. This is WSL's behaviour for every terminal, not something the
window adds; no environment variable crosses from the window. To turn it off, put this in
`/etc/wsl.conf` inside the distribution, then run `wsl --shutdown`:

```ini
[interop]
appendWindowsPath=false
```

**Uninstall.** Windows Settings → Apps → Installed apps → **rein-app** → Uninstall. The
window's own files stay; delete them by hand if you want them gone:
- `%APPDATA%\rein-app`: `config.toml`;
- `%LOCALAPPDATA%\rein-app`: `ui.json`, `projects.json` (recent projects).

Nothing inside WSL is touched.

### Linux and macOS

Build from source. On Linux you need WebKitGTK 4.1 and GTK 3 (Debian/Ubuntu:
`libwebkit2gtk-4.1-dev libgtk-3-dev libsoup-3.0-dev librsvg2-dev`). On both systems you
need Node.js to bundle the web UI:

```sh
(cd crates/rein-desktop/ui && npm ci --ignore-scripts && npm run build)
cargo build --release -p rein-app -p rein-desktop
./target/release/rein-app window ~/my-project
```

`rein-app window [DIR]` starts `rein-desktop` (found beside `rein-app`, then on `PATH`)
with `DIR` as the first tab. A second start hands over to the window that is already
running.

**WSLg is not supported.** Run the Windows build instead: WSLg cannot paint the WebKitGTK
window, so it shows up blank.

### Using the window

**The start screen** (no project open) has **Open a project…**, **Check installation** and
your recent projects, newest first, each with when it was last opened; click one or press
its number (`1`–`9`). `p` opens the project list, `o` a folder, `h` the health check, `u`
the update, `q` quits.

The first start asks how you like to work, with two cards:
- **simple** (the default): keys always go to the terminal, except a few shortcuts such as
  `Ctrl+Shift+P` (menu), `Ctrl+Shift+O` (open a folder), `Ctrl+Shift+T` (recent projects),
  `Ctrl+Tab`, `Alt+1`–`9`, `Ctrl+Shift+W`, `Ctrl+Shift+C` / `V`, `Ctrl+=` / `-` / `0` and
  `F1`. These shortcuts also work in modal style.
- **modal**: vim-style modes. `Ctrl+\ Ctrl+n` leaves the terminal, `Space` opens the
  leader keys, `:` the command line.

Everything is also a button: the tabs in the title bar, `+` (open a project), the version
(update), `☰` (the menu), and the status bar's project path, health badge and command
chip. The **menu** (`Ctrl+Shift+P`) is a searchable list of every action with its key; type
to filter, `Enter` runs. Messages appear for a few seconds at the bottom right.

The **status bar** shows the tab's project path (your home directory as `~`; a long path
loses its start, so the project's own folder stays visible), the health badge, and the last
command the window ran for you, with a copy button.

**Settings** (menu → Settings, `Ctrl+,`, or `:settings`) sets the input style, theme, font
size and title bar. It also has **Open config file location**, which shows the path of
`config.toml` and copies it.

**Settings files.**

| File | Linux | macOS | Windows | Written by |
|---|---|---|---|---|
| `config.toml` | `$XDG_CONFIG_HOME/rein-app/` (default `~/.config/rein-app/`) | the same, or `~/Library/Application Support/rein-app/` when `XDG_CONFIG_HOME` is unset | `%APPDATA%\rein-app\` | you; the window writes a commented template once and never again |
| `ui.json` | `$XDG_STATE_HOME/rein-app/` (default `~/.local/state/rein-app/`) | the same, or `~/Library/Application Support/rein-app/` when `XDG_STATE_HOME` is unset | `%LOCALAPPDATA%\rein-app\` | the window, for choices made in Settings and the first-run question |

**`config.toml` wins.** A setting that `config.toml` sets is shown in Settings as "Set in
config.toml" and cannot be changed there. Saving `config.toml` reloads the window. A line
the window cannot read is reported with its file, line and reason, and the previous
settings stay in force.

### tmux in the window

**The window has its own tmux server.** Sessions opened in the window run on a separate tmux
server, the socket `rein-app`, started with rein-app's own small config. **None of your tmux
settings apply there**: not `~/.tmux.conf`, not your plugins, your prefix key, key bindings,
mouse or status-line settings. Your own tmux server, its config and its sessions are not
touched, and `rein start` in an ordinary terminal works as before. The window uses tmux's
default prefix, `Ctrl+b`. Each pane's border shows its rein role and Claude account (for example
`engineer · personal`), drawn by the window's own config in the window's theme colours.

- **From a terminal**, reach the window's sessions with `tmux -L rein-app ls` and
  `tmux -L rein-app attach -t <session>`. Plain `tmux ls` does not list them.
- **Closing the window** detaches its tabs; the sessions keep running on the `rein-app`
  server, as they did on yours.
- **The window's files.** rein-app keeps two files in `~/.local/state/rein-app/tmux/`
  (inside WSL on Windows): `bin/tmux`, a small script that sends every `tmux` the tab runs to
  the `rein-app` server, and `tmux.conf`. It rewrites them each time a tab opens, so a change
  to them does not last; the folder can be deleted at any time and comes back. The window
  refuses to use them (and the tab says why) when the folder, `~/.local` or
  `~/.local/state` is a symlink, belongs to another user or can be written by other users, or
  when your home directory's path holds a space, quote or glob character. Making
  `~/.local` for the first time, the window makes it private (0700).
- **A project already running on your own tmux server is not found by the window.**
  Opening it in the window starts a second session, with a second Architect and Engineer,
  on the `rein-app` server. Stop the old one first (`rein start --kill` in a terminal), or
  keep using your own server (below).
- **To use your own tmux server instead** (your config and plugins, as before): Settings →
  **tmux server** → **My own**, or in `config.toml`:

  ```toml
  [tmux]
  server = "user"
  ```

  The change applies to tabs opened after it. With your own server the window still
  recolours the session it shows (`[tmux] overlay` / `status`) and puts your look back when
  the tab closes.

### The health badge

The status bar's health badge is the result of `rein doctor --scope install --json`. It
runs in your home directory (inside WSL on Windows) when the window starts, when a tab
opens, and when you click the badge or **Check installation**. It checks the **Rein
install** only, not the project in a tab:

| Badge | Meaning |
|---|---|
| ✓ **Install OK** (green check) | every install check passed |
| ⚠ **Install warning** (yellow triangle) | at least one warning |
| ✗ **Install failed** (red cross) | at least one check failed |
| ? **Health unknown** (question mark) | the check could not run (for example, WSL or `rein` is missing); clicking it shows why |

Clicking the badge opens **Installation health**: every check with its status icon, its
line and, where `rein doctor` gives one, the fix.

## Not supported

- **A shared `--home`.** The Rein home must be inside a private directory that only you can
  write. A `--home` inside a directory other users can write (such as `/tmp`) is not
  defended against.
- **Native Windows.** On Windows, rein-app runs Rein inside WSL2 only. The desktop window
  is a Windows program, but everything it starts runs inside WSL.
