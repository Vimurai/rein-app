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
