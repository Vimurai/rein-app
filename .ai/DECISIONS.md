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

## D-010 — E-6 release build tools: cargo-dist, cargo-deb, appimagetool (dependency_gate, Engineer-recorded)

**Date**: 2026-11-09
**Task**: E-6 (release pipeline)
**Decision**: ADOPT — operator chose "build the pipeline, guarded" on 2026-11-09 (installs the
tools, release workflow fails while `REIN_RELEASE_PUBKEY` is empty). These are **build-time
tools run in CI and on the release machine**; none is linked into the `rein-app` binary.

| Tool | Version | License | Use |
|---|---|---|---|
| cargo-dist (`dist`) | 0.34.0 (already pinned in `[workspace.metadata.dist]`) | MIT/Apache-2.0 | generate + run `release.yml`: archives, checksums, msi, Homebrew formula |
| cargo-deb | 3.8.0 | MIT | `.deb` for x86_64/aarch64 Linux (cargo-dist has no .deb installer) |
| appimagetool | pinned release + sha256 in the workflow | MIT | AppImage for x86_64 Linux |

### Why needed
distribution.md E-6 names Homebrew, `.deb`/AppImage and a Windows installer. cargo-dist covers
archives, Homebrew and msi; it builds neither `.deb` nor AppImage.

### Alternatives considered
1. **Hand-written release workflow** — rejected: cargo-dist's generated workflow is checked by
   `dist plan` and is the tool the blueprint and E-2 already chose.
2. **nfpm / fpm for .deb** — rejected: a second packaging config language; cargo-deb reads
   `Cargo.toml` metadata we already maintain.
3. **No AppImage** — possible; kept because the blueprint names it. It is the first thing to
   drop if appimagetool causes trouble (Homebrew + .deb cover Linux).

### Security track record / maintenance
cargo-dist (axodotdev) and cargo-deb (kornelski) are widely used and released in 2026.
appimagetool is fetched by URL in CI: pin the release asset and verify its sha256 before
running it. All three run only in the release job, on a pushed tag; the only secret in that
job is `HOMEBREW_TAP_TOKEN` (tap repo contents:write only), exposed only to the publish step.

### Rollback
Remove `release.yml`, the cargo-dist `ci`/`installers`/`tap` keys and the
`[package.metadata.deb]` block. No runtime code depends on any of them.

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

## D-013 — E-7 rulings: a symlink at a target is replaced, not refused; a snapshot's layout is trusted within the home owner's account, and an owner guard enforces that premise

**Date**: 2026-11-09
**Task**: review of E-7/E-8 (Engineer handoff 2026-11-09 15:30 UTC, divergences 1–6; security review E-7 P1-2, P2-2) → E-10
**Decision**: Accept the implemented symlink rule and the unsigned snapshot `layout.json` as built, reject a second local copy of the layout, and add a check that `install`/`update`/`rollback` run only as the owner of the home and of the snapshot.

### Why needed
D-011's step (a) said "no existing component of any target path is a symlink". Taken literally, one unshipped `bin/env-link` in a live home blocks restore-on-failure and rollback, because the snapshot carries the link back (SEC P1-2). Separately, `rollback` takes its scope from the snapshot's own `layout.json`; an edited file (`mirrored += secrets`) makes rollback sync and prune a directory the release never owned (SEC P2-2), and there is no signed copy of the layout on disk to compare against.

### Alternatives considered (snapshot layout, P2-2)
1. **Keep the last signed manifest's layout beside the home and require `snapshot.mirrored ⊆` it** (Engineer's option) — the copy lives in the same account and is exactly as editable as `layout.json`; it adds a second file to keep in step and no trust.
2. **Make rollback a signed operation**: cache the previous release's tarball + `.minisig` and re-verify with the baked key — real trust, but no rollback after a key rotation, none for the first update over an `install-rein.sh` home, and it restores the release as shipped rather than what was installed. A redesign for a same-account risk; revisit only if rollback ever needs to cross a privilege boundary.
3. **Accept, and enforce the premise (chosen)** — editing `layout.json` needs write access as the home's owner, who can already edit `secrets/`, `hooks/` and the gate directly, so no boundary is crossed *provided the writer runs as that same user*. That proviso was assumed, not checked: `sudo rein-app rollback` with the user's HOME (the default on macOS) would run as root steered by a user-editable file, and any root-run install leaves root-owned files in the home that rein then cannot update. The owner guard makes the proviso a refusal.

### Rulings on the rest
- (1) Symlink at a target: replaced by the rename, pruned and snapshotted as a link; a symlinked mirrored root or intermediate component is still refused. This is what `rsync -a --delete` does in the reference. Blueprint step (a) reworded.
- (2) An entry of any type at an operator-owned directory path is owned (never opened, snapshotted, pruned) — accepted; stricter than the blueprint.
- (3) No snapshot without a semver live `VERSION`; `Conflict` refusals; only directories with a real `VERSION` + `layout.json` count as snapshots — accepted, with one addition: the CLI says in one line when a populated home is updated without a snapshot (E-10).
- (4) Rollback uses the restored snapshot's layout, so a mirrored dir only the newer release lists stays in place — accepted; the next update prunes it.
- (6) `#[doc(hidden)] pub Faults`, exports `plan`/`SyncPlan`/`KEEP_SNAPSHOTS` — accepted; no production caller may set a fault.

### Constraints driving this decision
- No `unsafe` and no new dependency for one uid: the run's uid is the owner of the lock file it has just created (`MetadataExt::uid`).
- The app must never be a way to turn a user-writable file into a root action.

### Impact
- Unlocks: E-10 (owner guard + no-snapshot notice). Does not block E-6 or E-9.
- Risk if wrong: a home whose `$HOME` is owned by another uid than the invoking user (unusual mounts) is refused with the path named; there is deliberately no override flag.

### Rollback
Remove the guard call; nothing else depends on it. The symlink rule and snapshot format are unaffected.

---

### D-013 Addendum 2026-11-09 — E-10 review: the guard becomes a type-level precondition; a shared `--home` is unsupported
- **Guard-skippable public functions (security review E-10 P2-1): fix now, E-11.** `install_tree_locked` and `install_tree_with_faults` are public and trust the caller to have run `owner_guard`. Today's only caller does; the next one is the Phase 2 desktop app, which consumes exactly these library seams. `owner_guard` will return a proof value that only it can construct (borrowed from the `InstallLock`) and the lock-held writers take it, so forgetting the guard is a compile error — the same device as the closed `AllowedKey` enum. Rejected: a doc comment (what exists now; it is the thing a new caller does not read) and running the guard again inside each function (hides the precondition and re-stats on every call path while still letting a new entry point omit it).
- **Shared root-owned `--home` (P2-3/P2-4): accepted, documented, no code.** The guard checks the owners of the home, `.rein`, the snapshot root and `layout.json`, not `VERSION`/`tree/**`, and snapshot/fetch-dir cleanup has no owner check (`remove_dir_all` does not follow links). Reaching any of this needs a home inside a directory other users can write, which rein itself does not support; walking every entry's owner on each run would buy nothing for a private home. Recorded in `distribution.md` §Security as an explicit non-goal; `security_engineer` may mirror it into THREAT_MODEL.md.
- **Test hygiene (TESTS P1 + Engineer note):** CLI refusal tests must assert the path named in the error, and the Linux test that uses the real `/tmp` must skip when `/tmp/.rein.lock` exists — a lock left by a killed run in a sticky directory would otherwise fail that test for every other user of the machine. Part of E-11.

