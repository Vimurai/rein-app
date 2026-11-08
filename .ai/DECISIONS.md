# DECISIONS — rein-app (append-only)

Numbering is local to rein-app (D-001…). Decisions in the `rein` repo are cited as `rein D-###`.

---

## D-001 — Adopt the rein three-repo contract; rein-app is the shell, not a rewrite

**Date**: 2026-11-08
**Task**: P-1
**Decision**: rein-app implements `rein/plans/rein-ecosystem-contract.md` (rein D-150) verbatim: it installs, verifies, updates and (phase 2) draws; all Rein logic ships from `rein` as a signed release; contract items are decided in `rein`, never locally.

### Why needed
The project was a bare `rein init` scaffold. The upstream Architect handed a contract and a phased plan; this decision fixes that the contract is binding here and records the two local choices made while adopting it (task renumbering, fail-closed verifier default).

### Alternatives considered
1. **Full Rust port of Rein** — rejected upstream (rein D-150): 25k tested lines, POSIX security model, no user-visible gain. Not reopened.
2. **App decides the signature scheme locally to unblock E-3** — rejected: contract §2.1 reserves it for rein P-50; deciding here would fork the artifact format.
3. **Adopt the contract; ship a `SignatureVerifier` seam with a `Rejecting` default until P-50** — chosen: the pipeline, manifest and sha256 checks are testable now, and nothing can install from a real release until the scheme lands (fail closed by construction).

Also recorded: plan IDs E-1..E-5 were registered as **E-2..E-6** because `rein init` had minted a placeholder E-1 (closed as superseded). E-4 is the profile manager and E-5 the layout writer (a parallel insert swapped the plan order); `.ai/architect.md` §7 is the mapping of record.

### Constraints driving this decision
- Contract §1: rein-app never contains a copy of Rein logic; nothing in `rein` depends on rein-app.
- Contract §2.3 / rein D-132: only `hooks`, `enabledPlugins`, `extraKnownMarketplaces`, `attribution` may be written; hard-deny on `autoMode`, `permissions`, `defaultMode`.
- Contract §3: verified install, then `rein install` + `rein doctor` from the installed tree; tmux stays through phase 2; Windows via WSL2 only.

### Impact
- Unlocks: E-2..E-6 (Phase 1). Phase 2 blueprint (`desktop.md`) follows Phase 1.
- Risk if wrong: if `rein` changes the artifact/layout contract, `rein-release` and the layout-equality test break — by design they fail loudly rather than install.

### Rollback
Revise `.ai/architect.md` §3–§4 on a new rein D-### notice; the `Rejecting` backend can be re-selected by build flag to halt installs.

---

## D-002 — Marketplace registration shape written by the profile manager (answer to rein-plugins)

**Date**: 2026-11-08
**Task**: E-4
**Decision**: `rein-app profiles apply` registers the `rein-plugins` marketplace as a `github` marketplace source pinned to a **moving release branch `stable`** (not a per-release tag, not a sha, not the trunk), and writes `enabledPlugins` entries only for plugins the operator selects, as `"<plugin>@rein-plugins": true`.

