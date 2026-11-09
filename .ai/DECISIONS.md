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

## D-007 — Marketplace registration goes through `claude plugin marketplace add Vimurai/rein-plugins#stable`; the hand-written entry is retired (supersedes D-005 r1, D-002 "the app writes")

**Date**: 2026-11-08
**Task**: E-4
**Decision**: `profiles apply` registers the marketplace per profile with
`CLAUDE_CONFIG_DIR=<config_dir> claude plugin marketplace add Vimurai/rein-plugins#stable --scope user` through the `Shell` trait, then re-reads `settings.json` and **verifies** `extraKnownMarketplaces.rein-plugins.source == { source: "github", repo: "Vimurai/rein-plugins", ref: "stable" }`. A missing or different `ref` is a hard failure (exit 7, message names the observed entry) — the app never silently accepts trunk-tracking and never patches the entry by hand. The D-002 JSON remains the **expected shape** (it is what Claude Code writes); only the author of the write changes.

### Why needed
Engineer probe on Claude Code 2.1.319 (throwaway `CLAUDE_CONFIG_DIR`, local `directory` marketplace named `rein-plugins`): a hand-written user-scope `extraKnownMarketplaces` entry is absent from `claude plugin marketplace list`, `marketplace update` fails "not found", `plugin install` fails "not found in marketplace"; after `claude plugin marketplace add … --scope user` everything works and both `settings.json` and `plugins/known_marketplaces.json` are written. The CLI registry is `known_marketplaces.json`; settings alone do not populate it at user scope.

### Alternatives considered
1. **Hand-write settings AND `known_marketplaces.json`** — rejected: that file is Claude Code internal state with no documented schema; writing it is exactly the "reimplement the tool" failure the contract forbids for `rein`, applied to Claude Code.
2. **Hand-write the entry and hope a later Claude Code honours it** — rejected: fails today, and D-005 alt 4 was reserved for this case.
3. **`claude plugin marketplace add Vimurai/rein-plugins#stable --scope user` + read-back verification** — chosen. The `#ref` form is documented (plugins/install "Add a marketplace": "`owner/repo`. Add `#ref` to pin a branch or tag"); `--help` not listing a flag is consistent because the ref rides in the source string.

### Constraints driving this decision
- Contract §2.3 (we own the key) is satisfied by owning the *outcome* and verifying it; §2 forbids us only from reimplementing `rein`, and the same spirit applies to Claude Code internals.
- Fixed argv behind `Shell`; the source string is a build-time constant, never user input.
- `claude` absent → `profiles apply` still performs `attribution` and `hooks` (via `rein install`), skips marketplace + plugins, exits 4.

### Impact
- Unlocks: E-4 works for real on 2.1.319. rein-plugins' fixture assertions (a)/(b)/(c) stand unchanged — the resulting JSON is identical.
- Open verification: whether `#stable` is honoured as `ref` on `owner/repo` sources on 2.1.319 — provable only against a live remote or a git URL fixture. E-4 tests use a local `file://` git fixture with a `stable` branch: `claude plugin marketplace add file:///…/fixture.git#stable` and assert `ref` in the written entry. If the pin is NOT honoured, exit 7 fires and this decision is reopened before E-6 ships.
- Risk if wrong: none for users — the failure is loud and nothing is enabled.

### Rollback
One argv constant; `profiles apply --undo` (D-008) removes the allowed keys we introduced.

---

## D-008 — `profiles apply --undo` restores the four allowed keys by value from a once-written pre-Rein backup; it never restores the whole file

**Date**: 2026-11-08
**Task**: E-4 (security_engineer P0 finding)
**Decision**: The backup `settings.json.rein-bak` is written **once**, the first time rein-app touches a profile (the pre-Rein state), and never overwritten. `--undo` restores only the values of `hooks`, `enabledPlugins`, `extraKnownMarketplaces`, `attribution` from that backup (removing a key the backup lacked) and leaves every other key as it currently is. `--hooks-from <file>` is removed. A `config_dir` or `settings.json` that is a symlink escaping `config_dir` is refused. This supersedes the "byte-identical restore" wording in `distribution.md` §Rollback Plan and §Security trust boundary 3.

