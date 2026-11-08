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
2. **`rein-release::Verifier`** (rein D-153 r1–r2) — order is fixed: (a) parse the
   beside-tarball `manifest.json`, reject `schema != 1`; (b) compare `key_id` with the
   build-time baked minisign public key — mismatch fails with ONE line naming the newer
   rein-app version required (never a bare signature error); (c) **minisign** verify
   `rein-X.Y.Z.tar.gz.minisig` over the tarball with the baked key; (d) extract to staging,
   require the in-tarball `manifest.json` to be byte-identical to the beside one; (e) sha256
   + size of every listed file; reject a listed file missing, a tarball file not listed, a
   path with `..`/absolute/backslash, or a path outside `layout.mirrored ∪ operator_owned`;
   (f) apply `mode` from the manifest (`bin/rein`, `hooks/*.sh` are `0755`). The
   `SignatureVerifier` trait stays (the test suite injects a fixture key pair); the production
   backend is minisign and the `Rejecting` backend remains selectable by build flag as the kill switch.
3. **`rein-release::LayoutWriter`** (rein D-153 r5) — mirrors `install-rein.sh`: for every
   dir in `layout.mirrored` (`bin hooks mcp shared claude contracts templates copilot scripts
   config`) the staged tree REPLACES the installed one; a file in a mirrored dir that the
   tarball does not ship is deleted (`bin/` pruned explicitly). Paths listed in
   `layout.operator_owned` are never written, deleted or read by the app — read the list
   **from the manifest**, never hard-code it. Schema-1 list today: `accounts.json`, the gate's
   override file, `secrets/`, `run/`, `memory/`, `incidents.ndjson`, `telemetry.sqlite`,
   `INSIGHTS.md`, `memory-palace.md`, `config/rein-workspace.txt`, `config/install-stamp.json`,
   `mcp/*/node_modules/`, `mcp/*/package-lock.json`. **Pattern syntax** (rein D-153 addendum,
   binding): gitignore-style subset — literal path; trailing `/` = the directory and everything
   under it; `*` within one segment; `**` across segments; NO `!` negation, NO character
   classes, NO leading `/`. The matcher is a small pure function (`OperatorOwnedMatcher`)
   tested against rein's conformance fixture `tests/fixtures/release/operator-owned-cases.json`
   (`{path, expected}` rows) copied into our fixtures. Mechanics: build the next tree in `$HOME/.rein.staging-<ver>`
   (copy of operator-owned from the live tree + mirrored dirs from the tarball), then atomic
   rename with the previous tree kept as `$HOME/.rein.prev-<oldver>` (N=2); `VERSION` is
   written last, tmp-and-rename. Never edits any file after the swap.
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
   `extraKnownMarketplaces` and `enabledPlugins` — **both owned, neither authored by hand**
   (D-007, D-005): on Claude Code 2.1.319 a hand-written user-scope marketplace entry is not
   registered, so per profile with `CLAUDE_CONFIG_DIR=<config_dir>` the app runs
   `claude plugin marketplace add Vimurai/rein-plugins#stable --scope user`, then on
   `--enable` `claude plugin install <plugin>@rein-plugins --scope user` (closed list
   `rein-seo rein-ux rein-ops rein-data`), `--disable` → `claude plugin disable … --scope user`.
   After each call the app re-reads `settings.json` and VERIFIES the expected D-002 JSON
   (`source.repo == "Vimurai/rein-plugins"`, `source.ref == "stable"`, `"<p>@rein-plugins": true`);
   a missing/different `ref` is exit 7, never patched. All argv are constants behind `Shell`.
   `attribution` is written directly → validate JSON → temp file + rename. `claude` absent →
   `attribution` + `hooks` still applied, marketplace/plugins skipped, exit 4. Every pre-existing key
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
// manifest.json schema 1 — rein D-153 r2 (binding). Paths relative to the Rein home, '/', no '..'.
Manifest { schema: 1, version: SemVer, min_node: SemVer /* "22.5.0" */, key_id: String /* minisign key id */,
           layout: { mirrored: Vec<DirName>, operator_owned: Vec<RelPath|Glob> },
           files: Vec<{ path: RelPath, sha256: Hex64, size: u64, mode: OctalStr /* "0755" | "0644" */ }> }
ReleaseAssets { tarball: Path, signature: Path /* .minisig */, manifest: Path, tag: String }
VerifyReport { schema: Pass|Fail, key_id: Pass|NeedsApp(version), signature: Pass|Fail(reason),
               manifest_match: Pass|Fail, files: Vec<{path, Pass|Mismatch|Missing|Unlisted|OutsideLayout}> }
DoctorJson { version: SemVer, status: ok|warn|fail, checks: Vec<{ id: SnakeCase, status, line: String, fix: Option<String> }> }  // rein D-153 r3; phase 2 renders it
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
| `profiles apply [--profile NAME] [--enable LIST\|all] [--disable LIST] [--undo]` | — | per-profile diff of allowed keys, then write; `--enable/--disable` shell out to `claude plugin …` (D-005) | 0 · 4 `claude` missing · 5 parse error · 6 forbidden key attempted (bug) · 7 `claude plugin` failed |
| `launch` (Windows) | project dir | opens terminal into tmux | 0 · 20 WSL absent and install declined |
Library seams for phase 2: `rein_release::{fetch, verify, install}`, `rein_profiles::{enumerate, plan, apply}`,
each pure w.r.t. I/O behind `Fs`/`Http`/`Shell` traits so Tauri reuses them without a second code path.

## Security
- **Trust boundary 1 — network**: all downloaded bytes are untrusted until the minisign
  signature AND every sha256 pass. HTTPS only, pinned release host, size cap (default 200 MB),
  no redirects off-host. **Trust root = the minisign public key compiled into the binary**
  (rein D-153 r1); the app never fetches a key. A `key_id` mismatch is a "newer rein-app
  required" error, surfaced before any signature check. The `Rejecting` backend is a
  build-flag kill switch, not the default.
- **Trust boundary 2 — `$HOME/.rein`**: written once by LayoutWriter via staging + atomic
  rename. Tarball entries are checked for path traversal (`..`, absolute paths, symlinks
  escaping the root) before extraction. Operator files are carried over, never regenerated.
- **Trust boundary 3 — profiles**: closed `AllowedKey` enum; `hooks` content comes only from
  `rein install`'s own output (we re-read the file it wrote; we never author hook entries).
  Hard-deny on `autoMode`/`permissions`/`defaultMode` (`rein` D-132) on EVERY path including
  undo. Temp file + rename; `settings.json.rein-bak` is written ONCE (pre-Rein state) and
  never overwritten; `--undo` restores only the four allowed keys by value from it (D-008).
  `config_dir`/`settings.json` symlinks escaping `config_dir` are refused.
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
- **Profiles**: `profiles apply --undo` restores the four allowed keys by value from the
  once-written pre-Rein `settings.json.rein-bak` (removing keys the backup lacked); all other
  keys keep their current values — never a whole-file restore (D-008, security P0).
- **Package**: `brew uninstall rein` / cargo-dist uninstaller removes the binary only; the
  Rein home dir is operator data and is never removed by the app.
- **Feature kill**: the `Rejecting` signature backend can be re-selected by build flag to halt
  all installs if a verifier bug is found.

## Test Plan (acceptance for Engineer)
- Fixture release server signed with a TEST minisign key pair (never the production key),
  injected through `SignatureVerifier`; cases: good; bad `.minisig`; tampered file; missing
  manifest; manifest `schema: 2`; `key_id` mismatch (asserts the "newer rein-app" line);
  in-tarball manifest ≠ beside manifest; unlisted tarball file; `..` traversal entry; path
  outside `layout`; wrong `mode`. Each bad case asserts no write under a temp `$HOME`.
- Upgrade semantics: seed a temp `$HOME/.rein` with operator-owned files (incl.
  `config/rein-workspace.txt`, a fake `mcp/x/node_modules/`) and a stale `bin/old-tool`;
  after install the operator-owned files are byte-identical, `bin/old-tool` is gone, and
  `config/` otherwise matches the tarball.
- Layout equality: from a `rein` checkout at tag T, run `install-rein.sh` into temp HOME A;
  build the artifact with rein's own generator `rein release manifest <tree> --out <dir>`
  (unsigned — signing is the separate `rein release sign` step), sign it with OUR throwaway
  test minisign key (never rein's `tests/fixtures/release/` pair, which no app bakes in), run
  `rein-app install --version T` from the fixture server into temp HOME B; `diff -r` of A and
  B is empty except `VERSION` timestamp fields if any.
- Operator-owned matcher conformance: every row of rein's `operator-owned-cases.json` yields
  `expected`; the fixture is vendored and refreshed when rein bumps the manifest schema.
- Profile merge: property test — for random pre-existing JSON, after `apply` every original
  key/value is intact and only allowed keys changed; attempting a forbidden key fails to compile.
- Marketplace pin: a local `file://` git fixture with a `stable` branch;
  `claude plugin marketplace add file:///…/fixture.git#stable --scope user` must yield
  `source.ref == "stable"` in the written entry, else exit 7 (D-007). Stubbed `Shell` tests
  assert the exact argv per profile; one real-CLI test is gated on `claude` being present.
- Undo: after `apply` then operator edits to `permissions`, `--undo` leaves `permissions`
  untouched and reverts only the four allowed keys (D-008).
- WSL launcher: unit tests against a stubbed `Shell` covering no-WSL, WSL-no-distro, ready.

## E-## Task Breakdown (registered IDs)
- **E-2** (T2) Cargo workspace `rein-app`/`rein-release`/`rein-profiles`; `git init` if absent;
  cargo-dist config for mac/linux/windows; CI matrix (GitHub Actions) building + testing all
  three targets. Acceptance: `cargo test --workspace` green on all three in CI.
- **E-3** (T3) `rein-release` fetch + verify behind `Http`/`Fs` traits; `SignatureVerifier`
  trait with **minisign production backend** and baked public key, `Rejecting` as build-flag
  kill switch; manifest schema 1 + `key_id` + sha256/size/mode + traversal/layout checks;
  fixture server with the cases in §Test Plan; fail closed on every bad input. Depends on E-2.
  (Task text in state.sqlite predates rein D-153 — this blueprint + D-003 govern.)
- **E-4** (T3) `rein-profiles` enumerate/plan/apply with closed `AllowedKey` enum, backup +
  undo, property test for key preservation; `profiles list|apply`. Depends on E-2.
- **E-5** (T3) LayoutWriter (mirrored dirs replaced + pruned, operator-owned read from
  `layout.operator_owned`, `config/` mirrored with two kept files, atomic swap, N=2 prev,
  `VERSION` last) + Runner (`rein install`, `rein doctor` from the tree); layout-equality test
  against `install-rein.sh` and the upgrade-semantics test. Depends on E-3.
  (Task text in state.sqlite predates rein D-153 — this blueprint + D-003 govern.)
- **E-6** (T2) Homebrew formula (tap) + cargo-dist release workflow + `.deb`/AppImage;
  Windows WSL launcher behind `Shell` trait with stubbed tests; `update`/`rollback`
  commands wired to E-5. Depends on E-4, E-5.
