# Blueprint — Updates: an always-on watch for Rein, and the window updates itself (v1 · 2026-11-18, D-065, D-066)

> Operator direction 2026-11-18: "only on open the watcher for rein update activates; this
> should be immediate when rein changes, so the watcher should be active all the time. Also
> we should plan an updater for our own rein-app: when a new version is available a button
> should appear and automatically ask for update, with cool animations and everything."
> Read with `desktop.md` (§Components 5 HealthService and the CLI runner, V7 motion, V9
> project check, V12) and `status.md` §6 (notifications). Where they differ on update
> checks, this file wins.

## Goal
1. The window knows within seconds that the Rein on this machine changed, and within the
   half hour that a newer Rein was published — without the user opening anything.
2. The window (`rein-desktop`) can replace itself with a newer verified build on a click,
   and offers that by itself when one exists.

## What is wrong today (read in the code, 2026-11-18)
- `release_check` runs only when the Update dialog is opened (`release.tsx`). The version
  chip's "newer" dot is therefore never set by itself.
- `project_check` (is this project behind the installed Rein?) runs once per tab open
  (`shell.tsx`, "once per open"). A Rein update done in another terminal is not seen by an
  open tab until it is closed and opened again.
- The window is in no release. The Windows build is an unsigned workflow artifact that the
  operator copies by hand.