### Why needed
security_engineer found a P0: a whole-file restore could re-grant `permissions`, `defaultMode` or `autoMode` values that the operator had since tightened — the app would be writing hard-denied keys by the back door (rein D-132). A per-apply backup would also let a second `apply` capture a Rein-modified state as "original".

### Alternatives considered
1. **Whole-file restore** — rejected (the P0).
2. **No undo at all** — rejected: operators need a way out, and the Phase 2 settings UI promises one.
3. **Allowed-keys-only restore from a once-written backup** — chosen.

### Constraints driving this decision
- rein D-132 hard-deny: the app must be unable to write `autoMode`, `permissions`, `defaultMode` on any code path, including undo.
- Closed `AllowedKey` enum already exists; undo reuses the same writer.

### Impact
- Blueprint divergence recorded: undo is "allowed keys by value", not byte-identical. Unlocks E-4 completion.
- Risk if wrong: an operator who edited `attribution` by hand after apply loses that edit on undo — acceptable and documented in `--undo` output.

### Rollback
n/a — this closes a security finding; reverting would reopen it.

---

### D-007 Addendum 2026-11-08 — rein-plugins mirror and CI guard
- Mirrored as rein-plugins D-014. The GitHub-shorthand pin check is their E-8: CI runs `claude plugin marketplace add Vimurai/rein-plugins#stable --scope user` in an isolated config dir against the live repo and asserts `source.ref == "stable"`, `source.repo == "Vimurai/rein-plugins"`; a miss emits `[FIXTURE_REF_MISSING]` and I am notified to reopen D-007. Their reconcile layer now expects "not registered, nothing installed" on a settings-only start, matching the 2.1.319 finding.

## D-009 — Session-start reconciliation observed; D-007's explicit CLI path stays primary, the hand-written entry returns only as the `claude`-absent fallback; D-005 assertion (c) retracted

**Date**: 2026-11-08
**Task**: E-4; answers rein-plugins Q5
**Decision**: rein-plugins' E-4 fixture on Claude Code 2.1.319 shows that a **session start** registers a hand-written user-scope `extraKnownMarketplaces` entry (even an unauthenticated `claude -p` that fails "Not logged in" wrote `known_marketplaces.json`; reproduced twice), and one authenticated start installed an `enabledPlugins: true` plugin from settings alone (single sample, `directory` source). This reconciles with the Engineer's D-007 probe, which ran CLI subcommands before any start. Ruling:
1. **D-007 stands as the primary path**: `claude plugin marketplace add Vimurai/rein-plugins#stable --scope user` then `claude plugin install …` per profile, with read-back verification and exit 7 on a missing/different `ref`. Reason: immediate, deterministic, verifiable at `apply` time, documented; session-start reconciliation is deferred, undocumented at user scope, and unverifiable until the operator next starts Claude Code.
2. **`claude` absent → hand-write the D-002 marketplace entry (with `ref: "stable"`) as a fallback**, print "marketplace registration completes at the next Claude Code start", still exit 4 (runtime missing). Never hand-write `enabledPlugins: true`: install-from-settings at user scope rests on one sample and is not documented; the operator runs `profiles apply --enable` once `claude` exists.
3. **D-005 assertion (c)** ("the settings write alone installs nothing") is **retracted** and replaced by: "rein-app does not rely on install-from-settings at user scope; it may happen after an authenticated start and is harmless because rein-app only ever leaves `true` entries that Claude Code itself wrote or none at all".
4. **No change to D-008** (undo). After undo a plugin may remain installed but disabled; acceptable.

### Why needed
D-007 was ruled on a CLI-subcommand probe without a session start; the new fixture shows the missing half. Both repos' fixtures must assert the same model.

