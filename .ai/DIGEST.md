# DIGEST (Token Saver Cache) — updated 2026-11-09 by Architect

## Triad Health
- Stage: Executing (Phase 1 distribution; E-2..E-5 done, E-6 half-shipped, E-7..E-9 planned)
- Architect Focus: E-5/E-6 reviewed; LayoutWriter mechanics re-ruled (D-011); waiting on rein for P-50 artifacts and two contract answers
- Engineer Focus: E-7 in-place LayoutWriter + rollback (T3) → E-8 `install` default latest (T2); E-6 closes on PR #1 merge + first tag

## Current snapshot
- Product: rein-app — Rust shell for Rein: installs a signed `rein` release into `$HOME/.rein`, runs `rein install`/`doctor` from it, manages Claude profile settings (4 allowed keys), updates/rolls back; phase 2 wraps it in a Tauri window. Never contains Rein logic.
- Stack: Rust workspace (`rein-app` bin, `rein-release` lib, `rein-profiles` lib), cargo-dist + Homebrew tap + msi + `.deb`, WSL2 launcher on Windows; phase 2 Tauri + xterm.js over tmux.
- Contract: `rein` D-150 / D-153 (mirrored D-001, D-003). Contract items are decided in `rein`, not here.
- Shipped on master: E-3 verifier (minisign, schema 1), E-4 profile manager, E-5 LayoutWriter (whole-home swap — superseded) + Runner + `install`, E-6 code (`update`, `rollback`, `doctor`, `launch`).
- **D-011 (2026-11-09)**: the writer syncs only the manifest's mirrored dirs in place, per file (temp + rename, `hooks` last, `VERSION` last), like `install-rein.sh`. Everything else in the home is never opened. Snapshot = release payload only (~9 MB; the live home is 3.5 GB). Rollback restores code only. No "refuse while rein runs" check. Implemented by E-7.
- **D-012**: AppImage out of Phase 1; divergences 3–9 of the E-5/E-6 handoff ruled (E-8, E-9 scheduled, the rest accepted).
- Open queue: E-6 (PR #1 `engineer/e6-release-pipeline` awaiting operator merge; needs `HOMEBREW_TAP_TOKEN` secret and `REIN_RELEASE_PUBKEY` variable), E-7 (T3), E-8 (T2), E-9 (BLOCKED: E-7 + rein P-50 generator).
- Gate: no release tag before E-7 is on master.
- Known risks: no rein release can be installed until rein publishes a signed release + public key (P-50); release jobs unexercised until the first tag; D-010 lives only on the PR branch (DECISIONS.md will need a both-sides merge); two questions open with rein (complement rule for `operator_owned`, who purges retired MCP mirrors).
- Repo: `github.com/Vimurai/rein-app` (public, hosted CI green on 3 OSes). License Apache-2.0 (D-004) — awaiting operator confirmation.
- Blueprints: `.ai/architect.md` (index), `.ai/blueprints/distribution.md` (Phase 1, revised 2026-11-09). `desktop.md` not yet written.

## Recent changes (last 10)
- 2026-11-09 Architect: reviewed E-4/E-5/E-6; D-011 (in-place sync, carry-over + SEC P1-3 closed), D-012 (AppImage deferred, divergence rulings); distribution.md §Components 3/5/7/8, §Security, §Rollback, §Test Plan revised; E-7..E-9 registered.
- 2026-11-09 Engineer: E-6 code (082c1da) on master; release pipeline in PR #1 (93c4674); D-010 on that branch.
- 2026-11-09 Engineer: E-5 LayoutWriter + Runner + `install` (73fa5bc, 74f4ca8); E-4 profile manager (58b2413).
- 2026-11-08 Architect: D-005/D-007/D-008/D-009 (plugin + marketplace registration, undo semantics, session-start reconciliation).
- 2026-11-08 Architect: rein D-153 mirrored as D-003 — minisign, manifest schema 1, upgrade semantics.
- 2026-11-08 Architect: architect.md v1 + blueprints/distribution.md; E-2..E-6 registered; D-001, D-002.

---
DIGEST must be accurate or flagged as stale. If stale, invoke `skill: rein-digest`.
