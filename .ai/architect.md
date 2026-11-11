# ARCHITECT — rein-app (Architect-owned Blueprint, v1 · 2026-11-08)

> [!IMPORTANT]
> Source of truth for rein-app architecture. Written by the rein-app Architect from the
> three-repo contract handed by the `rein` Architect (`rein` D-150, commit db92fd6:
> `plans/rein-ecosystem-contract.md`, `plans/rein-app-plan.md`). Contract items are NOT
> decided here — open questions go to `rein · Architect`. Domain detail lives in
> `.ai/blueprints/` (§34 fragmentation); this file is the index + system contract.

## 0. Blueprint Index (JIT — load only what the task needs)
| Domain | File | Phase | Status |
|---|---|---|---|
| Distribution: installer, verifier, layout writer, profile manager, updater, WSL launcher | `.ai/blueprints/distribution.md` | 1 | ACTIVE — E-2..E-5, E-7, E-8, E-10..E-15 done (E-12..E-15 reviewed, D-015); E-18 (operator docs), E-19 (release rehearsal) open; E-6 awaits the first tag; E-9, E-16, E-17 blocked on rein/operator |
| Desktop: Tauri window, pane view, status, settings UI, bundled Node, signing | `.ai/blueprints/desktop.md` | 2 | ACTIVE (v1 2026-11-11, D-033) — the window: one pty running `rein start`, modal grid chrome, `config.toml`; E-34..E-39 registered. Windows window, bundled Node and signing are later revisions |

## 1. System Philosophy
- **Concept**: rein-app is the **shell**, not a rewrite. It installs, verifies, updates,
  supervises and (phase 2) draws. All Rein logic — the bash `rein` CLI, 17 Node MCP servers,
  hooks, safe-exec gate — stays in the `rein` repo and ships as a signed release artifact
  that the app consumes. (`rein` D-150; full Rust port rejected: 25k tested lines, POSIX
  security model, no user-visible gain.)
- **Value prop**: one command (`brew install rein`, a Windows installer, `.deb`/AppImage)
  that puts a verified Rein into the Rein home directory, wires Claude Code profiles
  correctly, and keeps them updated with rollback.
- **Users**: Rein operators on macOS, Linux and Windows-via-WSL2.
- **Non-goals (contract-fixed)**: native Windows without WSL2; bundling Claude Code (detect,
  offer the official installer); replacing tmux in phases 1–2; reimplementing any
  `rein doctor` check; carrying a copy of Rein logic.

## 2. Repo Position & Dependency Direction (contract §1)
```
rein (logic, v0.4.0) ──signed release──▶ rein-app (Rust shell)
                     ──release only for tests──▶ rein-plugins (marketplace) ──registered──▶ rein-app
```
- rein-app depends on: a signed `rein` release; a registered `rein-plugins` marketplace.
- Nothing in `rein` depends on rein-app. A change to the contract is a D-### in `rein` and
  a notice to this repo; we then revise this file, never the other way round.

## 3. What `rein` guarantees us (contract §2 — we build on, never re-derive)
1. **Release artifact** per tag `vX.Y.Z` (rein D-153): `rein-X.Y.Z.tar.gz` +
   `rein-X.Y.Z.tar.gz.minisig` (**minisign**; public key baked into the app at build time;
   `key_id` in the manifest so an old app fails with one line naming the app it needs) +
   `manifest.json` schema 1 (also inside the tarball): `{schema, version, min_node, key_id,
   layout: {mirrored[], operator_owned[]}, files: [{path, sha256, size, mode}]}`.
   Tarball == what `install-rein.sh` lays out. Full shape: `distribution.md` §Data Model.
2. **Stable Rein home layout** (`$HOME/.rein`): mirrored dirs `bin/ hooks/ mcp/ shared/ claude/
   contracts/ templates/ copilot/ scripts/ config/`; operator-owned per manifest
   `layout.operator_owned` (`accounts.json`, gate override, `secrets/`, `run/`, `memory/`,
   `incidents.ndjson`, `telemetry.sqlite`, `INSIGHTS.md`, `memory-palace.md`,
   `mcp/**/node_modules`, `mcp/**/package-lock.json`, `config/rein-workspace.txt`,
   `config/install-stamp.json`) + `VERSION`. Layout change == major version.