### Alternatives considered
1. **Revert to hand-writing both keys and rely on session-start reconciliation** — rejected: deferred effect, no `apply`-time verification of the `ref` pin, install side rests on one sample, and `ref` handling by the reconciler is unmeasured (a trunk-tracking clone would be silent).
2. **Keep D-007 and forbid the hand-written fallback entirely** — rejected: when `claude` is absent the operator gains nothing from refusing; the entry with an explicit `ref` is the documented settings shape and the reconciler honours it.
3. **D-007 primary + explicit-ref hand-write fallback + retract (c)** — chosen.

### What rein-plugins' CI should measure (Q5), per Claude Code version
- M1 Registration by start, unauthenticated, `github` source `Vimurai/rein-plugins` with `ref: "stable"` once live (and `directory` meanwhile): after one `claude -p` start, does `known_marketplaces.json` carry `ref: "stable"`, and does the marketplace clone's HEAD equal `origin/stable`? (Detects silent trunk-tracking.)
- M2 Install by start, authenticated, `github` source: three consecutive fresh config dirs with `enabledPlugins: {"rein-seo@rein-plugins": true}` only; count installs, record version/sha. Report-only.
- M3 (existing E-8) `claude plugin marketplace add Vimurai/rein-plugins#stable --scope user` → `source.ref == "stable"` in settings AND in `known_marketplaces.json`.
- M4 Idempotence: D-007 path run on a profile the reconciler already registered → exit 0, entry unchanged, no duplicate marketplace.
All report-only lines `[FIXTURE_Q5_M<n>] <version> <result>`; a M1 trunk-tracking result or a M3 missing `ref` reopens D-007/D-009 the same day.

### Impact
- E-4: one additional branch (fallback hand-write when `claude` is absent) and M4 idempotence test. Everything else unchanged.
- Risk if wrong: if the reconciler ignores `ref`, operators without `claude` at apply time track trunk until `profiles apply` runs again — mitigated by the exit-4 signal and M1.

### Rollback
Remove the fallback branch; D-007 alone remains.

---

### D-009 Addendum 2026-11-08 — rein-plugins mirror and measurement tasks
- Mirrored as rein-plugins D-016; their D-013 (c) / D-014 suspensions lifted. M3 = their E-8 (settings + `known_marketplaces.json`); M1/M2/M4 = their E-9. All report-only `[FIXTURE_Q5_M<n>] <claude version> <result>`. M1 runs on the `directory` source until the first `stable` tag, then `github`. M2 skips cleanly until the operator sets the CI secret. Same-day notice on M1 HEAD ≠ origin/stable or E-8 `[FIXTURE_REF_MISSING]`.

### D-009 Addendum 2026-11-08 — first CI data (rein-plugins PR #3, job `registration`, ubuntu-latest, Claude Code 2.1.319, no credential, `directory` source)
- `[FIXTURE_Q4_OBSERVED] before any start: registered=false` · `after one unauthenticated start: registered=true` — registration-by-start reproduced on a clean runner. Confirms the D-009 fallback (hand-written entry with `ref` when `claude` is absent) is honoured at the next start.
- `rein-seo@rein-plugins from settings alone (sync-install forced): not-installed; loaded-at-init=true` — with a `directory` source Claude Code loads the plugin in place from the marketplace copy without an `installed_plugins.json` record. Relevance to rein-app: our read-back (D-007) verifies `settings.json` (`extraKnownMarketplaces` entry, `enabledPlugins` `true` written by `claude plugin install`), never `installed_plugins.json`, so this does not affect exit 7. Phase 2 status must not treat "no install record" as "not loaded" — use `claude plugin list` or the init event.
- M1 (github source, HEAD vs origin/stable) and M4 now running on their E-4 branch; M2 awaits the operator's CI secret. Nothing reopens D-007/D-009.

---

