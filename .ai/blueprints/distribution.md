# Blueprint — Distribution (Phase 1: installer, verifier, profiles, updater, WSL launcher)

> Architect-owned. Derived from `rein` D-150 contract §2–§3 and `plans/rein-app-plan.md` Phase 1.
> Index: `.ai/architect.md` §0. Tasks: E-2..E-6 (see §Task Breakdown; plan IDs E-1..E-5 shifted by one).

## Goal & Architecture
Operators get Rein with one package-manager command on macOS, Linux and Windows (WSL2), and
the result is a byte-verified copy of a tagged `rein` release laid out exactly as
`install-rein.sh` does, with `rein install` and `rein doctor` run from that tree. Rust CLI,
three crates, no Rein logic inside. Phase 1 is a CLI only; phase 2 wraps it in a window.

## Core Concept
A **verified release pipeline**: `fetch → verify (signature, manifest, sha256) → stage →
atomic swap into $HOME/.rein → rein install → rein doctor`. Every step either succeeds
completely or leaves the previous tree untouched. Around it sit two side tools: a
**profile manager** that edits four allow-listed settings keys per Claude profile, and an
**updater/rollback** that keeps the last two trees.

## Components
1. **`rein-release::Fetcher`** — resolves tag → asset URLs on the `rein` GitHub release
   (`rein-X.Y.Z.tar.gz`, detached signature, `manifest.json`); downloads to a per-run
   staging dir; enforces size cap and timeout; never follows redirects off the release host.
2. **`rein-release::Verifier`** — (a) `SignatureVerifier` trait with a `Rejecting` default
   backend (every signature invalid) until `rein` P-50 chooses minisign vs sigstore;
   (b) manifest parse + schema check; (c) sha256 of every file in the tarball against the
   manifest, and rejection of any file not listed or any listed file missing. Any failure →
   `VerifyError` naming the check; nothing leaves staging.
3. **`rein-release::LayoutWriter`** — unpacks the verified tarball into
   `$HOME/.rein.staging-<version>`, checks the tree matches the stable layout
   (`bin/ hooks/ mcp/ shared/ scripts/ config/ secrets/ run/`, `VERSION`), then atomic
   rename to `$HOME/.rein` with the previous tree kept as `$HOME/.rein.prev-<oldversion>`
   (N=2). Preserves operator-owned files that live in the home dir but not in the tarball
   (`accounts.json`, `gate-override.json`, `config/*`, `secrets/*`, `run/*`) by carrying
   them across. Never edits any file after unpack.
4. **`rein-app::Runner`** — invokes `rein install` then `rein doctor` from
   `$HOME/.rein/bin` with a clean env (`CLAUDECODE` unset, PATH prefixed with
   `$HOME/.rein/bin`); streams output to the terminal verbatim; exit code propagates.
   Only contract §2.6 entry points are ever invoked.
5. **`rein-app::RuntimeCheck`** — detects Node (≥ manifest `minNode`, default 22), git,
   sqlite3, tmux, Claude Code (`claude --version`); prints a table; missing Claude Code →
   print the official installer URL, do not bundle. Does not install anything itself in phase 1.
6. **`rein-profiles`** — enumerates profiles: default config dir (`~/.claude`),
   `~/.claude-personal` if present, and every `accounts.<name>.config_dir` in
   `$HOME/.rein/accounts.json`. For each `<config_dir>/settings.json`: read → parse →
   merge ONLY `hooks` (sourced by calling `rein install`, never hand-authored),
   `enabledPlugins`, `extraKnownMarketplaces` (adds the `rein-plugins` marketplace entry —
   exact JSON and pin policy fixed in `.ai/DECISIONS.md` D-002: `github` source,
   `repo: "Vimurai/rein-plugins"` (confirmed), `ref: "stable"`, plugins enabled only on
   `--enable`; remote not live yet — tests use a fixture), `attribution` → validate JSON →
   write via temp file + rename. Every pre-existing key
   is preserved byte-for-byte in value. Writing `autoMode`, `permissions` or `defaultMode`
   is a compile-time impossibility (the key set is a closed enum).