### D-013 Addendum 2026-11-09 (2) — E-11 review: both remaining P2s accepted, no further task
- **`owner_guard_with` still exported, doc-hidden:** accepted. The public `owner_guard` always reads the real uid and is the only form production code may call; a `test-seams` cargo feature would split the build and test matrix to hide a function from code that already runs in the same process. Same status as the `Faults` seam (blueprint §Security).
- **The `--home /tmp` CLI test passes when it skips:** accepted. The refusal is proven for `install`, `update` and `rollback` by the injected-uid tests on every platform; the `/tmp` test is an extra real-uid check that runs on a fresh CI runner and may skip on a shared developer machine. A skip is not counted as coverage of anything the other tests do not already cover.
- With E-11 the owner-guard line of work is closed. Reopen only if `rollback` or `install` ever has to run across a privilege boundary (a privileged helper, a system-wide install).

---

## D-014 — Mirror rein D-155: strict tarball members, a shipped path wins, `manifest.json` stays in the home, three profile keys; plus what follows for rollback, exit codes and the baked key

**Date**: 2026-11-09
**Task**: notice from `rein · Architect` (rein commit 316218b, D-155, P-50 closed) → E-12..E-17; revises D-003, D-011 (scope rule wording), D-002/D-008 ("four keys")
**Decision**: Adopt rein D-155 r1–r7 as written (contract items are decided in `rein`), and settle the four consequences that are ours to decide.

### Why needed
rein wrote its release blueprint against its code and found the contract wrong in three places that our shipped code follows. Checked against our tree on 2026-11-09:
- **Profiles (r3).** `AllowedKey::Hooks` exists and `profiles apply` copies the default profile's `hooks` object into every other profile. Gate hooks are not profile settings at all; the copy must go.
- **Verifier (r1).** `verify.rs` accepts directory entries and pax extension headers and finds `manifest.json` anywhere in the archive; `in_layout` accepts a `files` path that is only operator-owned (a top-level `accounts.json` could ship). rein's writer emits none of these.
- **Writer (r2).** `sync::plan` drops every shipped path that matches `operator_owned`. `mcp/orchestrator-mcp/package-lock.json` is shipped and matches `mcp/*/package-lock.json`, so today it would be skipped — and `rein install` now checks the installed tree against the manifest and exits 3 on the difference. Every app install would fail. The home also needs the unpacked `manifest.json`, which we do not write.
D-155 r2 also confirms the D-011 scope rule (nothing outside the mirrored dirs is touched, listed or not), which answers our open question (a).

### Our own decisions (not contract items)
1. **Strict reader.** We refuse everything rein's writer does not emit — directory entries, links, pax/GNU extension headers, `manifest.json` not first, a `VERSION` member — instead of tolerating harmless-looking extras. An extension header can rename the member that follows it; a reader that accepts only typeflag `0` has no such parsing difference to exploit. Member *order* after the first is not enforced (rein's own `verify` does not enforce it either).
2. **Rollback runs `rein install`, then `rein doctor`.** D-011 said doctor only. With D-155 r4 the toolchain pin and the install stamp are written by `rein install` from the installed tree; after a rollback they would describe the newer release. A rollback is an install of the older payload and gets the same two calls. The snapshot therefore carries the home's `manifest.json`; rolling back to a clone-install payload removes it so `rein install` is not mistaken for artifact mode.
3. **Exit 13 for a failed `rein install`.** Until now a failed `rein install` and a failed `rein doctor` both exited 3. rein's exit 3 from `rein install` means "the tree on disk is not the manifest's" — right after our own sync that is our bug or a concurrent change, and callers (the Phase 2 window first) must tell it from an unhealthy doctor. rein's code is printed, never remapped further.
4. **The baked key comes from a committed file.** rein D-155 r6 commits the public key as `docs/contract/release-key.pub` and names it as the file we copy. We commit a byte-identical copy and build from it, replacing the `REIN_RELEASE_PUBKEY` repository variable: a variable can be changed in repository settings with no review and no history, and a source build has no key at all. Deferred as E-16 until the operator has generated the pair; the variable path keeps failing closed meanwhile.

### Alternatives considered
1. **Keep copying `hooks` "because it worked"** — contradicts the contract and writes gate configuration somewhere `rein doctor` does not audit. Rejected.
2. **Tolerate directory entries and pax headers** (current behaviour) — compatible with more tar writers, none of which sign rein releases. Rejected for the parsing-difference surface.
3. **Keep skipping shipped operator-owned paths and ask rein to drop the tracked lock file** — rein rejected that itself (D-155 alt 4): it would change what a clone install lays out.
4. **Have the app repair a tree mismatch** — reimplements `rein install`'s check; contract non-goal.
5. **Adopt as above (chosen).**

### Constraints driving this decision
- Contract direction: a change is a D-### in `rein`; we revise, never the reverse (architect.md §2).
- No release has shipped, so there is no migration: no profile in the wild carries a `hooks` key copied by the app, and no E-7-format snapshot lacks a `manifest.json` that should have one.
- No agent generates or handles a release key (rein D-155 r6).

### Impact
- Unlocks: E-12 (verifier), E-13 (writer), E-14 (profiles), E-15 (CLI); E-16 and E-17 wait on the operator's key and rein E-422; E-9 now waits on rein E-417/E-418/E-420.
- Still open with rein (question (b) of D-011): who removes a retired MCP server's directory under an app install. By D-155's rule its `node_modules` stays; `install-rein.sh` purges it. E-9 will show it.
- Risk if wrong: the strict reader refuses a future rein writer change (e.g. pax for long names). That is a schema/contract change on rein's side and would arrive as a notice; the refusal message names the member.

### Rollback
Each task reverts on its own. Reverting E-13 alone makes every artifact install fail rein's tree check, so E-12/E-13 revert together with a hold on releases.

---