3. **Settings we may own per profile** (`<config_dir>/settings.json`) — **three keys**
   (rein D-155 r3 corrected the contract): `enabledPlugins`, `extraKnownMarketplaces`, `attribution`.
   **Never** `autoMode`, `permissions`, `defaultMode` (hard-deny, `rein` D-132) and **never
   `hooks`**: gate hooks live in each project's `.claude/settings.json`, written only by
   `rein init` / `rein sync`. We write no file inside a project.
4. **Machine-readable health**: `rein doctor --json` (P-50 adds it; shape
   `{version, status: ok|warn|fail, checks: [{id, status, line, fix|null}]}`), `rein state health --short` (exists).
5. **Panes**: `rein pane <role>` is the only role binding. We call it; we never mint a role token.
6. **Stable headless entry points**: `rein install|sync|init|doctor|start|pane|account|gate on|status`.
   Flags added, never removed, inside a major.

## 4. What we guarantee `rein` (contract §3 — acceptance criteria for every E-##)
- Install the **verified** artifact into the Rein home dir — the unpacked `manifest.json`
  included, `VERSION` written last — then run `rein install` and
  `rein doctor` **from the installed tree**. Never reimplement them; never edit the tree after unpack.
  `rein install` recognises the artifact (`manifest.json` version == `VERSION`) and does every
  post-copy step itself: tree check, toolchain pin, stamp, profile lines (rein D-155 r4).
- Windows == launcher on Windows, Rein inside WSL2; tmux and gate stay POSIX.
- tmux stays the multiplexer through phase 2; `rein pane` / `rein start` unchanged.

## 5. Architecture & Technical Strategy
- **Language/stack**: Rust (stable), Cargo workspace. Packaging via `cargo-dist`
  (mac/linux/windows) + Homebrew formula. Phase 2 UI: Tauri (Rust core, web UI, xterm.js);
  `ratatui` TUI is the fallback if no window is wanted.
- **Workspace crates**:
  - `rein-app` (bin) — CLI entry: `install`, `update`, `rollback`, `doctor`, `profiles`, `launch` (Windows).
  - `rein-release` (lib) — fetch + verify (signature, manifest, per-file sha256) + unpack. Fail closed.
  - `rein-profiles` (lib) — enumerate Claude profiles; read-merge-validate-write of the three allowed keys only.
- **Process model**: the app shells out to `rein <cmd>` from the installed tree with a clean
  env (unset `CLAUDECODE` as `.ai/providers.json` does). It parses `--json` outputs; text
  output is displayed verbatim, never parsed, until P-50 lands.
- **Trust boundaries**: (a) network → verifier (untrusted bytes until signature + hashes
  pass); (b) app → `$HOME/.rein` (write only via the layout writer, only under the manifest's
  mirrored dirs + `VERSION`, per file by temp + rename; everything else in the home is never
  opened — D-011);
  (c) app → `<config_dir>/settings.json` (allow-list of three keys; every other key, `hooks`
  included, preserved byte-for-byte); (d) app → `rein` CLI (invoke only the §3.6 entry points).
- **Runtime deps (phase 1)**: Node 22+, git, sqlite3, tmux, Claude Code — detected and
  reported; Node declared as a package dependency (brew/apt). Bundled pinned Node is phase 2.

## 6. Phase Plan
| Phase | Scope | Duration | Tier | Blueprint |
|---|---|---|---|---|
| 1 | CLI installer/updater, brew + cargo-dist, profile manager, WSL launcher | 2–4 wk | 2 (3 at hooks/settings/verifier) | `distribution.md` |
| 2 | Tauri window, project list, tmux pane embed, doctor-JSON status, settings UI, bundled Node, Apple notarization + Windows code-signing | 6–10 wk | 3 at settings/gate boundary | `desktop.md` (later) |
| 3 | App-drawn panes without tmux (needs `rein` contract change: `rein pane` accepts a pty); native Windows (only if `rein` ports the gate) | — | own rulings | — |

