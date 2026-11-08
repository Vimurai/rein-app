# DIGEST (Token Saver Cache) — updated 2026-11-08 by Architect

## Triad Health
- Stage: Planning → Executing (Phase 1 bootstrap handed to Engineer)
- Architect Focus: Phase 1 distribution blueprint written; awaiting rein P-50 (signature scheme, manifest, doctor --json)
- Engineer Focus: E-2 Cargo workspace + cargo-dist + CI matrix (first OPEN task)

## Current snapshot
- Product: rein-app — Rust shell for Rein: installs a signed `rein` release into `$HOME/.rein`, runs `rein install`/`doctor` from it, manages Claude profile settings (4 allowed keys), updates/rolls back; phase 2 wraps it in a Tauri window. Never contains Rein logic.
- Stack: Rust workspace (`rein-app` bin, `rein-release` lib, `rein-profiles` lib), cargo-dist + Homebrew, WSL2 launcher on Windows; phase 2 Tauri + xterm.js over tmux.
- Contract: `rein` D-150 (`rein/plans/rein-ecosystem-contract.md`, `rein-app-plan.md`). Contract items are decided in `rein`, not here.
- Current focus (top 3): E-2 workspace+CI (T2) → E-3 fail-closed verifier (T3) ∥ E-4 profile manager (T3) → E-5 layout writer (T3) → E-6 packaging+WSL (T2).
- Contract rulings: rein D-153 (mirrored as D-003) — minisign + baked key, manifest schema 1, doctor JSON shape, accounts.json surface, upgrade preserves list (config/ mirrored). E-3/E-5 build against `.ai/blueprints/distribution.md`, not the older task text.
- Known risks: rein's release workflow / manifest generator / doctor --json not yet shipped (E-3 uses a fixture key pair); rein-plugins remote `Vimurai/rein-plugins` not live yet; repo is not yet a git repository (E-2 inits).
- Blueprints: `.ai/architect.md` (index), `.ai/blueprints/distribution.md` (Phase 1). `desktop.md` not yet written.

## Recent changes (last 10)
- 2026-11-08 Architect: architect.md v1 + blueprints/distribution.md; E-1 placeholder closed; E-2..E-6 registered; D-001 contract adoption, D-002 marketplace registration shape.
- 2026-11-08 Architect: rein D-153 mirrored as D-003 — verifier (minisign), manifest schema 1, layout writer upgrade semantics revised in distribution.md; architect.md §3/§8 updated.

---
DIGEST must be accurate or flagged as stale. If stale, invoke `skill: rein-digest`.
