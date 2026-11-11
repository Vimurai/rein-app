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

---
## Handoff — 2026-11-09 15:50 UTC
**From**: Architect → Engineer
**Trigger**: review of E-10 (handoff 15:46 UTC); D-013 addendum; E-11 registered

### What was built
- Read the guard in `layout.rs` (`owner_guard`, `check_owners`, `InstallLock::run_uid`) and its call sites: matches D-013; the handle-based uid and the `$HOME`-through-its-symlink rule are improvements on the blueprint, now written into step 3a.
- `.ai/blueprints/distribution.md`: step 3a (type-level precondition), §Security trust boundary 2 (shared `--home` unsupported), §Test Plan "Owner guard" (E-11 additions), §Task Breakdown E-11. `.ai/architect.md` §0, §7; DIGEST.

### Decisions made
- **D-013 addendum**, P2 "guard-skippable public fns": **fix now → E-11.** Your token idea is the ruling: `owner_guard` returns a proof value only it can construct, borrowed from the lock, and the lock-held writers take it. Reason for not deferring: Phase 2 consumes exactly these library seams, and a doc comment is what a new caller does not read.
- **D-013 addendum**, P2 "shared root-owned `--home`": **accepted, no code.** A home inside a directory other users can write is unsupported (as for rein itself); recorded in §Security. `security_engineer` may mirror it into THREAT_MODEL.md.
- Test hygiene folded into E-11: CLI refusal tests assert the named path (TESTS P1); the real-`/tmp` test skips when `/tmp/.rein.lock` exists — a lock left by a killed run in a sticky directory would fail it for every other user of the machine.

### Blueprint divergence (if any)
- NONE.

### Next action needed
1. **E-11 (T3)** — the only actionable task. The name and shape of the proof type are yours; what is binding is: no public constructor, borrowed from the `InstallLock`, required by every lock-held writer, a `compile_fail` doctest. `security_engineer` before commit. (Registered without `depends_on`: the DAG depth limit refused E-10 → E-11; E-10 is DONE anyway.)
2. **E-9** stays externally blocked on rein P-50; **E-6** waits on the operator.
- After E-11, hand back once; I will not add more Phase 1 work unless rein or the operator moves.

### Open risks
- Phase 1 cannot ship until PR #1 is merged and rein publishes a signed release + public key (P-50).
- The notice for `rein · Architect` (14:55 UTC entry) is still undelivered.
---

---
## Handoff — 2026-11-09 16:08 UTC
**From**: Engineer → Architect
**Trigger**: E-11 DONE; Engineer queue empty (E-6 waits on the operator, E-9 on rein P-50)

### What was built
- E-11 (3e466c0, a8332e9): `OwnerChecked<'l>` — no public constructor, borrows the `InstallLock`, records the
  checked home, refused unless the lock held is that home's own lock (SEC P1 in the first pass). Required by
  `install_tree_locked`, `install_tree_with_faults`, rollback's `rollback_locked` and the app's scratch dir;
  each calls `covers()`. Public `owner_guard` always uses the real uid; injectable `owner_guard_with` is
  doc-hidden. compile_fail doctests pinned to E0061/E0451 plus a no_run positive example. Refusal tests for
  install/update/rollback assert the named path; the `/tmp` CLI test skips on a concurrent lock and is the only
  user of that path. ARCH_PASS, TESTS_PASS, SEC_PASS (2nd pass). CI green ×3 (Windows needed a cfg fix).

### Decisions made
- None.

### Blueprint divergence
- NONE. Open SEC P2s for your call (none blocking):
  1. `owner_guard_with` is still exported (doc-hidden); a `test-seams` cargo feature would take it out of the
     public API (in-process code is not a security boundary, so the reviewer accepted it).
  2. The Linux `--home /tmp` CLI test passes silently when it skips; CI should not count a skip as coverage.

### Next action needed
- None for the Engineer until the operator merges PR #1 / sets `HOMEBREW_TAP_TOKEN` (+ `REIN_RELEASE_PUBKEY`
  after P-50), or you hand off that rein ships `rein release manifest` (E-9).

### Open risks
- Unchanged: Phase 1 cannot ship before PR #1 is merged and rein publishes a signed release + key.
---

---
## Handoff — 2026-11-09 16:10 UTC
**From**: Architect → Engineer
**Trigger**: review of E-11 (handoff 16:08 UTC); D-013 addendum 2; no new tasks

### What was built
- Reviewed E-11 from the handoff, the stamps and the guard's exported surface: `OwnerChecked` bound to the held lock's home is stricter than the task asked and is accepted.
- `.ai/blueprints/distribution.md` §Security (doc-hidden test surface), §Task Breakdown status; `.ai/architect.md` §0, §7; DIGEST.

### Decisions made
- **D-013 addendum 2** — both P2s accepted, no task: `owner_guard_with` stays exported and doc-hidden (no `test-seams` feature); the `/tmp` CLI test may skip (the refusal is covered by the injected-uid tests on every platform). The owner-guard line of work is closed.

### Blueprint divergence (if any)
- NONE.

### Next action needed
- **Nothing to execute.** Phase 1 code is complete. No reply or hand-back is needed for this entry.
- Resume when one of these happens: the operator merges PR #1 and sets `HOMEBREW_TAP_TOKEN` / `REIN_RELEASE_PUBKEY` (E-6: first tag), or I hand off that rein ships `rein release manifest` (E-9).

### Open risks
- Phase 1 cannot ship before PR #1 is merged and rein publishes a signed release + key (P-50).
- The notice for `rein · Architect` (14:55 UTC entry) is still undelivered.
---

---
## Handoff — 2026-11-09 17:05 UTC
**From**: Architect → Engineer
**Trigger**: rein P-50 closed (rein D-155, commit 316218b) — three contract corrections; mirrored as D-014; E-12..E-17 registered

### What was built
- Read rein's D-155, `.ai/blueprints/release-artifact.md` and the amended contract §2–3, then checked each ruling against our code.
- `.ai/blueprints/distribution.md` revised: §Components 2 (member rules), 3 (scope rule, shipped path wins, steps b/c/e/f), 4 (what `rein install` does now, exit 13), 6 (three keys), 7 (rollback runs `rein install`, `doctor --json`); §Data Model (DoctorJson, Snapshot, AllowedKey); §API; §Security trust boundaries 1–3; §Rollback Plan; §Test Plan; §Task Breakdown. `.ai/architect.md` §0, §3, §4, §5, §7, §8; DIGEST.

### Decisions made
- **D-014** — rein D-155 adopted as written, plus four decisions of ours: strict reader; rollback runs `rein install` then `rein doctor`; a failed `rein install` is exit 13; the baked key comes from a committed copy of rein's `docs/contract/release-key.pub` (later, E-16).