## 7. Task Map (Phase 1 bootstrap — IDs as registered in state.sqlite)
The plan's E-1..E-5 were registered as **E-2..E-6** because `rein init` had already minted
a placeholder E-1 (closed as superseded). Mapping:
| Plan | Registered | Tier | Depends on |
|---|---|---|---|
| E-1 cargo workspace + cargo-dist + CI matrix | **E-2** | 2 | — |
| E-2 `rein-release` fetch + verify, fail closed | **E-3** | 3 | E-2 |
| E-4 `rein-profiles` merge/validate, four keys only | **E-4** | 3 | E-2 |
| E-3 layout writer + `rein install`/`doctor` invocation | **E-5** | 3 | E-3 |
| E-5 Homebrew formula + cargo-dist release + updater/rollback + WSL launcher | **E-6** | 2 | E-4, E-5 |
Tier 3 tasks require `security_engineer` review before commit (ENGINEER.md mid-task trigger).
Execution order: E-2 → (E-3 ∥ E-4) → E-5 → E-6.
Follow-ups from the E-5/E-6 review (2026-11-09, D-011/D-012):
| Registered | Scope | Tier | Depends on |
|---|---|---|---|
| **E-7** | LayoutWriter + rollback reworked to the in-place sync (D-011) | 3 | E-5 |
| **E-8** | `install` without `--version` resolves the newest tag | 2 | E-6 code |
| **E-9** | layout-equality test vs `install-rein.sh` — BLOCKED on rein P-50 | 2 | E-7 |
| **E-10** | owner guard for install/update/rollback + "no snapshot" notice (D-013) | 3 | E-7 |
| **E-11** | owner guard as a compile-time precondition; test tightening (D-013 addendum) | 3 | E-10 |
E-7, E-8, E-10 and E-11 are DONE (2026-11-09).
Contract corrections from rein D-155 (D-014), registered 2026-11-09:
| Registered | Scope | Tier | Depends on |
|---|---|---|---|
| **E-12** | verifier: strict tarball member rules, files only under mirrored dirs | 3 | — |
| **E-13** | writer: shipped path wins, `manifest.json` in the home and in snapshots | 3 | E-12 |
| **E-14** | profiles: three keys, `hooks` removed | 3 | — |
| **E-15** | CLI: rollback runs `rein install` + `rein doctor`, exit 13, `doctor --json` | 2 | E-13 |
| **E-16** | trust root from a committed key file — DONE 2026-11-10 (0d573f6) | 3 | — |
| **E-17** | vendor rein's shared fixtures — BLOCKED on rein E-422 | 2 | — |
E-12..E-15 are DONE (2026-11-10) and accepted as built (D-015). Registered 2026-11-10:
| Registered | Scope | Tier | Depends on |
|---|---|---|---|
| **E-18** | `README.md` + `docs/operator-guide.md` (distribution.md §Components 9) | 2 | — |
| **E-19** | release rehearsal without publishing: `dist plan`, local host build, findings in DEVOPS.md | 2 | — |
| **E-20** | `doctor` exits 14 for its own failures, usage errors exit 64 (D-016) — before E-18 finishes | 2 | — |
| **E-21** | `LICENSE` + `NOTICE` in every release artifact (D-016) | 2 | — |
E-16 and E-18..E-21 are DONE (2026-11-10), reviewed in D-017. Registered after that review:
| Registered | Scope | Tier | Depends on |
|---|---|---|---|
| **E-22** | DONE (D-018) — release-key guard as one tested script, tag gate against rein's `main`, CODEOWNERS (D-017) | 3 | — |
| **E-23** | DONE, no change needed — comment in `wix/main.wxs` already ASCII | 1 | — |
| **E-24** | CODEOWNERS also covers `crates/rein-release/`, `Cargo.toml`, `Cargo.lock` (D-018) | 1 | — |
| **E-25** | DONE (D-019) — rehearse the `-rc` tag with `dist plan`; record what it needs in DEVOPS.md | 2 | — |
| **E-26** | DONE (D-021) — verifier refuses non-ASCII and case-colliding manifest paths (rein D-155 addendum 3, D-020) | 3 | — |
| **E-33** | 120 s limit on the scoped doctor run; backslash escaped (D-032) | 2 | — |
| **E-32** | DONE (D-032) — install verdict from `rein doctor --scope install --json` `status` (rein D-158, D-029) | 3 | — |
| **E-31** | manifest size limits at parse; set lookup in `under_mirrored`; linear sync scans (D-027) | 2 | — |
| **E-30** | profiles: `config_dir` terminal safety, shell-quoted dry-run line, duplicate-row warning (D-024) | 3 | — |
| **E-29** | refuse shipped case-variant of an owned pattern; owned matching folds ASCII case for protection; escaped error text (D-023) | 3 | — |
| **E-28** | profile enumeration: accounts row beats the built-in `personal`; only `default` reserved; terminal-safety name check (D-022) | 3 | — |
| **E-27** | sync: case-variant prunes before writes, post-sync existence check; `operator_owned` patterns ASCII-checked (D-021) | 3 | — |
E-9 now waits on rein E-417/E-418/E-420 and follows E-13. AppImage is out of Phase 1 (D-012). Phase 1 ships when
PR #1 is merged, rein publishes a signed release + public key (P-50), and the first tag builds.