### D-014 Addendum 2026-11-09 — rein's answers (rein D-155 addendum, commit 3166bb3)
- **Retired MCP server directories: `rein install` removes them, the app's writer does not.** The mirror rule is unchanged (an unshipped `mcp/<retired>/node_modules/` matches `operator_owned` and is kept by our sync); `rein install` in artifact mode then removes any `mcp/<name>/` with no shipped file beneath it, and the legacy `gemini/` / `agents/` mirrors. The layout-equality test (E-9) therefore compares after `rein install` on both sides and expects no retired-server difference. This closes question (b) of D-011; nothing is open with rein.
- **Our write order is confirmed and both in-between states fail closed in rein**: `manifest.json` disagreeing with `VERSION` (or without one) is `[INSTALL_INCOMPLETE]`, exit 3; a differing tree is `[INSTALL_TREE_MISMATCH]`. No shared lock. What this asks of us: never end a successful or restored run with the pair in disagreement — already required of step (f) and rollback in E-13.
- **Rollback then `rein install` is guaranteed safe**: the stamp is rewritten (`source: "restored"` when the snapshot came from a clone install). D-014 decision 2 stands as written.
- **The strict reader is backed by a writer guarantee**: no extension record, directory entry or link is ever emitted; lifting that is a schema change with a notice to us first. D-014 decision 1 stands.
- No task changes. E-9's registered text still calls a retired server's directory an expected diff; `distribution.md` §Test Plan governs.

---

## D-015 — E-12..E-15 review: `shipped_owned` and two stricter refusals accepted; a newly shipped owned path may outlive a restore; settings are preserved by value, not by byte; `rein ci run` is not test evidence here

**Date**: 2026-11-10
**Task**: review of E-12..E-15 (Engineer handoff 2026-11-10 13:54 UTC; divergences 1–3; security review E-13 P2-2, E-14 P2; tests review E-12 P2) → E-18, E-19
**Decision**: Accept E-12..E-15 as built with no code follow-up, record the three divergences in `distribution.md`, accept the two security P2s as documented behaviour, and name `cargo test --workspace` plus hosted CI as the only test evidence in this repository.

### Why needed
The Engineer shipped the D-014 corrections with three things the blueprint did not say and three P2 findings left for a ruling. Two of the findings turn on wording in the blueprint ("byte-identical", "owned paths are never deleted") that the code cannot or should not satisfy literally.

### Rulings
1. **`shipped_owned` in a snapshot's `layout.json` — accepted.** Since D-014 a snapshot may hold a path that matches an `operator_owned` pattern (the shipped lock file). Without a list, `snapshot::load` would have to accept any owned-matching path in `tree/`, and a restore could then write over operator data. The list keeps the old refusal for everything the snapshot did not record as shipped. It is not a trust boundary (D-013: the file is editable by the home's owner); it keeps an honest snapshot from being read too widely. The field is optional and defaults to empty; unknown fields are refused.
2. **A directory at the home's `manifest.json` or `VERSION` is a `Conflict` in preflight — accepted.** Otherwise the rename fails after the whole sync and forces a restore.
3. **Header-only first pass in the verifier — accepted.** Stricter than "before any byte reaches the home": nothing is extracted even into staging before every member rule has passed.
4. **E-13 SEC P2-2 — accepted, no cleanup.** When a release ships, for the first time, a path that matches an owned pattern and the sync then fails (or the release is rolled back), the file stays. Related: a snapshot records an owned-matching path only when the incoming source ships it.
5. **E-14 SEC P2 — the guarantee is value identity with key order kept.** The app parses and re-serialises `settings.json`; whitespace can change. The blueprint's "byte-identical" for `hooks` and "byte-for-byte in value" are reworded. D-008's list of restored keys reads `enabledPlugins`, `extraKnownMarketplaces`, `attribution` since D-014; its entry is left as written (append-only log).
6. **E-12 TESTS P2 — closed, nothing to do.** The symlink, directory, hard-link, pax, NUL-typeflag and GNU long-name rows assert `kind` in the tree as committed; the one row that does not ("directory entry carrying data") is refused by the same rule as its neighbour.
7. **`rein ci run` skips every step in a Rust repository and still reports PASS.** Evidence for DONE here is `cargo test --workspace` and the hosted matrix. The missing cargo step is a framework gap and goes to `rein · Architect` as a notice; it is not fixed in this repo.

### Alternatives considered (ruling 4)
1. **Delete the lingering file on restore** — the writer knows within one run that the path was absent before (it is shipped and not in the snapshot). Rejected: rein D-155 r2 says a path not in the installed release's `files` that matches a pattern is kept, and that rule is a contract item decided in `rein`; for the rollback case the app has no record across runs of who wrote the file, so the two paths would behave differently.
2. **Make the snapshot hold every path the *installed* release shipped** (parse the live `manifest.json` when taking a snapshot) — closes the related gap, at the price of a second parser input from unsigned local data inside the security-reviewed writer, for a case that needs rein to change its set of shipped owned-matching paths between two releases. Today there is one such path and every release ships it. Rejected for now.
3. **Accept and document, with the recovery (chosen)** — the worst outcome is a stale file at an owned path; if `rein install` reports a tree mismatch the app exits 13 and `rein-app install --version X` rewrites every shipped path.

### Alternatives considered (ruling 5)
1. **Edit the file textually so untouched bytes stay** — a JSON-aware splice of three keys; a new parser-level component in the trust-boundary-3 code to preserve whitespace nobody depends on. Rejected.
2. **State the real guarantee (chosen).**

### Constraints driving this decision
- Contract items are decided in `rein` (architect.md §2); `operator_owned` semantics are one.
- No release has shipped: no snapshot without `shipped_owned` exists that should have it.
- E-12/E-13/E-14 carry ARCH/TESTS/SEC passes; reopening reviewed Tier 3 code needs a defect, and none was found.

### Impact
- Unlocks: E-18 (operator docs — README and guide state these behaviours), E-19 (release rehearsal without publishing). E-6, E-9, E-16, E-17 stay blocked on the operator and rein.
- Risk if wrong: (4) a double rollback across releases that differ in shipped owned-matching paths ends in exit 13 until `install --version` is re-run. (5) a tool that diffs `settings.json` textually sees a whitespace change after `profiles apply`.

### Rollback
Rulings 1–3 describe shipped code; reverting them means reverting E-12/E-13. Ruling 4 is reversed by a task implementing alternative 2. Ruling 5 by alternative 1.

---

## D-016 — CLI surface as built is the contract for §API (exit 1, global `--home`, `--dry-run`); exit 6 retired; two exit-code collisions fixed; NOTICE ships in every artifact

**Date**: 2026-11-10
**Task**: Engineer findings on E-18 (six §API / `--help` mismatches) and E-19 (rehearsal findings 1–3) → E-20, E-21
**Decision**: Bring the §API table in line with the binary where the binary is right, change the binary in the two places where its exit codes are ambiguous, and add the license files to every artifact before any tag.

### Why needed
E-18 documents the CLI for operators and the §API table is what Phase 2 and scripts read. The table had drifted from the binary in six places, and checking them against the source turned up two exit codes that mean two things. E-19 found that no artifact carries `NOTICE`, and the `.msi` and `.deb` carry no `LICENSE` either.

