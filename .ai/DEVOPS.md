# DEVOPS — build, test, CI, release

> Engineer-owned. Every CI/CD change is recorded here first (`skill: ci_gate`).
> rein-app has a **hosted** pipeline (GitHub Actions), unlike `rein` itself (local CI, D-072).

## Commands
| Purpose | Command |
|---|---|
| Format check | `cargo fmt --all --check` |
| Lint | `cargo clippy --workspace --all-targets --locked -- -D warnings` |
| Test | `cargo test --workspace --locked` |
| Release build | `cargo build --profile dist` (cargo-dist profile) |

Order (hosted equivalent of the ci_gate step order): `fmt → clippy → test → build → release`.
Release runs only on version tags (E-6).

## Change log

### 2026-11-08 — E-2: initial CI matrix + cargo-dist config
**What / why**
- `.github/workflows/ci.yml`: on push to `master` and every pull request, run fmt, clippy
  (`-D warnings`) and `cargo test --workspace` on `ubuntu-latest`, `macos-latest`,
  `windows-latest`. Acceptance for E-2: green on all three.
- `Cargo.toml` `[workspace.metadata.dist]` (cargo-dist 0.34.0) declaring the five
  blueprint targets and a `dist` build profile. **No** `ci`/`installers` keys and no
  `release.yml` yet: the release workflow, Homebrew tap, `.deb`/AppImage and Windows
  installer are E-6. Adding `ci = "github"` before E-6 would make `dist` demand a
  generated release workflow.

**Security implications**
- No secrets. `permissions: contents: read` at workflow level; no write tokens.
- Only third-party action: `actions/checkout`, pinned to the commit SHA of v6.1.0
  (`d23441a48e516b6c34aea4fa41551a30e30af803`), `persist-credentials: false`.
  Toolchain installed with the runner's own `rustup`, no toolchain action.
- Network: crates.io only (no crate dependencies exist yet; `--locked` forbids silent
  lockfile drift once they do).
- `concurrency` cancels superseded runs on the same ref.

**Rollback**
- Delete or revert `.github/workflows/ci.yml`; nothing else depends on it.
- cargo-dist metadata is inert until a `dist` command runs; revert the
  `[workspace.metadata.dist]` / `[profile.dist]` blocks to remove it.

**Verification**
- Local (Linux x86_64): fmt, clippy and test run before commit.
- The GitHub remote does not exist yet, so the three-OS matrix cannot be proven green
  until the repo is pushed. E-2 acceptance is therefore **pending first push**.
