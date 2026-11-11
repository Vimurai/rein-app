# DIGEST (Token Saver Cache) — updated 2026-11-11 19:12 UTC by Architect

## Triad Health
- Stage: Executing (Phase 1 code complete and proven against rein's real artifact; the first tag waits on rein's signed release and the operator)
- Architect Focus: E-32 reviewed (D-032); next is review of E-33, the rc version-bump task when the operator says go, then the Phase 2 blueprint (`desktop.md`)
- Engineer Focus: E-33 — 120 s limit on the scoped doctor run, backslash escaping (T2)

## Current snapshot
- Product: rein-app — Rust shell for Rein: installs a signed `rein` release into `$HOME/.rein`, runs `rein install` and the install-scope doctor from it, manages Claude profile settings (3 allowed keys), updates/rolls back; phase 2 wraps it in a Tauri window. Never contains Rein logic.
- Stack: Rust workspace (`rein-app` bin, `rein-release` lib, `rein-profiles` lib), cargo-dist 0.34.0 + Homebrew tap + msi + `.deb`, WSL2 launcher on Windows; phase 2 Tauri + xterm.js over tmux.
- Contract: decided in `rein` (D-150 / D-153 / D-155 / D-158), mirrored here (D-001, D-003, D-014, D-020, D-025, D-029). rein publishes `docs/app-contract.md`, three schemas under `docs/contract/`, and shared fixtures under `tests/fixtures/release/` — all vendored here from rein d84dd17 / cac86a4 (E-17, E-32).
- **Proven end to end (E-9, E-32):** against rein `main` cac86a4, rein's real artifact (252 files) passes our verifier; a rein-app install equals `install-rein.sh` (documented differences only), both cases; `install` exits 0. The test (`crates/rein-app/src/layout_equality.rs`, gated on `REIN_CHECKOUT`) is **local only** — a GitHub Ubuntu runner cannot pin Node, so it cannot assert exit 0 there (D-032). Run it against the rein release commit before every tag.
- **Install verdict (D-029, D-032):** after install/update/rollback: `rein install`, then `rein doctor --scope install --json`; `status` fail → exit 3; warn/ok → 0; no valid document → 14. The app never recomputes the verdict or branches on a check id. `rein-app doctor` stays a passthrough.
- **Trust root:** `docs/contract/release-key.pub` committed and baked (key id 5C9C75A7C8FA8056), byte-identical to rein `main`'s (verified). Tag gate: every publishing release run fetches rein main's copy and fails unless identical. One tested guard script; CODEOWNERS advisory (one human account). The production key signs rein releases and nothing else. Secret key is in the operator's keeping; no agent has read it.
- **Verifier rules added 2026-11-10/11:** printable-ASCII paths; no ASCII-case collisions (D-020); no shipped path or directory spelling that is a case variant of an owned pattern, per pattern (D-023 r1, D-026); owned matching itself stays case-sensitive (D-025, rein addendum 3 r9); size limits 128 mirrored / 256 patterns / 100000 files / 1024 bytes / 64 segments (D-027; rein today 10 / 14 / 252 / 55 / 4, and rein warns before half a limit); all manifest- and rein-derived text escaped.
- **Sync:** case-variant prune entries removed before the writes + post-sync existence check (D-021); only mirrored dirs plus `manifest.json`/`VERSION` are ever written or deleted (rein addendum 3 r10).
- **Profiles:** accounts rows before the built-in `personal`, only `default` reserved (D-022); a row with an unsafe `config_dir` is skipped, dry-run line shell-quoted (D-024); rein's profile-settings table read as permissions (D-030); `accounts.json` is never written by the app (rein D-153 addendum 2).
- **Release candidate (D-019):** an rc tag needs the version set to `0.1.0-rc.1` (workspace version, two pins in `crates/rein-app/Cargo.toml`, `Cargo.lock`) — a task opened only on the operator's go, then set back for the final. An rc skips Homebrew. Uninstall an rc `.msi` before the final. Operator tag sequence: `.ai/DEVOPS.md` "E-25".
- Open queue: E-33 (T2). DONE and accepted since 2026-11-10: E-16, E-18..E-32 (E-23 no change), E-9, E-17. E-6 (first tag) waits on: rein E-419 (signing) → E-424 (signed v0.5.0), the operator's rc call, `HOMEBREW_TAP_TOKEN` (final tag only).
- Operator items open: tag ruleset (`v*` by the operator only) on rein-app; on rein the `release` environment with `REIN_RELEASE_KEY` and a tag restriction; delete `/tmp/claude-1000/e22-pentest.TTzCCF`; confirm Apache-2.0 (D-004); say when Phase 2 starts.
- Known risks: nothing in hosted CI exercises a real rein artifact; `.msi`, macOS and `.deb` release jobs unexercised until a tag; no signed rein release exists yet.
- Process: the Architect commits `.ai` with `git commit --only <paths>` (a plain commit once swept the Engineer's staged source). The Architect write gate blocks files outside the project, including the memory directory.
- Repo: `github.com/Vimurai/rein-app` (public, hosted CI green on 3 OSes at 391a4b5). Blueprints: `.ai/architect.md` (index), `.ai/blueprints/distribution.md` (Phase 1, revised through D-032). `desktop.md` not yet written.

- **Phase 2 started (D-033, 2026-11-11):** `.ai/blueprints/desktop.md` v1 — the window ("Modeline") is a separate crate `rein-desktop` (Tauri 2 + xterm.js): one pty per tab running `rein start` (tmux draws the panes, the app names no session), nvim-style modes (TERMINAL / NORMAL / COMMAND, escape `C-\ C-n`), chrome on the cell grid (tabline, statusline, command line, floating overlays), a closed action list, one read-only `config.toml`, six themes, tiling-WM options. Web view untrusted: closed IPC table, no shell/fs/http plugin. Linux + macOS only in this revision. Queue: E-34 open; E-35..E-39 blocked in order. Three non-blocking questions for rein are listed in the blueprint and not yet sent.

## Recent changes (last 10)
- 2026-11-11 Architect: D-033 — Phase 2 window blueprint (`desktop.md`); E-34..E-39 registered.
- 2026-11-11 Architect: D-032 — E-32 accepted; no recomputed verdict; layout test stays local; E-33 registered.
- 2026-11-11 Engineer: E-32 install verdict from the scoped doctor (391a4b5, CI ×3 green; layout test exit 0 at rein cac86a4).
- 2026-11-11 Architect: D-029 (mirror rein D-158), D-030 (profile-settings table), D-031 (layout case 2), D-028 (E-9 first run).
- 2026-11-11 Engineer: E-9 green at rein f7b3554 (be06b2e); E-17 shared fixtures (9c4a0d0); E-31 size limits (7273f32).
- 2026-11-11 Architect: D-027 + addendum — manifest size limits.
- 2026-11-10 Architect: D-020..D-026 — case rules, sync prune order, profile enumeration, `config_dir` safety; D-025 withdrew D-023 r2.
- 2026-11-10 Engineer: E-26..E-30 (24ed040, 9adfd72, 7df5c73, f298c5b, df2bcb6).
- 2026-11-10 Architect: D-017..D-019 — tag gate, guard script, CODEOWNERS, rc rehearsal rulings.
- 2026-11-10 Engineer: E-16, E-18..E-25.
- 2026-11-08/09: architect.md v1 + distribution.md; D-001..D-014; E-2..E-15.

---
DIGEST must be accurate or flagged as stale. If stale, invoke `skill: rein-digest`.
