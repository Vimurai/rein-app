# BRIEF (Living product brief) — rein-app

## Product Summary
- What is this? The Rust **shell** around Rein: installer, verifier, updater, profile manager, and (phase 2) desktop window. The Rein logic stays in the `rein` repo and ships as a signed release the app consumes.
- Who is it for? Rein operators on macOS, Linux, and Windows via WSL2.
- What problem does it solve? One package-manager command yields a byte-verified Rein install, correctly wired Claude Code profiles, and safe updates with rollback.

## Goals
- `brew install rein` / Windows installer / `.deb`+AppImage produce an identical `$HOME/.rein` to `install-rein.sh`.
- Fail closed on any unverified artifact; never a partial install.
- Edit only the four contract-allowed settings keys per Claude profile; preserve everything else.
- Phase 2: a window that embeds the tmux session and renders `rein doctor --json`.

## Non-goals
- Native Windows without WSL2; bundling Claude Code; replacing tmux (phases 1–2); any copy or reimplementation of Rein logic or doctor checks.

## Constraints
- Security: signed release + per-file sha256; closed allow-list for settings keys (never `autoMode`/`permissions`/`defaultMode`); only contract §2.6 `rein` entry points invoked; no role-token minting.
- Performance: install < 10 s excluding download; < 100 MB memory; streamed hashing.
- Compliance: none beyond Apple notarization and Windows code-signing (phase 2).
- Deployment: cargo-dist + Homebrew tap; GitHub releases.

## UX Notes (Architect-owned)
- Target platforms: macOS (arm64/x64), Linux (x64/arm64), Windows 10/11 launcher → WSL2 Ubuntu.
- Key flows: install → verify report → doctor output; update → notes → confirm; profiles list/apply/undo; rollback.
- Accessibility: CLI output readable without color; phase 2 UI follows platform conventions.
- Performance budgets (frontend): phase 2 — pane attach < 500 ms, status refresh < 1 s.

## SEO/Content Notes (Architect-owned)
- Not applicable (desktop/CLI product).

## Frontend Notes (Architect-owned)
- Framework: phase 2 Tauri (Rust core, web UI, xterm.js); `ratatui` fallback.
- State approach: thin — state of record is the installed tree and profile files.
- Routing: n/a phase 1.
- UI patterns: status table mirrors doctor's own ✓/✗ lines and fix text.

## Architecture Notes (Engineer-owned)
- Modules/boundaries: see `.ai/architect.md` §5 and `.ai/blueprints/distribution.md`.
- Data model:
- Integration points:
- API contracts (high level):

## Security/DevOps Notes (Engineer-owned)
- Threat model:
- Secrets:
- CI/CD:
- Observability:
