# Blueprint — Distribution (Phase 1: installer, verifier, profiles, updater, WSL launcher)

> Architect-owned. Derived from `rein` D-150 contract §2–§3 and `plans/rein-app-plan.md` Phase 1.
> Index: `.ai/architect.md` §0. Tasks: E-2..E-9 (see §Task Breakdown; plan IDs E-1..E-5 shifted by one).
> Revised 2026-11-09: LayoutWriter and rollback mechanics (D-011), E-5/E-6 divergence rulings (D-012),
> E-7 rulings — target symlinks, snapshot trust, owner guard (D-013); rein D-155 mirrored (D-014):
> strict tarball members, shipped path wins, `manifest.json` in the home, three profile keys.
> Revised 2026-11-10 (D-015, review of E-12..E-15): snapshot `shipped_owned`, header-only first
> pass, top-level `Conflict`, value-identical settings, operator docs (§Components 9) and the
> release rehearsal (E-18, E-19).

## Goal & Architecture
Operators get Rein with one package-manager command on macOS, Linux and Windows (WSL2), and
the result is a byte-verified copy of a tagged `rein` release laid out exactly as
`install-rein.sh` does, with `rein install` and `rein doctor` run from that tree. Rust CLI,
three crates, no Rein logic inside. Phase 1 is a CLI only; phase 2 wraps it in a window.

## Core Concept
A **verified release pipeline**: `fetch → verify (signature, manifest, sha256) → snapshot the
installed payload → per-file sync of the mirrored dirs inside $HOME/.rein → rein install →
rein doctor` (D-011). Nothing reaches the home before the whole release has verified; the
sync replaces each file atomically and restores from its snapshot if it cannot finish.
Operator data is never part of the pipeline. Around it sit two side tools: a
**profile manager** that edits three allow-listed settings keys per Claude profile, and an
**updater/rollback** that keeps the last two payload snapshots.

## Components
1. **`rein-release::Fetcher`** — resolves tag → asset URLs on the `rein` GitHub release
   (`rein-X.Y.Z.tar.gz`, detached signature, `manifest.json`); downloads to a per-run
   staging dir; enforces size cap and timeout; never follows redirects off the release host.
2. **`rein-release::Verifier`** (rein D-153 r1–r2) — order is fixed: (a) parse the
   beside-tarball `manifest.json`, reject `schema != 1`; (b) compare `key_id` with the
   build-time baked minisign public key — mismatch fails with ONE line naming the newer
   rein-app version required (never a bare signature error); (c) **minisign** verify
   `rein-X.Y.Z.tar.gz.minisig` (prehashed form only) over the tarball with the baked key;
   (d) extract to staging,
   require the in-tarball `manifest.json` to be byte-identical to the beside one; (e) sha256
   + size of every listed file; reject a listed file missing, a tarball file not listed, a
   path with `..`/absolute/backslash, or a `files` path that is not under a `layout.mirrored`
   directory (rein D-155 r2: nothing ships outside the mirror, so a top-level or
   owned-only path in `files` is a refusal);
   **path alphabet and case rule (rein D-155 addendum 3, mirrored D-020; E-26):** in the
   manifest parse — as built, after the signature check and the header-only member scan,
   before the member-set check, staging and any write (D-021 corrects D-020's "before
   signature") — every `files` path, every `layout.mirrored` name and every
   `layout.operator_owned` pattern (E-27) must be printable ASCII (bytes 0x20–0x7E), and no
   two paths may be equal after ASCII-lowercasing (A–Z only) while differing in bytes —
   every directory prefix of a path counts as a path (`shared/Lib` beside
   `shared/lib/x.mjs` collides). Either is a verification failure (exit 2) naming the path
   or the two paths; nothing is staged or written. The tarball member set equals `files`,
   so members need no second check. **Case-variant of an owned path (D-023, E-29):** a
   `files` path, or a directory prefix of one, that matches a `layout.operator_owned`
   pattern only after both are ASCII-lowercased (it does not match as written) is refused
   in the same place — on a case-insensitive home it would write release bytes over
   operator data. The test is **per pattern** (D-026): a path that some pattern matches
   only after folding is refused even when another pattern matches it exactly. A path
   that no pattern matches fold-only, and that a pattern matches exactly as written, is
   still allowed (shipped-path-wins, D-015). **Directory spellings (D-026):** a
   `layout.mirrored` name, or any directory prefix of a `files` path, that is equal after
   ASCII-lowercasing to the leading literal segments of an owned pattern but differs from
   them in bytes is refused the same way — the scan builds live paths from the manifest's
   spelling, so `mirrored: ["Shared"]` beside the pattern `shared/toolchain.sh` would make
   the exactly-owned file look unowned on a case-insensitive home and prune it. **Error text (D-023):** every string taken from a manifest
   — paths, patterns, `version`, `min_node`, `key_id` — is escaped before it is printed.
   Windows-reserved names are not ruled (rein has not);
   (f) apply `mode` from the manifest (`bin/rein`, `hooks/*.sh` are `0755`).
   **Member rules (rein D-155 r1, binding — a strict reader that accepts only what rein's
   writer emits):** the FIRST member is `manifest.json`; every other member is a regular file
   (ustar typeflag `0`) named in `files`; the member set equals `files` exactly. Refused, for
   the whole artifact and before any byte reaches the home: a directory entry, any link type,
   a pax or GNU extension header, a device/FIFO, a duplicate name, `manifest.json` anywhere
   but first, a `VERSION` member. As built (E-12, accepted D-015): a **header-only first
   pass** over the whole archive applies every member rule before pass 2 extracts anything,
   even into staging; pass 2 re-counts what it writes; only zero padding may follow the
   end-of-archive marker and nothing may follow the gzip stream. The beside-tarball manifest is an untrusted convenience
   copy: it may fail a run early (schema, `key_id`) but every decision that lets bytes into
   the home is taken from the in-tarball manifest after the signature has passed. The
   `SignatureVerifier` trait stays (the test suite injects a fixture key pair); the production
   backend is minisign and the `Rejecting` backend remains selectable by build flag as the kill switch.
3. **`rein-release::LayoutWriter`** (rein D-153 r5; mechanics **D-011**, which supersedes the
   whole-home staging swap of blueprint v1 / E-5) — an **in-place, per-file sync of the
   mirrored directories**, the way `install-rein.sh` does it (its E-229 / E-304 comments are
   the reference). `$HOME/.rein` itself is never renamed, copied or moved. For every dir in
   `layout.mirrored` (`bin hooks mcp shared claude contracts templates copilot scripts
   config`) the release content REPLACES the installed one; a file in a mirrored dir that the
   tarball does not ship is deleted. **Scope rule (binding; confirmed by rein D-155 r2):** the
   writer opens only (i) paths under a `layout.mirrored` directory, (ii) `manifest.json` and
   (iii) `VERSION` at the top of the home. Every other entry of the home — on
   the `operator_owned` list or not (`approvals.sqlite*`, `cache.sqlite*`, `ci/`, `privacy/`,
   `trash/`, `clean.log`, anything a later rein adds) — is never read, copied, written or
   deleted. **Inside mirrored dirs `operator_owned` protects from deletion only, and a shipped
   path wins over a pattern (rein D-155 r2):** a path in `files` is always written, even when
   a pattern matches it (`mcp/orchestrator-mcp/package-lock.json` is shipped and matches
   `mcp/*/package-lock.json`); a path not in `files` is deleted unless it matches a pattern.
   An owned path the release does not ship is still never opened, and prune never descends
   into an owned directory. `rein install` checks the installed tree against the manifest
   afterwards (exit 3 on any difference), so a skipped shipped file is a failed install. Read the list
   **from the manifest**, never hard-code it. Schema-1 list today (fourteen entries):
   `accounts.json`, `gate-override.json`, `secrets/`, `run/`, `memory/`, `incidents.ndjson`, `telemetry.sqlite`,
   `INSIGHTS.md`, `memory-palace.md`, `config/rein-workspace.txt`, `config/install-stamp.json`,
   `shared/toolchain.sh`, `mcp/*/node_modules/`, `mcp/*/package-lock.json`. **Pattern syntax** (rein D-153 addendum,
   binding): gitignore-style subset — literal path; trailing `/` = the directory and everything
   under it; `*` within one segment; `**` across segments; NO `!` negation, NO character
   classes, NO leading `/`. The matcher is a small pure function (`OperatorOwnedMatcher`)
   tested against rein's conformance fixture `tests/fixtures/release/operator-owned-cases.json`
   (rows of path, shipped yes/no, expected `kept` / `replaced` / `deleted` — rein D-155;
   published with rein E-422) copied into our fixtures; until it lands our interim fixture
   uses the same three-column shape and carries the precedence case.
   **Steps** (all under `$HOME/.rein.lock`; the release is already fully verified, outside the home):
   a. *Preflight, no writes*: **owner guard** (D-013, unix) — the process must run as the
      owner of `$HOME`, of `$HOME/.rein` when it exists and, for rollback, of the snapshot
      root and its `layout.json`; the uid is read from the lock file the run has just created
      (no `unsafe`, no new dependency); a mismatch (e.g. `sudo rein-app update` with the
      user's HOME) refuses, naming the path. `$HOME` is judged by the directory it resolves
      to; `.rein` and snapshot paths are never followed; a uid that cannot be read refuses.
      The guard is a **type-level precondition** (E-11): it returns a proof value that only
      it can construct, borrowed from the lock, and every lock-held function that writes
      takes that value — a caller (phase 2 included) cannot reach the writer without having
      run the guard, the same way a forbidden settings key cannot be written. Then: `$HOME/.rein` is absent (fresh install →
      create it) or a real directory; every live mirrored dir is a real directory and no
      *intermediate* component of a target path is a symlink — otherwise refuse, naming the
      path (`install-rein.sh` `[INSTALL_BIN_SYMLINK]`). A symlink **at** a target path is not
      a refusal: the rename replaces the link itself, an unshipped link is pruned as a link
      and snapshotted as a link (as `rsync -a --delete` does) — refusing it would let one
      stray `bin/x -> …` block restore-on-failure and rollback. Nothing is ever written
      through a symlink. Also refused: a directory where a release file goes or a file where
      a directory is needed (`Conflict`) — a directory at the home's top-level `manifest.json`
      or `VERSION` included (D-015; a symlink there is replaced by the rename, never
      followed) — and a shipped name ending in the writer's own temp suffix. Inside a mirrored dir an entry of ANY type at an operator-owned path — a symlink
      or file where an owned directory belongs (`mcp/x/node_modules -> store`) — is owned:
      never opened, snapshotted or pruned, unless the release ships that exact path (then it
      is a target like any other; a symlinked owned directory above a shipped path is an
      intermediate symlink and refuses).
   b. *Plan (pure)*: `ship` = every manifest file (the verifier has already proven each is
      under a mirrored dir); `prune` = every
      file or symlink under a live mirrored dir that is neither in `ship` nor operator-owned.
      **Owned matching is case-sensitive (rein's contract; D-025 withdraws D-023 ruling
      2):** rein's conformance table `tests/fixtures/release/operator-owned-cases.json`
      confirmed by rein D-155 addendum 3 r9, 2026-11-10 — every protected name is written
      by a tool under its exact spelling, so no fold. A live file **inside a mirrored
      directory** that is a case variant of an owned pattern is an ordinary unshipped file:
      snapshotted, then pruned. **Top-level entries are out of scope (rein addendum 3
      r10, rule 1 of the installer rule):** the sync writes and deletes only inside the
      mirrored directories, plus `manifest.json` and `VERSION`; every other top-level
      entry (`ci/`, `trash/`, anything later) is left alone, matched or not. As built, the
      scan walks only mirrored roots, so this holds; the pure `plan` function must give
      the same answer for a path outside the mirrored directories when rein's corrected
      table arrives (E-17). A live symlink whose name is a case variant of a
      `ship` path is a preflight `Conflict` naming it, not an `EEXIST` from the snapshot.
   c. *Snapshot*: copy the non-owned content of the live mirrored dirs **plus the live file at
      every `ship` path** (so a shipped path that a pattern also matches can be restored) to
      `$HOME/.rein.prev-<oldver>/tree/`, with `VERSION`, `layout.json` (the `mirrored` +
      `operator_owned` lists used, plus `shipped_owned`: the owned-matching paths this
      snapshot holds because they were shipped — on load, a tree path that matches a pattern
      is accepted only when listed there, an owned directory only to reach such a path;
      D-015) and the live top-level `manifest.json` when there is one
      beside it; built under a temp name and renamed when
      complete, mode 0700, `~N` suffix on a name clash, symlinks copied as links. This is the
      release payload only (≈9 MB), never operator data. No live tree → no snapshot. Keep the
      N=2 newest snapshots. A live tree whose `VERSION` is missing or not semver gets no
      snapshot either (nothing to label or roll back to): the sync still runs and the CLI
      says in one line that no snapshot was taken and rollback is unavailable for this
      update. Only a directory with a real `VERSION` and `layout.json` counts as a snapshot;
      any other `.rein.prev-*` is the operator's and is never pruned or restored.
   d. *Sync*: mirrored dirs in manifest order with **`hooks` last** (a hook never lands before
      the `mcp/` and `shared/` it calls). Per `ship` file: temp file in the target's own
      directory → mode from the manifest → rename over the target, so a reader sees old or
      new, never half (a byte-identical file whose mode already matches MAY be skipped). Then
      remove that dir's `prune` entries and any directory left empty.
      **Case-variant prunes go first (D-021, E-27):** a `prune` entry whose ASCII-lowercased
      path equals the ASCII-lowercased path of a `ship` entry (or of a directory prefix of
      one) names, on a case-insensitive home (APFS, drvfs), the same file the sync is about
      to write — removing it after the write would delete the shipped file and still exit 0.
      Such entries are removed **before** that directory's writes; all other prunes stay
      after. The result is the same on both filesystem kinds: the old name is gone and the
      new one is present. An **empty** case-variant directory is also removed before the
      writes, so the new spelling lands (as built E-27, ratified D-023). A directory that
      still holds unshipped or owned content keeps its old-case name on a case-insensitive
      home is accepted. The rule lives in the one `plan`/`apply` path, so update, rollback
      and failure-restore all get it. **Post-sync check:** after the last directory, every
      `ship` path must exist as a regular file of the listed size; a miss is a sync failure
      and takes the existing failure-restore path.
   e. After `hooks`: the in-tarball `manifest.json`, byte for byte, to `$HOME/.rein/manifest.json`
      (tmp-and-rename) — it is how `rein install` recognises an artifact install (rein D-155
      r1/r4: `manifest.version == VERSION`). Then `VERSION` (one line, `manifest.version`)
      last, tmp-and-rename, so a failed sync never advertises a version. rein confirmed this
      order (D-155 addendum r2) and fails closed on both in-between states: a `manifest.json`
      that is unreadable, not schema 1, or disagrees with `VERSION` (or has none beside it) is
      `[INSTALL_INCOMPLETE]`, exit 3, nothing done; a tree that differs from a matching
      manifest is `[INSTALL_TREE_MISMATCH]`. rein reads no lock of ours. **Consequence for
      us:** the home must never be left with a `manifest.json` that disagrees with `VERSION`
      once a run has ended successfully or restored — step (f) and rollback own that pair.
   f. *Failure in d/e*: stop, sync the snapshot from (c) back with the same routine — the
      snapshot's `manifest.json` is put back, or the live one removed when the snapshot has
      none (a clone install has no manifest) — report
      `Sync { restored }`; `VERSION` stays at the old version. On a fresh install there is
      nothing to restore: the partial tree has no `VERSION` and a re-run completes it (and
      no `manifest.json` is left beside an absent or invalid `VERSION`).
      **Accepted residue (D-015):** a path the failed (or rolled-back) release shipped for
      the first time AND that matches an `operator_owned` pattern stays on disk after the
      restore — by rein D-155 r2 a path not in the restored release's `files` that matches a
      pattern is kept, and the app keeps no record across runs of who wrote an owned path.
      Likewise a snapshot holds an owned-matching path only when the *incoming* source ships
      it. Both leave at worst a stale file at an owned path; if `rein install` then reports
      a tree mismatch (exit 13), `rein-app install --version <that version>` rewrites every
      shipped path. Reopen if rein ever changes the set of shipped owned-matching paths
      between releases (today: `mcp/orchestrator-mcp/package-lock.json`, in every release).
   A temp file left by a killed run is an unshipped file and is pruned by the next run. No
   file is edited after `VERSION` is written. Disk and time are proportional to the release,
   not to the home (measured live home: 3.5 GB, of which `ci/` 3.0 GB and `mcp/*/node_modules`
   551 MB; release payload 8.9 MB).
4. **`rein-app::Runner`** — invokes `rein install` then `rein doctor` from
   `$HOME/.rein/bin` with a clean env (`CLAUDECODE` unset, PATH prefixed with
   `$HOME/.rein/bin`); streams output to the terminal verbatim; exit code propagates.
   Only contract §2.6 entry points are ever invoked. `rein install` finishes an unpacked
   artifact itself (rein D-155 r4): tree check against `manifest.json`, Node floor, hook
   interpreter probe, toolchain pin, install stamp (`source: "artifact"`), shell profile
   lines; it also removes a retired MCP server's whole directory and the legacy `gemini/` /
   `agents/` mirrors (D-155 addendum r1) — the app's writer leaves those alone, as the
   mirror rule says. The app reimplements none of it. Its exits: 3 = `[INSTALL_TREE_MISMATCH]`,
   `[INSTALL_INCOMPLETE]` or a gate
   dependency off its pin, 1 = Node floor or `[HOOK_BASH_MISSING]`. The app reports a failed
   `rein install` as its own exit 13 (rein's code printed) and a failed `rein doctor` as exit
   3, and never retries or repairs either.
   **Install verdict (rein D-158, mirrored D-029; E-32, built when rein E-427 merges):**
   after `install`, `update` and `rollback` the app runs `rein install`, then
   **`rein doctor --scope install --json`**, parses the document and judges by its
   top-level `status`: `fail` → exit 3; `warn` and `ok` → exit 0. Host and project checks
   never fail an install. The call reads nothing relative to the current directory. A
   document that is missing, not valid JSON, or without a known `status` is exit 14, never
   a pass. Every check record carries a required `scope` (`install` | `host` | `project`);
   the app never filters by check id. The checks of the document are printed for the
   operator (name, status, message); `rein-app doctor [--json]` stays a passthrough of the
   full `rein doctor` in the operator's directory. Also from D-158: no `rein` command
   writes a path listed in `manifest.files` (rein E-426, `npm ci`), so rein's tree check
   exempts nothing.
5. **`rein-app::RuntimeCheck`** — detects Node (≥ manifest `minNode`, default 22), git,
   sqlite3, tmux, Claude Code (`claude --version`); prints a table; missing Claude Code →
   print the official installer URL, do not bundle. Does not install anything itself in phase 1.
   Node ≥ `min_node` and git are **required** (exit 4); sqlite3, tmux and Claude Code are
   reported only — `rein doctor` judges them (D-012).
6. **`rein-profiles`** — enumerates profiles: default config dir (`~/.claude`),
   `~/.claude-personal` if present, and every `accounts.<name>.config_dir` in
   `$HOME/.rein/accounts.json` (read-only here: rein's `accounts.py` is the file's only
   writer, and only `config_dir` is relied on — rein D-153 addendum 2). **Enumeration order
   and names (D-022, E-28):** (1) `default` = `~/.claude`; (2) every `accounts.json` row, the
   row's `config_dir` authoritative for its name — a row named `personal` is an ordinary
   account; (3) the built-in `~/.claude-personal`, named `personal`, only when no row has
   that name and the directory is not already listed. A directory is listed once. Only
   `default` is reserved: a row named `default` is silent when it points at `~/.claude` and
   otherwise skipped with a warning that says exactly that. A row name is not re-validated
   against a grammar of ours (rein's `accounts.py` owns the grammar); it is checked only
   for terminal safety — printable ASCII 0x21–0x7E, at most 64 bytes — and a failing row is
   skipped with a warning that shows the name escaped. No warning is ever printed for a
   row that is then listed. **A row that is not listed never suppresses the built-in
   (D-024):** "no row has that name" means no *listed* row — an unusable `personal` row
   (missing or relative `config_dir`, unsafe bytes, or a duplicate of a directory already
   listed) gets its warning and the built-in is still considered. A row dropped as a
   duplicate is warned about, naming the profile that already holds the directory.
   **`config_dir` terminal safety (D-024, E-30):** a row whose `config_dir` contains a C0
   control, DEL, a C1 control or a Unicode bidirectional-override/isolate character is
   skipped with an escaped warning — such a path never reaches a table, an error or a
   command line. The dry-run `would run …` line shell-quotes the path. `apply` prints one
   note when a profile's `config_dir` has a final component that does not start with
   `.claude` (rein-app writes wherever `accounts.json` points; the guide says so). For each `<config_dir>/settings.json`: read → parse →
   merge ONLY the **three** keys the contract allows (rein D-155 r3, corrects contract §2.3):
   `enabledPlugins`, `extraKnownMarketplaces`, `attribution`. **Never `hooks`**: the gate's
   hook entries are not in a profile at all — they are in each project's
   `.claude/settings.json`, written only by `rein init` / `rein sync` — and the app writes no
   file inside a project. The app neither copies, authors nor removes a `hooks` key anywhere.
   `extraKnownMarketplaces` and `enabledPlugins` are **both owned, neither authored by hand**
   (D-007, D-005): on Claude Code 2.1.319 a hand-written user-scope marketplace entry is not
   registered, so per profile with `CLAUDE_CONFIG_DIR=<config_dir>` the app runs
   `claude plugin marketplace add Vimurai/rein-plugins#stable --scope user`, then on
   `--enable` `claude plugin install <plugin>@rein-plugins --scope user` (closed list
   `rein-seo rein-ux rein-ops rein-data`), `--disable` → `claude plugin disable … --scope user`.
   After each call the app re-reads `settings.json` and VERIFIES the expected D-002 JSON
   (`source.repo == "Vimurai/rein-plugins"`, `source.ref == "stable"`, `"<p>@rein-plugins": true`);
   a missing/different `ref` is exit 7, never patched. All argv are constants behind `Shell`.
   `attribution` is written directly → validate JSON → temp file + rename. `claude` absent →
   `attribution` applied, the D-002 marketplace entry is hand-written WITH
   `ref: "stable"` as a fallback (Claude Code reconciles it at its next session start —
   D-009), plugins skipped, exit 4. The D-007 path must be idempotent over an entry the
   reconciler already registered. Every pre-existing key
   is preserved **by value, in its original key order** (D-015): the file is parsed and
   re-serialised (`preserve_order`, `arbitrary_precision`), so whitespace may change but no
   key, value, number spelling or ordering does — "byte-identical" was never the guarantee
   for a file the app rewrites. Writing `autoMode`, `permissions`, `defaultMode` or
   `hooks` is a compile-time impossibility (the key set is a closed enum of three).
7. **`rein-app::Updater`** — `rein-app update`: list newest `rein` tag, fetch its
   changelog/release notes, show diff vs installed `VERSION`, install on confirm (`--yes`
   for CI). The newest tag comes from the GitHub releases API (metadata only; the release is
   still verified and version-bound); `install` without `--version` uses the same resolver.
   `rein-app rollback` (D-011): take a snapshot of the current tree (step 3c), sync the newest
   earlier snapshot back with the LayoutWriter routine (`ship` = the snapshot's files, layout
   from its validated `layout.json`), put its `manifest.json` back (or remove the live one
   when the snapshot has none), write its `VERSION`, remove the snapshot just restored
   (so a second rollback returns to the newer version), then run **`rein install` and
   `rein doctor`** exactly as after an install (D-014): a rollback is an install of the older
   payload, and only `rein install` can re-pin the toolchain and rewrite the stamp for it
   (rein guarantees this is safe — D-155 addendum r3: in artifact mode the stamp is always
   rewritten; after a rollback to a clone-install payload it becomes `source: "restored"` and
   doctor prints one note line; `config/rein-workspace.txt` is not recreated by anything but
   `bash install-rein.sh`, which the CLI says in its rollback summary for that case).
   Rollback restores **code only**: operator data is never rolled back.
   `rein-app doctor [--json]` passes the flag through to `rein doctor`; phase 1 never parses
   the document (exit 0 or 1 means stdout is one; anything else means it is not). Rollback works with the
   restored snapshot's layout, so a mirrored dir that only the newer release lists stays in
   place (unused by the older code; the next update prunes it).
8. **`rein-app::WslLauncher`** (Windows target only) — `wsl --status`; if no distro,
   `wsl --install -d Ubuntu`; runs the Linux `rein-app update --yes` inside the distro (a
   fresh home installs, exit 10 "already current" counts as success; a Linux `rein-app` must
   already be in the distro); opens
   Windows Terminal (`wt.exe`) into the project's tmux session via `wsl -e rein start`;
   refuses a project path containing `;` (the `wt.exe` command separator).
   All `wsl.exe`/`wt.exe` calls go through a `Shell` trait so tests stub them.
9. **Operator documentation** (E-18; no code) — `README.md` at the repo root and
   `docs/operator-guide.md`. The README says in its first screen what rein-app is (the
   installer/updater shell for Rein) and is not (no Rein logic; it never edits a project),
   the status line "no release has been published yet" until E-6 closes, the install
   channels as *planned* (Homebrew tap, `.deb`, Windows launcher over WSL2), building from
   source (every build bakes the committed key `docs/contract/release-key.pub` — E-16; a
   build with the `reject-all-signatures` feature trusts nothing and refuses every release), and the license. The guide documents only behaviour this blueprint states:
   every command and exit code of §API, in one table; what an install touches (the scope
   rule: mirrored dirs + `manifest.json` + `VERSION`; operator data never opened);
   snapshots and `rollback` (code only, two kept, second rollback returns); `profiles
   apply` (three keys, never `hooks`, the once-written `.rein-bak`, `--undo` by value);
   refusals an operator can meet and what to do (held lock → the named PID; `sudo` / owner
   mismatch → run as the home's owner; symlinked home or mirrored dir; exit 13 → rein's own
   code and message, re-run `install`); the unsupported shared `--home`; Windows = WSL2
   only. Nothing in either file names a version, URL or key that does not exist yet, and
   neither restates rein's contract — they name rein's `docs/app-contract.md` in plain
   text, without a link, as the file rein will publish with its first release (it does not
   exist yet — D-016 ruling 11), and may link the `Vimurai/rein` repository itself; the link
   to the file is added when rein publishes it (with E-17).

## Data Model
```
// manifest.json schema 1 — rein D-153 r2, D-155 (binding; normative text: rein `.ai/blueprints/release-artifact.md`,
// published as rein `docs/app-contract.md` + `docs/contract/manifest.schema.json`). Paths relative to the Rein home,
// '/', no '..'; every `files` path is under a `mirrored` dir; `mode` is "0755" or "0644" and nothing else.
Manifest { schema: 1, version: SemVer, min_node: SemVer /* "22.5.0" */, key_id: String /* minisign key id */,
           layout: { mirrored: Vec<DirName>, operator_owned: Vec<RelPath|Glob> },
           files: Vec<{ path: RelPath, sha256: Hex64, size: u64, mode: OctalStr /* "0755" | "0644" */ }> }
ReleaseAssets { tarball: Path, signature: Path /* .minisig */, manifest: Path, tag: String }
VerifyReport { schema: Pass|Fail, key_id: Pass|NeedsApp(version), signature: Pass|Fail(reason),
               manifest_match: Pass|Fail, files: Vec<{path, Pass|Mismatch|Missing|Unlisted|OutsideLayout}> }
DoctorJson { schema: 1, version: SemVer, status: ok|warn|fail,
             checks: Vec<{ id: SnakeCase, subject: Option<String>, status, line: String, fix: Option<String> }> }
             // rein D-153 r3 + D-155 r5; (id, subject) unique, print order; phase 2 renders it, phase 1 passes it through.
             // Not a polling probe — `rein state health --short` is.
InstalledTree { root: $HOME/.rein, version: SemVer (from VERSION), snapshots: Vec<Snapshot> /* newest first, ≤ 2 */ }
Snapshot { root: $HOME/.rein.prev-<ver>[~N], version: SemVer (root/VERSION),
           layout: { mirrored, operator_owned, shipped_owned: Vec<RelPath> /* optional; default empty; unknown fields refused */ }
                   (root/layout.json — mirrored + operator_owned validated like a manifest layout on read; a tree path matching
                    a pattern must be listed in shipped_owned — D-015),
           manifest: Option<root/manifest.json> /* the home's manifest.json at snapshot time; absent for a clone install */,
           tree: root/tree/<mirrored dir>/… /* release payload only: non-owned content + the files at shipped paths; nothing outside mirrored dirs */ }
SyncPlan { ship: Vec<RelPath>, prune: Vec<RelPath> }   // pure function of (manifest | snapshot, live mirrored dirs)
Profile { name: String, config_dir: AbsPath, settings_path: AbsPath, exists: bool }
AllowedKey = EnabledPlugins | ExtraKnownMarketplaces | Attribution   // closed enum of three (rein D-155 r3); no Hooks
ProfilePatch { profile: Profile, changes: Map<AllowedKey, serde_json::Value> }
RuntimeReport { node: Found(ver)|Missing|TooOld(ver), git, sqlite3, tmux, claude: Found|Missing }
```
Persistence: none of our own beyond the `$HOME/.rein.prev-*` snapshots. State of record is the installed
tree's `VERSION` and the profiles' `settings.json`. No database, no app config file in phase 1.

## API / Interface Contracts
CLI (`rein-app`):
| Command | Input | Output | Exit |
|---|---|---|---|
| `install [--version X.Y.Z]` | tag (default latest) | verify report, `rein install` and `rein doctor` output | 0 ok · 2 verify fail · 3 `rein doctor` failed · 4 runtime missing · 13 `rein install` failed (its own code printed) |
| `update [--yes]` | — | available version + notes, then install | as install · 10 already current · 11 offline |
| `rollback` | — | restored version (code only — says operator data is untouched), `rein install` and `rein doctor` output | 0 · 3 · 13 as install · 12 no snapshot |
| `doctor [--json]` | — | passthrough of `rein doctor [--json]`, stdout untouched | rein's exit code · 14 `rein doctor` could not be started or ended without an exit code (never 1 — E-20) |
| `profiles list` | — | table of Profile | 0 |
| `profiles apply [--profile NAME]… [--enable LIST\|all] [--disable LIST\|all] [--undo] [--dry-run]` | `--profile` repeatable (default: every profile) | per-profile diff of allowed keys, then write; `--enable/--disable` shell out to `claude plugin …` (D-005); `--dry-run` prints the diff, writes nothing and runs no `claude` command | 0 · 4 `claude` missing · 5 parse error · 7 `claude plugin` failed |
| `launch [DIR]` (Windows; DIR default `.`) | project dir | opens terminal into tmux | 0 · 4 no `rein-app` inside the WSL distro · 20 WSL absent and install declined |
**All commands (D-016):** `--home DIR` is a global flag (default `$HOME`; must be absolute).
Exit **1** = refused before any write, or any other error — held lock, owner mismatch,
symlinked home / mirrored dir / intermediate component, `Conflict`, no absolute home,
download-directory failure, `launch` off Windows, a profiles I/O error; the message names the
path or cause. Exit **64** = usage error (unknown command or flag; E-20) — never 2, which
means a release failed verification. For `doctor`, 1 is rein's own code (the JSON document
with status `fail`); the app's own failures there are 14. Exit 6 is retired and never
reused: a forbidden settings key cannot be written by any code path that compiles.
Library seams for phase 2: `rein_release::{fetch, verify, install}`, `rein_profiles::{enumerate, plan, apply}`,
each pure w.r.t. I/O behind `Fs`/`Http`/`Shell` traits so Tauri reuses them without a second code path.

## Security
- **Trust boundary 1 — network**: all downloaded bytes are untrusted until the minisign
  signature AND every sha256 pass. HTTPS only, pinned release host, size cap (default 200 MB),
  no redirects off-host. **Trust root = the minisign public key compiled into the binary**
  (rein D-153 r1); the app never fetches a key. A `key_id` mismatch is a "newer rein-app
  required" error, surfaced before any signature check. The `Rejecting` backend is a
  build-flag kill switch, not the default. **Where the baked key comes from (D-014):** a
  file committed in this repo, copied byte for byte from rein `docs/contract/release-key.pub`
  (rein D-155 r6) — reviewable in git, identical in CI and source builds, rotated by a
  commit. **As built (E-16, 2026-11-10):** `docs/contract/release-key.pub` is committed
  (`.gitattributes` `-text`, so no checkout rewrites its line ends) and baked with
  `include_str!` through a strict parser; a file that does not parse yields the `Rejecting`
  backend. The `REIN_RELEASE_PUBKEY` variable is gone. **Key-file guard (D-017):** one
  script, `.github/scripts/check-release-key.sh`, is the only implementation of the file
  check (regular file, no NUL, no carriage return — tested on the raw bytes, because Git
  Bash drops a CR inside `$(...)` — two lines, `Ed` algorithm bytes, comment id == key id);
  the release workflows call it, and CI runs it against a fixture corpus on all three OSes.
  **Tag gate (D-017, as built E-22, D-018):** on every run of the release workflow that can
  publish (`!github.event.pull_request` — tag pushes today), before any artifact is built, it
  fetches `docs/contract/release-key.pub` from rein's `main` over HTTPS and compares it byte
  for byte with the committed copy — absent or different fails the release. The fetched
  bytes are compared and discarded, never built from. `CODEOWNERS` names the operator for
  `docs/contract/**`, `.gitattributes`, `.github/**`, `crates/rein-release/**`, `Cargo.toml`
  and `Cargo.lock` (D-018). It is advisory while the repository has one human account —
  GitHub does not let an author approve their own pull request — so the binding control is
  a tag ruleset that lets only the operator create `v*` tags (operator setting, D-018). A
  tag placed on a commit older than E-22 would run the workflow without the gate; the
  ruleset and the operator being the only tagger cover that. **The production key signs rein
  releases and nothing else:** no fixture is ever signed with it; the positive proof that
  the baked key matches the operator's secret key is the first real rein release verifying
  (E-9 / the rc install). No
  agent generates, handles or guesses a release key. **Outside the signature (rein D-155
  r7):** the npm dependencies `rein install` fetches; only rein's gate pins cover them.
- **Trust boundary 2 — `$HOME/.rein`**: written only by LayoutWriter, only under the mirrored
  dirs plus `manifest.json` and `VERSION`, per file via temp + rename, after the whole release has verified
  (D-011). Tarball members are regular files only; a member the manifest does not list, an
  absolute name, a `..` segment, a directory entry, an extension header or any link type
  refuses the whole artifact before the first byte reaches the home (rein D-155 r1). Operator data is never opened, so there is nothing
  to tear, lose or duplicate: a rein that is running during an install keeps writing its own
  SQLite/WAL files undisturbed, and `hooks/` is never absent — every gate hook is the old or
  the new file at every instant (security review E-5 P1-3; no "refuse while rein runs" check).
  A snapshot is local, unsigned data in the same trust domain as the home: it is a real 0700
  directory (never a symlink), its `layout.json` is validated like a manifest layout, and
  only paths under its listed mirrored dirs are ever restored. **Accepted residual risk
  (D-013, security review E-7 P2-2):** whoever can edit a snapshot's `layout.json` can widen
  what `rollback` syncs and prunes (`mirrored += secrets`). That needs write access as the
  home's owner, which already allows editing `secrets/`, `hooks/` and the gate directly; a
  second local copy of the layout would be exactly as editable and adds no trust. What makes
  the premise hold is the **owner guard** (step 3a): the writer never runs as a different
  user than the one who owns the home and the snapshot, so a root-run rollback can never be
  steered by a user-edited file, and no root-owned file lands in a user's home. The `Faults`
  seam and the uid-injectable `owner_guard_with` are `#[doc(hidden)]` test surface; no
  production caller (phase 2 included) may use them — production code calls `owner_guard`,
  which always reads the real uid. They stay exported without a cargo feature: code in the
  same process is not a security boundary, and a feature would split the test matrix.
  **Unsupported, documented, not defended (D-013 addendum):** a `--home` inside a directory
  that other users can write (`/tmp`, a shared root-owned directory). The guard checks the
  owners of the home, `.rein`, the snapshot root and its `layout.json`, not every entry
  beneath them, and snapshot/fetch-dir cleanup carries no owner check; a private home
  directory is the precondition, as it is for rein itself.
- **Trust boundary 3 — profiles**: closed `AllowedKey` enum of three (`enabledPlugins`,
  `extraKnownMarketplaces`, `attribution`). `hooks` is not ours in any file: the app never
  reads a `hooks` value to copy it, never writes one, and writes nothing inside a project
  (rein D-155 r3).
  Hard-deny on `autoMode`/`permissions`/`defaultMode`/`hooks` (`rein` D-132, D-155) on EVERY path including
  undo. Temp file + rename; `settings.json.rein-bak` is written ONCE (pre-Rein state) and
  never overwritten; `--undo` restores only the three allowed keys by value from it (D-008).
  `config_dir`/`settings.json` symlinks escaping `config_dir` are refused.
- **Trust boundary 4 — process**: only contract §2.6 `rein` entry points are executed, with
  args from a fixed list; no user-supplied strings reach a shell. Role tokens are never minted
  (`rein pane` owns that).
- **Secrets**: none stored. `secrets/` under the Rein home is carried over opaque.
- **Review**: E-3, E-4, E-5 are Tier 3 → `security_engineer` before commit.

## Execution Constraints
- Install end-to-end (excluding download) < 10 s on a laptop; sha256 of the full tarball
  streamed, not buffered. Memory < 100 MB. Install I/O is bounded by the release payload,
  never by the size of the home; extra disk is at most two payload-sized snapshots.
- **Manifest size limits (D-027, E-31):** a manifest is refused at parse (exit 2, naming the
  limit) above **128** `layout.mirrored` names, 256 `layout.operator_owned` patterns, 100000
  `files`, 1024 bytes per path or pattern, or **64** path segments (128 and 64 replace the
  first draft's 64 and 32 — D-027 addendum: rein has 10 mirrored names and paths 5 deep). Each limit is at least ten
  times what rein's generator emits (measured at rein 3f68eff, recorded in `.ai/DEVOPS.md`).
  With them, every parse-time and plan-time check is linear or near-linear in the manifest:
  the case and owned checks are table passes (E-29), `under_mirrored` is a set lookup, and
  the sync's folded-path and symlink scans keep no per-prefix copies. Only a manifest
  signed with the release key reaches these checks, so this bounds a mistake or a stolen
  key's nuisance value, not an anonymous attacker.
- Single-writer: a lock file `$HOME/.rein.lock` prevents concurrent `install`/`update`/`rollback`
  and is held through `rein install` / `rein doctor`. A lock left by a killed run is cleared
  by the operator (the message names the PID); a liveness check is deferred to Phase 2 (D-012).
- rein may be running while the writer runs: no process check, no refusal (D-011).
- No network in tests: fixture server (local HTTP) serves good tarball, bad signature,
  tampered file, missing manifest, unlisted file, traversal entry.
- Phase 1 CLI is dependency-light: `reqwest` (rustls), `sha2`, `tar`+`flate2`, `serde_json`,
  `clap`, `semver`, `tempfile`. Each new dep passes `skill: dependency_gate`.
- Binary targets: `x86_64/aarch64-apple-darwin`, `x86_64/aarch64-unknown-linux-gnu`, `x86_64-pc-windows-msvc` (launcher only).

## Rollback Plan
- **Install/update**: the previous release payload is kept as the snapshot
  `$HOME/.rein.prev-<ver>`; `rein-app rollback` syncs it back (its `manifest.json` included)
  and re-runs `rein install` and `rein doctor`. A sync
  that fails midway restores from the snapshot it has just taken and leaves `VERSION` at the
  old version; if that restore also fails the error says so and names the snapshot, and
  re-running `install`/`rollback` converges (the sync is idempotent). Operator data is never
  part of an install or a rollback.
- **Profiles**: `profiles apply --undo` restores the three allowed keys by value from the
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
  Member rules (E-12, rein D-155 r1), one refused fixture each: a directory entry; a symlink
  and a hard link member; a pax extension header; `manifest.json` not first; a `VERSION`
  member; a `files` path at the top level (`accounts.json`) and one under a non-mirrored
  directory; a legacy (non-prehashed) signature. The good fixture is built the way rein's
  writer builds it: `manifest.json` first, regular files only, no directory entries.
- Upgrade semantics: seed a temp `$HOME/.rein` with operator-owned files (incl.
  `config/rein-workspace.txt`, a fake `mcp/x/node_modules/`) and a stale `bin/old-tool`;
  after install the operator-owned files are byte-identical, `bin/old-tool` is gone, and
  `config/` otherwise matches the tarball.
- Shipped path wins (E-13, rein D-155 r2): the release ships
  `mcp/orchestrator-mcp/package-lock.json`; a different live file at that path is replaced,
  an unshipped `mcp/x/package-lock.json` and `shared/toolchain.sh` are kept, and after a
  failed sync or a rollback the earlier lock file is back byte for byte.
- `manifest.json` in the home (E-13): after install it is byte-identical to the in-tarball
  manifest and is written after `hooks`, before `VERSION` (asserted at the fault seam's stop
  points); a failed sync and a rollback put the earlier one back; rolling back to a snapshot
  without one removes it.
- Untouched outside the mirror (D-011): seed top-level entries the schema-1 list does not
  name (`approvals.sqlite` + `-wal`/`-shm`, `ci/x`, `trash/y`) and a mode-0000 file under
  `secrets/`; after install and after rollback each has the same inode and bytes (never
  copied), and the 0000 file proves nothing outside the mirror was opened.
- Sync failure: a fault seam fails the k-th write for every k; after each run the mirrored
  dirs are byte-equal to the pre-install tree, `VERSION` is the old one, and at every stop
  point each `hooks/*` file exists and equals its old or its new content.
- Snapshot: holds no unshipped operator-owned path and nothing from outside the mirrored dirs
  (besides its own `VERSION`, `layout.json`, `manifest.json`); N=2 kept;
  a tampered `layout.json` (`..`, a `/` in a mirrored name) is refused.
- Rollback: operator files written after the update survive; the restored code equals the
  earlier payload; a second rollback returns to the newer version; no snapshot → exit 12.
  With a stubbed `Process` (E-15): `rein install` then `rein doctor` run after the sync, in
  that order, with the lock held; a failing `rein install` exits 13 and a failing
  `rein doctor` exits 3, for `install`, `update` and `rollback` alike; `doctor --json` hands
  the flag to rein and returns its stdout and exit code unchanged.
- Refusals before any write: a symlinked mirrored dir, a symlinked intermediate component, a
  symlinked `$HOME/.rein`, a file/directory conflict, a held lock. A symlink at a target is
  replaced (its old target file is byte-unchanged); an unshipped link is pruned as a link.
- Owner guard (E-10, unix): the comparison is a pure function over uids, unit-tested for
  equal / home differs / `.rein` differs / snapshot differs; one CLI test per command
  (`install`, `update`, `rollback`) asserts that a mismatch exits non-zero with the path
  named, writes nothing and leaves no lock behind (the mismatch is injected through the
  function's uid inputs — tests never need root). E-11 adds: each CLI refusal asserts the
  offending path appears in the message; a `compile_fail` doctest proves a lock-held writer
  cannot be called without the guard's proof value; a test that needs a real foreign-owned
  directory (`--home /tmp`) skips when `/tmp/.rein.lock` already exists instead of failing,
  and no two tests share that path concurrently.
- No-snapshot notice: updating a populated home without a semver `VERSION` prints the one
  line and still completes.
- Layout equality (E-9, blocked on rein E-417/E-418/E-420): from a `rein` checkout at tag T, run `install-rein.sh` into temp HOME A;
  build the artifact with rein's own tools `rein release stage <dir>` then
  `rein release manifest <dir> --out <out> --pubkey <our test .pub>`
  (unsigned — signing is the separate `rein release sign` step), sign it with OUR throwaway
  test minisign key (never rein's `tests/fixtures/release/` pair, which no app bakes in), run
  `rein-app install --version T` from the fixture server into temp HOME B. The comparison is
  taken **after `rein install` has run on both sides** (D-155 addendum r1); `diff -r` of A and
  B is then empty except the documented differences: B has `manifest.json` (a clone install
  removes it), the install stamp says `artifact` in B and a clone path in A, and
  `config/rein-workspace.txt` exists only in A. A second case seeds both homes with a
  retired server directory (`mcp/gone-mcp/node_modules/x`): our writer keeps it, `rein
  install` removes it on both sides, and the diff stays empty — no expected difference is
  left for retired servers.
  **As run against rein 3f68eff (D-028):** the test drives `install_cmd::install` with the
  real process runner — the `rein-app` binary has, and gets, no seam for a test key or a
  test server; everything after key and source selection is the production path. Two more
  documented differences, both per-install values in owned files: `secrets/role-hmac.key`
  (random) and `shared/toolchain.sh` (the `REIN_TC_RECORDED=` timestamp). Assertions: the
  tree diff; `install` exits 0 or 3; and the output of `rein doctor` contains neither
  `[INSTALL_TREE_MISMATCH]` nor `[INSTALL_INCOMPLETE]` — exit 3 for host reasons (no
  `~/.claude`, no Chromium) is tolerated in a sterile home, an install-integrity failure
  is not. The test is gated on `REIN_CHECKOUT`, and is **red at 3f68eff** because
  `rein install` rewrites the shipped `mcp/orchestrator-mcp/package-lock.json` and rein's
  own tree check then reports a mismatch (raised with rein). It is not weakened to pass,
  E-9 stays open until a rein commit makes it green, and CI wiring follows that.
- Operator-owned conformance: every row of rein's `operator-owned-cases.json` (path, shipped,
  expected `kept` / `replaced` / `deleted`) holds for our plan function; the fixture is
  vendored and refreshed when rein bumps the manifest schema (interim copy until rein E-422).
- Shared contract fixtures (E-17, blocked on rein E-422): `profile-settings-cases.json`
  (settings before, the write attempted, accept / refuse) against `rein-profiles`. The
  table states what an app **may** write, not what it must be able to write (D-030), so
  the harness asserts two things: every `refuse` row is unreachable by our writer (the
  safety half — no exception), and every `accept` row is reachable by `plan` with a
  `Patch`, except rows named in an explicit list with a reason, each asserted to be
  unreachable by `plan` so the list cannot go stale. Today the list has one row, "remove
  an allowed key": `apply` never removes a key; only `apply --undo` does, when the backup
  lacked it, and a file-level undo test produces exactly that row's `after`. No "remove
  key" operation is added to `Patch`;
  `layout-v1.json` equals the layout our fixture manifests carry. rein's corrected
  `operator-owned-cases.json` (addendum 3 r10) changes the two top-level unshipped rows
  (`runner.json`, `insights.md`) from `deleted` to `kept`, moves their pattern facts to
  `glob_cases`, and adds `shared/Toolchain.sh` unshipped → `deleted` and `ci/x.json`
  unshipped → `kept`; the conformance test must then decide "kept" for any path outside
  the mirrored directories without consulting the matcher.
- Profile merge: property test — for random pre-existing JSON, after `apply` every original
  key/value is intact and only allowed keys changed; attempting a forbidden key fails to compile.
  E-14 (rein D-155 r3): a pre-existing `hooks` value — in the default profile and in every
  other — is value-identical, in the same key order, after `apply` and after `--undo`
  (D-015; not byte-identical — the file is re-serialised); nothing is copied from one
  profile to another; the `compile_fail` doctest covers `hooks`.
- Marketplace pin: a local `file://` git fixture with a `stable` branch;
  `claude plugin marketplace add file:///…/fixture.git#stable --scope user` must yield
  `source.ref == "stable"` in the written entry, else exit 7 (D-007). Stubbed `Shell` tests
  assert the exact argv per profile; one real-CLI test is gated on `claude` being present.
- Undo: after `apply` then operator edits to `permissions`, `--undo` leaves `permissions`
  untouched and reverts only the three allowed keys (D-008).
- WSL launcher: unit tests against a stubbed `Shell` covering no-WSL, WSL-no-distro, ready.
- **Evidence of record (D-015):** `cargo test --workspace` (doctests included) locally plus
  the hosted CI matrix on the commit. `rein ci run` has no cargo step and skips every step
  in this repository; its PASS proves nothing here and is never cited as test evidence.
- Operator docs (E-18): every command, flag and exit code in `docs/operator-guide.md`
  matches the §API table and `rein-app --help` / `<command> --help` output; no statement
  claims a published release, a download URL or a key.
- Key-file guard and tag gate (E-22): a test runs `.github/scripts/check-release-key.sh` over
  a fixture corpus — the committed key passes; a symlink, a NUL byte, one line, three lines,
  CRLF, a non-`Ed` algorithm, a comment id that differs from the key id, an empty and a
  missing file each fail — in the CI matrix on Linux, macOS and Windows. The comparison
  step is a second script taking the two file paths (equal → 0, different or absent → non-
  zero), tested the same way without network; only the workflow does the fetch. No workflow
  carries its own copy of either check.
- Exit codes (E-20): with a stubbed `Process`, `doctor` and `doctor --json` return 14 when
  rein is not started and when it ends without a code, and rein's 0 / 1 / other codes
  unchanged; an unknown subcommand and an unknown flag exit 64 with usage on stderr;
  `--help` and `--version` exit 0.
- License files (E-21): the host-target dist archive lists `LICENSE` and `NOTICE`; a locally
  built `.deb` (when `cargo-deb` is available; otherwise by its asset list) installs both
  under `/usr/share/doc/rein-app/`; the `.msi` source (`wix/main.wxs`) installs both beside
  the binary — the `.msi` itself stays unexercised until a tag and is listed as such in
  `.ai/DEVOPS.md`.
- Release rehearsal (E-19): `dist plan` succeeds for every configured target; a local
  `dist build` for the host target yields an archive whose binary runs `--version` and
  which carries `LICENSE` and `NOTICE`; nothing is tagged, pushed or published; the result
  and the list of what stays unexercised until the first tag are in `.ai/DEVOPS.md`.

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
  DONE as a whole-home staging swap; its mechanics are superseded by E-7 (D-011). The lock,
  Runner, RuntimeCheck and `install` command stand.
- **E-6** (T2) Homebrew formula (tap) + cargo-dist release workflow + `.deb` (AppImage
  deferred out of Phase 1 — D-012);
  Windows WSL launcher behind `Shell` trait with stubbed tests; `update`/`rollback`
  commands wired to E-5. Depends on E-4, E-5. Code half on master; pipeline in PR #1. Closes
  when PR #1 is merged and the first tag builds — and **no release tag is pushed before E-7
  is on master**.
- **E-7** (T3) Rework the LayoutWriter and `rollback` to the in-place sync of §Components 3
  (steps a–f) and 7: scope rule, snapshot of the release payload only, `hooks` last, restore
  on failure through a fault seam, rollback restores code only; remove the whole-home staging
  swap, the carry-over copies and `OrphanedPrevious`; §Test Plan cases "Untouched outside the
  mirror" through "Refusals". Depends on E-5. `security_engineer` before commit.
- **E-8** (T2) Make `install --version` optional: with no version, resolve the newest tag
  through the `update` resolver (§Components 7, §API). Depends on E-6 code (`latest.rs`).
- **E-10** (T3) Owner guard of §Components 3 step a for `install`/`update`/`rollback`
  (refuse when the run's uid — read from its own lock file — differs from the owner of
  `$HOME`, `$HOME/.rein`, or the snapshot being restored), and the one-line "no snapshot
  taken" notice of step c. §Security trust boundary 2, §Test Plan "Owner guard", "No-snapshot
  notice" (D-013). Depends on E-7. `security_engineer` before commit.
- E-7, E-8, E-10 and E-11 are DONE (2026-11-09). rein D-155 (2026-11-09, mirrored as D-014)
  then corrected the contract in three places; E-12..E-15 bring the code in line, E-16/E-17
  and E-9 wait on rein or the operator.
- **E-12** (T3) Verifier, strict member rules of §Components 2 (rein D-155 r1): `manifest.json`
  first; regular files only; directory entries, links, pax/GNU extension headers and a
  `VERSION` member refused; every `files` path under a `layout.mirrored` dir; prehashed
  signatures only. §Test Plan "Member rules". `security_engineer` before commit.
- **E-13** (T3) LayoutWriter per rein D-155 r1/r2: a shipped path is always written
  (`operator_owned` protects from deletion only); snapshot also holds the live file at every
  shipped path and the home's `manifest.json`; `manifest.json` written after `hooks`, before
  `VERSION`; restore and rollback put it back or remove it; interim conformance fixture in
  the three-column shape. §Components 3 scope rule, steps a–f; §Test Plan "Shipped path
  wins", "`manifest.json` in the home", "Snapshot". Depends on E-12. `security_engineer`.
- **E-14** (T3) `rein-profiles` per rein D-155 r3: remove `Hooks` from `AllowedKey` and the
  hooks copy in `profiles apply`; three keys in apply and undo. §Components 6, §Security
  trust boundary 3, §Test Plan "Profile merge". `security_engineer` before commit.
- **E-15** (T2) CLI: `rollback` runs `rein install` then `rein doctor`; a failed
  `rein install` is exit 13 on `install`/`update`/`rollback`; `doctor --json` passthrough.
  §Components 4, 7; §API. Depends on E-13.
- E-16, E-18..E-21 are DONE (2026-11-10), reviewed in D-017.
- E-22 (guard script, tag gate, `CODEOWNERS`) is DONE and E-23 needed no change (already
  ASCII since 6bf6244); both reviewed in D-018.
- E-24 and E-25 are DONE, reviewed in D-019. E-26 is DONE, reviewed in D-021.
- **E-32** (T3, BLOCKED on rein E-427) Install verdict from `rein doctor --scope install
  --json` (§Components 4 "Install verdict"; D-029).
- **E-31** (T2) Manifest size limits and linear lookups (§Execution Constraints
  "Manifest size limits"; D-027).
- **E-30** (T3) `config_dir` terminal safety, shell-quoted dry-run line, duplicate-row
  warning, unlisted row never suppresses the built-in, note for an unusual `config_dir`
  (§Components 6; D-024).
- **E-29** (T3) Refuse a shipped path that is a case variant of an owned pattern (the
  protective fold of the task text is withdrawn, D-025); symlink case-variant is a preflight Conflict;
  all manifest-derived error text escaped (§Components 2, §Components 3 step b; D-023).
- **E-28** (T3) Profile enumeration: accounts rows before the built-in `personal`, only
  `default` reserved, terminal-safety name check, no false warning (§Components 6; D-022).
- **E-27** (T3) Sync removes case-variant prune entries before the writes and checks every
  shipped path after the sync; `operator_owned` patterns join the ASCII rule (§Components 2,
  §Components 3 step d; D-021).
- **E-26** (T3) Verifier refuses non-printable-ASCII paths and ASCII-case-colliding paths at
  manifest parse (§Components 2 "path alphabet and case rule"; D-020).
- **E-24** (T1, DONE) Extend `.github/CODEOWNERS` to `/crates/rein-release/`, `/Cargo.toml`,
  `/Cargo.lock` (§Security trust boundary 1; D-018).
- **E-25** (T2) Rehearse the release-candidate tag: run `dist plan` for `v0.1.0-rc.1` and
  `v0.1.0`, record in `.ai/DEVOPS.md` what an rc tag needs (§Test Plan "Release
  rehearsal"; D-016 ruling on the rc tag). Changes no version.
- **E-16** (T3, DONE) Bake the trust
  root from a committed copy of that file instead of the `REIN_RELEASE_PUBKEY` variable;
  release guard checks the file (`ci_gate` first). §Security trust boundary 1 (D-014).
- **E-17** (T2, BLOCKED on rein E-422) Vendor rein's shared fixtures and run them: replace the
  interim `operator-owned-cases.json`, add `profile-settings-cases.json` and `layout-v1.json`.
  §Test Plan "Operator-owned conformance", "Shared contract fixtures".
- E-12..E-15 are DONE (2026-11-10) and reviewed (D-015): no code follow-up.
- **E-18** (T2) Write `README.md` and `docs/operator-guide.md` per §Components 9; §Test Plan
  "Operator docs". No source change.
- **E-19** (T2) Rehearse the release pipeline without publishing: `dist plan` for all
  targets, a local host-target `dist build`, archive contents checked; findings in
  `.ai/DEVOPS.md`. No tag, no push, no workflow edit — a workflow defect found is reported
  to the Architect (a fix is its own task behind `skill: ci_gate`). §Test Plan "Release
  rehearsal".
- **E-20** (T2) CLI exit codes that collide (D-016): `doctor` exits 14, not 1, when
  `rein doctor` cannot be started or ends without an exit code; a usage error exits 64, not
  clap's 2 (`--help` / `--version` stay 0). §API. Before E-18 is finished.
- **E-21** (T2) Ship `LICENSE` and `NOTICE` in every release artifact (E-19 finding 1,
  Apache-2.0 §4(d)): dist archives, `.msi`, `.deb`. `skill: ci_gate` first if a workflow file
  changes. §Test Plan "License files".
- **E-11** (T3) Make the owner guard unskippable: `owner_guard` returns a proof value with no
  public constructor, borrowed from the `InstallLock`; `install_tree_locked`,
  `install_tree_with_faults` and rollback's lock-held writer take it; `compile_fail` doctest;
  CLI refusal tests assert the named path; the `/tmp` test skips on an existing lock
  (§Components 3 step a, §Test Plan "Owner guard"; D-013 addendum). Depends on E-10.
- **E-9** (T2, BLOCKED on rein E-417/E-418/E-420: `rein release stage|manifest` and artifact
  mode in `rein install`) Write the layout-equality test of
  §Test Plan, compared after `rein install` on both sides; any diff beyond the documented
  three is reported to the Architect, not adjusted for. (The registered task text still
  names a retired MCP server's directory as an expected diff — rein's D-155 addendum r1
  removed that: `rein install` deletes it on both sides. This blueprint governs.) Depends on E-13.
