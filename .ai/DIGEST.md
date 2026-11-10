# DIGEST (Token Saver Cache) — updated 2026-11-10 14:00 UTC by Architect

## Triad Health
- Stage: Executing (D-155 corrections E-12..E-15 shipped and accepted as built — D-015; the unblocked work is docs + a release rehearsal)
- Architect Focus: E-12..E-15 reviewed (D-015); next is review of E-18/E-19, then the Phase 2 blueprint (`desktop.md`) when the operator says go
- Engineer Focus: E-18 operator docs (T2) ∥ E-19 release rehearsal without publishing (T2)

## Current snapshot
- Product: rein-app — Rust shell for Rein: installs a signed `rein` release into `$HOME/.rein`, runs `rein install`/`doctor` from it, manages Claude profile settings (3 allowed keys), updates/rolls back; phase 2 wraps it in a Tauri window. Never contains Rein logic.
- Stack: Rust workspace (`rein-app` bin, `rein-release` lib, `rein-profiles` lib), cargo-dist + Homebrew tap + msi + `.deb`, WSL2 launcher on Windows; phase 2 Tauri + xterm.js over tmux.
- Contract: `rein` D-150 / D-153 / **D-155** (mirrored D-001, D-003, D-014). Normative text: rein `.ai/blueprints/release-artifact.md`. Contract items are decided in `rein`, not here.
- Shipped on master: verifier with the strict tarball reader (E-3, E-12), profile manager on three keys (E-4, E-14), in-place LayoutWriter + payload snapshots + shipped-path-wins + `manifest.json` in the home (E-7, E-13), `install`/`update`/`rollback` (runs `rein install` + `rein doctor`; exit 13 for a failed `rein install`)/`doctor [--json]`/`launch` (E-5, E-6 code, E-8, E-15), owner guard as a compile-time proof (E-10, E-11), release pipeline (PR #1 merged).
- **D-014 (rein D-155)**: tarball = `manifest.json` first + regular files only (strict reader); a shipped path wins over an `operator_owned` pattern (patterns protect from deletion only); the unpacked `manifest.json` stays at the top of the home and `VERSION` is written last; `rein install` finishes an artifact itself; profiles take THREE keys, never `hooks`; rollback runs `rein install` + `rein doctor`; the baked key will come from a committed copy of rein's `docs/contract/release-key.pub`.
- **D-015 (review of E-12..E-15, no code follow-up)**: snapshot `layout.json` carries `shipped_owned`; a directory at `manifest.json`/`VERSION` is a preflight refusal; the verifier runs a header-only first pass; a newly shipped owned-matching path may outlive a failed sync or a rollback (accepted; recovery is `install --version`); settings are preserved by value and key order, not by byte; `rein ci run` checks nothing in this repo — test evidence is `cargo test --workspace` + hosted CI.
- D-011 (in-place sync, nothing outside the mirror is opened), D-012 (AppImage out of Phase 1), D-013 (target symlinks, snapshot trust, owner guard) stand.
- **D-016 (E-18/E-19 findings)**: §API now matches the binary — exit 1 = refused/other error on every command, global `--home`, `profiles apply --dry-run`, `launch` exit 4; exit 6 retired; `doctor` exits 14 (not 1) for its own failures and a usage error exits 64 (not 2) — E-20; `LICENSE` + `NOTICE` in every artifact — E-21; rein's `docs/app-contract.md` is named unlinked until rein publishes it.
- Open queue: E-20 (exit codes 14 / 64) → E-18 (`README.md` + `docs/operator-guide.md`); E-21 (license files in artifacts). E-19 done (rehearsal; findings in DEVOPS.md). E-6 (first tag — needs `HOMEBREW_TAP_TOKEN` and a release key). BLOCKED externally: E-9 (rein E-417/E-418/E-420), E-16 (operator's key committed in rein), E-17 (rein E-422).
- Phase 1 ships when: the operator generates the minisign pair, rein publishes a signed release (rein E-417..E-422), E-16 bakes the key, and the first tag builds (E-6).
- Known risks: no rein release exists yet; release jobs unexercised (E-19 rehearses what runs without a tag); no README / operator docs yet (E-18); `rein ci run` has no cargo step (framework gap, to raise with rein).
- Repo: `github.com/Vimurai/rein-app` (public, hosted CI green on 3 OSes). License Apache-2.0 (D-004) — awaiting operator confirmation.
- Blueprints: `.ai/architect.md` (index), `.ai/blueprints/distribution.md` (Phase 1, revised 2026-11-10 for D-011..D-015). `desktop.md` not yet written.

## Recent changes (last 10)
- 2026-11-10 Architect: D-016 — ruled on E-18's seven §API mismatches and E-19's findings; §API table revised; E-20, E-21 registered.
- 2026-11-10 Engineer: E-19 release rehearsal done — `dist plan` OK x5, host build OK; NOTICE in no artifact; repo has no variables/secrets set.
- 2026-11-10 Architect: reviewed E-12..E-15 — accepted as built, D-015 (seven rulings); distribution.md revised (§Components 2/3/6/9, §Data Model Snapshot, §Test Plan); E-18, E-19 registered.
- 2026-11-10 Engineer: E-12 strict tarball reader (5dad595), E-14 three profile keys (fbbc951), E-13 shipped path wins + `manifest.json` in the home (2085e51), E-15 rollback re-installs / exit 13 / `doctor --json` (8d4837d); hosted CI green x3.
- 2026-11-09 Architect: rein D-155 addendum mirrored (D-014 addendum) — retired-server removal is `rein install`'s, write order confirmed, E-9 comparison point fixed.
- 2026-11-09 Architect: rein D-155 mirrored (D-014); distribution.md and architect.md revised; E-12..E-17 registered; E-9/E-16/E-17 marked BLOCKED.
- 2026-11-09 rein: P-50 closed (D-155, commit 316218b) — three contract corrections for rein-app.
- 2026-11-09 Operator: PR #1 (release pipeline) merged.
- 2026-11-09 Architect: reviewed E-11; D-013 addendum 2 — owner-guard work closed.
- 2026-11-09 Engineer: E-11 `OwnerChecked` proof (3e466c0, a8332e9); E-10 owner guard (5b8bc6b); E-7 in-place sync (1ac3f78, d1c62af); E-8 (ded24eb).
- 2026-11-09 Architect: D-011 (in-place sync), D-012 (AppImage deferred), D-013 (symlinks, snapshot trust, owner guard).
- 2026-11-08 Architect: architect.md v1 + distribution.md; D-001..D-009.

---
DIGEST must be accurate or flagged as stale. If stale, invoke `skill: rein-digest`.