### Rulings (E-18)
1. **Exit 1 = refused before any write, or any other error, on every command** — added to §API. It was always in the code and never in the table.
2. **`--home DIR` is global** — accepted; `profiles` needs it for `accounts.json`, and one rule is easier to document than a per-command list.
3. **`profiles apply --dry-run`** — accepted: prints the diff, writes nothing, runs no `claude` command.
4. **`--profile` repeatable, `--disable all`** — accepted.
5. **`launch [DIR]` exits 4 when the distro has no `rein-app`** — accepted (4 is "runtime missing" everywhere).
6. **Exit 6 is dropped and retired.** A forbidden key cannot be written by code that compiles, so the row described nothing. The number is not reused.
7. **`doctor` must not exit 1 for its own failures (found in this review).** `rein doctor --json` exits 0 or 1 with a JSON document on stdout; the app returned 1 too when rein could not be started or ended without a code, so a caller could not tell "status fail" from "no document". Those cases become exit 14.
8. **A usage error must not exit 2 (found in this review).** clap's default 2 is also "release failed verification" on `install`. Usage errors become 64.

### Rulings (E-19)
9. **`LICENSE` and `NOTICE` ship in every artifact** (Apache-2.0 §4(d)) — E-21, before the first tag.
10. **A prerelease tag (`v0.1.0-rc.1`) as the first real exercise of the pipeline** is recommended to the operator; it publishes a public GitHub prerelease, so it is the operator's call and action, not a task. It needs the release key first — the build fails closed without one.
11. **rein's `docs/app-contract.md` does not exist yet** (E-18 item 7): the operator docs name it in plain text, unlinked, as the file rein publishes with its first release, and may link the `Vimurai/rein` repository. Leaving the reference out would hide where the contract lives; a dead link breaks §Components 9. The link is added when rein publishes the file.

### Alternatives considered
1. **Change the binary to match the old table** (drop `--dry-run`, make `--home` install-only) — removes working, tested behaviour to protect a table. Rejected.
2. **Keep exit 6 "for the future"** — a documented code no path can produce misleads operators and Phase 2. Rejected.
3. **Leave `doctor`'s exit 1 and have callers test for empty stdout** — works, but the blueprint already promises "0 or 1 means stdout is the document"; one code change keeps the promise. Rejected.
4. **Leave clap's exit 2 and document the overlap** — a script that retries on "verify failed" would retry a typo. Rejected; the fix is a few lines.
5. **As ruled (chosen).**

### Constraints driving this decision
- No release has shipped, so no exit code is relied on by anyone yet; changing two now is free, later it is a breaking change.
- Exit codes 0 and 1 of `doctor` belong to rein (contract); the app's own codes stay ≥ 10 or follow sysexits (64).

### Impact
- Unlocks: E-18 (documents the revised §API after E-20), E-20, E-21.
- Risk if wrong: `rein doctor` itself could one day exit 14 or 64 — rein's contract lists 0 and 1 only for `--json`; a collision would arrive as a contract notice.

### Rollback
Revert E-20 and restore the two rows; E-21 is config and reverts on its own.

---

### D-014 Addendum 2026-11-10 — the operator has generated the release key; E-16 opens before rein's commit; a one-time key-file move by the Engineer
- **Fact:** the operator generated the minisign pair on 2026-11-10 (`~/release-key.pub`, `~/release-key.key`). No agent has read either file's secret content.
- **Operator instruction (explicit, this session):** the Engineer moves the secret key file to the operator's iCloud Drive and places the public key in the rein checkout. This is a one-time exception to "no agent handles a release key", limited to a file move: the secret key is never opened, printed, hashed into a log, copied anywhere else or committed. The rule stands for everything else — no agent signs, generates or substitutes a key.
- **E-16 opens now.** D-014 decision 4 made E-16 wait until rein had *committed* `docs/contract/release-key.pub`. The reason was to be sure both repos carry the same bytes. The same file from the same operator is now in rein's working tree (uncommitted — rein's own session commits it, under rein's gates), so E-16 copies it from there. **Check that replaces the wait:** before the first tag (E-6), the copy in this repo is compared byte for byte with the file as committed on rein's `main`; a difference blocks the tag.
- **Not delegated:** rein's release tooling (rein E-417..E-422) and the commit in rein are rein's work, done by rein's sessions. The rein-app Engineer writes no rein source and makes no commit there.

---

## D-017 — E-16 review: one tested guard script, a tag gate that compares our key with rein's `main`, CODEOWNERS; the production key signs no fixture

**Date**: 2026-11-10
**Task**: review of E-16, E-18, E-20, E-21 (Engineer handoff 2026-11-10 14:42 UTC; security review E-16 P2-1..P2-3) → E-22, E-23
**Decision**: Accept E-16, E-18, E-20 and E-21 as built; turn the pre-tag key comparison into a workflow step that blocks the release; move the key-file guard into one script with a test; add CODEOWNERS; and refuse an operator-signed test fixture.

### Why needed
E-16 made a committed file the trust root. Three things around it rest on a person remembering: comparing the file with rein's copy before a tag (D-014 addendum), reviewing a change to it, and keeping three pasted copies of the guard shell identical — a guard that has never run on macOS or Windows Git Bash.

### Rulings
1. **Tag gate — scripted, blocking (P2-1).** On a tag push, before any artifact build, the release workflow fetches `docs/contract/release-key.pub` from rein's `main` and compares it byte for byte with ours. Absent or different fails the release. This also makes the order mechanical: no rein-app tag can build until rein has committed the same key. The fetched bytes are never built from, so "the app never fetches a key" stands.
2. **CODEOWNERS (P2-1) — yes**, for `docs/contract/**`, `.gitattributes` and `.github/**`. It binds only once branch protection requires code-owner review, which is the operator's repository setting.
3. **Guard shell (P2-3) — one script, tested on three OSes.** `.github/scripts/check-release-key.sh` replaces the three copies; a fixture corpus runs through it in the CI matrix, so Git Bash and macOS are exercised before a tag, not by one.
4. **No fixture signed with the production key.** The secret key signs rein releases and nothing else. A signed test blob would be a production signature over non-release content, would need the operator to fetch the key back for a test, and proves only what the first real release proves anyway. The parse, key-id and same-code-path tests stand as E-16's acceptance.
5. **Stale blueprint text fixed**: `REIN_RELEASE_PUBKEY` is gone; every build bakes the committed key; `reject-all-signatures` is the trust-nothing build.
6. **E-21 as built accepted**: `allow-dirty = ["msi"]` and README in the archives. The non-ASCII comment in `main.wxs` (declared windows-1252) becomes ASCII — E-23.
7. **E-20 as built accepted**, including a rein exit code outside 0..=255 mapping to 14.

### Alternatives considered (ruling 1)
1. **Keep it a checklist line in E-6** — the check that guards the trust root would depend on memory on the one day everything else is also new. Rejected.
2. **Vendor rein as a git submodule and compare locally** — pins a rein commit in this repo for one 113-byte file and adds a submodule to every clone. Rejected.
3. **Fetch and compare in the release workflow (chosen)** — one HTTPS GET to a public repository, compared and discarded.