## Core Concept
**One `UpdateWatch` in the core holds three facts and tells the web side when one
changes: the installed Rein version, the newest published Rein version, and the newest
published window version.** The web side draws them; it never starts a check on its own
timer (a hidden web view's timers are throttled — same reason as `status.md`).

## Components

### 1. Installed-Rein watch (core)
- Reads the one line of `VERSION` in the Rein home (the path Phase 1 already resolves; at
  most 64 bytes, `[0-9A-Za-z.+-]` only, anything else is "unknown").
- Linux / macOS: a file read every 5 s — no process.
- Windows: the file is inside WSL. One constant read-only `wsl.exe` line every 30 s, and
  at once when the window gains focus. When E-76's status read exists the line moves into
  it (5 s, no added process); until then this is the cost. The Engineer measures it.
- On a change (not on the first read): send `updates` (below); run the install health
  check; run `project_check` again for every open, started tab — one at a time, the
  active tab first — so the "project is out of date" bar appears where it applies and
  goes where it no longer does. The "once per open" rule becomes "once per open and once
  per installed version". `update_dont_ask` keeps its meaning.
- Never while a release or profile run of the window's own is in progress; the run's end
  triggers one read.

### 2. Published-Rein check (core)
- Runs the existing `rein-app update --check` through the existing CLI runner (constant
  argv; exit 0 newer · 10 current · 11 offline; only `installed:` / `newest:` are read).
- When: 30 s after start; then every `interval_min` (default 30, 10–1440); and when the
  window gains focus if the last check is older than 10 minutes. One at a time; skipped
  while any CLI run is in progress; offline is silent and keeps the last known answer.
- **The Engineer reads first and reports:** what `update --check` fetches (which host,
  whether it is rate-limited without a token) and what one check costs through `wsl.exe`.
  If 30 minutes is not safe for that host, say so and hand back before building.

### 3. Window self-update (core) — Tier 3
- **Scope: in-place update on Windows only** (the per-user NSIS installer). On Linux and
  macOS the package manager owns the files (`.deb`, Homebrew): the same button and card
  appear, and the sheet shows the one command to run and a Copy button. Nothing is
  downloaded there.
- **Reuse, not a new updater:** `rein-release` already fetches over HTTPS and verifies a
  minisign signature against a key baked at build time. The window's update uses that
  code path with a second baked key — **the rein-app key, not rein's**.
  The Engineer reads first and reports, before any write:
  (a) does `rein-release`'s fetch and verify build and run in the native Windows
  executable (today it runs only in the CLI inside WSL)?
  (b) the flags of the Tauri NSIS installer for a silent / passive per-user update and a
  relaunch, and whether it can replace a running window;
  (c) what the size of the window's executable becomes.
  If (a) fails, STOP and hand back — the fallback (Tauri's updater plugin) reverses
  D-035 "no Tauri plugin" and needs `dependency_gate` and a new ruling.
- **As read, ruled 2026-11-20 (E-86's three reads, D-066 addendum 1):**
  (a) passed with one part unobserved — a real HTTPS GET from the native Windows
  executable. E-86 shows it once on windows-latest before it closes; if that fails it is
  the STOP above. `ReqwestHttp` gains a timeout argument (the check uses 20 s); progress
  and Cancel are a counting, cancelling `Write` passed to `download`, with no change to
  the trait. `fetch_release` / `ReleaseSource` are not reused.
  (b) The installer's argv is the constant `/P /UPDATE /R`. Under `/P` the installer
  kills a window that is still running, so **the window saves first and starts the
  installer last**: verify and compare → do everything the ordinary quit does (the V12
  record) → start the installer detached → exit at once, with no work after the start.
  The installer's kill is then only a fallback and loses nothing. No wait is added.
  (c) +6.0 MiB on the Linux proxy is accepted and aws-lc stays: one crypto stack for the
  CLI and the window, no feature change, no dependency gate. The Windows figure is
  reported from the first desktop-windows run; over +10 MiB, hand back before closing.
- **Check:** GET the constant URL
  `https://github.com/Vimurai/rein-app/releases/latest/download/desktop-latest.json`
  (≤ 64 KiB, strict parse, schema 1). Same schedule as component 2. HTTPS only, no
  redirect to another scheme, 20 s limit.
- **Offer:** only if `version` is greater than the running version (semver, no
  downgrade, no same version) and `key_id` equals the baked key's id (else one line
  naming that a newer window must be installed by hand — the rule of `distribution.md`).
- **Install (on the user's click only):** download the installer named in the document
  to the app's own cache directory (per-run name, size from the document, at most
  200 MiB, progress as bytes) → verify the minisign signature of the file → compare
  sha256 with the document → start it with the flags of (b) → exit the window. The
  installer starts the new window. Any failed step deletes the file and says which step
  failed in one fixed sentence; nothing is started.
- **The projects keep running** (V12): quitting for the update is an ordinary quit; the
  new window restores the tabs. The card says so.
- After the restart the new window sees its version differs from `updated_from` in the
  app's state file and shows "Updated to X." once.

### 4. The update button and the ask (web)
- **Button.** When either a newer Rein or a newer window is known, a pill appears in the
  title bar left of the version chip: an arrow-up icon and "Update". It replaces the
  chip's dot. Click → the Update dialog, which now has two rows: "Rein 0.5.0 → 0.5.1"
  and "rein-app window 0.1.0 → 0.2.0", each with its own button and notes.
- **Ask, once per version.** The first time a version is known, a card rises at the
  bottom right, not modal, never taking focus from the terminal:
  "rein-app 0.2.0 is available. Your projects keep running during the update."
  **Later** · **Update and restart** (for Rein: **Later** · **Update Rein**, which opens
  the existing confirm). **Later** records the version in the app's state file; that
  version does not ask again, the button stays. A hidden or minimised window raises the
  notification of `status.md` §6 instead (event `update`, sentence fixed, no version text
  from the network in a system notification).
- **Progress** (window update): the card becomes "Downloading 0.2.0 — 41 %" with the
  2 px progress line, then "Checking the signature…", then "Restarting…". **Cancel**
  while downloading. A failure shows its sentence and **Try again**.
- **Motion** — D-054 stands (CSS only, motion tokens, nothing under reduced motion, an
  idle window does not animate); this is the one place the window is allowed to be
  expressive, each effect once:
  - the button enters by sliding 8 px and fading (200 ms), then its accent ring pulses
    twice and rests;
  - the card rises 12 px and fades in (220 ms), leaves the same way;
  - the progress line is determinate; the percentage counts in the text;
  - on a verified download a check mark draws itself (stroke, 300 ms) before
    "Restarting…";
  - after the restart, "Updated to 0.2.0" appears as a toast with the same check mark
    and a "What's new" link that opens the notes in the reader.
  No effect delays a click; no effect loops.

### 5. Release side (CI) — Tier 3, blocked on the operator
- On a tag, the release workflow builds the Windows installer of the window, signs the
  file with the rein-app minisign key (a repository secret), and uploads the installer,
  its `.minisig` and `desktop-latest.json` as release assets. `ci_gate` first.
- **Operator must do, once:** create the rein-app update key pair and store the secret
  in the repository; say go for the first tag (pending since D-048). Until then
  components 3–4 are built and tested against a test key and a local server, and the
  window's check answers "no release yet" silently.
- An OS code-signing certificate is **not** needed for this: a file the app downloads
  itself carries no "from the internet" mark, so the unknown-publisher warning of a
  browser download does not apply. The first install by hand still shows it (candidate
  10 stays open).

## Data Model
```
UpdatesState (core → web)
  rein:   { installed: String?, newest: String?, checked_at: Time?, offline: bool }
  app:    { running: String, newest: String?, notes: String?, mode: in_place | command | none,
            command: String? }            // command: the package-manager line, a constant per channel
  job:    idle | { downloading: {done, total} } | verifying | restarting | { failed: Step }
Step = fetch | too_large | signature | checksum | start      // closed; one fixed sentence each

desktop-latest.json (schema 1, untrusted until used as below)
  { schema: 1, version, key_id, notes (≤ 4 KiB, plain text),
    windows_x64: { url (https, host github.com or objects.githubusercontent.com), size, sha256, sig } }

app state file gains   asked: { rein: String?, app: String? }   // versions answered "Later"
                       updated_from: String?
config.toml gains      [updates] check = true          // false: no background check at all
                                 interval_min = 30     // 10–1440
                                 app = true            // false: never offer the window's update
```

## API / Interface Contracts
- **Event `updates`** (core → web): the full `UpdatesState`, on any change.
  **Command `updates_get()`** for a web view that has just loaded.
- **`updates_check()`** — "Check now" in the dialog; refused while a check or job runs.
- **`app_update_start()`** / **`app_update_cancel()`** — no argument: the core installs
  only the version it last verified the document for. Errors are the `Step` sentences.
- **`updates_later(which: rein | app)`** — records the known version as asked.
- `release_check` / `release_apply` unchanged; the dialog uses the watch's answer first.
- No command takes a URL, a version, a path or a file name from the web side. The
  capability file gains exactly these command names and nothing else — no plugin.

## Security
- Trust is the baked rein-app public key. The document and the download are untrusted:
  the installer runs only after its minisign signature verifies and its sha256 matches.
  TLS alone is never the proof.
- No downgrade and no same-version install; a `key_id` mismatch never downloads.
- The URL of the installer must be HTTPS on the two allowed hosts; redirects are followed
  only within them. Size is bounded before and during the download.
- `notes` is plain text drawn by the closed structured-text component (no links, images
  or HTML), capped, and never placed in a system notification.
- The downloaded file lives in the app's cache directory under a per-run name, is
  created exclusively, and is deleted on every failure and at the next start.
- The installer is started with a constant argument list; no text from the document
  reaches a command line.
- The watch's `wsl.exe` line is constant and read-only. `security_engineer` reviews
  component 3 before its commit; `SEC_CLEARED` is required.

## Execution Constraints
- No timer on the web side. One check at a time; a check never overlaps a CLI run.
- Background network: at most one request per kind per `interval_min`, plus focus
  checks no closer than 10 minutes. `[updates] check = false` makes none.
- The installed-version read on Windows: target under 250 ms, measured and reported.
- `ipc.rs` and `shell.tsx` do not grow: new modules `updates.rs`, `app_update.rs`,
  `ui/src/updates.ts`, `ui/src/update-ui.tsx`.

## Tests
- Core, pure with a clock passed in: the schedule (start delay, interval, focus rule,
  skip during a run); a version change triggers the project re-checks once, active first;
  document parse (schema, hosts, size, version order, key id); each `Step` failure leaves
  no file and starts nothing.
- End to end against a local HTTPS-less test double behind a build flag used only by
  tests, with the test key: good update reaches "start" with the constant argv; a
  tampered file stops at `signature`; a wrong sha at `checksum`; an older version is not
  offered.
- Window (Xvfb), proof on screen: Rein's `VERSION` changed under a running window → the
  out-of-date bar appears on an open tab without reopening it; the button and the card
  appear for a newer window; Later hides the card and keeps the button; progress,
  failure and "Updated to" states. Screens in both themes (D-052).
- Windows: the real installer flags from read (b) run once by the Engineer on this
  machine with a test build, tabs restored after it; figures in the handoff.

## Rollback
- `[updates] check = false` returns to today's behaviour (checks only when the dialog is
  opened); `app = false` hides the window's update alone.
- A bad window release: publish a higher version; the user can install any older
  installer by hand (no downgrade through the app, by design).
- Reverting the tasks' commits removes the watch and the updater; no file format outside
  the app's own state and config changes.

## Deferred
- In-place update on macOS and Linux (package managers own those installs).
- A beta channel; staged roll-out; automatic install without a click.
- OS code signing (candidate 10).

## Task Breakdown
1. **E-85 (Tier 2)** — components 1 and 2, the `updates` event, the button and the Rein
   row and card (component 4 for Rein). The read of component 2 first.
2. **E-86 (Tier 3)** — component 3 with the three reads first; test key; `security_engineer`.
3. **E-87 (Tier 2)** — component 4 for the window: card, progress, motion, "Updated to".
4. **E-88 (Tier 3)** — component 5; must not start until the operator has made the key
   and said go for a tag.