Literal JSON the app writes (merged into each allowed profile's `settings.json`; every other key preserved):
```json
{
  "extraKnownMarketplaces": {
    "rein-plugins": {
      "source": { "source": "github", "repo": "<owner>/rein-plugins", "ref": "stable" }
    }
  },
  "enabledPlugins": {
    "rein-seo@rein-plugins": true,
    "rein-ux@rein-plugins": true,
    "rein-ops@rein-plugins": true,
    "rein-data@rein-plugins": true
  }
}
```
- Key under `extraKnownMarketplaces` == the marketplace `name` in `.claude-plugin/marketplace.json` (`rein-plugins`). `repo` owner is a build-time constant (`<owner>` to be confirmed by rein-plugins; assumed `vimurai`). `path` is omitted → default `.claude-plugin/marketplace.json`, which matches their layout.
- `enabledPlugins` key format is `<entry-name>@<marketplace-name>` (Claude Code's install id); value is boolean. Default on `apply`: marketplace registered, **no** `enabledPlugins` written; `--enable <list>|all` writes `true` for the named plugins; `--disable` writes `false`; `--undo` restores the backup. The app never removes other marketplaces or plugin entries.
- Pinning: Claude Code marketplace sources accept `ref` (branch/tag) but **not** `sha` (per marketplace-reference), so a commit pin is impossible at this layer. The app writes `ref: "stable"` once and never rewrites it per release.

### Why needed
rein-plugins' registration fixture (their E-4) is blocked on the exact JSON; their rollback story depends on whether we pin or track.

### Alternatives considered
1. **Track the trunk (omit `ref`)** — rejected: every trunk commit reaches users on `claude plugin marketplace update`; rein-plugins would have no release channel and rollback would require reverting trunk.
2. **Pin a per-release tag (`ref: "v1.2.0"`)** — rejected: the app would have to know rein-plugins' version and rewrite every profile on each plugin release, coupling app releases to plugin releases, and the contract gives the app no channel to learn that version.
3. **Pin a moving branch `stable`** — chosen: rein-plugins controls what users get by fast-forwarding `stable` to a release tag's commit; rollback == reset `stable` to the previous tag's commit (a branch can move backwards; a tag re-tag works too but a branch is the conventional moving ref). The app's write is static.

### Constraints driving this decision
- Contract §2.3: the app owns `extraKnownMarketplaces` and `enabledPlugins` per profile; nothing else.
- Contract §4: rein-plugins never touches profile settings, so the app must write the registration.
- Claude Code marketplace-source fields: `github` → `repo`, `ref`, `path`, `sparsePaths`; no `sha`.

### Impact
- Unlocks: E-4 (profile manager) has its fixed payload; rein-plugins' registration fixture can mirror this JSON.
- Risk if wrong: if rein-plugins prefers tags only, they create a `stable` branch anyway or we change one constant (`ref`) in E-4 — a one-line change, no schema impact.

### Rollback
Change the `ref` constant or omit it; `profiles apply --undo` restores every profile's prior `settings.json` from `.rein-bak`.

### Addendum 2026-11-08 — owner confirmed by rein-plugins (their D-006 mirrors this entry)
- `repo` is **`Vimurai/rein-plugins`** (capital V, same org as `github.com/Vimurai/rein`); `ref` `stable` is maintained by rein-plugins as the release pointer; no `path` needed.
- The GitHub remote does not exist yet (bootstrapped in their E-2; operator creates the remote). Treat the owner as confirmed intent: E-4 bakes the constant and its tests use a fixture, never the live URL. rein-plugins will notify us when the first tag is on `stable`; until then `profiles apply` on a real machine registers a marketplace that Claude Code cannot yet fetch — acceptable, since nothing is enabled by default.

---

## D-003 — Mirror rein D-153: minisign with baked key, manifest schema 1, doctor JSON, accounts surface, upgrade semantics

**Date**: 2026-11-08
**Task**: E-3, E-5 (and phase 2 status rendering)
**Decision**: rein-app binds to `rein` D-153 (commit d041ef9) for all five P-50 items; `.ai/blueprints/distribution.md` Components 2–3, §Data Model, §Security and §Test Plan are revised accordingly and supersede the E-3/E-5 task text in state.sqlite where they differ.

### Why needed
E-3 and E-5 could not be implemented for real without the signature scheme, manifest shape and upgrade-preserve list. One of our assumptions (preserve all of `config/`) was wrong.

### Alternatives considered
1. **Keep `Rejecting` as the default backend and add minisign later** — rejected: the ruling is final; shipping a verifier that cannot verify a real release would be a second task for no gain. `Rejecting` stays as a build-flag kill switch.
2. **Hard-code the operator-owned list in the app** — rejected: D-153 r5 puts it in `manifest.layout.operator_owned` precisely so the app reads the rule from the artifact; hard-coding would drift on the next rein release.
3. **Mirror D-153 verbatim into the blueprint, read layout from the manifest, bake the key** — chosen.

### Constraints driving this decision
- D-153 r1: trust root is compiled into the binary; a key rotation is an app release; `key_id` mismatch must name the required app version.
- D-153 r2: reject unknown `schema`, out-of-layout path, unlisted tarball file; apply `mode`.
- D-153 r5: `config/` mirrored except `rein-workspace.txt` and `install-stamp.json`; `bin/` pruned; `VERSION` written last via tmp-and-rename; `install-rein.sh` is the reference.

### Impact
- Unlocks: E-3 and E-5 have binding inputs; phase 2 status view has the doctor JSON shape.
- Risk if wrong: a rein release signed with a rotated key is uninstallable until the operator updates rein-app — intended, and visible via the `key_id` line.

### Rollback
Manifest `schema: 2` from rein with a migration note; this app fails closed on unknown schemas, so the rollback path is a coordinated app release.

---

### D-003 Addendum 2026-11-08 — rein D-153 addendum (commit c586480): generator and matcher syntax
- **Generator**: `rein release manifest <tree> [--out <dir>]` (rein `src/shared/release-manifest.mjs`), runnable from a checkout without install, produces `manifest.json` + tarball; signing is separate (`rein release sign`, minisign, key from `REIN_RELEASE_KEY`). E-5's layout-equality test uses this generator for the tarball and OUR OWN throwaway test key for signing; rein's `tests/fixtures/release/` key pair is never trusted or baked.
- **`layout.operator_owned` syntax**: gitignore-style subset (literal; trailing `/` = dir subtree; `*` one segment; `**` across segments; no `!`, no classes, no leading `/`). The schema-1 list uses `mcp/*/node_modules/` and `mcp/*/package-lock.json` (single-segment `*`, not `**`). Conformance fixture `tests/fixtures/release/operator-owned-cases.json` is vendored into rein-app and our matcher must pass every row. `distribution.md` Component 3 + §Test Plan updated.

## D-004 — License Apache-2.0 and repository `Vimurai/rein-app` (match the rein ecosystem)

**Date**: 2026-11-08
**Task**: E-2 follow-up (Cargo workspace metadata), E-6 (cargo-dist/Homebrew metadata)
**Decision**: rein-app is licensed **Apache-2.0** and its canonical repository is **`https://github.com/Vimurai/rein-app`**; both go into `[workspace.package]` (`license`, `repository`) and the Homebrew formula in E-6. Recorded as Architect intent pending operator ratification — the operator creates the GitHub remote.

### Why needed
The Engineer left `license`/`repository` out of the workspace because neither was decided; cargo-dist and Homebrew need both.

### Alternatives considered
1. **MIT or MIT/Apache dual** — rejected: `rein` (same org, the logic this app installs) is Apache-2.0; a different license across the three repos invites questions with no benefit. Dual licensing adds a second text to maintain for a shell app.
2. **Wait for the operator before naming the repo** — rejected: `rein` is `Vimurai/rein` and rein-plugins confirmed `Vimurai/rein-plugins`; the pattern is unambiguous and the string is a one-line change if the operator chooses otherwise.
3. **Apache-2.0 + `Vimurai/rein-app`** — chosen.

### Constraints driving this decision
- Same org as `github.com/Vimurai/rein` (origin of the rein checkout) and `Vimurai/rein-plugins` (their D-006).
- Apache-2.0 NOTICE/LICENSE files must ship in the cargo-dist archives (E-6).

### Impact
- Unlocks: workspace metadata now; E-6 formula/dist metadata later.
- Risk if wrong: two strings in Cargo.toml and the formula; no code impact.

### Rollback
Change `license`/`repository` in `[workspace.package]` and the formula; relicensing before first public release has no downstream users.

---

### D-004 Addendum 2026-11-08 — repository settled; license still pending operator confirmation
- Operator created public `github.com/Vimurai/rein-app` and master was pushed; hosted CI run 42512783295 green on ubuntu/macos/windows → E-2 acceptance met. The repository URL is no longer an assumption.
- Applied in f99a1b6: `license`/`repository` in `[workspace.package]`, verbatim Apache-2.0 `LICENSE`, `NOTICE` "Copyright 2026 Vimurai". The license choice remains Architect intent until the operator confirms.

---

## D-006 — Phase 1 crate dependencies (dependency_gate, Engineer-recorded)

> Committed as "D-005" in ff5335e; renumbered D-006 because the Architect's plugin-install ruling also took D-005.

**Date**: 2026-11-08
**Task**: E-3 (rein-release), E-4/E-5/E-6 (clap)
**Decision**: ADOPT the set below. Basis: the crate list is the one `.ai/blueprints/distribution.md`
§Execution Constraints already names (plus `minisign-verify`, implied by D-003/rein D-153 r1);
the operator directed "do all, don't ask" on 2026-11-08. Recorded by the Engineer; Architect may revise.

| Crate | Version | License | Use | Kind |
|---|---|---|---|---|
| serde (+derive) | 1.0.229 | MIT/Apache-2.0 | manifest + settings structs | runtime |
| serde_json | 1.0.154 | MIT/Apache-2.0 | manifest.json, settings.json | runtime |
| semver | 1.0.28 | MIT/Apache-2.0 | `version`, `min_node` | runtime |
| sha2 | 0.11.1 | MIT/Apache-2.0 | per-file sha256 (streamed) | runtime |
| tar | 0.4.46 | MIT/Apache-2.0 | tarball read (entries checked before any write) | runtime |
| flate2 | 1.1.11 | MIT/Apache-2.0 | gzip (pure-Rust miniz_oxide backend) | runtime |
| minisign-verify | 0.3.0 | MIT | signature verify only, zero deps (jedisct1, minisign author) | runtime |
| reqwest (blocking, rustls, no default features) | 0.13.5 | MIT/Apache-2.0 | HTTPS release download | runtime |
| clap (derive) | 4.6.7 | MIT/Apache-2.0 | CLI parsing (E-4 onward) | runtime |
| tempfile | 3.27.0 | MIT/Apache-2.0 | test temp HOMEs | dev |
| minisign | 0.10.0 | MIT | sign fixture releases with a throwaway TEST key | dev |

### Why needed
Each maps to one pipeline step in distribution.md Components 1–2 and 6; none duplicates another.

### Alternatives considered
1. **Implement ourselves** — rejected for crypto (sha256, ed25519/minisign), gzip and HTTP/TLS;
   hand-rolled versions are the larger risk. Accepted for small things: hex encoding, the
   operator-owned glob matcher, error types (no `thiserror`/`hex`/`glob` crates).
2. **`ureq` instead of `reqwest`** — lighter (no tokio), but the blueprint names reqwest;
   swapping is a one-module change behind the `Http` trait if binary size matters later.
3. **`minisign` (full) at runtime** — rejected: verify-only crate is smaller, no key-handling surface.

### Size / weight
reqwest blocking + rustls dominates (tokio, hyper, rustls, ring/aws-lc). Everything else is small.
All licenses are compatible with Apache-2.0 (D-004).

### Security track record
Mainstream, maintained crates; current versions have no RustSec advisory known to the Engineer at
recording time (cargo-audit not installed locally — a `cargo audit` CI step is proposed for E-6).
`tar` has had path-traversal advisories historically: we never call `unpack`; every entry path,
type and size is validated by our code before a byte is written (E-3).

### Maintenance
All released within the last year; serde/reqwest/clap/tar/flate2 are rust-lang-adjacent staples.

### MSRV impact
sha2 0.11 needs Rust 1.88 → workspace `rust-version` raised 1.85 → 1.88. CI builds on stable.

### Rollback
Each crate sits behind one module (`Http`, verifier, matcher, CLI); remove the dependency and that
module. `Cargo.lock` is committed and CI runs `--locked`.

## D-005 — Plugin enablement goes through `claude plugin install`; the app never hand-writes `enabledPlugins: true` (revises D-002 consequence "true = install + enable")

**Date**: 2026-11-08
**Task**: E-4 (profile manager); answers rein-plugins Q4
**Decision**: At user scope an `enabledPlugins` entry does **not** install a plugin — Claude Code auto-installs from `enabledPlugins` only under **managed settings** (docs: plugins/org "Require a marketplace and its plugins"; plugins/install "Choose an install scope": a committed project entry "turns the plugin on … but doesn't download it"). Therefore `rein-app profiles apply`:
1. **Registers the marketplace by writing `extraKnownMarketplaces`** exactly as D-002 (a hand-written source object is documented as valid in any settings file and "Loads"). Unchanged.
2. **`--enable <list>|all`** runs, per profile, `claude plugin install <plugin>@rein-plugins --scope user` with `CLAUDE_CONFIG_DIR=<config_dir>` (and `--yes` is NOT passed: rein-plugins uses relative-path sources, no `command` source). Claude Code performs the download and writes `enabledPlugins."<plugin>@rein-plugins": true` itself; the app then re-reads the file and verifies the key is present. The app never authors a `true` entry by hand.
3. **`--disable <list>`** runs `claude plugin disable <plugin>@rein-plugins --scope user` (writes `false`). **`--undo`** restores the `.rein-bak` settings file; installed plugin caches are left in place (harmless: not enabled == not loaded).
4. All `claude plugin …` calls go through the existing `Shell` trait (stubbed in tests, fixed argv, no user strings). If `claude` is absent, `profiles apply --enable` exits 4 (runtime missing) after still registering the marketplace; RuntimeCheck already reports Claude Code and prints the official installer URL.
5. Registration can be verified with `claude plugin marketplace list`, not `claude plugin list` (the latter lists installed plugins only — the empty result rein-plugins observed is expected, not a registration failure).

### Why needed
rein-plugins' registration fixture found `claude plugin list` empty after merging D-002 JSON at user and project scope; D-002 had assumed "true = install + enable" at those scopes. The docs confirm auto-install is a managed-settings behaviour.

### Alternatives considered
1. **Write to a managed-settings location** (`managed-settings.json`) — rejected: contract §2.3 allows per-profile `settings.json` only; managed settings are an operator/admin surface, need elevated paths, and lock the user out of toggling.
2. **Hand-write `enabledPlugins: true` and rely on `CLAUDE_CODE_SYNC_PLUGIN_INSTALL=1`** — rejected: that variable only makes `-p`/CI runs wait for background installs that managed/project settings trigger; it does not create an install at user scope.
3. **Shell out to `claude plugin install` for the install, keep D-002's registration JSON** — chosen: the documented path, writes the same key in the same file, also populates the plugin cache and `known_marketplaces.json` which a hand-written entry cannot.
4. **Also delegate registration to `claude plugin marketplace add Vimurai/rein-plugins#stable`** — not chosen as the primary path (the hand-written entry is contract-sanctioned, works without `claude` present, and keeps the pin explicit), but allowed as the fallback if a Claude Code version is found to ignore a hand-written user-scope entry; the Engineer records which path the version on hand needed.

### Constraints driving this decision
- Contract §2.3: the app may own `extraKnownMarketplaces` and `enabledPlugins` per profile; nothing says it must author them by hand.
- No user-supplied strings reach a shell; fixed argv behind `Shell` (distribution.md §Security trust boundary 4).
- Plugin names are a closed list in the app (`rein-seo`, `rein-ux`, `rein-ops`, `rein-data`) validated against `^[A-Za-z0-9][A-Za-z0-9._-]*$` before argv.

### Impact
- Unlocks: rein-plugins fixture asserts registration + (after `claude plugin install`) enablement; their D-006 consequence (3) becomes "true after `claude plugin install`", not "true causes install".
- Risk if wrong: if a future Claude Code auto-installs from user-scope `enabledPlugins`, our path still works (install is idempotent).

### Rollback
Revert to hand-writing `enabledPlugins` (one function) — only if Claude Code documents user-scope auto-install.

---

### D-005 Addendum 2026-11-08 — evidence from rein-plugins (their D-013 mirrors D-005)
- **Confirmed**: in an isolated `CLAUDE_CONFIG_DIR`, `claude plugin marketplace add` + `claude plugin install <p>@rein-plugins` wrote exactly the D-002 shape into that profile's settings.json (key `rein-plugins`; `"<p>@rein-plugins": true`). Our hand-written marketplace entry is byte-compatible with what Claude Code writes, and `true` is Claude Code's own output.
- **Still open**: whether a hand-written user-scope `extraKnownMarketplaces` entry is honoured on a settings-only start (and installs nothing). rein-plugins collects it in CI (`[FIXTURE_Q4_OBSERVED]` lines) once the operator sets an API secret there; no credentials are copied locally. Until the lines arrive, the `claude plugin marketplace add Vimurai/rein-plugins#stable` fallback stays in E-4's design and E-4 may ship with the hand-written path as primary.