### Constraints driving this decision
- The comparison must fail closed: a 404 (rein has not committed the key) is a failure, not a skip.
- Pull-request runs must not need rein's copy to exist; the gate runs on tag pushes only.
- No agent handles the secret key (D-014; the one-time move on 2026-11-10 was the operator's instruction and is over).

### Impact
- Unlocks: E-22, E-23. E-6 gains a hard precondition: rein's `main` carries the same public key.
- Risk if wrong: a GitHub raw-content outage at tag time fails the release; re-running the workflow is the fix.

### Rollback
Remove the gate step and restore the inline guards from 0d573f6; the baked key is unaffected.

---

## D-018 — E-22/E-23 accepted; CODEOWNERS widened; tag ruleset is the binding control

**Date**: 2026-11-10
**Task**: review of E-22, E-23 (Engineer handoff 2026-11-10 18:49 UTC)
**Decision**: E-22 is accepted as built and E-23 is closed without a change; CODEOWNERS grows to the code that consumes the key and the release metadata; the operator is advised to add a tag ruleset and not to require code-owner review while there is one human account.

### Why needed
The Engineer reported two divergences from D-017 and left one question (CODEOWNERS scope) and three repository settings to rule on.

### Rulings
1. **E-22 accepted** (6450f22, 70c55b9): one guard script, a compare script, a 26-case corpus on three OSes, the gate in `plan` after checkout. E-23 needed no change — the comment was ASCII since 6bf6244.
2. **Gate condition `!github.event.pull_request` stands.** It is the same condition as `publishing`, so every run that can publish is gated, including a future `workflow_dispatch`. Stricter than "tag pushes only"; the blueprint now says so.
3. **The guard refuses any CR byte, on raw bytes.** Git Bash drops a CR inside `$(...)`, which let a CRLF key pass on Windows only. The blueprint's guard list now names it.
4. **CODEOWNERS also covers `/crates/rein-release/`, `/Cargo.toml`, `/Cargo.lock`** (E-24): the crate that parses and uses the key, the dist metadata that shapes a release, and the lock that pins the signature library.
5. **CODEOWNERS is advisory for now.** GitHub does not let an author approve their own pull request, and every commit here is made under the operator's one account, so "require code-owner review" would block every merge or be bypassed every time. Not recommended until a second maintainer exists.
6. **A tag ruleset is recommended** (operator setting): only the operator may create `v*` tags. It also covers the one residual gap — a tag on a commit older than E-22 runs a workflow without the gate.
7. **`--max-filesize` without Content-Length (P4) accepted.** The fetched file is only compared for byte identity and then deleted; an oversized body fails the comparison.
8. **The rc tag is rehearsed before the operator pushes it** (E-25): whether cargo-dist accepts `v0.1.0-rc.1` with crate version `0.1.0` is unverified.

### Alternatives considered
1. **Require code-owner review now** — rejected: unsatisfiable with one account (ruling 5).
2. **CODEOWNERS on the whole repository** — rejected: with one owner it says nothing a narrower list does not, and it hides which paths are release-critical.
3. **Narrow list + tag ruleset** — chosen.

### Constraints driving this decision
- rein D-155 r6 / D-014: the trust root is a committed copy of rein's file; the operator alone releases.

### Impact
- Unlocks: E-24, E-25. E-6 preconditions unchanged: `HOMEBREW_TAP_TOKEN`, rein's `main` carrying the key, a signed rein release.
- Risk if wrong: without the ruleset, anyone with push rights could tag an old commit and release ungated; today only the operator has push rights.

### Rollback
Drop the added CODEOWNERS lines; the ruleset is a repository setting the operator can delete.

---

## D-019 — E-24/E-25 accepted; the rc version bump waits for the operator; no WiX change for rc→final

**Date**: 2026-11-10
**Task**: review of E-24, E-25 (Engineer handoff 2026-11-10 18:57 UTC)
**Decision**: E-24 and E-25 are accepted. The rc tag needs a version bump that is made only when the operator says the rc goes ahead. The Windows `.msi` is left as is: an installed rc is uninstalled by hand before the final `.msi`.

### Why needed
E-25 found that cargo-dist releases a tag only when it equals the package version, and that cargo-wix maps `0.1.0-rc.1` to MSI `0.1.0.1`, which Windows Installer does not treat as older than `0.1.0`.

### Rulings
1. **E-24 accepted** (5655b9e). **E-25 accepted**: the record and the operator sequence in `.ai/DEVOPS.md` are the release rehearsal of record for tags.
2. **The version bump is not made now.** An rc tag cannot pass the tag gate until rein commits the key on `main`, and a bumped `master` would ship `0.1.0-rc.1` in any source build meanwhile. When the operator says go, one Engineer task sets the version to `0.1.0-rc.1` in the three manifest places plus `Cargo.lock`; a second sets it back to `0.1.0` before the final tag.
3. **No WiX change.** The rc `.msi` is installed only by whoever tests the prerelease. `AllowSameVersionUpgrades` would also let any same-version `.msi` replace an installed one, for every later release, to solve a one-time step. The operator guide's release section says: uninstall an rc `.msi` before installing the final one.
4. **An rc needs no `HOMEBREW_TAP_TOKEN`** (the Homebrew job is skipped for prereleases); the final tag does.

### Alternatives considered
1. **`AllowSameVersionUpgrades` in `main.wxs`** — rejected (ruling 3).
2. **Skip the rc and tag `v0.1.0` directly** — rejected: the `.msi`, macOS and `.deb` jobs have never run; a failed final tag burns the version.
3. **Bump now** — rejected (ruling 2).

### Constraints driving this decision
- cargo-dist 0.34.0 tag/version equality; Windows Installer compares three version fields.

### Impact
- Unlocks: nothing new; the Engineer queue is empty until the operator calls the rc or rein delivers.
- Risk if wrong: a tester keeps an rc `.msi` installed and gets two entries or a refused install of the final; uninstalling fixes it.

### Rollback
Add the WiX attribute later if rc builds become routine.

---

## D-020 — Mirror rein D-155 addendum 3: the verifier refuses non-ASCII and case-colliding paths

**Date**: 2026-11-10
**Task**: E-26 (relayed by the Engineer from rein c07b4e4)
**Decision**: `rein-release` refuses, at manifest parse, any manifest whose paths are not printable ASCII or that contains two paths equal under ASCII-lowercasing but different in bytes (directory prefixes included).

### Why needed
rein's generator now refuses such path sets (`[RELEASE_CASE_COLLISION]`). On a case-insensitive filesystem (macOS default) a colliding release would have one file silently overwrite another during the in-place sync, after verification passed. Our verifier is a strict reader that accepts only what rein's writer emits (rein D-155 r1), so the reader carries the same rule.

### Rulings
1. The rule is rein's, mirrored exactly: printable ASCII 0x20–0x7E; ASCII-only folding (A–Z); every directory prefix of a path counts.
2. It is checked in the manifest parse, before signature and tarball work. The tarball member set must equal `files`, so members need no second check.
3. The outcome is a verification failure, exit 2. No new exit code.
4. It applies on every OS, not only macOS: one rule, one test set, and a home can be moved between filesystems.
5. Snapshot `layout.json` is not re-checked: its paths came from a manifest that passed.
6. Not ruled: Windows-reserved names (rein has not ruled them), and a shipped path that differs only by case from an operator-owned file already in the home.

### Alternatives considered
1. **Trust the signature** — rejected: a signed artifact from an older or buggy generator would still overwrite silently; the strict-reader rule exists for this.
2. **Check in the layout writer against the live filesystem** — rejected: depends on the machine, and refuses after staging instead of before anything is read.
3. **Check at manifest parse** — chosen.

### Constraints driving this decision
- rein D-155 r1 (strict reader), D-155 addendum 3; contract items are decided in rein and mirrored here.

### Impact
- Unlocks: E-26.
- Risk if wrong: a legitimate release with a non-ASCII or case-distinct path is refused — but rein's generator cannot produce one.

### Rollback
Remove the two checks; no stored format changes.

---

## D-021 — E-26 accepted; case-variant prunes run before the writes; post-sync existence check

**Date**: 2026-11-10
**Task**: review of E-26 (Engineer handoff 2026-11-10 22:57 UTC); E-27
**Decision**: E-26 is accepted. The sync removes a prune entry that is a case variant of a shipped path before it writes, and checks every shipped path after the sync.

### Why needed
The security review of E-26 found a case D-020 does not cover: release N has `shared/locate.sh`, release N+1 ships `shared/Locate.sh`. `plan` compares names exactly, so the old name is a prune entry; `apply` writes, then prunes. On a case-insensitive home (APFS, drvfs) both names are one file, so the prune deletes the file just written and the install exits 0. Reproduced by the Engineer on drvfs. Rollback and failure-restore have the same shape.

### Rulings
1. **E-26 accepted** (24ed040). The check runs where `Manifest::parse` always ran — after the signature check and the member scan, before staging and any write. D-020 ruling 2's "before signature" was wrong wording; the code is right, and parsing after the signature check is the better order.
2. **Case-variant prunes go first.** A prune entry whose ASCII-lowercased path equals that of a shipped path (or a directory prefix of one) is removed before that directory's writes. Other prunes stay after the writes. Same outcome on both filesystem kinds, and no dependence on probing the filesystem.
3. **Post-sync check.** Every shipped path must exist as a regular file of the listed size; a miss is a sync failure and takes the existing failure-restore. This turns any future "exit 0 with a file missing" into a refused update.
4. **`operator_owned` patterns join the ASCII rule**, and matcher errors escape non-printable bytes (terminal-control bytes from a manifest must not reach the terminal).
5. **Not ruled here:** NTFS 8.3 short names (`LONGFI~1.SH`). A Rein home on NTFS is not a supported install target (Windows installs inside WSL); it is passed to rein as a note.

### Alternatives considered
1. **Never prune a case variant** (Engineer's option a) — rejected: on a case-sensitive home the old file would stay beside the new one, and the mirror would no longer equal the manifest (E-9).
2. **Prune everything before writing** (option b in full) — rejected: every removed file would be missing for the whole write phase; today a reader sees old or new.
3. **Post-check only** (option c) — rejected alone: it detects the loss, then restores the old release, so a case-only rename could never be installed on macOS.
4. **Case-variant prunes first + post-check** — chosen.

### Constraints driving this decision
- In-place sync where a reader sees old or new (D-011); mirror equals manifest (rein D-155 r2).

### Impact
- Unlocks: E-27.
- Risk if wrong: a case-renamed file is absent between its removal and its write within one directory pass; a crash there is covered by the snapshot restore.

### Rollback
Revert the prune ordering; the post-sync check is independent and can stay.

---

## D-022 — Profile enumeration: an accounts row beats the built-in `personal`; only `default` is reserved

**Date**: 2026-11-10
**Task**: E-28 (bug seen on the operator's machine, relayed by the Engineer)
**Decision**: `accounts.json` rows are listed before the built-in `~/.claude-personal`; a row named `personal` is an ordinary account whose `config_dir` is authoritative; only `default` is reserved; row names get a terminal-safety check instead of a second name grammar.

### Why needed
`rein-app profiles list` warned `account "personal": name is reserved … skipped` and then listed `personal`. The built-in source reserved the name, so the real row was skipped. Here both pointed at the same directory, so only the warning was wrong; a `personal` row pointing elsewhere would have been dropped silently and never received the allowed-key writes. Our name grammar also differed from rein's (rein D-153 addendum 2: a reader relies on `config_dir` and accepts what `accounts.py` wrote).

### Rulings
1. Order: `default`, then accounts rows, then the built-in `personal` only when no row has that name and its directory is not already listed.
2. Only `default` is reserved (it is never a row in rein's file). A `default` row pointing at `~/.claude` is silent; pointing elsewhere it is skipped with an accurate warning.
3. Name check is terminal safety only: printable ASCII 0x21–0x7E, at most 64 bytes; warnings show a failing name escaped.
4. No warning for a row that is listed.

### Alternatives considered
1. **Drop the built-in `~/.claude-personal`** (Engineer's option a) — rejected: a machine with that directory and no `accounts.json` row would lose a profile it has today; the built-in costs nothing once a row outranks it.
2. **Keep the built-in first, silence the same-directory case** (option b as stated) — rejected: a row pointing elsewhere still loses to a guess.
3. **Row first, built-in as fallback** — chosen.

### Constraints driving this decision
- rein D-153 addendum 2: `accounts.py` is the only writer and owns the name grammar; readers rely on `config_dir`.
- E-4 P2: names reach the terminal, so control bytes are refused.

### Impact
- Unlocks: E-28.
- Risk if wrong: a hand-edited row with an odd but printable name is now listed where it was skipped; its `config_dir` still passes the existing absolute-path checks.

### Rollback
Restore the previous `enumerate`; no stored data changes.

---

## D-023 — A shipped case variant of an owned path is refused; owned protection folds ASCII case

**Date**: 2026-11-10
**Task**: E-29 (security review of E-27, relayed by the Engineer); ratifies one E-27 detail
**Decision**: The verifier refuses a shipped path that matches an operator-owned pattern only after ASCII-lowercasing; the "is this path owned" decision that protects live files folds ASCII case; E-27 is cleared for sign-off with this finding fixed in E-29, not accepted as a risk.

### Why needed
On a case-insensitive home, a release shipping `shared/Toolchain.sh` against the owned pattern `shared/toolchain.sh` writes release bytes over the operator's file; a later release that stops shipping it prunes it, because the case-sensitive matcher no longer sees it as owned. Both installs exit 0. Separately, and older than E-27: an operator's own `shared/Toolchain.sh` is pruned by any update.

### Rulings
1. **Refuse at manifest parse** a `files` path (or directory prefix) that matches an owned pattern only after folding. An exact match stays allowed (shipped-path-wins, D-015).
2. **Protection folds ASCII case on every filesystem.** A live path that is a case variant of an owned pattern is never opened, snapshotted or pruned. Over-protecting costs a stray file on a case-sensitive home; under-protecting costs operator data.
3. **E-27 detail ratified:** an empty case-variant directory is removed before the writes so the new spelling lands. A directory that still holds unshipped or owned content keeps its old spelling (review P3-2, accepted).
4. **A live symlink that is a case variant of a shipped path** is a preflight `Conflict` naming it (review P3-1).
5. **All manifest-derived strings are escaped in error text** (review P3-4; completes D-021 ruling 4).
6. **Accepted, no change (review P3-3):** APFS also folds some non-ASCII characters; our fold is ASCII-only. Shipped paths are ASCII by rule, so this needs a non-ASCII live name, and the post-sync check then refuses and restores.
7. **E-27 (9adfd72) is cleared for sign-off.** Ruling 1's gap exists with or without E-27; E-29 closes it before any release is installed.

### Alternatives considered
1. **Accept the risk** — rejected: silent loss of operator data with exit 0.
2. **Probe the filesystem and apply the rules only where it is case-insensitive** — rejected: machine-dependent verification, and a home can move.
3. **Refuse at parse + fold for protection, everywhere** — chosen.

### Constraints driving this decision
- rein D-155: operator-owned paths are never opened, snapshotted or pruned; D-015 shipped-path-wins for exact paths.

### Impact
- Unlocks: E-29. The matcher's protective fold is stricter than rein's written contract; rein's Architect is told.
- Risk if wrong: a release legitimately shipping a case variant of an owned name is refused — rein's own case rule makes that unlikely.

### Rollback
Remove the parse refusal and the fold; no stored format changes.

---

## D-024 — E-28 accepted; an accounts row with an unsafe `config_dir` is skipped; unlisted rows never hide the built-in

**Date**: 2026-11-10
**Task**: review of E-28 (7df5c73); E-30
**Decision**: E-28 is accepted. A row whose `config_dir` holds control or bidi-override characters is skipped rather than escaped at each print; the dry-run command line shell-quotes its path; a row that is not listed never suppresses the built-in `personal`.

### Why needed
Since E-4, an absolute `config_dir` from `accounts.json` reached the terminal raw: in the `profiles list` table, in errors, and in an unquoted `would run CLAUDE_CONFIG_DIR=… claude …` line that invites copy-paste. The security review of E-28 showed it with `"/h/\u001b[2J; rm -rf ~ #"`. E-28 also left three enumeration edges unspecified.

### Rulings
1. **E-28 accepted**, including its divergence: an unusable `personal` row gets a warning and does not suppress the built-in.
2. **Skip at the source** (reviewer's option b): `config_dir` with C0, DEL, C1 or bidi override/isolate characters → row skipped, warning escaped. No real config directory contains these, and one check covers every present and future print.
3. **The dry-run line shell-quotes the path** — spaces and quotes are legal in a path.
4. **Duplicate rows warn**, naming the profile that already holds the directory; a row that is not listed, for any reason, never suppresses the built-in (review P3-3).
5. **`apply` notes an unusual target**: one line when `config_dir`'s final component does not start with `.claude` (review P3-1). rein-app still writes there — `accounts.json` is the operator's statement — and the guide says so (P3-2).
6. **No change** for `same_dir` on missing directories (P3-4).

### Alternatives considered
1. **Escape at every print** (option a) — rejected: every new print site must remember; the unsafe path would still be handed to `claude` as an environment value.
2. **Refuse to write outside `$HOME/.claude*`** — rejected: rein's `accounts.py` allows any absolute directory; the contract is rein's.
3. **Skip unsafe rows + quote + note** — chosen.

### Constraints driving this decision
- rein D-153 addendum 2 (readers rely on `config_dir`; rein owns the file).

### Impact
- Unlocks: E-30.
- Risk if wrong: an account whose directory truly contains such a character is not managed by rein-app; the warning says why.

### Rollback
Remove the skip; quoting and the warnings are independent.

---

## D-025 — D-023 ruling 2 withdrawn: owned matching stays case-sensitive, as rein's table rules

**Date**: 2026-11-10
**Task**: E-29 (blocked by the Engineer on a contract conflict)
**Decision**: The protective ASCII case fold of D-023 ruling 2 is withdrawn. Operator-owned matching is case-sensitive, exactly as rein's conformance table says. E-29 ships the parse refusal, the symlink Conflict and the error escaping, without the fold.

### Why needed
rein's published table (`tests/fixtures/release/operator-owned-cases.json` at 3f68eff, and our interim copy) has the row: path `insights.md`, pattern `INSIGHTS.md`, not shipped → **deleted**, labelled "matching is case-sensitive". D-023 ruling 2 made that row "kept". Contract items are decided in rein and mirrored here (D-001); I ruled on one without checking the table. The Engineer stopped instead of editing the fixture, which was right.

### Rulings
1. D-023 ruling 2 is withdrawn. D-023 rulings 1 and 3–7 stand.
2. E-29's item (2) is dropped; its code is removed from the working tree before commit. The conformance table is not edited.
3. The residual case is accepted for now: a file the operator creates as a case variant of an owned name (e.g. `shared/Toolchain.sh` beside the pattern `shared/toolchain.sh`) is an unshipped file — it is snapshotted and then pruned, recoverable from the snapshot for two updates. Files written by rein's own tooling use the exact spelling.
4. The question goes to rein's Architect: should owned protection fold ASCII case (table row → kept, `install-rein.sh` to match)? rein-app follows the answer.

### Alternatives considered
1. **Keep the fold and change the fixture row** — rejected: a contract change made in the wrong repository; E-17 vendors rein's table verbatim and would fail.
2. **Fold only on a case-insensitive home** — rejected in D-021: machine-dependent behaviour.
3. **Withdraw the fold, ask rein** — chosen.

### Constraints driving this decision
- D-001 / rein D-150: contract direction. E-9 layout equality with `install-rein.sh`.

### Impact
- Unblocks E-29.
- Risk if wrong: the residual case in ruling 3.

### Rollback
If rein amends its table, restore the fold from the Engineer's E-29 working tree in a new task.

---

## D-026 — Directory spellings that are case variants of an owned pattern's leading segments are refused at parse

**Date**: 2026-11-10
**Task**: E-29 (security review round 2, relayed by the Engineer)
**Decision**: `Manifest::parse` also refuses a `layout.mirrored` name, or a directory prefix of a shipped path, that is ASCII-case-equal to but byte-different from the leading literal segments of an operator-owned pattern. The fold-only test of D-023 ruling 1 is per pattern.

### Why needed
Release with `mirrored: ["bin","Shared"]`, `operator_owned: ["shared/toolchain.sh"]`, files `bin/rein`, `Shared/x.sh` passes parse and verify. On a case-insensitive home the scan lists the operator's file as `Shared/toolchain.sh`, the case-sensitive owned test misses, and the file is pruned — a file that matches its pattern exactly. Reproduced by the Engineer on drvfs. This is not the residual accepted in D-025.

### Rulings
1. Refuse at parse (Engineer's option a), applied to mirrored names and to every directory prefix of a shipped path. Exit 2, naming the spelling and the pattern.
2. D-023 ruling 1 is per pattern: a shipped path that any one pattern matches only after folding is refused, even when another pattern matches it exactly — the file the first pattern protects would be overwritten.
3. In E-29 as well: the case-variant symlink `Conflict` gets its own wording (it is not "file vs directory"). The refusal that names the shipped spelling rather than the link's (review P3-4) stays as it is.
4. If rein's real tree at 3f68eff or any row of rein's tables trips ruling 1 or 2, stop and report — do not loosen.

### Alternatives considered
1. **Scan with each root's on-disk spelling** (option b) — rejected: reads the filesystem to decide ownership, covers roots but not deeper prefixes, and behaves differently per machine.
2. **Accept** — rejected: loss of an exactly-owned operator file with a success exit.
3. **Refuse at parse** — chosen: same model as D-020 and D-023 ruling 1.

### Constraints driving this decision
- rein D-155 r1 (strict reader); rein addendum 3 r9 keeps matching case-sensitive, so the spellings must be unambiguous instead.

### Impact
- Unblocks E-29. Stricter than rein's generator rule r8 (which compares shipped paths with patterns); rein's Architect is told.
- Risk if wrong: a legitimate manifest is refused; rein's mirrored names and owned patterns are fixed lowercase spellings today.

### Rollback
Remove the check; no stored format changes.

---

## D-027 — E-29 accepted; manifests get size limits at parse

**Date**: 2026-11-11
**Task**: review of E-29 (f298c5b); E-31
**Decision**: E-29 is accepted as built after five security rounds. The remaining cost findings are closed by refusing oversized manifests at parse and by making three lookups linear, in one Tier 2 task after E-9.

### Why needed
The E-29 review left four performance findings: pattern × file × depth cost with no cap, `under_mirrored` quadratic in mirrored × files, `folded_ship` memory quadratic in one path's length, and an all-pairs symlink scan per directory. All need a manifest signed with the release key.

### Rulings
1. **E-29 accepted**: parse refusals as table passes (linear in pattern × depth), `CaseVariantSymlink` with its own wording, item (2) of the task void per D-025.
2. **Limits at parse**: 64 mirrored names, 256 owned patterns, 100000 files, 1024 bytes per path or pattern, 32 segments. Each must be ≥ 10× what rein's generator emits at 3f68eff; the Engineer measures first and stops if one is not.
3. **Linear lookups**: `under_mirrored` by set; no per-prefix copies in `folded_ship`; no all-pairs comparison in the symlink scan.
4. **Tier 2, after E-9**: no trust decision changes, and E-9 supplies the real numbers.
5. rein's Architect is told the limits, since they bound what a future rein release may ship.

### Alternatives considered
1. **Leave as is** — rejected: a 10-second parse from a mistaken manifest is cheap to prevent.
2. **Only fix the algorithms** — rejected: limits make every future check bounded without re-auditing each.
3. **Limits + linear lookups** — chosen.

### Constraints driving this decision
- §Execution Constraints: install under 10 s, memory under 100 MB.

### Impact
- Unlocks: E-31 (after E-9).
- Risk if wrong: a real rein release outgrows a limit and is refused; the 10× margin and rein's notice make that a planned change.

### Rollback
Raise or remove a limit constant.

### D-027 Addendum — 2026-11-11: two limits raised to keep the 10× margin

rein's Architect reported today's numbers (tracked tree, an approximation of the generator's output): 246 files, deepest path 5 segments, longest path 59 bytes, 10 `layout.mirrored` names, 13 `layout.operator_owned` patterns; a path can never exceed 255 bytes (ustar split). Against ruling 2's own "≥ 10×" test, two first-draft limits fail: 64 mirrored names (6.4×) and 32 segments (6.4×).

- **`layout.mirrored`: 128 names. Path depth: 64 segments.** The other three stand (256 patterns, 100000 files, 1024 bytes).
- The E-31 task text still carries 64 and 32; this addendum and the blueprint override it. The Engineer still measures the generator's real output in E-9 and stops if any limit is under 10×.
- rein accepted the notice duty: growth past about half of a limit is told to rein-app before the release (to be recorded as a rein D-153 addendum). rein's generator does not enforce these limits.

---

## D-028 — E-9 first run: what the layout-equality test asserts; the rein-side findings go to rein

**Date**: 2026-11-11
**Task**: E-9 (first real run against rein 3f68eff, reported by the Engineer)
**Decision**: The test asserts the tree diff, `install` exit 0 or 3, and no install-integrity token in `rein doctor`'s output. It is committed as written, red at 3f68eff, and not weakened. Three findings are rein's to rule; rein-app changes no behaviour meanwhile.

### Why needed
rein's real artifact (252 files, v0.4.0) passes our verifier and the trees match, but `rein-app install` exits 3 every time: `rein install` rewrites the shipped `mcp/orchestrator-mcp/package-lock.json`, and rein's own tree check then reports `[INSTALL_TREE_MISMATCH]`. `rein doctor` also fails in a sterile home for host reasons, and it judges whatever directory the operator ran `rein-app` from.

### Rulings
1. **Test seam:** driving `install_cmd::install` with the real process runner is accepted. The binary gets no way to trust a test key or a test server.
2. **Two more documented differences:** `secrets/role-hmac.key` and `shared/toolchain.sh` (timestamp line) — per-install values in owned files.
3. **Assertions:** tree diff; exit ∈ {0, 3}; doctor output has neither `[INSTALL_TREE_MISMATCH]` nor `[INSTALL_INCOMPLETE]`.
4. **Red is the right result at 3f68eff.** The test is committed (it is skipped without `REIN_CHECKOUT`), the result is recorded in `.ai/DEVOPS.md`, E-9 stays open, and CI wiring waits until a rein commit makes it green.
5. **To rein, not decided here:** (a) the lock-file rewrite versus the tree check; (b) whether `rein doctor` can tell install integrity from host checks (its `--json`, rein E-421); (c) which directory an app should run `rein doctor` in after install/update/rollback, since project checks judge the operator's current directory.
6. **No interim change in rein-app:** exit 3 keeps meaning "`rein doctor` failed" (D-012: doctor judges), and the working directory is unchanged until rein answers 5c.
7. **E-31 is unblocked from E-9:** the generator's real counts are known (mirrored 10, patterns 14, files 252, longest path 55 bytes, depth 4); every limit is ≥ 12.8×.

### Alternatives considered
1. **Make the test pass by ignoring doctor's result** — rejected: it would hide the one finding that makes every install exit 3.
2. **Change rein-app to run doctor in `$HOME` now** — rejected: how the app invokes doctor is contract (rein D-153); a guess here could be wrong in the other direction.
3. **Commit strict, report, wait** — chosen.

### Constraints driving this decision
- Contract direction (D-001); D-012 (doctor judges).

### Impact
- E-6 gains a precondition: a rein release on which `rein-app install` can exit 0 on a healthy machine.
- Risk if wrong: none in shipped behaviour; the test stays red longer.

### Rollback
Not applicable — no behaviour change.
