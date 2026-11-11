# DIGEST (Token Saver Cache) — updated 2026-11-10 19:05 UTC by Architect

## Triad Health
- Stage: Executing (Phase 1 code complete; the first tag waits on rein and the operator)
- Architect Focus: E-24/E-25 reviewed (D-019); waiting on rein's reply and the operator's rc call; then the Phase 2 blueprint (`desktop.md`) when the operator says go
- Engineer Focus: E-27 sync case-variant prune ordering + post-sync check (T3)

## Current snapshot
- Product: rein-app — Rust shell for Rein: installs a signed `rein` release into `$HOME/.rein`, runs `rein install`/`doctor` from it, manages Claude profile settings (3 allowed keys), updates/rolls back; phase 2 wraps it in a Tauri window. Never contains Rein logic.
- Stack: Rust workspace (`rein-app` bin, `rein-release` lib, `rein-profiles` lib), cargo-dist + Homebrew tap + msi + `.deb`, WSL2 launcher on Windows; phase 2 Tauri + xterm.js over tmux.
- Contract: `rein` D-150 / D-153 / **D-155** (mirrored D-001, D-003, D-014). Normative text: rein `.ai/blueprints/release-artifact.md`. Contract items are decided in `rein`, not here.
- Shipped on master (local, not pushed past 0d573f6's CI run): strict verifier (E-3, E-12), profiles on three keys (E-4, E-14), in-place LayoutWriter + snapshots + shipped-path-wins + `manifest.json` in the home (E-7, E-13), `install`/`update`/`rollback`/`doctor [--json]`/`launch` with exit 13/14/64 (E-5, E-8, E-15, E-20), owner guard (E-10, E-11), release pipeline (PR #1), **trust root baked from committed `docs/contract/release-key.pub`** (E-16, key id 5C9C75A7C8FA8056), `README.md` + `docs/operator-guide.md` (E-18), LICENSE + NOTICE in every artifact (E-21).
- **Release key**: the operator generated the minisign pair 2026-11-10. Secret key is in the operator's iCloud Drive (moved by operator instruction, never read by an agent). The public key is in this repo (committed) and in rein's working tree **uncommitted** on branch `chore/closures-2026-11-09`.
- **D-017 (built, E-22)**: the key-file guard is one tested script (26-case corpus on 3 OSes); every release run that can publish fetches rein `main`'s `release-key.pub` and fails unless it is byte-identical to ours (so no tag can build before rein commits the key); CODEOWNERS; the production key signs no test fixture.
- **D-018**: E-22/E-23 accepted; CODEOWNERS widens to `crates/rein-release/`, `Cargo.toml`, `Cargo.lock` and is advisory with one human account; the binding control is an operator-set tag ruleset (`v*` tags by the operator only), which also covers a tag on a pre-E-22 commit.
- D-014 (rein D-155 mirror), D-015 (E-12..E-15 review), D-016 (§API matches the binary; exit 6 retired) stand; D-011, D-012, D-013 stand.
- **D-019**: E-24/E-25 accepted. An rc tag needs the version set to `0.1.0-rc.1` (workspace version, two pins in `crates/rein-app/Cargo.toml`, `Cargo.lock`) — made only on the operator's go, then set back for the final. No WiX change: uninstall an rc `.msi` before the final. An rc skips Homebrew, so needs no tap token. Operator tag sequence: `.ai/DEVOPS.md` "E-25".
- **rein's plan (rein D-156, reply 2026-11-10)**: key is on rein's remote `main` (a1c3af0); Architect verified 2026-11-10 that its git blob id equals ours (1c893ca9…), so the tag gate's comparison will pass. Order: E-417 → E-418 → E-420 (unblocks our E-9) → E-421 → E-422 (unblocks our E-17) → E-419 → signed release **v0.5.0** (rein E-424; assets `rein-0.5.0.tar.gz`, `.minisig`, `manifest.json`; operator tags, no agent signs; v0.4.0 gets no assets). rein E-425 adds a cargo step to `rein ci run`. rein's Architect will message after E-423, E-420, E-422. Operator still owes rein: GitHub environment `release` with secret `REIN_RELEASE_KEY`, a tag restriction.
- **D-020**: mirrors rein D-155 addendum 3 — the verifier refuses non-printable-ASCII paths and ASCII-case-colliding paths (directory prefixes count) at manifest parse, exit 2 (E-26).
- **D-021**: E-26 accepted (24ed040). A case-only rename across releases deleted the shipped file on a case-insensitive home (prune after write) — E-27 removes case-variant prune entries before the writes and checks every shipped path after the sync; `operator_owned` patterns join the ASCII rule. rein D-153 addendum 2 mirrored: `accounts.json` closed shape, rein-app never writes it.
- **D-022**: `profiles list` falsely warned that account `personal` was skipped — E-28 lists accounts rows before the built-in `~/.claude-personal` (row's `config_dir` wins), reserves only `default`, and checks names for terminal safety only.
- **D-023**: E-27 (9adfd72) cleared for sign-off. Its security review found operator-data loss on a case-insensitive home when a shipped path is a case variant of an owned pattern — E-29 refuses that at manifest parse, folds ASCII case in the owned-protection decision, and escapes all manifest-derived error text.
- rein mirrors the shipped-side half of D-023 in its generator (rein D-155 addendum 3 r8, built in rein E-422): a shipped path matching an owned pattern only after ASCII-lowercasing is refused at generation. The live-side protective fold stays ours. Case-only renames across releases and 8.3 names are open on rein's side (addendum 3 r7, convention only).
- **D-025**: D-023's protective case fold is withdrawn — rein's conformance table rules owned matching case-sensitive (`insights.md` vs `INSIGHTS.md` → deleted). E-29 ships the parse refusal, symlink Conflict and escaping only. rein confirmed (D-155 addendum 3 r9): no fold. rein also found its own table wrong (r10): two top-level unshipped rows said `deleted`, but an installer deletes only inside mirrored dirs (+ `manifest.json`, `VERSION`). Our scan walks only mirrored roots (checked by the Architect in `sync.rs`), so `~/.rein/ci/` and `trash/` are safe; the interim fixture copy and the pure `plan` conformance follow the corrected table with E-17.
- **D-026**: E-29 also refuses at parse a mirrored name or shipped directory prefix that is a case variant of an owned pattern's leading segments (`mirrored: ["Shared"]` beside `shared/toolchain.sh` pruned an exactly-owned file on a case-insensitive home); the fold-only test is per pattern.
- Process: the Architect commits `.ai` with `git commit --only <paths>` — a plain `git commit` swept the Engineer's staged source into 64f38b7 (split by the Engineer into a254315 / 9adfd72).
- **D-024**: E-28 (7df5c73) accepted. Control bytes in an accounts.json `config_dir` reached the terminal raw since E-4 — E-30 skips such rows, shell-quotes the dry-run command line, warns on duplicate rows, and notes an unusual target directory.
- **rein `main` at 3f68eff** holds rein E-417/E-418/E-420 (staged tree as artifact source, `rein release manifest` + keyless `rein release verify`, `rein install` finishing an unpacked artifact) — **E-9 unblocked 2026-11-10**, run against that commit from a throwaway copy of rein. Not yet on rein `main`: E-421 (`doctor --json`), E-422 (contract docs, schemas, `profile-settings-cases.json`, case rules in the generator), E-419 (signing). E-17 stays blocked on E-422.
- Open queue: E-29 (T3), E-30 (T3), E-9 (T2). E-27 (9adfd72) and E-28 (7df5c73) are committed locally, awaiting the operator's push approval. E-6 (first tag) waits on: `HOMEBREW_TAP_TOKEN`, rein committing the public key on `main`, a signed rein release. BLOCKED externally: E-17 (rein E-422).
- Phase 1 ships when: rein commits the key and publishes a signed release (rein E-417..E-422), the operator sets `HOMEBREW_TAP_TOKEN`, and the first tag builds (E-6; an `-rc` tag first is recommended).
- Known risks: no rein release exists yet; `.msi`, macOS and `.deb` release jobs unexercised until a tag; `rein ci run` has no cargo step (framework gap, not yet raised with rein — no rein session running).
- Repo: `github.com/Vimurai/rein-app` (public, hosted CI green on 3 OSes). License Apache-2.0 (D-004) — awaiting operator confirmation.
- Blueprints: `.ai/architect.md` (index), `.ai/blueprints/distribution.md` (Phase 1, revised 2026-11-10 for D-011..D-017). `desktop.md` not yet written.

## Recent changes (last 10)
- 2026-11-10 Architect: D-018 — E-22/E-23 accepted; CODEOWNERS scope + tag ruleset ruling; E-24, E-25 registered.
- 2026-11-10 Engineer: E-22 guard script, tag gate, CODEOWNERS (6450f22, 70c55b9; hosted CI green x3); E-23 needed no change.
- 2026-11-10 Architect: D-017 — E-16/E-18/E-20/E-21 accepted; tag gate, single guard script, CODEOWNERS; E-22, E-23 registered; stale key text in distribution.md fixed.
- 2026-11-10 Engineer: E-16 trust root from the committed key (0d573f6), E-18 docs (b82b3be), E-21 license files (6bf6244), E-20 exit 14/64 (dd57a63); hosted CI green x3. Operator key files moved/copied as instructed.
- 2026-11-10 Architect: D-014 addendum — operator key generated, E-16 opened before rein's commit.
- 2026-11-10 Architect: D-016 — §API matches the binary; E-20, E-21 registered.
- 2026-11-10 Engineer: E-19 release rehearsal — `dist plan` OK x5, host build OK; findings in DEVOPS.md.
- 2026-11-10 Architect: D-015 — E-12..E-15 accepted as built; E-18, E-19 registered.
- 2026-11-10 Engineer: E-12 (5dad595), E-14 (fbbc951), E-13 (2085e51), E-15 (8d4837d).
- 2026-11-09 Architect: rein D-155 mirrored (D-014 + addendum); E-12..E-17 registered.
- 2026-11-09 Engineer/Architect: E-7, E-8, E-10, E-11; D-011, D-012, D-013.
- 2026-11-08 Architect: architect.md v1 + distribution.md; D-001..D-009.

---
DIGEST must be accurate or flagged as stale. If stale, invoke `skill: rein-digest`.