Phase 2 window (D-033, `desktop.md`), registered 2026-11-11 on the operator's request while
E-6 still waits on rein:
| Registered | Scope | Tier | Depends on |
|---|---|---|---|
| **E-34** | `rein-desktop` crate: Tauri 2 window, closed capabilities, CSP, grid shell, `rein-app window` | 2 | — |
| **E-35** | `PtyHost` + xterm.js stage running `rein start` in the project directory | 3 | E-34 |
| **E-36** | modes, default keymap, which-key, command line | 2 | E-34 |
| **E-37** | `config.toml` + six themes + tiling-WM window options | 2 | E-36 |
| **E-38** | project store, start screen, picker, tabs | 2 | E-35, E-36 |
| **E-39** | `:checkhealth`, `:update`, `:rollback`, `:profiles` over the Phase 1 seams | 3 | E-38 |

## 8. Contract Rulings received from `rein` (P-50 → rein D-153, mirrored as rein-app D-003)
All five open questions are closed; `distribution.md` carries the detail. Summary:
1. Signature: **minisign**, public key baked in at build; rotation == app release; `key_id` check first.
2. Manifest: **schema 1** as in §3.1; reject unknown schema, out-of-layout path, unlisted tarball file.
3. `rein doctor --json`: shape in §3.4; until it ships, pass text through verbatim, parse nothing.
4. `accounts.json` (rein D-153 addendum 2, 2026-11-10 — replaces the withdrawn "everything
   else opaque, preserved byte-for-byte" clause): closed shape
   `{"accounts": {"<name>": {"config_dir", "added"}}}`, no other key at either level; rein's
   `accounts.py` is the only writer. rein-app **never writes the file** — a future account
   UI calls `rein account add` / `rein account remove`. As a reader it relies on
   `config_dir` (absolute) only, ignores unknown row keys, and treats `added` as optional.
   Docs land as rein `docs/accounts.md` + `docs/contract/accounts.schema.json` (rein E-422).
5. Upgrade semantics: `config/` is **mirrored**, not preserved (two files kept); operator-owned
   list is read from the manifest's `layout.operator_owned`, never hard-coded. Reference stays `install-rein.sh`.
Still pending from `rein` (not blocking): the P-50 E-## that publish the release workflow,
manifest generator, `doctor --json`, and accounts docs; rein-plugins' first tag on `stable`.
**rein D-155 (2026-11-09, P-50 closed; mirrored as rein-app D-014)** — normative text is rein
`.ai/blueprints/release-artifact.md`; `plans/rein-ecosystem-contract.md` §2–3 amended:
- r1 tarball: `manifest.json` first, then regular files only in manifest order; no directory
  entries, links or `VERSION` member; signature in minisign's prehashed form.
- r2 layout: an installer writes only inside `layout.mirrored` plus `manifest.json` and
  `VERSION`; `operator_owned` protects from deletion only and a shipped path wins
  (`mcp/orchestrator-mcp/package-lock.json`); `shared/toolchain.sh` joins the list (fourteen).
  This answers our question (a) below.
- r3 profiles: three keys, never `hooks` (§3.3 above).
- r4 `rein install` finishes an unpacked artifact; exit 3 = tree mismatch or gate pin,
  exit 1 = Node floor or missing `/bin/bash -p`.
- r5 doctor document: `"schema": 1`, optional `subject` per check, `(id, subject)` unique.
- r6 key custody: the operator generates the pair; the public key is committed in rein as
  `docs/contract/release-key.pub`, the file we copy. r7: npm dependencies are outside the signature.
- Shared fixtures arrive with rein E-418 and E-422 (`tests/fixtures/release/`).
**rein D-155 addendum (2026-11-09, rein commit 3166bb3)** — answers to our D-014 notice:
`rein install` in artifact mode removes a retired server's `mcp/<name>/` and the legacy
mirrors (the app's writer leaves them); a `manifest.json` that disagrees with `VERSION` is
`[INSTALL_INCOMPLETE]`, exit 3 (our write order hooks → `manifest.json` → `VERSION` is
confirmed, no shared lock); `rein install` is safe on a rolled-back tree (stamp rewritten,
`source: "restored"` for a clone-install snapshot); rein's writer never emits an extension
record, directory entry or link. No open question remains with rein.
Questions sent to `rein` on 2026-11-09 (D-011) — both answered, (a) by D-155 r2, (b) by the addendum:
(a) confirm that everything outside `layout.mirrored` is untouched by an install and that
`operator_owned` decides only inside mirrored dirs — or extend the list (`approvals.sqlite*`,
`cache.sqlite*`, `telemetry.sqlite-wal/-shm`, `ci/`, `privacy/`, `trash/`, `clean.log`);
(b) who removes a retired MCP server's directory and the legacy mirrors under an app install.

## 9. Error Handling & Edge Cases (system level)
- Verification failure of any kind → nothing is written to `$HOME/.rein`; exit non-zero with
  the failing check named. The release is unpacked and verified outside the home; the sync
  then replaces files one by one (each atomic) and restores from its snapshot if it cannot
  finish — the same guarantee `install-rein.sh` gives, not a whole-tree rename (D-011).
- `rein doctor` reports ✗ → app shows doctor's own fix line; app adds no remediation of its own.
- Profile file unparseable → refuse to write that profile, continue others, exit non-zero.
- Offline → `update` says so and exits; installed tree untouched.
- Rollback keeps N=2 snapshots of the release payload (never operator data); `rollback`
  syncs the newest one back and re-runs `rein doctor`.
- rein running during an install is normal: operator data is never touched and no hook is
  ever missing, so there is no "stop rein first" check.

## 10. Development Cycle (Plan-Build-Test)
1. **Plan**: Architect writes/updates this file + `.ai/blueprints/*.md`, registers E-## via
   `task-planner`, hands off via `rein-handoff`.
2. **Build**: Engineer implements per blueprint; Tier 3 → `security_engineer`.
3. **Test**: `cargo test` per crate; fixture release server; Tester role (`skill: rein-test`).

## 30. Bootloader Resilience
- `rein-preflight` is the only preflight path (D-075); its Bash fallback reads `state.json`;
  then ENGINEER.md Emergency Recovery; last resort `cat .ai/TASKS.md`.

## 33. Token Economics — JIT Read Limits
- 6-file JIT limit per task. DIGEST first, then TASKS.md, then the ONE domain blueprint the
  task names. Never `ls -R`, never read `src/**` without a named file.

## 34. Architectural Fragmentation
- This file is the index (~150 lines). Domain detail in `.ai/blueprints/<domain>.md`.
  Load one blueprint per task, never all.