### Blueprint divergence (if any)
What on master now contradicts the contract (none of it was wrong when written — the contract text was):
1. `sync::plan` drops shipped paths that match `operator_owned`. `mcp/orchestrator-mcp/package-lock.json` is shipped and matches `mcp/*/package-lock.json`; `rein install` now checks the tree against the manifest and exits 3, so **every app install would fail** until E-13.
2. The home gets no `manifest.json`; `rein install` needs it to recognise an artifact install (E-13).
3. `verify.rs` accepts directory entries and pax headers and finds `manifest.json` anywhere; `in_layout` lets an owned-only path ship (E-12).
4. `AllowedKey::Hooks` and the hooks copy in `profiles apply` (E-14).
5. `rollback` runs doctor only; a failed `rein install` and a failed `rein doctor` both exit 3 (E-15).

### Next action needed
1. **E-12 (T3)** verifier → **E-13 (T3)** writer → **E-15 (T2)** CLI, in that order (E-13 relies on E-12's guarantee that every `files` path is under a mirrored dir). **E-14 (T3)** profiles is independent — do it whenever. Read the named blueprint sections before each; `security_engineer` before each Tier 3 commit.
2. E-13 detail worth reading twice: an owned path the release does NOT ship is still never opened; only the exact shipped path is. A symlinked owned directory above a shipped path is an intermediate symlink and refuses.
3. **BLOCKED, do not start**: E-9 (rein E-417/E-418/E-420), E-16 (the operator's key committed in rein — never generate or substitute a key), E-17 (rein E-422). I will hand off when each unblocks.
4. E-6 stays open for the first tag.

### Open risks
- rein has built none of D-155 yet (its E-417..E-422 are open), so E-12..E-15 are tested against our own fixtures built the way rein's blueprint says its writer will; E-9 and E-17 are the real cross-check.
- Still open with rein: who removes a retired MCP server's directory under an app install.
- The earlier notice for `rein · Architect` is superseded: question (a) is answered by D-155 r2; (b) is re-sent today.
---

---
## Handoff — 2026-11-09 17:15 UTC
**From**: Architect → Engineer
**Trigger**: rein answered our D-014 notice (rein D-155 addendum, commit 3166bb3); D-014 addendum; no new tasks, queue unchanged

### What was built
- `.ai/blueprints/distribution.md`: step 3e (rein's `[INSTALL_INCOMPLETE]` refusal and what it asks of us), §Components 4 (what `rein install` removes; exit list), §Components 7 (rollback then `rein install` is guaranteed safe), §Test Plan "Layout equality" (compare after `rein install` on both sides), E-9 note. `.ai/architect.md` §8. DIGEST.

### Decisions made
- **D-014 addendum** — mirror of rein's answers; none of our decisions changed.

### Blueprint divergence (if any)
- NONE.

### Next action needed
- Carry on with E-12 → E-13 → E-15 and E-14; nothing in their scope changed. Two points that sharpen what is already asked:
  1. **E-13 / E-15**: rein now refuses (`[INSTALL_INCOMPLETE]`, exit 3) when `manifest.json` disagrees with `VERSION` or has none beside it. So a run that ends — success, failure restore, or rollback — must leave the pair consistent: restored together, or `manifest.json` removed when the snapshot has none. Include that pair in the every-k fault test's end-state assertion.
  2. **E-13**: do NOT remove a retired MCP server's directory in the writer. An unshipped `mcp/<retired>/node_modules/` stays `kept`; `rein install` removes it afterwards.
- **E-9** (still BLOCKED): its registered text calls a retired server's directory an expected diff; that is obsolete — `distribution.md` §Test Plan governs.
- No reply needed for this entry.

### Open risks
- Nothing is open with rein. rein has still built none of D-155 (its E-417..E-422), so our fixtures are ours until E-9 and E-17 unblock.
---

---
## Handoff — 2026-11-10 13:54 UTC
**From**: Engineer → Architect
**Trigger**: E-12, E-13, E-14, E-15 complete (rein D-155 / D-014 corrections)

### What was built
- **E-12** (5dad595): strict tarball reader — `crates/rein-release/src/{verify,manifest,error}.rs`, tests/support, verify_release. A header-only first pass refuses before any write. Rules: `manifest.json` comes first; members are regular files with ustar typeflag '0' only; names are unique and safe; no `VERSION` member; only zero padding after end-of-archive and nothing after the gzip stream. Pass 2 re-counts what it writes, and the member set must equal `files`. `files` paths must sit under `layout.mirrored`. Signatures stay prehashed-only. One refused fixture per rule.
- **E-14** (fbbc951): `AllowedKey::Hooks` removed, leaving three keys. `profiles apply` neither reads nor copies `hooks`, and `apply` and `--undo` leave `hooks` as they found it. compile_fail doctest added.
- **E-13** (2085e51): `crates/rein-release/src/{sync,snapshot,layout,rollback}.rs`, plus the conformance fixture reshaped to three columns. A shipped path wins over `operator_owned`. The snapshot holds the shipped live files and `manifest.json`. `manifest.json` is written after `hooks` and before `VERSION`, and the manifest/VERSION pair stays consistent at every fault point k.
- **E-15** (8d4837d): `crates/rein-app/src/{install_cmd,update_cmd,runner,main}.rs`. `rollback` runs `rein install` and then `rein doctor` under the lock. A failed `rein install` is exit 13 on install, update and rollback, with rein's own code printed. `doctor --json` passes stdout and exit code through.
- Tier-3 stamps (ARCH/TESTS/SEC) are recorded for E-12/13/14 and ALIGN_PASS for E-15. Hosted CI is green on 8d4837d on all three OSes.

### Decisions made
- None of my own. All of this implements D-014 and its addendum.

### Blueprint divergence (if any)
1. **E-13 `shipped_owned`**: the snapshot's `layout.json` records the shipped paths that match an `operator_owned` pattern. `snapshot::load` accepts a tree path matching an owned pattern only if it is listed there. This narrows restore under D-013 snapshot trust. ARCH P2 asks to record it in distribution.md §Data Model Snapshot.
2. **E-13**: a directory found at `manifest.json` or `VERSION` is refused before any write. This is not in the blueprint.
3. **E-12**: the "before any byte is written" requirement is met by a header-only first pass, which is stricter than required and stays in scope.

### Next action needed
- Review E-12..E-15 and rule on the open P2s:
  - **E-13 SEC P2-2**: when a failing release newly ships a path that matches an owned pattern, failure restore leaves the file in place, because owned paths are never deleted. Accept it, or rule on cleanup.
  - **E-14 SEC P2**: the spec says `hooks` stay byte-identical, but the settings rewrite keeps them value-identical (a pre-existing JSON re-serialisation). D-008 text still mentions `hooks` and is stale.
  - **E-12 TESTS P2**: the dir/symlink/hardlink/pax rows match `UnsupportedEntry` without asserting `kind`.
- Then decide what's next: E-6 needs the operator (`HOMEBREW_TAP_TOKEN` plus the minisign pair for the first tag); operator docs; or the Phase 2 desktop.md.
- E-9 was auto-unblocked when E-13 closed. I set it back to BLOCKED, since it still waits on rein's `rein release manifest`.

### Open risks
- `rein ci run` checks nothing in this Rust repo: every step skips because it has no cargo step, yet the DONE gate counts that as PASS. Hosted CI and `cargo test --workspace` are the only real evidence. The local CI runner may need a cargo step.
- E-9, E-16 and E-17 remain blocked externally on rein (E-417..E-422, release key).
---

---
## Handoff — 2026-11-10 14:00 UTC
**From**: Architect → Engineer
**Trigger**: review of E-12..E-15 complete (D-015); E-18, E-19 registered

### What was built
- Review only, no source change. E-12..E-15 are accepted as built — no code follow-up.
- `.ai/DECISIONS.md` D-015; `.ai/blueprints/distribution.md` revised (§Components 2, 3 steps a/c/f, 6, new 9; §Data Model Snapshot; §Test Plan; §Task Breakdown); `.ai/architect.md` §0/§7; DIGEST.

### Decisions made
- **D-015**, your open items in order:
  - E-13 `shipped_owned` in the snapshot's `layout.json`: accepted, now in §Data Model Snapshot and step c.
  - Directory at `manifest.json` / `VERSION` refused in preflight: accepted, now in step a.
  - Header-only first pass: accepted, now in §Components 2.
  - E-13 SEC P2-2 (a newly shipped owned-matching path stays after a failed sync or a rollback): accepted, no cleanup — rein D-155 r2 says such a path is kept; recovery is `install --version`. Recorded in step f.
  - E-14 SEC P2: the guarantee is value identity with key order kept, not byte identity; blueprint reworded. D-008's entry stays as written (append-only); it reads three keys since D-014.
  - E-12 TESTS P2: closed — the rows already assert `kind` in the committed tree.
  - `rein ci run`: its PASS is not test evidence in this repo. Keep citing `cargo test --workspace` + hosted CI. The missing cargo step is rein's to fix; do not add one here.

### Blueprint divergence (if any)
- NONE open — the three divergences from your handoff are now written into the blueprint.

### Next action needed
- **E-18** (T2): `README.md` + `docs/operator-guide.md` per distribution.md §Components 9. Document only what the blueprint states; if `--help` output and the §API table disagree, report it, don't document around it.
- **E-19** (T2): release rehearsal without publishing — `dist plan`, local host `dist build`, archive check, findings in `.ai/DEVOPS.md`. No tag, no push, no workflow edit.
- The two are independent; either order.
- E-6, E-9, E-16, E-17 stay as they are (operator / rein). Thanks for re-blocking E-9.

### Open risks
- Phase 1 cannot ship until the operator generates the minisign pair and rein publishes a signed release; nothing in this queue changes that.
- E-19 may show the release workflow needs changes; those come back to me as findings, not as edits.
---

---
## Handoff — 2026-11-10 14:06 UTC
**From**: Architect → Engineer
**Trigger**: rulings on the E-18 mismatches (items 1–7) and the E-19 findings (D-016); E-20, E-21 registered

### What was built
- No source change. `.ai/DECISIONS.md` D-016; `.ai/blueprints/distribution.md` §API table rewritten with an "All commands" paragraph, §Components 9 (contract reference), §Test Plan "Exit codes" and "License files", §Task Breakdown E-20/E-21.

### Decisions made
- **D-016**, your items in order:
  1. Exit 1 = refused before any write, or any other error, on every command — now in §API.
  2. `--home DIR` is global — accepted.
  3. `profiles apply --dry-run` — accepted (prints the diff, writes nothing, runs no `claude` command; tell me if the binary does more).
  4. `--profile` repeatable, `--disable all` — accepted.
  5. `launch [DIR]` exit 4 (no rein-app in the distro) — accepted.
  6. Exit 6 — row dropped, number retired.
  7. `docs/app-contract.md` — your option (a): name it in plain text, no link, as the file rein publishes with its first release; linking the `Vimurai/rein` repo is fine.
- Two things I found while checking your list, both now E-20: `doctor` returns 1 when rein cannot be started or ends without a code, which collides with rein's "exit 1 = JSON document, status fail" → 14; clap's usage-error exit 2 collides with "verify failed" → 64.
- E-19 finding 1 (NOTICE) → E-21. Findings 2 and 3 (rc tag, missing variables/secrets) go to the operator; nothing for you.

### Blueprint divergence (if any)
- NONE open after this revision. §API is the binary's surface plus the two E-20 changes.

### Next action needed
- **E-20** (T2) first, then finish **E-18** against the revised §API so the guide documents the final codes.
- **E-21** (T2), independent. `skill: ci_gate` first if a workflow file changes.
- E-19 is accepted as done; the acceptance line about NOTICE is carried by E-21.

### Open risks
- The `.msi` license-file change in E-21 cannot be built locally; it stays unexercised until a tag.
---

---
## Handoff — 2026-11-10 14:20 UTC
**From**: Architect → Engineer
**Trigger**: operator instruction — move the release key, place the public key in rein; E-16 opened (D-014 addendum 2026-11-10)

### What was built
- No source change. `.ai/DECISIONS.md` D-014 addendum 2026-11-10; `distribution.md` §Security trust boundary 1; E-16 set to OPEN.

### Decisions made
- The operator generated the minisign pair today and asked, in the Architect pane, that the Engineer do the two file operations below. I cannot (this pane writes only `.ai/` and `plans/`). If your own permission settings ask the operator to approve, that approval is theirs to give — this message is not it.
- E-16 no longer waits for rein's commit; it copies the public key from the rein checkout's working tree. A byte comparison against rein's `main` before the first tag replaces the wait.

### Blueprint divergence (if any)
- NONE.

### Next action needed
1. **Secret key — move only.** `mkdir -p /mnt/c/Users/Emir/iCloudDrive/rein-release-key` then `mv ~/release-key.key` into it. Do not open, print, hash, copy elsewhere or commit the file. Confirm with `ls -l` on the destination and that `~/release-key.key` is gone. If the move across filesystems fails, stop and report — do not retry another way.
2. **Public key into rein's tree.** `mkdir -p ~/Dev/rein/docs/contract` and `cp ~/release-key.pub ~/Dev/rein/docs/contract/release-key.pub`. Leave it uncommitted — rein's own session commits it. Write nothing else in the rein checkout and make no commit there.
3. **E-16** (T3, now OPEN): copy from `~/Dev/rein/docs/contract/release-key.pub`, as registered (`ci_gate` first, `security_engineer` before commit).
4. Then the queue as before: E-20 → E-18, E-21.

### Open risks
- rein has not committed the public key yet; the pre-tag byte comparison is the guard.
- rein's release tooling (E-417..E-422) is not yours and not started; nothing here can ship until it exists.
---

---
## Handoff — 2026-11-10 14:42 UTC
**From**: Engineer → Architect
**Trigger**: E-20, E-21, E-18, E-16 complete (E-19 closed earlier); queue exhausted

### What was built
- **E-20** (dd57a63): `doctor` exits 14 (never 1) when `rein doctor` can't start, ends without a code, or exits outside 0..=255. Usage errors exit 64; `--help`, `--version` and `help` exit 0. Tests: a doctor stub plus `tests/cli_usage.rs`.
- **E-21** (6bf6244): LICENSE and NOTICE are now in the tar/zip archives (dist `include`), the .deb (`/usr/share/doc/rein-app/`) and the .msi (`main.wxs` components). `allow-dirty` gains `"msi"`, because dist regenerates `main.wxs` and `wix.license` takes only one file. README.md is now auto-included in the archives too. The .msi stays unexercised until a tag.
- **E-18** (b82b3be): `README.md` and `docs/operator-guide.md`, against the revised §API. `app-contract.md` is named without a link.
- **E-16** (0d573f6): `docs/contract/release-key.pub` is a byte copy of the rein checkout's file, operator key 5C9C75A7C8FA8056. It is baked in with `include_str!` through a strict `MinisignVerifier::from_pub_file`. `.gitattributes` marks the file `-text`; without it, Windows checkouts would get CRLF and every Windows release would fail. The three workflow guards check the committed file (no symlink, no NUL, two lines, "Ed" algorithm bytes, comment id == key id), and the GITHUB_ENV write is gone. README's key paragraph is updated. The `ci_gate` record and results are in DEVOPS.md.
- **Operator file operations (confirmed by the operator directly in this pane):**
  - `~/release-key.key` was moved, never opened, to `/mnt/c/Users/Emir/iCloudDrive/rein-release-key/release-key.key`. The drvfs mount shows it as 0777, so the original 0600 mode is not kept there.
  - `~/release-key.pub` was copied to `~/Dev/rein/docs/contract/release-key.pub` and left uncommitted. That checkout is on branch `chore/closures-2026-11-09`, not main.
- Hosted CI is green on ubuntu, macos and windows for b82b3be and 0d573f6. The reviews: E-20, E-18 and E-21 had Tier 2 reviews. E-16 had Tier 3: round 1 found the P1 CRLF problem, which is fixed; round 2 ran all six critics, all passing.

### Decisions made
- None of my own.

### Blueprint divergence (if any)
- `distribution.md` lines 316-317 and 555 still describe the `REIN_RELEASE_PUBKEY` variable. §Components 9 says "a source build without a baked key refuses every release", but since E-16 every build bakes the committed key. The README now says so, and names the `reject-all-signatures` feature as the way to build a binary that trusts nothing.
- E-16 acceptance: "a release build verifies a fixture signed with the matching key" is covered by a test key going through the same `from_pub_file` path, plus key-id and parse tests of the baked file. A fixture signed with the real key would need the operator to sign one; an agent must never handle the secret.

### Next action needed
- Rule on the E-16 P2s:
  1. Make the byte comparison against rein's `main` a scripted step that blocks the tag in E-6. rein has not committed its copy yet.
  2. Add a CODEOWNERS rule for `docs/contract/**` and `.gitattributes`.
  3. The guard shell is copied into three files and has no automated test. It has never run on macOS or Windows Git Bash; it fails closed if a tool is missing there.
- Rule on whether an operator-signed fixture for the positive real-key test is wanted.
- Engineer queue is empty. E-6 waits on the operator (`HOMEBREW_TAP_TOKEN`; the first tag). E-9 and E-17 wait on rein (E-417..E-422).

### Open risks
- rein must commit `docs/contract/release-key.pub` byte-identical to ours before the first tag.
- The .msi license paths and the guard on Windows Git Bash and macOS stay unexercised until a tag; a `-rc` tag would exercise them.
---

---
## Handoff — 2026-11-10 14:46 UTC
**From**: Architect → Engineer
**Trigger**: review of E-16, E-18, E-20, E-21 complete (D-017); E-22, E-23 registered

### What was built
- No source change. All four tasks accepted as built. `.ai/DECISIONS.md` D-017; `distribution.md` §Security trust boundary 1 (as-built key text, "Key-file guard", "Tag gate"), §Components 9, §Test Plan, §Task Breakdown; `architect.md` §7; DIGEST.

### Decisions made
- **D-017**, your items in order:
  1. Byte comparison with rein's `main`: scripted, runs on tag pushes only, before any artifact build, fails closed on 404 or difference.
  2. CODEOWNERS: yes — `docs/contract/**`, `.gitattributes`, `.github/**`.
  3. Guard shell: one script, called from the three places, tested over a fixture corpus on all three CI OSes.
  4. Operator-signed fixture: no. The production key signs rein releases and nothing else; E-16's acceptance stands on the tests you wrote.
- Both stale blueprint spots are fixed. `allow-dirty = ["msi"]`, README in the archives, and the out-of-range exit code → 14 are accepted.

### Blueprint divergence (if any)
- NONE open.

### Next action needed
- **E-22** (T3): items 1–3 above in one task; `ci_gate` first, `security_engineer` before commit.
- **E-23** (T1): ASCII-only comment in `wix/main.wxs`.
- After these the queue is empty until rein or the operator moves.

### Open risks
- The tag gate will fail every tag until rein commits the key on `main`. That is intended.
- The mode of the secret key on the drvfs mount (0777) is a property of that mount, not something to fix from here.
---

---
## Handoff — 2026-11-10 18:49 UTC
**From**: Engineer → Architect
**Trigger**: E-22, E-23 complete; queue exhausted

### What was built
- **E-23**: no change needed. The `main.wxs` comment was already ASCII in 6bf6244 (E-21 fix round), and the file is ASCII-only with CRLF kept.
- **E-22** (6450f22, follow-up 70c55b9):
  - `.github/scripts/check-release-key.sh` is now the only place the guard logic lives. The three workflow places call it.
  - New `compare-release-key.sh`, and a 26-case corpus, `test-release-key-scripts.sh`. A refusal must exit exactly 1 with its own reason, so a crash fails the case. The corpus runs in `ci.yml` on all three OSes.
  - The tag gate sits in `plan` right after checkout. It fetches rein's main copy over HTTPS (`-f`, size-capped), compares, deletes the fetched bytes, and fails closed on a 404.
  - CODEOWNERS: `@Vimurai` for `/docs/contract/`, `/.gitattributes`, `/.github/`. `.gitattributes` adds `*.sh text eol=lf`.
  - The `Cargo.toml` re-apply note now lists the tag gate as a hand edit in `release.yml`.
- **Hosted CI found a real Windows difference.** Git Bash's `$(...)` drops the CR, so the guard accepted a CRLF file on windows-latest only. A raw-byte CR check now catches it (70c55b9). CI is green on all three OSes, and the corpus passes on each; Windows records the symlink case as a SKIP, as designed.
- Tier 3: all six critics passed. The P1 they found (CRLF `.sh` files on Windows) is fixed.

### Decisions made
- None of my own.

### Blueprint divergence (if any)
- The gate condition is `!github.event.pull_request`, the same condition as `plan`'s `publishing` output, not literally "tag pushes only". The two are equivalent under today's triggers, and this one also covers a future `workflow_dispatch` (security review P2).
- The guard now also refuses any CR byte, which the blueprint's list doesn't name. It only tightens the check and matches `from_pub_file`.

### Next action needed
- Review E-22. Engineer queue is empty.
- **For the operator** (repository settings, not code):
  - a tag ruleset, so a tag on a commit from before E-22 can't run the ungated `release.yml`;
  - branch protection requiring code-owner review, so CODEOWNERS binds;
  - delete `/tmp/claude-1000/e22-pentest.TTzCCF`, the security review's probe copies, which the safe-exec hook blocked;
  - rein must commit `docs/contract/release-key.pub` on `main` before any tag can pass the gate.
- Review P3, yours to rule: CODEOWNERS does not cover `crates/rein-release/`, which consumes the key, or the dist metadata in `Cargo.toml`.

### Open risks
- `--max-filesize` is not enforced when the server sends no Content-Length (curl 7.81 on ubuntu-22.04). The file is 113 bytes from GitHub, so this is P4.
- The gate's live fetch is unexercised until a tag.
---

---
## Handoff — 2026-11-10 18:52 UTC
**From**: Architect → Engineer
**Trigger**: E-22, E-23 reviewed (D-018); E-24, E-25 registered

### What was built
- Review only. E-22 accepted as built (6450f22, 70c55b9); E-23 closed with no change.
- `.ai/blueprints/distribution.md` §Security trust boundary 1 and §Task Breakdown, `.ai/architect.md` §7, `.ai/DECISIONS.md` D-018, `.ai/DIGEST.md` updated.

### Decisions made
- **D-018**: the gate condition `!github.event.pull_request` stands; the raw-byte CR refusal stands; CODEOWNERS widens to `/crates/rein-release/`, `/Cargo.toml`, `/Cargo.lock`; CODEOWNERS is advisory while there is one human account, so required code-owner review is not recommended; a tag ruleset (operator setting) is the binding control; the `--max-filesize` P4 is accepted.

### Blueprint divergence (if any)
- NONE open — both reported divergences are now the blueprint's text.

### Next action needed
- **E-24** (T1): add the three CODEOWNERS paths and the "advisory" comment.
- **E-25** (T2): `dist plan` for `v0.1.0-rc.1` and `v0.1.0`; record results and the operator's tag sequence in `.ai/DEVOPS.md`. Change no version, create no tag, push nothing; report a needed version bump instead of making it.

### Open risks
- The gate's live fetch is unexercised until a tag. rein is public with default branch `main`, so the raw URL will resolve once rein commits the key.
- The probe directory `/tmp/claude-1000/e22-pentest.TTzCCF` is left for the operator to delete.
---

---
## Handoff — 2026-11-10 18:57 UTC
**From**: Engineer → Architect
**Trigger**: E-24, E-25 complete; queue exhausted

### What was built
- **E-24** (5655b9e, pushed): CODEOWNERS adds `/crates/rein-release/`, `/Cargo.toml` and `/Cargo.lock`, and its comment now says the list is advisory. Hosted CI is green on all three OSes.
- **E-25**: `.ai/DEVOPS.md` "E-25: release-candidate tag rehearsal". No version change, no tag, nothing pushed for it.

### Decisions made
- None.

### Blueprint divergence (if any)
- NONE.

### Next action needed
- **A version bump is needed for the rc tag (reported, not made).** `dist plan --tag=v0.1.0-rc.1` exits 255 at version 0.1.0. A copy with `[workspace.package] version = "0.1.0-rc.1"` and the two path-dependency `version` pins in `crates/rein-app/Cargo.toml` plans fine, as a prerelease; `Cargo.lock` changes too, since the build is `--locked`.
- **Rule on the Windows rc→final upgrade.** cargo-wix maps the rc to MSI 0.1.0.1 and the final to 0.1.0. Windows Installer ignores the 4th field, so the final `.msi` does not upgrade an installed rc. Either change the WiX setup (e.g. `AllowSameVersionUpgrades`) or write the operator step "uninstall the rc first". The `.deb` (`0.1.0~rc.1`) upgrades fine.
- Confirmed: an rc is published as a GitHub prerelease, Homebrew publish is skipped for it, and the tag gate applies to it.
- **Operator context:** at the operator's request I opened a rein Engineer pane, `rein pane engineer` in `~/Dev/rein`, tmux `rein-app:4`, on the `personal` account. It hasn't been given a task yet. The operator wants rein's key commit on `main` and E-417..E-422 done so a signed rein release exists.

### Open risks
- Nothing can be installed by rein-app until rein publishes a signed artifact. rein's v0.4.0 GitHub release has no assets.
---

---
## Handoff — 2026-11-10 19:05 UTC
**From**: Architect → Engineer
**Trigger**: E-24, E-25 reviewed (D-019); no new tasks

### What was built
- Review only. E-24 and E-25 accepted. `.ai/DECISIONS.md` D-019, `.ai/DIGEST.md` updated.

### Decisions made
- **D-019**: the rc version bump is not made until the operator says the rc goes ahead (then one task bumps to `0.1.0-rc.1`, a later one sets `0.1.0` back); no WiX change — an installed rc `.msi` is uninstalled by hand before the final; an rc needs no `HOMEBREW_TAP_TOKEN`.

### Blueprint divergence (if any)
- NONE.

### Next action needed
- Nothing to execute. The queue stays empty until the operator calls the rc or rein delivers (key on `main`, E-417..E-422, a signed release).
- The notice to `rein · Architect` was delivered 2026-11-10 (key commit, E-417..E-422, `rein ci run` cargo gap).

### Open risks
- The "uninstall the rc `.msi` first" line is not yet in `docs/operator-guide.md`; it rides with the rc bump task.
---

---
## Handoff — 2026-11-10 22:41 UTC
**From**: Architect → Engineer
**Trigger**: rein D-155 addendum 3 relayed; D-020 written; E-26 registered

### What was built
- `.ai/DECISIONS.md` D-020; `.ai/blueprints/distribution.md` §Components 2 "path alphabet and case rule" and §Task Breakdown; `.ai/architect.md` §7; `.ai/DIGEST.md`.

### Decisions made
- **D-020**: yes, mirror the rule. Checked in `Manifest::parse` (not the layout writer), on every OS, exit 2, ASCII-only folding, directory prefixes count, `layout.mirrored` names included. Snapshot `layout.json` is not re-checked. No Windows-reserved-name rule.

### Blueprint divergence (if any)
- NONE.

### Next action needed
- **E-26** (T3): implement and test per the task text; `security_engineer` before commit; add the refusal to the operator guide's list.

### Open risks
- Not ruled: a shipped path differing only by case from an operator-owned file already in the home on a case-insensitive filesystem. Raise it with rein if a real case appears.
- Key status: rein's remote `main` now carries the public key, blob id equal to ours (verified by the Architect), so the tag gate's comparison will pass.
---

---
## Handoff — 2026-11-10 22:57 UTC
**From**: Engineer → Architect
**Trigger**: E-26 complete

### What was built
- **E-26** (24ed040, hosted CI green ×3, run 42918663301):
  - `paths.rs`: `check_path_alphabet` (0x20–0x7E) and `check_case_collisions`, an ASCII-lowercased per-segment tree walk where directory prefixes count and both paths are named. Memory is linear in path length.
  - `Manifest::parse`: applies both to every `layout.mirrored` name and `files` path. `validate_layout` is untouched, so snapshot `layout.json` is not re-checked (D-020 r5).
  - Tests: unit rows in paths.rs and manifest.rs; 3 `verify_release` CASES; `install_cmd::a_case_colliding_release_exits_2_with_the_home_untouched`.
  - `docs/operator-guide.md`: refusal row added.
  - Reviews: [ARCH_PASS] [SEC_PASS] [TESTS_PASS]; security_engineer pass done.

### Decisions made
- None new.

### Blueprint divergence (if any)
- E-26 / D-020 r2 say "before signature or tarball work". `Manifest::parse` runs where it always ran: after the signature check and `scan_members`, before the member-set check, staging and any write. ARCH and SEC both say: fix the wording, not the code.

### Next action needed
- **Ruling needed (security_engineer P2, outside D-020):** a case-only rename across releases deletes the shipped file on a case-insensitive home (APFS, drvfs).
  - Mechanism: v0.4 has `shared/locate.sh`, v0.5 ships `shared/Locate.sh`. `sync.rs` `plan` compares names exactly, `apply` writes then prunes, and the prune removes `shared/locate.sh`, which is the file just written. The install exits 0. Reproduced on /mnt/c drvfs with a fixture that mirrors plan, write, prune.
  - Rollback and failure-restore have the same issue in reverse.
  - Options proposed: (a) never prune a live entry whose ASCII-folded path, or a prefix of it, equals a shipped path; (b) prune before write within a root; (c) after sync, confirm every shipped path exists with its hash.
- P3s for your call:
  - NTFS 8.3 short names (`LONGFI~1.SH`) alias long names and pass the case rule. Possibly a rein addendum refusing `~<digit>` segments.
  - `operator_owned` patterns are not ASCII-checked, and matcher errors print them raw.
- FYI sent earlier: rein D-153 addendum 2 (accounts.json closed shape; the app never writes it). rein-app already complies. Stale wording remains at profile.rs:5.

### Open risks
- The P2 above (silent file loss on a case-only rename).
- E-6, E-9 and E-17 remain blocked on the operator and on rein (E-418 PR #36 re-running CI; E-419..E-422 not started; no signed release).
---

---
## Handoff — 2026-11-10 23:02 UTC
**From**: Architect → Engineer
**Trigger**: E-26 reviewed (D-021); E-27 registered

### What was built
- `.ai/DECISIONS.md` D-021; `.ai/blueprints/distribution.md` §Components 2 (order wording, `operator_owned` patterns) and §Components 3 step d (case-variant prunes, post-sync check), §Task Breakdown; `.ai/architect.md` §7 and §3 item 4 (accounts.json, rein D-153 addendum 2); `.ai/DIGEST.md`.

### Decisions made
- **D-021**: E-26 accepted; the "before signature" wording was mine and wrong — the code order stands. P2 ruling: your option (b) narrowed to case-variant prune entries only, plus option (c) as an existence + size check. Option (a) is rejected (stale file beside the new one on a case-sensitive home). `operator_owned` patterns are ASCII-checked and matcher errors escape non-printable bytes. 8.3 short names are not ruled here.

### Blueprint divergence (if any)
- NONE open.

### Next action needed
- **E-27** (T3): failing test first, then the fix; it also carries the `profile.rs:5` comment.

### Open risks
- A directory can keep its old-case name on a case-insensitive home; accepted.
---

---
## Handoff — 2026-11-10 23:06 UTC
**From**: Architect → Engineer
**Trigger**: `profiles list` false warning relayed; D-022 written; E-28 registered

### What was built
- `.ai/DECISIONS.md` D-022; `.ai/blueprints/distribution.md` §Components 6 "Enumeration order and names", §Task Breakdown; `.ai/architect.md` §7; `.ai/DIGEST.md`.

### Decisions made
- **D-022**: neither option as stated. Accounts rows are listed before the built-in `personal`, and the row's `config_dir` wins; the built-in is a fallback when no row has that name. Only `default` is reserved. The name check is terminal safety only (0x21–0x7E, ≤ 64 bytes), as you proposed.

### Blueprint divergence (if any)
- NONE.

### Next action needed
- Finish **E-27** first, then **E-28** (T3): failing test first; `security_engineer` before commit.

### Open risks
- None new.
---

---
## Handoff — 2026-11-10 23:14 UTC
**From**: Architect → Engineer
**Trigger**: E-27 security findings ruled (D-023); E-29 registered

### What was built
- `.ai/DECISIONS.md` D-023; `.ai/blueprints/distribution.md` §Components 2 ("Case-variant of an owned path", "Error text"), §Components 3 steps b and d, §Task Breakdown; `.ai/architect.md` §7; `.ai/DIGEST.md`.

### Decisions made
- **D-023**: P2-1 is fixed, not accepted — your proposed parse refusal plus the ASCII fold for the protect decision, on every filesystem. Removing empty case-variant directories before the writes is ratified. P3-1 → preflight `Conflict`; P3-4 → escape all manifest-derived error text; P3-2 and P3-3 accepted as they are.
- **E-27 (9adfd72) is cleared for Tier-3 sign-off**: the P2-1 gap exists with or without E-27 and is tracked as E-29.

### Blueprint divergence (if any)
- NONE open.

### Next action needed
- Close out E-27 and E-28, then **E-29** (T3): failing tests first; `security_engineer` before commit.

### Open risks
- Process: my commit 64f38b7 swept your staged E-27 files into a `.ai` commit; thank you for splitting it (a254315 / 9adfd72). I commit `.ai` with `git commit --only <paths>` from here on.
---

---
## Handoff — 2026-11-10 23:18 UTC
**From**: Architect → Engineer
**Trigger**: E-28 reviewed (D-024); E-30 registered

### What was built
- `.ai/DECISIONS.md` D-024; `.ai/blueprints/distribution.md` §Components 6, §Task Breakdown; `.ai/architect.md` §7; `.ai/DIGEST.md`.

### Decisions made
- **D-024**: E-28 accepted with its divergence (an unusable `personal` row does not suppress the built-in — now the blueprint's text, extended to duplicates). P2: option (b), skip the row, plus shell-quoting the dry-run path. P3-1 → a one-line note in `apply`; P3-2 → guide sentence; P3-3 → duplicate rows warn and never hide the built-in; P3-4 → no change.

### Blueprint divergence (if any)
- NONE open.

### Next action needed
- **E-29**, then **E-30** (T3): failing test first; `security_engineer` before commit.

### Open risks
- None new. E-27 and E-28 wait for the operator's push approval.
---

---
## Handoff — 2026-11-10 23:24 UTC
**From**: Architect → Engineer
**Trigger**: rein `main` at 3f68eff carries rein E-417, E-418, E-420 — **E-9 is unblocked**

### What was built
- E-9 set to OPEN. `.ai/DIGEST.md` updated. No blueprint change.

### Decisions made
- None new. Notes that bind E-9 (they refine the task text, they do not change the test):
  - "tag T" is rein commit **3f68eff** on `main`; the version is whatever rein's generator stamps from that tree.
  - Work from a throwaway copy of rein (e.g. `git clone` of the local checkout into a temp dir, checked out at 3f68eff). Write nothing and commit nothing in `~/Dev/rein`.
  - Sign with the throwaway test key only. The production key signs nothing here (D-017).
  - rein E-420 now removes a retired mirror in artifact mode, so the "retired MCP server's directory" exception may no longer appear. Report every remaining diff; do not adjust the writer to hide one.
  - At 3f68eff rein's generator does not yet apply the case/ASCII rules (they arrive with rein E-422). If our verifier (E-26, E-29) refuses rein's real tree, that is a finding to report, not something to loosen.

### Blueprint divergence (if any)
- NONE.

### Next action needed
- Order: **E-29**, **E-30**, then **E-9** (T2).
- **E-17 stays blocked**: `operator-owned-cases.json` is on rein `main`, but `profile-settings-cases.json` and the schemas arrive with rein E-422.

### Open risks
- E-9 is the first time rein-app reads an artifact made by rein's own generator; expect contract mismatches and report them rather than patching around them.
---

---
## Handoff — 2026-11-10 23:31 UTC
**From**: Architect → Engineer
**Trigger**: E-29 contract conflict ruled (D-025)

### What was built
- `.ai/DECISIONS.md` D-025; `.ai/blueprints/distribution.md` §Components 3 step b (owned matching is case-sensitive), §Task Breakdown E-29; `.ai/DIGEST.md`.

### Decisions made
- **D-025**: your option (c). D-023 ruling 2 (the protective fold) is withdrawn — rein's table rules case-sensitive matching and I ruled against it without checking. You were right to stop and not touch the fixture.

### Blueprint divergence (if any)
- The E-29 task text still lists item (2); it is void. Say so in the task's completion summary.

### Next action needed
- **E-29**: remove the fold from matcher/owned/scan/snapshot; keep r1 (parse refusal), r3 (symlink Conflict), r4 (escaping). Drop or invert the fold's tests so the "operator-created case variant" case asserts today's contract behaviour (pruned, present in the snapshot). `operator_owned_conformance` must pass with the fixture untouched. Then E-30, E-9.

### Open risks
- An operator-created case variant of an owned name is pruned on update (recoverable from the snapshot). Asked of rein · Architect; a new task follows if rein amends its table.
---

---
## Handoff — 2026-11-10 23:44 UTC
**From**: Architect → Engineer
**Trigger**: E-29 security round 2 ruled (D-026)

### What was built
- `.ai/DECISIONS.md` D-026; `.ai/blueprints/distribution.md` §Components 2 ("per pattern", "Directory spellings"); `.ai/DIGEST.md`.

### Decisions made
- **D-026**: P2-1 → your option (a), for mirrored names and every directory prefix of a shipped path. P2-2 → your reading of D-023 r1 is confirmed: per pattern, and a path another pattern owns exactly is still refused. P3-3 → give the symlink Conflict its own wording, in E-29. P3-4 → no change. The unreachable ARCH P2 needs nothing.

### Blueprint divergence (if any)
- NONE open.

### Next action needed
- Finish **E-29** with (a), re-run `security_engineer`, commit; then **E-30**, **E-9**.
- If rein's tree at 3f68eff or any row of rein's tables trips the new refusals, stop and report.

### Open risks
- `verify_release.rs:53` fails on drvfs for mode bits; environmental, hosted CI is the evidence.
---

---
## Handoff — 2026-11-11 12:34 UTC
**From**: Architect → Engineer
**Trigger**: E-29 reviewed (D-027); E-31 registered (blocked on E-9)

### What was built
- `.ai/DECISIONS.md` D-027; `.ai/blueprints/distribution.md` §Execution Constraints "Manifest size limits", §Task Breakdown; `.ai/architect.md` §7; `.ai/DIGEST.md`.

### Decisions made
- **D-027**: E-29 accepted as built (f298c5b). The remaining P3s become one task: size limits at `Manifest::parse` plus three linear lookups.

### Blueprint divergence (if any)
- NONE.

### Next action needed
- **E-30** close-out, then **E-9**, then **E-31** (T2; opens when E-9 is DONE). In E-9, record the real counts from rein's generator (mirrored, patterns, files, longest and deepest path) — E-31 needs them.

### Open risks
- E-27, E-28, E-29 (and E-30 when committed) wait for the operator's push approval; hosted CI has not run on them.
---

---
## Handoff — 2026-11-11 12:52 UTC
**From**: Architect → Engineer
**Trigger**: E-9 first run ruled (D-028); E-31 unblocked

### What was built
- `.ai/DECISIONS.md` D-028; `.ai/blueprints/distribution.md` §Test Plan "Layout equality"; E-31 set OPEN with no dependency; `.ai/DIGEST.md`.

### Decisions made
- **D-028**: your test seam is accepted (no seam in the binary). Items 3 and 5: as you proposed — document the two per-install differences; assert exit ∈ {0, 3} and no `[INSTALL_TREE_MISMATCH]` / `[INSTALL_INCOMPLETE]`. Items 4, 5 (doctor scope) and 6 (cwd) are raised with rein · Architect; rein-app changes nothing meanwhile.

### Blueprint divergence (if any)
- NONE open.

### Next action needed
- **E-9**: finish the test with the strict assertions and run the retired-server case as far as it goes. Commit it red-at-3f68eff (skipped without `REIN_CHECKOUT`), record the run in `.ai/DEVOPS.md`, and leave E-9 **OPEN** — do not mark it DONE and do not wire CI yet.
- **E-31** (T2): now OPEN; limits 128 / 256 / 100000 / 1024 / 64 (D-027 addendum).

### Open risks
- Every `rein-app install` of rein at 3f68eff exits 3. The first tag (E-6) needs a rein release where a healthy machine gets exit 0.
---

---
## Handoff — 2026-11-11 13:01 UTC
**From**: Architect → Engineer
**Trigger**: rein D-158 mirrored (D-029); rein `main` at d84dd17 carries E-422 — **E-17 is unblocked**; E-32 registered BLOCKED

### What was built
- `.ai/DECISIONS.md` D-029; `.ai/blueprints/distribution.md` §Components 4 "Install verdict", §Task Breakdown; `.ai/architect.md` §7; `.ai/DIGEST.md`. E-17 → OPEN; E-32 → BLOCKED.

### Decisions made
- **D-029**: rein fixes all three E-9 findings itself (rein E-426 `npm ci`, E-427 doctor scopes). rein-app's only change is E-32: judge an install by the `status` of `rein doctor --scope install --json` (fail → 3; warn, ok → 0; no valid document → 14). Not before rein E-427 merges.

### Blueprint divergence (if any)
- NONE.

### Next action needed
- Order: **E-31**, then **E-17** (T2). E-9 stays OPEN and red; E-32 stays BLOCKED until I name rein's E-427 commit.
- E-17 notes (they refine the task text):
  - Vendor from rein **d84dd17**, from a throwaway copy; write nothing in `~/Dev/rein`.
  - The corrected `operator-owned-cases.json` has top-level unshipped rows as `kept` and new rows `shared/Toolchain.sh` → `deleted`, `ci/x.json` → `kept`, plus `glob_cases`. The conformance harness must answer "kept" for any path outside the mirrored directories without consulting the matcher (blueprint §Test Plan "Shared contract fixtures"). If that needs a change in `plan` itself rather than in the harness, stop and report.
  - Vendor the three schemas under `docs/contract/` too, noting the commit. `doctor.schema.json` will gain a required `scope` with rein E-427; do not add it yourself.
  - rein's `docs/app-contract.md` now exists: README and the operator guide may link it (D-016 had it named unlinked).
  - Report any row our code fails; do not edit a fixture.

### Open risks
- Until rein E-426 and E-427 merge, every `rein-app install` of rein exits 3.
---

---
## Handoff — 2026-11-11 13:12 UTC
**From**: Architect → Engineer
**Trigger**: E-17 profile-settings row ruled (D-030)

### What was built
- `.ai/DECISIONS.md` D-030; `.ai/blueprints/distribution.md` §Test Plan "Shared contract fixtures"; `.ai/DIGEST.md`.

### Decisions made
- **D-030**: your option (a), with the harness's meaning made explicit. `refuse` rows: unreachable, no exceptions. `accept` rows: reachable by `plan`, or on an explicit list with a reason, each listed row asserted unreachable by `plan`. "Remove an allowed key" is on the list and is produced by a file-level `--undo` test. No `Patch` remove operation.

### Blueprint divergence (if any)
- NONE.

### Next action needed
- Finish and commit **E-17**. Then the queue is empty until rein's E-426/E-427 commits arrive (E-9 green, E-32).

### Open risks
- None new.
---

---
## Handoff — 2026-11-11 13:20 UTC
**From**: Architect → Engineer
**Trigger**: rein `main` at f7b3554 carries rein E-426 (`npm ci`; no rewrite of a shipped lock) — re-run E-9

### What was built
- `.ai/DIGEST.md` updated. No decision, no blueprint change.

### Decisions made
- None new (D-028 and D-029 cover it).

### Blueprint divergence (if any)
- NONE.

### Next action needed
- After E-17: **re-run E-9** against rein **f7b3554** from a fresh throwaway clone, with the assertions unchanged (tree diff; exit 0 or 3; no `[INSTALL_TREE_MISMATCH]` / `[INSTALL_INCOMPLETE]`), including the retired-server case. The lock's size and sha256 differ from 3f68eff's, so regenerate the artifact; do not reuse the old one.
  - Green → record the run in `.ai/DEVOPS.md`, name f7b3554 in the test's note, and close E-9 (`skill: rein-task`). CI wiring is a later task, planned together with E-32 so the pinned rein commit has the doctor scopes.
  - Any remaining diff or token → report it; change nothing to get green.
- Expect exit 3 for host reasons in the clean home until rein E-427; that is within the assertion.
- A new owned file appears on both sides: `mcp/<server>/node_modules/.rein-lock.sha256`. It is under `node_modules/`; if it shows in the diff, report it rather than adding an exception.

### Open risks
- E-32 stays BLOCKED until rein E-427 merges.
---

---
## Handoff — 2026-11-11 13:31 UTC
**From**: Architect → Engineer
**Trigger**: E-9 case 2 ruled (D-031)

### What was built
- `.ai/DECISIONS.md` D-031; `.ai/blueprints/distribution.md` §Test Plan "Layout equality" (case 2); `.ai/DIGEST.md`.

### Decisions made
- **D-031**: your option (a). Side A re-runs `install-rein.sh` in case 2 — that is how a clone install is updated. The "on both sides" wording was mine and wrong for clone mode. Assertions unchanged.

### Blueprint divergence (if any)
- NONE open.

### Next action needed
- **E-9**: change side A of case 2, re-run at f7b3554. Green → record both runs (case 1 green; case 2 red as first written, green after D-031) in `.ai/DEVOPS.md`, commit, close E-9. Anything else → report.
- After that the queue is empty until I name rein's E-427 commit (E-32).

### Open risks
- None new.
---

---
## Handoff — 2026-11-11 13:54 UTC
**From**: Engineer → Architect
**Trigger**: E-9 green and DONE; queue empty

### What was built
- **E-9** (1e1657f, be06b2e; DEVOPS 1531392):
  - The layout-equality test is green at rein f7b3554, with both cases passing.
  - Case 2 updates the clone side by re-running `install-rein.sh` (D-031).
- **E-17** DONE (9c4a0d0 and fmt fix e039391, CI ×3 green). **E-31** DONE (7273f32, CI ×3 green).

### Decisions made
- None new.

### Blueprint divergence (if any)
- NONE beyond the rulings already recorded (D-028 test seam, D-030 UNDO_ONLY, D-031).

### Next action needed
- Queue empty.
- E-32 waits on rein E-427: name its commit.
- E-6 waits on the operator.
- be06b2e and the .ai commits are local; they go out with the next approved push.

### Open risks
- The E-17 push first failed CI at `cargo fmt --check`, because a path was edited after fmt had run. Before each commit I now run `cargo fmt --all -- --check`.
---
