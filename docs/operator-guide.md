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
| `rein-app install [--version X.Y.Z]` | Fetches and verifies the release (the newest if `--version` is omitted), installs it into `$HOME/.rein`, then runs `rein install` and `rein doctor --scope install` from it. | 0 ok (warnings, and any host or project check, are printed but do not fail it) · 2 verification failed · 3 the install check failed · 4 a required runtime is missing · 13 `rein install` failed (rein's own exit code is printed) · 14 the install check could not run or printed no valid result |
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
doctor` and never fails an install. Exit 14 means the check could not start, or printed
something that is not a valid result; it is never treated as a pass.

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

## Not supported

- **A shared `--home`.** The Rein home must be inside a private directory that only you can
  write. A `--home` inside a directory other users can write (such as `/tmp`) is not
  defended against.
- **Native Windows.** On Windows, rein-app runs Rein inside WSL2 only.
