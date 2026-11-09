# COMM — Architect ↔ Engineer handoff log (append-only)

---
## Handoff — 2026-11-08 18:57 UTC
**From**: Architect → Engineer
**Trigger**: P-1 complete — rein-app blueprint v1 written, Phase 1 queue E-2..E-6 registered

### What was planned
- `.ai/architect.md` v1 — index + system contract adopted from `rein` D-150
  (`/home/emir/Dev/rein/plans/rein-ecosystem-contract.md`, `rein-app-plan.md`). Read §3–§5, §7.
- `.ai/blueprints/distribution.md` — Phase 1 domain blueprint: Fetcher, Verifier
  (`SignatureVerifier` seam, `Rejecting` default), LayoutWriter, Runner, RuntimeCheck,
  `rein-profiles`, Updater, WslLauncher; data model, CLI contract, security, test plan.
- Tasks (state.sqlite): E-1 placeholder closed; **E-2** (T2, OPEN) → **E-3** verifier (T3) ∥
  **E-4** profiles (T3) → **E-5** layout writer (T3) → **E-6** packaging/updater/WSL (T2).
  Only E-2 is unblocked now; the rest auto-open as dependencies close.
- `.ai/DIGEST.md`, `.ai/BRIEF.md` filled; `.ai/LOG.md` stamped.

### Decisions made
- **D-001** — contract adopted; rein-app is the shell, not a rewrite; task ID shift E-1..E-5 → E-2..E-6; fail-closed `Rejecting` signature backend until `rein` P-50.
- **D-002** — marketplace registration JSON for E-4: `extraKnownMarketplaces.rein-plugins = { source: { source: "github", repo: "<owner>/rein-plugins", ref: "stable" } }`; `enabledPlugins` keys `<plugin>@rein-plugins: true`, written only on `--enable`. Owner pending confirmation from rein-plugins (assumed `vimurai`) — make it a single constant.

### Blueprint divergence
- NONE — nothing implemented yet. This is the first blueprint.

### Next action needed (Engineer)
1. Preflight, then read `.ai/architect.md` §5/§7 and `.ai/blueprints/distribution.md` (one blueprint only — §33 JIT).
2. Execute **E-2**: `git init` (this directory is NOT a git repo yet — initialise it first so
   COMM.md commits and reviews work), Cargo workspace with crates `rein-app` (bin),
   `rein-release` (lib), `rein-profiles` (lib), cargo-dist config for mac/linux/windows,
   GitHub Actions matrix. Each new crate dependency goes through `skill: dependency_gate`.
   CI config → `skill: ci_gate`.
3. E-3/E-4/E-5 are Tier 3 — dispatch `security_engineer` before commit. E-3's signature
   backend MUST stay `Rejecting`; do not pick minisign or sigstore locally (owed by `rein` P-50).
4. After each task: `skill: rein-task`.

### Open risks
- `rein` P-50 outstanding: signature scheme + trust-root distribution, manifest.json schema,
  `rein doctor --json`. Sent to `rein · Architect` 2026-11-08; until answered E-3 parses the
  assumed manifest shape in `distribution.md` §Data Model and treats it as provisional.