7. **`rein-app::Updater`** — `rein-app update`: list newest `rein` tag, fetch its
   changelog/release notes, show diff vs installed `VERSION`, install on confirm (`--yes`
   for CI). `rein-app rollback`: swap `.rein` ↔ `.rein.prev-*`, re-run `rein doctor`.
8. **`rein-app::WslLauncher`** (Windows target only) — `wsl --status`; if no distro,
   `wsl --install -d Ubuntu`; runs the Linux `rein-app install` inside the distro; opens
   Windows Terminal (`wt.exe`) into the project's tmux session via `wsl -e rein start`.
   All `wsl.exe`/`wt.exe` calls go through a `Shell` trait so tests stub them.

## Data Model
```
Manifest { version: SemVer, min_node: u32, layout: Vec<DirName>,
           files: Vec<{ path: RelPath, sha256: Hex64, size: u64 }> }     // shape pending P-50
ReleaseAssets { tarball: Path, signature: Path, manifest: Path, tag: String }
VerifyReport { signature: Pass|Fail(reason), manifest: Pass|Fail, files: Vec<{path, Pass|Mismatch|Missing|Unlisted}> }
InstalledTree { root: $HOME/.rein, version: SemVer (from VERSION), prev: Option<{root, version}> }
Profile { name: String, config_dir: AbsPath, settings_path: AbsPath, exists: bool }
AllowedKey = Hooks | EnabledPlugins | ExtraKnownMarketplaces | Attribution   // closed enum
ProfilePatch { profile: Profile, changes: Map<AllowedKey, serde_json::Value> }
RuntimeReport { node: Found(ver)|Missing|TooOld(ver), git, sqlite3, tmux, claude: Found|Missing }
```
Persistence: none of our own beyond `$HOME/.rein.prev-*`. State of record is the installed
tree's `VERSION` and the profiles' `settings.json`. No database, no app config file in phase 1.

## API / Interface Contracts
CLI (`rein-app`):
| Command | Input | Output | Exit |
|---|---|---|---|
| `install [--version X.Y.Z] [--home DIR]` | tag (default latest) | verify report, doctor output | 0 ok · 2 verify fail · 3 doctor fail · 4 runtime missing |
| `update [--yes]` | — | available version + notes, then install | as install · 10 already current · 11 offline |
| `rollback` | — | swapped version, doctor output | 0 · 12 no previous tree |
| `doctor` | — | passthrough of `rein doctor` (and `--json` once P-50 lands) | rein's exit code |
| `profiles list` | — | table of Profile | 0 |
| `profiles apply [--profile NAME] [--marketplace URL]` | — | per-profile diff of allowed keys, then write | 0 · 5 parse error · 6 forbidden key attempted (bug) |
| `launch` (Windows) | project dir | opens terminal into tmux | 0 · 20 WSL absent and install declined |
Library seams for phase 2: `rein_release::{fetch, verify, install}`, `rein_profiles::{enumerate, plan, apply}`,
each pure w.r.t. I/O behind `Fs`/`Http`/`Shell` traits so Tauri reuses them without a second code path.

## Security
- **Trust boundary 1 — network**: all downloaded bytes are untrusted until signature AND
  every sha256 pass. HTTPS only, pinned release host, size cap (default 200 MB), no
  redirects off-host. `Rejecting` signature backend means the pipeline cannot install from
  a real release until P-50 lands — intentional fail-closed default.
- **Trust boundary 2 — `$HOME/.rein`**: written once by LayoutWriter via staging + atomic
  rename. Tarball entries are checked for path traversal (`..`, absolute paths, symlinks
  escaping the root) before extraction. Operator files are carried over, never regenerated.
- **Trust boundary 3 — profiles**: closed `AllowedKey` enum; `hooks` content comes only from
  `rein install`'s own output (we re-read the file it wrote; we never author hook entries).
  Hard-deny on `autoMode`/`permissions`/`defaultMode` (`rein` D-132). Temp file + rename;
  original kept as `settings.json.rein-bak` for one generation.
