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
| Distribution: installer, verifier, layout writer, profile manager, updater, WSL launcher | `.ai/blueprints/distribution.md` | 1 | ACTIVE — E-2..E-6 |
| Desktop: Tauri window, pane view, status, settings UI, bundled Node, signing | `.ai/blueprints/desktop.md` | 2 | NOT WRITTEN — authored when Phase 1 ships |

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
1. **Release artifact** per tag `vX.Y.Z`: `rein-X.Y.Z.tar.gz` + detached signature
   (minisign **or** sigstore — pending `rein` P-50) + `manifest.json` {version, min Node,
   file list with sha256 per file, install layout}. Tarball == what `install-rein.sh` lays out.
2. **Stable Rein home layout** (`$HOME/.rein`): `bin/ hooks/ mcp/ shared/ scripts/ config/
   secrets/ run/ accounts.json gate-override.json` (+ `VERSION`). Layout change == major version.
3. **Settings we may own per profile** (`<config_dir>/settings.json`): `hooks` (via
   `rein install`, byte-identical), `enabledPlugins`, `extraKnownMarketplaces`, `attribution`.
   **Never** `autoMode`, `permissions`, `defaultMode` (hard-deny, `rein` D-132).
4. **Machine-readable health**: `rein doctor --json` (pending P-50), `rein state health --short` (exists).
5. **Panes**: `rein pane <role>` is the only role binding. We call it; we never mint a role token.
6. **Stable headless entry points**: `rein install|sync|init|doctor|start|pane|account|gate on|status`.
   Flags added, never removed, inside a major.

## 4. What we guarantee `rein` (contract §3 — acceptance criteria for every E-##)
- Install the **verified** artifact into the Rein home dir, then run `rein install` and
  `rein doctor` **from the installed tree**. Never reimplement them; never edit the tree after unpack.
- Windows == launcher on Windows, Rein inside WSL2; tmux and gate stay POSIX.
- tmux stays the multiplexer through phase 2; `rein pane` / `rein start` unchanged.

## 5. Architecture & Technical Strategy
- **Language/stack**: Rust (stable), Cargo workspace. Packaging via `cargo-dist`
  (mac/linux/windows) + Homebrew formula. Phase 2 UI: Tauri (Rust core, web UI, xterm.js);
  `ratatui` TUI is the fallback if no window is wanted.
- **Workspace crates**:
  - `rein-app` (bin) — CLI entry: `install`, `update`, `rollback`, `doctor`, `profiles`, `launch` (Windows).
  - `rein-release` (lib) — fetch + verify (signature, manifest, per-file sha256) + unpack. Fail closed.
  - `rein-profiles` (lib) — enumerate Claude profiles; read-merge-validate-write of the four allowed keys only.
- **Process model**: the app shells out to `rein <cmd>` from the installed tree with a clean
  env (unset `CLAUDECODE` as `.ai/providers.json` does). It parses `--json` outputs; text
  output is displayed verbatim, never parsed, until P-50 lands.
- **Trust boundaries**: (a) network → verifier (untrusted bytes until signature + hashes
  pass); (b) app → `$HOME/.rein` (write only via the layout writer, once, atomically);
  (c) app → `<config_dir>/settings.json` (allow-list of four keys; every other key preserved
  byte-for-byte); (d) app → `rein` CLI (invoke only the §3.6 entry points).
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

## 8. Open Questions — owed by `rein` (P-50), NOT decided here
1. Signature scheme: minisign vs sigstore. Until decided, `rein-release` exposes a
   `SignatureVerifier` seam with no production backend; the fixture server tests the
   manifest/sha256 path and the fail-closed behaviour of a missing/invalid signature.
2. `rein doctor --json` full mode vs app parses text. Until decided: display text verbatim.
3. Where the `accounts.json` schema is documented. Observed shape today (informational, not
   a contract): `{ "accounts": { "<name>": { "config_dir": "<abs path>", "added": "YYYY-MM-DD" } } }`.
   The profile manager treats any other key as opaque and preserves it.
4. Manifest format details (file-list encoding, layout field) — P-50.

## 9. Error Handling & Edge Cases (system level)
- Verification failure of any kind → nothing is written to `$HOME/.rein`; exit non-zero with
  the failing check named. No partial installs: unpack to a staging dir, verify, then atomic rename.
- `rein doctor` reports ✗ → app shows doctor's own fix line; app adds no remediation of its own.
- Profile file unparseable → refuse to write that profile, continue others, exit non-zero.
- Offline → `update` says so and exits; installed tree untouched.
- Rollback keeps N=2 trees; `rollback` swaps symlink/dir and re-runs `rein doctor`.

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