- accounts.json schema undocumented — treat only `accounts.<name>.config_dir` as meaningful.
- Operator-owned-file list for the layout writer (accounts.json, gate-override.json, config/*,
  secrets/*, run/*) is our assumption pending `rein` confirmation; the layout-equality test
  against `install-rein.sh` is the ground truth.
- No git repository yet → this COMM entry could not be committed (D-104); commit it as the
  first bookkeeping commit after `git init` in E-2.
---

---
## Handoff — 2026-11-08 19:10 UTC
**From**: Architect → Engineer
**Trigger**: rein D-153 received — all five P-50 contract items ruled; blueprint revised (D-003)

### What changed
- `.ai/blueprints/distribution.md` Components 2–3, §Data Model, §Security, §Test Plan, E-3/E-5
  breakdown — now binding on: minisign + public key baked in at build (`key_id` check first,
  "newer rein-app required" line on mismatch); manifest.json schema 1 incl. `mode` and
  `layout.{mirrored, operator_owned}`; upgrade semantics (`config/` MIRRORED except
  `rein-workspace.txt` + `install-stamp.json`; operator-owned list read from the manifest;
  `bin/` pruned; `VERSION` last). `rein doctor --json` shape recorded for phase 2.
- `.ai/architect.md` §3.1, §3.2, §3.4, §8 updated. `.ai/DECISIONS.md` D-003 appended.

### Decisions made
- **D-003** mirrors rein D-153. Where the E-3/E-5 task text in state.sqlite differs from the
  blueprint (it predates the ruling), the blueprint + D-003 govern.

### Blueprint divergence
- NONE in code (nothing implemented). One Architect assumption corrected: `config/` is mirrored, not preserved.

### Next action needed (Engineer)
- Continue E-2 unchanged. When E-3 opens: production backend is minisign (vet the crate via
  `skill: dependency_gate`), tests use a generated TEST key pair, `Rejecting` stays as a
  build-flag kill switch. When E-5 opens: read `layout.operator_owned` from the manifest; add
  the upgrade-semantics test from §Test Plan.

### Open risks
- rein has not yet shipped the release workflow, manifest generator or `doctor --json`
  (their P-50 E-## follow). E-3/E-5 are testable against fixtures regardless.
- Still no git repository here; commit this COMM.md with the previous entry after `git init`.
---

---
## Handoff addendum — 2026-11-08 19:20 UTC (Architect → Engineer)
rein D-153 addendum received (commit c586480 in rein). Blueprint `distribution.md` Component 3
and §Test Plan updated; D-003 addendum in DECISIONS.md. For E-5: implement
`OperatorOwnedMatcher` per the gitignore-subset syntax and vendor rein's
`tests/fixtures/release/operator-owned-cases.json`; build the equality-test artifact with
`rein release manifest <tree> --out <dir>` from a rein checkout and sign it with our own
throwaway minisign key. No change to E-2.
---

---
## Handoff — 2026-11-08 19:35 UTC
**From**: Architect → Engineer
**Trigger**: E-2 DONE acknowledged; license + repository decided (D-004)

### Review of E-2
- Workspace matches `distribution.md`: three crates, zero deps, `unsafe_code = "forbid"`,
  cargo-dist targets only (ci/installers deferred to E-6 as planned). ACCEPTED, no divergence.

### Decisions made
- **D-004**: license **Apache-2.0**, repository **`https://github.com/Vimurai/rein-app`**
  (matches `Vimurai/rein` and `Vimurai/rein-plugins`). Architect intent; operator creates the
  remote and may overrule — both are one-line strings.

### Next action needed (Engineer)
- Small follow-up inside E-3 or as its first commit: add `license = "Apache-2.0"`,
  `repository = "https://github.com/Vimurai/rein-app"` to `[workspace.package]`, add the
  Apache-2.0 `LICENSE` file at the root (full text, copyright "Vimurai"). Tier 1, no new task.
- Then E-3 and E-4 as open (both Tier 3 → `security_engineer` before commit). For E-4 use a
  fixture for `Vimurai/rein-plugins`; the remote is not live.

### Open risks
- Hosted CI matrix unproven until the operator creates the remote and pushes.
- `rein ci run` has no Rust step; it does not validate the Rust build (noted for the
  Tester role — a `cargo test` step in `rein ci` is a rein-side ask, not ours).
---

---
## Handoff — 2026-11-08 20:05 UTC
**From**: Architect → Engineer
**Trigger**: rein-plugins Q4 — `enabledPlugins` does not install at user scope; D-005 revises E-4

### What changed (binding for E-4)
- `profiles apply` still writes `extraKnownMarketplaces` per D-002 (hand-written, valid).
- `--enable` now shells out per profile: `CLAUDE_CONFIG_DIR=<config_dir> claude plugin install <plugin>@rein-plugins --scope user`, then re-reads settings.json and verifies `enabledPlugins."<plugin>@rein-plugins" == true`. The app NEVER writes a `true` entry itself. `--disable` → `claude plugin disable … --scope user`. `--undo` restores `.rein-bak`.
- `claude` calls go through the `Shell` trait (fixed argv; plugin names from a closed list, regex-validated). Missing `claude` → marketplace still registered, exit 4.
- `distribution.md` Component 6 + API table updated; DECISIONS.md D-005 has the docs citations.

### Blueprint divergence
- NONE in code. Architect assumption in D-002 ("true = install + enable" at user scope) was wrong; corrected.

### Next action needed (Engineer)
- Apply D-005 in E-4 (in progress). Test: stubbed `Shell` records the exact argv per profile; a fixture settings.json gains the key only via the stub's simulated write; `--enable` with `claude` absent registers the marketplace and exits 4.
- Record in your E-4 summary which Claude Code version you verified against and whether a hand-written user-scope `extraKnownMarketplaces` entry shows up in `claude plugin marketplace list` (D-005 alt 4 fallback decision).
---

---
## Handoff — 2026-11-08 20:40 UTC
**From**: Architect → Engineer
**Trigger**: E-4 probe (hand-written marketplace entry not registered on 2.1.319) + security P0 on `--undo`

### Rulings (binding for E-4)
- **D-007**: registration = `claude plugin marketplace add Vimurai/rein-plugins#stable --scope user`
  per profile via `Shell`, then read-back verification of the D-002 JSON incl. `ref == "stable"`;
  mismatch → exit 7, never patched by hand. The hand-written path is retired. `claude` absent →
  `attribution` + `hooks` applied, marketplace/plugins skipped, exit 4.
- **D-008**: approve the P0 fix as you described — once-written pre-Rein backup, `--undo`
  restores the four allowed keys by value only, `--hooks-from` removed, escaping symlinks refused.
  Blueprint §Rollback Plan + §Security trust boundary 3 updated; the "byte-identical" wording is gone.
- Decision numbering: your D-006 stands; mine continue at D-007/D-008. Future Engineer-originated
  decisions: check `grep -c '^## D-' .ai/DECISIONS.md` immediately before writing.

### Tests to add (distribution.md §Test Plan)
- `file://` git fixture with a `stable` branch proving `#stable` lands as `source.ref`.
- Undo leaves operator-edited `permissions` untouched.

### Open
- Whether `owner/repo#stable` (GitHub shorthand) pins `ref` identically to the git-URL form is
  provable only once `Vimurai/rein-plugins` is live; rein-plugins' CI will report. Exit 7 guards it.
---

---
## Handoff — 2026-11-08 21:05 UTC
**From**: Architect → Engineer
**Trigger**: rein-plugins fixture — session start registers hand-written marketplace entries (and may install); D-009 re-rules

### What changes for E-4 (small)
- D-007 path stays primary (marketplace add #stable → install → read-back → exit 7 on ref mismatch).
- NEW fallback: `claude` absent → write the D-002 marketplace entry by hand WITH `ref: "stable"`
  (reuse the AllowedKey writer), print "marketplace registration completes at the next Claude
  Code start", exit 4. Still never hand-write `enabledPlugins: true`.
- NEW test: idempotence — running the D-007 path on a profile whose entry a session start already
  registered exits 0, leaves the entry unchanged, creates no duplicate.
- D-008 unchanged.

### Blueprint divergence
- NONE in code; Architect model corrected (D-005 (c) retracted). distribution.md Component 6 updated.
---

---
## Handoff — 2026-11-09 14:45 UTC
**From**: Engineer → Architect
**Trigger**: E-4, E-5 DONE; E-6 code half on master, release pipeline in PR #1 (awaiting operator review)

### What was built
- E-4 closed (shipped 58b2413; ARCH/TESTS/SEC pass).
- E-5 (73fa5bc, 74f4ca8): `crates/rein-release/src/layout.rs` LayoutWriter (staging 0700 → rename
  swap, `VERSION` last, `.rein.prev-<ver>` N=2 with `~N` on a label clash, `$HOME/.rein.lock`
  removed only by its owner and held through `rein install`/`rein doctor`, live directory modes
  kept); `crates/rein-app/src/runner.rs` Runner + RuntimeCheck; `rein-app install --version`.
  ARCH_PASS, TESTS_PASS, SEC_PASS (2nd pass).
- E-6 code (082c1da): `latest.rs` (GitHub API newest tag), `rollback.rs`, `rein-app update|rollback|doctor`,
  `wsl.rs` WslLauncher + `rein-app launch`. ALIGN_PASS.
- E-6 pipeline: PR #1 (`engineer/e6-release-pipeline`, 93c4674) — cargo-dist release.yml, msi,
  Homebrew → new repo `Vimurai/homebrew-tap`, .deb job, key guard. PR checks green; the merge was
  refused by the permission classifier (needs operator review). DEVOPS E-6, D-010.

### Decisions made
- D-010 (Engineer, dependency_gate): cargo-dist / cargo-deb / appimagetool as build tools; operator
  chose "build the pipeline now, guarded" and "create the tap".

### Blueprint divergence
1. **Carry-over beyond operator_owned (needs ruling + rein contract item).** Live homes hold
   `approvals.sqlite`, `cache.sqlite`, `*.sqlite-wal/-shm`, `ci/`, `privacy/`, `trash/`, none on the
   schema-1 `operator_owned` list. The writer carries every non-mirrored top-level entry (as
   install-rein.sh leaves them); the literal blueprint would drop committed WAL data.
2. **SEC P1-3 (open, Architect):** copying while rein runs can tear SQLite+WAL pairs and drop writes
   made mid-install (approval ledger); `trash/`, `ci/` duplicated ×3 with N=2. Options: refuse while
   rein processes are live, or swap the mirrored dirs inside the live tree instead of copying
   operator data out. `rollback` likewise restores the older tree's operator files (newer ones stay
   in the swapped-out tree; the CLI says so).
3. Layout-equality test vs install-rein.sh not written — rein has no `rein release manifest` (P-50).
4. `install` requires `--version`; `update` resolves the newest tag via api.github.com (added to the
   host policy, metadata only; the release is still verified and version-bound).
5. RuntimeCheck: node ≥ min_node and git required; sqlite3/tmux/claude reported only.
6. Previous trees ordered by `VERSION` mtime; no live tree + a previous tree → refuse (`OrphanedPrevious`);
   a release file at an operator-owned path is never written, even on a fresh install.
7. `launch` runs `rein-app update --yes` inside WSL (not `install`), requires a Linux rein-app
   already in the distro, refuses project paths containing `;` (wt.exe separator).
8. AppImage deferred (no pinnable appimagetool release); cargo-dist 0.34.0 renders local reusable
   jobs as `uses: $/…` — one hand fix under `allow-dirty = ["ci"]`.
9. Deferred SEC P2s: stale lock after SIGINT (no liveness check without unsafe), runner env denylist
   beyond `CLAUDECODE`, O_NOFOLLOW copies, fresh-install dirs 0755 regardless of umask; no test for
   a swap failing midway (no seam).

### Next action needed
- Rule on divergences 1–2 (and ask rein to extend `operator_owned`); decide AppImage.
- E-6 stays OPEN until PR #1 is merged by the operator and the first tag can build:
  operator adds `HOMEBREW_TAP_TOKEN` (secret), and `REIN_RELEASE_PUBKEY` (variable) once rein P-50 lands.

### Open risks
- No rein release can be installed until rein publishes a signed release and its public key.
- Release jobs (key guard, .deb) are unexercised until the first tag.
---

---
## Handoff — 2026-11-09 14:55 UTC
**From**: Architect → Engineer
**Trigger**: review of E-4/E-5/E-6 (handoff 14:45 UTC); rulings D-011, D-012; E-7..E-9 registered

### What was built
- Reviewed E-5 `layout.rs` / `rollback.rs`, the E-6 code, PR #1's workflow shape, and rein's `install-rein.sh`.
- `.ai/blueprints/distribution.md` revised: §Core Concept, §Components 3 (rewritten, steps a–f), 5, 7, 8, §Data Model (`Snapshot`, `SyncPlan`), §API (`rollback` row), §Security trust boundary 2, §Execution Constraints, §Rollback Plan, §Test Plan (five new cases), §Task Breakdown (E-6 note, E-7..E-9).
- `.ai/architect.md` §0, §5(b), §7, §8, §9; `.ai/DIGEST.md` refreshed.

### Decisions made
- **D-011** — ruling on divergences 1–2 and SEC P1-3: neither option offered. The whole-home staging swap goes; the writer syncs the mirrored dirs **in place, per file**, exactly as `install-rein.sh` does. Measured on a live home: 3.5 GB total (`ci/` 3.0 GB, `mcp/*/node_modules` 551 MB) against an 8.9 MB release payload — E-5 copies all of it on every install and keeps two more copies. rein's installer also rejects a directory swap in its own comments (E-304): a renamed-away `hooks/` leaves every gate hook missing. Consequences: anything outside `layout.mirrored` is never opened (so no carry-over list question, no torn WAL, no lost approval writes, no duplication); the snapshot is the release payload only; `rollback` restores code only; no "refuse while rein runs" check.
- **D-012** — AppImage deferred out of Phase 1 (no pinnable appimagetool). Divergences: (3) → E-9; (4) `install` must default to latest → E-8, the `api.github.com` resolver is accepted; (5), (6), (7), (8) accepted as built; (9) stale lock → Phase 2, env denylist and 0755 accepted, `O_NOFOLLOW` moot after E-7, "no swap-failure test" closed by E-7's fault seam.
- The blueprint's swap was my design error, not yours — E-5 implemented what was written and flagged the right problems.

### Blueprint divergence (if any)
- E-5 as shipped now diverges from the revised blueprint by design; E-7 closes it. Lock, Runner, RuntimeCheck and the `install` command stand unchanged.

### Next action needed
1. **E-7 (T3)** first — read `distribution.md` §Components 3 and 7 before touching `layout.rs`. `security_engineer` before commit. The snapshot layout (`.rein.prev-<ver>/{tree/,VERSION,layout.json}`) and the step order are binding; how you build the fault seam is yours.
2. **E-8 (T2)** — `install` without `--version`.
3. **E-9** stays BLOCKED after E-7 until I hand off that rein's `rein release manifest` exists; if it auto-opens, skip it.
4. **E-6 / PR #1**: still the operator's merge. Before it merges, merge master into `engineer/e6-release-pipeline`: `.ai/DECISIONS.md` will conflict (your D-010 vs D-011/D-012 appended on master) — keep both, D-010 first, and drop the bridging note above D-011. Remove AppImage from the pipeline plan in DEVOPS.md if it is still listed as planned. **No release tag before E-7 is on master.**

### Open risks
- `rein · Architect` is not live, so the notice below is undelivered; D-011 does not wait for the answers (we behave per (a)).
- A retired MCP server's directory keeps its protected `node_modules` after an app install (`install-rein.sh` removes it itself) — E-9 will show this as the one expected diff until rein rules.
- Per-file sync gives up whole-tree atomicity; mitigations are in D-011 (verify first, `hooks` last, `VERSION` last, restore from snapshot).

### Notice for `rein · Architect` (UNDELIVERED — relay when that session is up)
rein-app D-011 changes how the app lays a release out: in-place per-file sync of `layout.mirrored`, nothing else in the home is opened. Two contract questions; rein-app proceeds per (a) until answered:
- (a) Please state in the manifest contract that every path outside `layout.mirrored` is untouched by an install and that `layout.operator_owned` decides only inside mirrored dirs. Live homes hold rein data the schema-1 list does not name: `approvals.sqlite*`, `cache.sqlite*`, `telemetry.sqlite-wal/-shm`, `ci/`, `privacy/`, `trash/`, `clean.log`. If you would rather keep the list exhaustive, extend it and the conformance fixture.
- (b) `install-rein.sh` removes a retired MCP server's mirror directory (including its `node_modules`) and the legacy `gemini/` and `agents/` mirrors. Under an app install, who does that — a manifest field, or `rein install`? The app never deletes an operator-owned path on its own.
- FYI: AppImage is out of rein-app Phase 1 (D-012); `plans/rein-app-plan.md` line 14 lists it.
- Still needed from P-50: `rein release manifest`, a signed release, the public key + `key_id`, `doctor --json`.
---

---
## Handoff — 2026-11-09 15:30 UTC
**From**: Engineer → Architect
**Trigger**: E-7, E-8 DONE; E-9 left OPEN (blocked on rein P-50); PR #1 merged with master

### What was built
- E-7 (1ac3f78, d1c62af): `crates/rein-release/src/{sync,snapshot}.rs` new; `layout.rs`, `rollback.rs`
  rewritten per D-011 (steps a–f, payload-only snapshots `.rein.prev-<ver>[~N]/{tree/,VERSION,layout.json}`,
  hooks last, VERSION last, restore through a `Faults` seam, code-only rollback). 20 layout tests incl.
  every-k fault restore, hooks old-or-new at every stop point, inode+bytes of untouched files.
  ARCH_PASS, TESTS_PASS, SEC_PASS (2nd pass). Hosted CI green ×3 (Windows needed a test-only path fix).
- E-8 (ded24eb): `install` without `--version` → newest tag via the `update` resolver; offline exit 11.
- PR #1: master merged into `engineer/e6-release-pipeline` (34e8465) — DECISIONS D-010 then D-011/D-012,
  bridging note dropped; DEVOPS says AppImage is out of Phase 1. Still awaits the operator's merge.

### Decisions made
- None new.

### Blueprint divergence
1. **Step (a) symlink rule (needs your ruling on the wording):** preflight refuses a symlink at a mirrored
   root or an intermediate component, but a symlink AT the target is replaced by the rename (never written
   through). SEC P1-2: with the literal rule, one unshipped `bin/env-link` blocks both restore-on-failure and
   rollback (the snapshot carries the link back).
2. A symlink or file where an owned **directory** belongs (`mcp/x/node_modules -> store`) counts as owned —
   never opened, snapshotted or pruned (SEC P1-1).
3. Snapshot only when the live `VERSION` is semver; `Conflict` refusals (file/dir mismatch, `*.rein-tmp`
   ship names); `previous_trees()` lists only dirs with real `VERSION` + `layout.json` (an operator's
   `.rein.prev-manual` is never pruned; E-5-format trees are ignored).
4. Rollback snapshots the current payload with the older snapshot's layout: a mirrored dir only the newer
   release lists is neither snapshotted nor pruned by rollback (stays in place). Low impact.
5. **Open SEC P2-2:** an edited snapshot `layout.json` (e.g. `mirrored += secrets`) widens rollback's scope —
   same-UID only; there is no trusted copy of the live layout to check against. Options: keep the last signed
   manifest's layout beside the home and require snapshot.mirrored ⊆ it, or accept and record.
6. Public `#[doc(hidden)] Faults` (`fail_at`, `stop_at`); new exports `plan`, `SyncPlan`, `KEEP_SNAPSHOTS`.

### Next action needed
- Rule on 1 and 5; record 2–4/6 if accepted.
- E-9 when rein ships `rein release manifest`. E-6 closes when the operator merges PR #1 and adds
  `HOMEBREW_TAP_TOKEN` (+ `REIN_RELEASE_PUBKEY` after P-50). E-7 is on master, so a tag is no longer blocked by it.

### Open risks
- Operator cleanup: a review probe dir `/tmp/tmp.dEv4ugM5jL` (~590 MB) could not be removed (safe-exec gate).
---

---
## Handoff — 2026-11-09 15:35 UTC
**From**: Architect → Engineer
**Trigger**: review of E-7/E-8 (handoff 15:30 UTC); ruling D-013; E-10 registered

### What was built
- Read `sync.rs`, `snapshot.rs`, `layout.rs`, `rollback.rs` against D-011: scope rule, step order, payload-only snapshots, restore through the seam and code-only rollback are as specified. No rework asked.
- `.ai/blueprints/distribution.md`: step 3a reworded (owner guard, target-symlink rule, `Conflict`, owned-path-of-any-type), step 3c (no snapshot without a semver `VERSION`, what counts as a snapshot), §Components 7 (rollback uses the snapshot's layout), §Security trust boundary 2 (accepted residual risk), §Test Plan (two new cases), §Task Breakdown E-10. `.ai/architect.md` §0, §7; DIGEST.

### Decisions made
- **D-013**, ruling on divergence 1: accepted as you built it — a symlink AT a target is replaced by the rename, pruned and snapshotted as a link; root and intermediate symlinks stay refused. The blueprint wording was wrong, not the code.
- **D-013**, ruling on divergence 5 (SEC P2-2): **accept and record; do not add a second copy of the layout.** A copy beside the home is exactly as editable as `layout.json`, so it adds no trust. The risk is same-account only — but that premise was never checked: under `sudo` with the user's HOME the writer would run as root steered by a user-editable file. E-10 turns the premise into a refusal (owner guard).
- Divergences 2, 3, 4, 6 accepted (3 with one addition: a one-line notice when a populated home is updated without a snapshot — part of E-10).

### Blueprint divergence (if any)
- NONE open — the blueprint now matches the code except for E-10's two additions.

### Next action needed
1. **E-10 (T3)** — owner guard + no-snapshot notice. Read `distribution.md` §Components 3 step a first. The uid comes from the lock file's own metadata; keep the comparison a pure function so tests inject a mismatch without root. No override flag. `security_engineer` before commit.
2. **E-9**: shows OPEN since E-7 closed, but is still externally blocked — skip it until a handoff from me says rein's `rein release manifest` exists.
3. **E-6**: nothing for you until the operator merges PR #1 and sets `HOMEBREW_TAP_TOKEN` / `REIN_RELEASE_PUBKEY`.
- After E-10 the Engineer queue is empty until the operator or rein moves; hand back once and stop.

### Open risks
- Phase 1 cannot ship until rein publishes a signed release and its public key (P-50) and PR #1 is merged; neither is ours to do.
- The notice for `rein · Architect` in the 14:55 UTC entry is still undelivered (that session is not running).
- Operator cleanup still pending: `/tmp/tmp.dEv4ugM5jL` (~590 MB).
---

---
## Handoff — 2026-11-09 15:46 UTC
**From**: Engineer → Architect
**Trigger**: E-10 DONE; Engineer queue empty (E-6 waits on the operator, E-9 on rein P-50)

### What was built
- E-10 (5b8bc6b): owner guard for `install`/`update`/`rollback` per D-013 — run uid from the lock file's
  open handle (fstat at `acquire`; a failed read refuses), `$HOME` judged through its symlink, `.rein` and
  snapshot paths not followed; pure `check_owners`; `RunUid` fn pointer for tests; no override flag; no-op on
  Windows. No-snapshot notice via `InstallOutcome.unsnapshotted` / `outcome_lines`. Linux CLI test runs the
  real binary with `--home /tmp` (root-owned) for a real mismatch without root. ARCH_PASS, TESTS_PASS,
  SEC_PASS (2nd pass: P1 path-based uid → handle; P1 symlinked `$HOME` judged by the link). CI green ×3.

### Decisions made
- None.

### Blueprint divergence
- NONE against D-013. Open SEC P2s for your call (none blocking):
  1. Public `install_tree_locked` / `install_tree_with_faults` rely on the caller having run the guard
     (`install_cmd` does; `install_tree` and `rollback_tree` run it themselves). A compile-time token
     returned by `owner_guard` would make it unskippable.
  2. Shared root-owned `--home` only: rollback's second check covers the snapshot root + `layout.json`, not
     its `VERSION`/`tree/**`; install prunes `.rein.prev-*` and removes `.rein.fetch-*` without owner checks
     (`remove_dir_all` does not follow links). Candidate THREAT_MODEL lines.

### Next action needed
- Nothing for the Engineer until the operator merges PR #1 / sets `HOMEBREW_TAP_TOKEN` (+ `REIN_RELEASE_PUBKEY`
  after P-50), or you hand off that rein ships `rein release manifest` (E-9).

### Open risks
- Operator cleanup `/tmp/tmp.dEv4ugM5jL` (~590 MB) may still be pending.
---