- **Trust boundary 4 — process**: only contract §2.6 `rein` entry points are executed, with
  args from a fixed list; no user-supplied strings reach a shell. Role tokens are never minted
  (`rein pane` owns that).
- **Secrets**: none stored. `secrets/` under the Rein home is carried over opaque.
- **Review**: E-3, E-4, E-5 are Tier 3 → `security_engineer` before commit.

## Execution Constraints
- Install end-to-end (excluding download) < 10 s on a laptop; sha256 of the full tarball
  streamed, not buffered. Memory < 100 MB.
- Single-writer: a lock file `$HOME/.rein.lock` prevents concurrent `install`/`update`/`rollback`.
- No network in tests: fixture server (local HTTP) serves good tarball, bad signature,
  tampered file, missing manifest, unlisted file, traversal entry.
- Phase 1 CLI is dependency-light: `reqwest` (rustls), `sha2`, `tar`+`flate2`, `serde_json`,
  `clap`, `semver`, `tempfile`. Each new dep passes `skill: dependency_gate`.
- Binary targets: `x86_64/aarch64-apple-darwin`, `x86_64/aarch64-unknown-linux-gnu`, `x86_64-pc-windows-msvc` (launcher only).

## Rollback Plan
- **Install/update**: previous tree preserved as `$HOME/.rein.prev-<ver>`; `rein-app rollback`
  swaps back and re-runs `rein doctor`. If the atomic rename itself fails midway, the staging
  dir is left for inspection and `$HOME/.rein` is untouched (rename is the only mutation).
- **Profiles**: `settings.json.rein-bak` restored by `profiles apply --undo`; the backup is
  byte-identical to the pre-apply file.
- **Package**: `brew uninstall rein` / cargo-dist uninstaller removes the binary only; the
  Rein home dir is operator data and is never removed by the app.
- **Feature kill**: the `Rejecting` signature backend can be re-selected by build flag to halt
  all installs if a verifier bug is found.

## Test Plan (acceptance for Engineer)
- Fixture release server with the six cases above; each bad case asserts no write under a
  temp `$HOME`.
- Layout equality: run `install-rein.sh` from a checked-out `rein` at tag T into temp HOME A,
  run `rein-app install --version T` (fixture tarball built from the same tree) into temp
  HOME B; `diff -r` of A and B is empty except `VERSION` timestamp fields if any.
- Profile merge: property test — for random pre-existing JSON, after `apply` every original
  key/value is intact and only allowed keys changed; attempting a forbidden key fails to compile.
- WSL launcher: unit tests against a stubbed `Shell` covering no-WSL, WSL-no-distro, ready.

## E-## Task Breakdown (registered IDs)
- **E-2** (T2) Cargo workspace `rein-app`/`rein-release`/`rein-profiles`; `git init` if absent;
  cargo-dist config for mac/linux/windows; CI matrix (GitHub Actions) building + testing all
  three targets. Acceptance: `cargo test --workspace` green on all three in CI.
- **E-3** (T3) `rein-release` fetch + verify behind `Http`/`Fs` traits; `SignatureVerifier`
  trait with `Rejecting` default; manifest + sha256 + traversal checks; fixture server with
  the six cases; fail closed on every bad input. Depends on E-2.
- **E-4** (T3) `rein-profiles` enumerate/plan/apply with closed `AllowedKey` enum, backup +
  undo, property test for key preservation; `profiles list|apply`. Depends on E-2.
- **E-5** (T3) LayoutWriter (staging, layout check, operator-file carry-over, atomic swap,
  N=2 prev) + Runner (`rein install`, `rein doctor` from the tree); layout-equality test
  against `install-rein.sh`. Depends on E-3.
- **E-6** (T2) Homebrew formula (tap) + cargo-dist release workflow + `.deb`/AppImage;
  Windows WSL launcher behind `Shell` trait with stubbed tests; `update`/`rollback`
  commands wired to E-5. Depends on E-4, E-5.
