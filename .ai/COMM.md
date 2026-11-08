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