> D-010 (Engineer, dependency_gate — cargo-dist / cargo-deb / appimagetool as release build
> tools) is recorded on branch `engineer/e6-release-pipeline` (PR #1) and lands with it. The
> two entries below were written on master while PR #1 was open; when the branch is merged
> with master, keep D-010 above D-011.

## D-011 — The LayoutWriter syncs the mirrored directories in place; the Rein home is never copied or swapped, and operator data is never opened

**Date**: 2026-11-09
**Task**: review of E-5 (Engineer handoff 2026-11-09 14:45 UTC, divergences 1–2; security review E-5 P1-3) → E-7
**Decision**: Replace the whole-home staging swap (blueprint v1 §Components 3, shipped in E-5) with an in-place per-file sync of the manifest's mirrored directories, a snapshot of the release payload only, and a rollback that restores code only.

### Why needed
E-5 built the next tree in `$HOME/.rein.staging-<ver>` by copying every non-mirrored live entry, then renamed the home aside. Three things follow, all confirmed on a live home on 2026-11-09:
- **Size.** The home is 3.5 GB (`ci/` 3.0 GB, `mcp/*/node_modules` 551 MB); the release payload in the mirrored dirs is 8.9 MB. Every install copied the 3.5 GB, and N=2 previous trees kept up to 7 GB more. The "< 10 s" constraint was unreachable.
- **Live data.** rein's MCP servers hold `approvals.sqlite`, `cache.sqlite`, `telemetry.sqlite` open with WAL files. A copy can tear a db/WAL pair; a process that opened a file before the swap keeps writing into the tree that was renamed away, so those writes (the approval ledger among them) are missing from the new home (SEC P1-3). `rollback` restored the older tree's operator files, i.e. rolled operator data back.
- **The gate.** Between `rename(.rein → .rein.prev)` and `rename(staging → .rein)` there is no `$HOME/.rein`: every Claude Code hook path is missing for that window. `install-rein.sh` rejects a directory swap for exactly this reason (its E-304 comment: "a renamed-away hooks/ would leave a window in which every gate hook is missing, and mcp/ carries node_modules the source tree does not have").
The blueprint's swap was this Architect's design, not a contract item; contract §2/§4 and rein D-153 r5 name `install-rein.sh` as the reference, and that script syncs per file.

### Alternatives considered
1. **Keep the swap, carry only `operator_owned`** (blueprint v1 literally) — drops committed data not on the schema-1 list (`approvals.sqlite*`, `cache.sqlite*`, `ci/`, `privacy/`, `trash/`); still copies 551 MB of `node_modules`; still has the gate window.
2. **Keep the swap, carry everything non-mirrored** (E-5 as shipped) — no data dropped at copy time, but the size, torn-WAL, lost-write and gate-window problems above.
3. **Keep the swap, refuse while rein processes are live** (SEC P1-3 option a) — the app would need its own process detection (a `rein doctor` concern we must not reimplement), an operator updating from inside a Rein session could never update, and the 3.5 GB copy and data rollback remain.
4. **Swap each mirrored directory by rename** — `hooks/` still disappears for an instant, and `mcp/` contains operator-owned `node_modules` that would have to be moved between trees.
5. **In-place per-file sync of the mirrored dirs (chosen)** — identical to the reference; I/O bounded by the release; operator data never opened, so nothing to tear, lose, duplicate or roll back; each file is old or new at every instant and `hooks` goes last; SEC P1-3 closes without a process check.

### Constraints driving this decision
- Contract: "Tarball == what `install-rein.sh` lays out"; operator-owned paths "never touched by an install" (rein D-153 r5).
- Security: the safe-exec gate hooks live in the home; no install step may make them absent.
- Carry-over ruling (divergence 1): **anything outside `layout.mirrored` is not ours** — never read, copied, written or deleted, whether or not `operator_owned` names it. `operator_owned` decides only inside mirrored dirs (protect from write/prune). The verifier rule "a release path outside `mirrored ∪ operator_owned` is rejected" is unchanged.
- What is given up: whole-tree atomicity. A sync that dies midway leaves a mixed tree until the automatic restore (or a re-run) completes — the same exposure `install-rein.sh` and `rein sync` have today, bounded by verifying everything before the first write, per-file rename, `hooks` last, `VERSION` last, and restore from the snapshot.

### Impact
- Unlocks: E-7 (rework), E-9 (layout-equality test, once rein P-50 ships the generator). E-6's first release tag waits for E-7.
- No migration: no rein-app release exists, so no operator has an E-5-format `.rein.prev-*` tree.
- Asked of `rein` (contract clarification, not decided here): confirm the complement rule above in the manifest contract (or extend `operator_owned`), and rule who removes a retired MCP server's directory and legacy mirrors under an app install (`install-rein.sh` does both itself).
- Risk if wrong: a release whose files only work as a set is briefly mixed on a home with running sessions. Mitigation is the ordering above; rein already lives with this on every `rein sync`.

### Rollback
Revert E-7; E-5's writer returns unchanged (it is self-contained in `layout.rs`/`rollback.rs`). Do not ship a release from that state.

---

## D-012 — E-5/E-6 divergence rulings: AppImage deferred out of Phase 1; smaller divergences accepted or scheduled

**Date**: 2026-11-09
**Task**: review of E-5/E-6 (Engineer handoff 2026-11-09 14:45 UTC, divergences 3–9)
**Decision**: AppImage is not a Phase 1 deliverable; the remaining divergences are accepted as built except `install` requiring `--version` (E-8) and the unwritten layout-equality test (E-9).

### Why needed
The handoff listed nine divergences from `distribution.md`. D-011 settles 1–2; these need a recorded answer so the critics stop re-raising them and the blueprint matches the code.

### Alternatives considered (AppImage)
1. **Build with appimagetool's rolling `continuous` release** — not pinnable by version or checksum; an unpinned build tool in the release job contradicts the SHA-pinning rule in DEVOPS.md.
2. **Vendor or self-build appimagetool** — real work and a new supply-chain item for a format whose audience is already served.
3. **Defer (chosen)** — Linux is covered by the cargo-dist tarball + shell installer, `.deb`, and Homebrew on Linux. Reopen when appimagetool publishes a tagged, checksummed release, or an operator asks for it.

### Rulings on the rest
- (3) Layout-equality test: stays required; tracked as E-9, BLOCKED on rein P-50 (`rein release manifest`).
- (4) `install` requires `--version`: not accepted — the API table's "default latest" stands; E-8. `update` resolving the tag through `api.github.com` (metadata only, release still verified and version-bound) is accepted.
- (5) RuntimeCheck: Node ≥ `min_node` and git required, sqlite3/tmux/Claude Code reported only — accepted; `rein doctor` owns those verdicts.
- (6) Snapshot ordering by `VERSION` mtime and "a release file at an operator-owned path is never written, even fresh" — accepted. `OrphanedPrevious` is removed by E-7 (the home never disappears under D-011).
- (7) `launch` runs `rein-app update --yes` inside WSL, needs a Linux rein-app in the distro, refuses `;` in the project path — accepted.
- (8) cargo-dist 0.34.0 hand fix under `allow-dirty = ["ci"]` — accepted as recorded in DEVOPS.md; re-check on any cargo-dist bump.
- (9) Deferred security P2s: stale lock after SIGINT → Phase 2 (the message names the PID; the desktop app needs a real liveness check anyway). Runner env denylist beyond `CLAUDECODE` → accepted as is (architect.md §5). `O_NOFOLLOW` copies → moot after E-7 (no operator data is copied; E-7 refuses symlinked targets). Fresh-install directory modes 0755 → accepted. "No test for a swap failing midway" → closed by E-7's fault seam.

### Constraints driving this decision
- Release-job supply chain: every tool pinned (DEVOPS.md).
- Never reimplement a `rein doctor` check (contract non-goal).

### Impact
- Unlocks: E-6 can close without AppImage once PR #1 is merged and the first tag builds (after E-7).
- Risk if wrong: Linux users without apt or brew use the tarball installer — a worse first run, not a blocked one.

### Rollback
Add an AppImage job to `build-linux-packages.yml` under a new dependency_gate entry; nothing else depends on its absence.

---
