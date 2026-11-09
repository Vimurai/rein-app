# DIGEST (Token Saver Cache) — updated 2026-11-09 17:05 UTC by Architect

## Triad Health
- Stage: Executing (rein D-155 corrected the contract in three places; E-12..E-15 bring shipped code in line)
- Architect Focus: D-155 mirrored as D-014; next is review of E-12..E-15, then operator docs / release rehearsal / Phase 2 blueprint as the operator chooses
- Engineer Focus: E-12 verifier (T3) → E-13 writer (T3) → E-15 CLI (T2); E-14 profiles (T3) is independent

## Current snapshot
- Product: rein-app — Rust shell for Rein: installs a signed `rein` release into `$HOME/.rein`, runs `rein install`/`doctor` from it, manages Claude profile settings (3 allowed keys), updates/rolls back; phase 2 wraps it in a Tauri window. Never contains Rein logic.
- Stack: Rust workspace (`rein-app` bin, `rein-release` lib, `rein-profiles` lib), cargo-dist + Homebrew tap + msi + `.deb`, WSL2 launcher on Windows; phase 2 Tauri + xterm.js over tmux.
- Contract: `rein` D-150 / D-153 / **D-155** (mirrored D-001, D-003, D-014). Normative text: rein `.ai/blueprints/release-artifact.md`. Contract items are decided in `rein`, not here.
- Shipped on master: verifier (E-3), profile manager (E-4), in-place LayoutWriter + payload snapshots + code-only rollback (E-7), `install`/`update`/`rollback`/`doctor`/`launch` (E-5, E-6 code, E-8), owner guard as a compile-time proof (E-10, E-11), release pipeline (PR #1 merged).
- **D-014 (rein D-155)**: tarball = `manifest.json` first + regular files only (strict reader); a shipped path wins over an `operator_owned` pattern (patterns protect from deletion only); the unpacked `manifest.json` stays at the top of the home and `VERSION` is written last; `rein install` finishes an artifact itself (tree check, toolchain pin, stamp); profiles take THREE keys, never `hooks`; rollback runs `rein install` + `rein doctor`; a failed `rein install` is exit 13; the baked key will come from a committed copy of rein's `docs/contract/release-key.pub`.
- Why it matters now: today's writer skips `mcp/orchestrator-mcp/package-lock.json`, so `rein install`'s tree check would fail every app install; `profiles apply` still copies `hooks` between profiles.
- D-011 (in-place sync, nothing outside the mirror is opened), D-012 (AppImage out of Phase 1), D-013 (target symlinks, snapshot trust, owner guard) stand.
- Open queue: E-12, E-14 (OPEN); E-13 (after E-12), E-15 (after E-13); E-6 (first tag — needs `HOMEBREW_TAP_TOKEN` and a release key). BLOCKED externally: E-9 (rein E-417/E-418/E-420), E-16 (operator's key committed in rein), E-17 (rein E-422).
- Phase 1 ships when: E-12..E-15 are on master, the operator generates the minisign pair and rein publishes a signed release, first tag builds.
- rein D-155 addendum (3166bb3): `rein install` removes retired MCP server dirs (our writer keeps them; E-9 compares after `rein install` on both sides); a `manifest.json` ≠ `VERSION` is `[INSTALL_INCOMPLETE]` exit 3; rollback + `rein install` is safe. Nothing is open with rein.
- Known risks: no rein release exists yet (rein's E-417..E-422 not built); release jobs unexercised; no README / operator docs in this repo.
- Repo: `github.com/Vimurai/rein-app` (public, hosted CI green on 3 OSes). License Apache-2.0 (D-004) — awaiting operator confirmation.
- Blueprints: `.ai/architect.md` (index), `.ai/blueprints/distribution.md` (Phase 1, revised 2026-11-09 for D-011..D-014). `desktop.md` not yet written.

## Recent changes (last 10)
- 2026-11-09 Architect: rein D-155 addendum mirrored (D-014 addendum) — retired-server removal is `rein install`'s, write order confirmed, E-9 comparison point fixed; no task changes.
- 2026-11-09 Architect: rein D-155 mirrored (D-014); distribution.md §Components 2/3/4/6/7, §Data Model, §API, §Security, §Test Plan revised; architect.md §3/§4/§5/§7/§8; E-12..E-17 registered; E-9/E-16/E-17 marked BLOCKED.
- 2026-11-09 rein: P-50 closed (D-155, commit 316218b) — three contract corrections for rein-app.
- 2026-11-09 Operator: PR #1 (release pipeline) merged.
- 2026-11-09 Architect: reviewed E-11; D-013 addendum 2 — both remaining P2s accepted, owner-guard work closed.
- 2026-11-09 Engineer: E-11 `OwnerChecked` proof bound to the held lock (3e466c0, a8332e9); E-10 owner guard (5b8bc6b).
- 2026-11-09 Architect: D-013 (target-symlink rule, snapshot trust, owner guard).
- 2026-11-09 Engineer: E-7 in-place sync + payload snapshots + code-only rollback (1ac3f78, d1c62af); E-8 `install` defaults to latest (ded24eb).
- 2026-11-09 Architect: D-011 (in-place sync), D-012 (AppImage deferred, divergence rulings).
- 2026-11-09 Engineer: E-6 code (082c1da); E-5 (73fa5bc, 74f4ca8); E-4 (58b2413).
- 2026-11-08 Architect: architect.md v1 + distribution.md; D-001..D-009.

---
DIGEST must be accurate or flagged as stale. If stale, invoke `skill: rein-digest`.
