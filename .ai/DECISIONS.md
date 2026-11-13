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

---

## D-029 — Mirror rein D-158: the install verdict comes from `rein doctor --scope install --json`

**Date**: 2026-11-11
**Task**: E-32 (blocked on rein E-427); unblocks E-17
**Decision**: After install, update and rollback, rein-app runs `rein install`, then `rein doctor --scope install --json`, and judges by the document's `status`: `fail` is exit 3, `warn` and `ok` are exit 0. This replaces "a failed `rein doctor` is exit 3" (D-012) once rein E-427 ships.

### Why needed
E-9's first run showed every install exits 3: partly a rein defect (lock-file rewrite, fixed by rein E-426 with `npm ci`), partly because a full `rein doctor` fails on a clean machine for host reasons and judges the operator's current directory. rein ruled all three (D-158) and changed the contract in two places.

### Rulings (mirrored from rein D-158)
1. No `rein` command writes a path in `manifest.files`; rein's tree check exempts nothing. Nothing to change here; our E-9 assertion is the right one.
2. Every doctor check record has a required `scope` (`install` | `host` | `project`); `schema` stays 1.
3. Installer rule 5: the app runs `rein doctor --scope install --json` and judges by `status`. Host and project checks are never an install failure.

### Rulings (rein-app's own)
4. A missing, unparsable or status-less document is exit 14, never a pass.
5. The app never filters or branches on a check id — that would be Rein logic in the app.
6. `rein-app doctor [--json]` stays a passthrough of the full `rein doctor` in the operator's directory.
7. Until rein E-427 merges, behaviour is unchanged and the layout-equality test stays red; E-32 is registered blocked.
8. E-17 is unblocked: rein `main` at d84dd17 carries `docs/app-contract.md`, the three schemas and the shared fixtures, including the corrected `operator-owned-cases.json` (addendum 3 r10).

### Alternatives considered
1. **Parse today's doctor text for install tokens** — rejected by rein and here: Rein logic in the app.
2. **Run doctor in `$HOME`** — superseded: install scope reads nothing from the current directory.
3. **Mirror D-158** — chosen.

### Constraints driving this decision
- Contract direction (D-001). Phase 1 until now "never parses" doctor output; one top-level field of a schema'd document is the narrowest possible exception.

### Impact
- Unlocks: E-17 now; E-32 when rein E-427 merges. E-6 needs rein v0.5.0, which follows E-425 → E-426 → E-427 → E-419 → E-424.
- Risk if wrong: none before E-427.

### Rollback
Return to exit-code propagation of the full `rein doctor`.

---

## D-030 — rein's profile-settings table states what an app may write; E-17's harness tests it that way

**Date**: 2026-11-11
**Task**: E-17 (one row blocked, relayed by the Engineer)
**Decision**: The `accept` rows of `profile-settings-cases.json` are permissions, not requirements. Our harness asserts that no `refuse` row is reachable, and that every `accept` row is reachable except rows on an explicit, self-checking list. The one such row, "remove an allowed key", is shown through `profiles apply --undo`.

### Why needed
12 of 13 rows pass a harness that demanded "accept ⇔ our writer can produce `after`". The row before `{"attribution":{"commit":""}}`, after `{}` is `accept`, but `rein_profiles::plan` only sets or merges allowed keys. rein-app removes an allowed key only in `--undo`, when the backup lacked it.

### Rulings
1. `refuse` rows: unreachable by our writer, no exceptions. This is the half that protects operator settings.
2. `accept` rows: reachable by `plan`, or named on a list with a reason; the harness asserts each listed row is in fact unreachable by `plan`.
3. "Remove an allowed key" is covered by a file-level undo test that ends with exactly the row's `after` (Engineer's option a).
4. No "remove key" `Patch` operation is added: no command would use it, and unused write power over settings is risk without benefit.
5. The fixture is not edited. rein's Architect is told how we read the table.

### Alternatives considered
1. **Add `Patch::Remove`** (option b) — rejected (ruling 4).
2. **Ask rein to reword the row** (option c) — not needed: "accept" already means "allowed".
3. **Harness reads the table as permissions + undo test** — chosen.

### Constraints driving this decision
- rein D-155 r3: three allowed keys, never `hooks`; rein owns the table.

### Impact
- Unblocks E-17.
- Risk if wrong: rein meant "must be able to"; then one operation is added later.

### Rollback
Tighten the harness back to a biconditional.

---

## D-031 — Layout-equality case 2: the clone side is updated by re-running `install-rein.sh`

**Date**: 2026-11-11
**Task**: E-9 (re-run at rein f7b3554, reported by the Engineer)
**Decision**: In the retired-server case, side A re-runs `install-rein.sh` instead of only `rein install`. The blueprint's "`rein install` removes it on both sides" was my wording and was wrong for a clone install.

### Why needed
At f7b3554 case 1 is green (rein E-426 fixed the lock rewrite). Case 2 is red: the seeded `mcp/gone-mcp/` is removed on side B by the artifact-mode `rein install` (rein E-420), but stays on side A, where the test ran `rein install` only. The Engineer checked by hand: in a clone home `rein install` leaves the directory and re-running `install-rein.sh` removes it.

### Rulings
1. Case 2 compares like with like: an update on each side. A clone install is updated by re-running `install-rein.sh`; an artifact install by the app's sync plus `rein install`. Side A therefore re-runs `install-rein.sh`.
2. The assertions do not change: empty diff apart from the documented differences; exit 0 or 3; no install-integrity token.
3. Both runs at f7b3554 (case 1 green; case 2 red as first written, then with ruling 1) are recorded in `.ai/DEVOPS.md`.
4. rein's Architect is told what was observed, in case clone-mode `rein install` was meant to remove retired mirrors too; if rein changes that, the test needs no further change.

### Alternatives considered
1. **Ask rein to make clone-mode `rein install` remove retired mirrors** — not ours to require; rein stated the removal for artifact mode (E-420).
2. **Document `mcp/gone-mcp` as an expected difference** — rejected: it would bring back the exception rein D-155 addendum r1 removed.
3. **Re-run the installer script on side A** — chosen.

### Constraints driving this decision
- rein D-155 addendum r1; E-9 compares what an operator ends up with.

### Impact
- E-9 can close when case 2 is green with ruling 1.
- Risk if wrong: none in shipped behaviour — this is test setup.

### Rollback
Restore the single `rein install` on side A.

---

## D-032 — E-32 accepted; the app does not recompute rein's verdict; the layout test stays out of hosted CI

**Date**: 2026-11-11
**Task**: review of E-32 (391a4b5); E-33
**Decision**: E-32 is accepted as built. The proposal "a check worse than the top-level status → exit 14" is rejected. The scoped doctor run gets a 120-second limit. The layout-equality test is a local pre-release check, not a hosted CI job.

### Why needed
E-32 shipped with hosted CI green and the layout test at exit 0 against rein cac86a4. The Engineer left one reviewer proposal, the §API wording, the hosted-runner question and three deferred P3s.

### Rulings
1. **E-32 accepted**, including: exit code and `status` must agree; strict schema-1 parse; 4 MiB stdout cap.
2. **No verdict of our own.** `status` is rein's judgment of its checks (rein D-158). Recomputing it from the check list is Rein logic in the app and would break on any future rule rein adds (a tolerated failing check, a new status).
3. **§API**: exit 3 = the install check's `status` is `fail`; exit 14 = no valid result. Blueprint updated.
4. **Time limit (E-33):** 120 seconds for the scoped doctor run, then kill and exit 14. It makes no network call and reads a few hundred files, so 120 s is generous; without it a stuck child holds the install lock indefinitely. `rein install` stays unlimited — it may download packages.
5. **Backslash escaped** in printed rein text (review P3-3), in E-33.
6. **Accepted, no change (P3-2):** a few Unicode format characters outside the listed set stay unescaped; they are not terminal controls.
7. **Hosted CI:** read from rein's code (not run), a GitHub Ubuntu runner cannot pin Node, `rein install` exits 0 with the toolchain unpinned and the install check would fail. The layout test therefore cannot assert exit 0 there. It is run locally against the rein release commit before every rein-app tag and recorded in `.ai/DEVOPS.md`. This is a known gap: nothing in hosted CI exercises a real rein artifact.

### Alternatives considered
1. **Adopt the reviewer's consistency rule** — rejected (ruling 2).
2. **Wire the layout test into hosted CI asserting exit 0 or 3** — rejected: it would pass while the one thing it exists to show (a clean install) is failing.
3. **A self-hosted runner** — not now: operator cost for one test; revisit in Phase 2.

### Constraints driving this decision
- rein D-158 (the verdict is rein's); D-001.

### Impact
- Unlocks: E-33. The pre-tag checklist gains "layout test green against the rein release commit".
- Risk if wrong: a regression in real-artifact install is caught only at the pre-tag run, not per commit.

### Rollback
Raise or drop the limit constant; wire the test into CI if a suitable runner appears.

---

## D-033 — Phase 2 starts: the desktop window is one terminal running `rein start`, with modal, grid-drawn chrome ("Modeline")

**Date**: 2026-11-11
**Task**: operator request (Phase 2 start); E-34..E-39
**Decision**: The window is a new crate `rein-desktop` (Tauri 2 + xterm.js) holding one pty per project tab whose child is `rein start`; tmux draws the panes; the app adds a tabline, a statusline and a command line, driven by nvim-style modes, a closed action list and one `config.toml`.

### Why needed
The operator asked for the window to be planned now, for people who live in Linux, tmux and nvim, with the design left to the Architect. `architect.md` §6 fixed the stack and scope but not the interaction model, the process model of the pane view, or the security boundary of the web view.

### Rulings
1. **One pty, child `rein start`, cwd = project.** The app never names a tmux session, draws a split or binds a role. The operator's `tmux.conf` works unchanged. Closing a tab detaches; the session keeps running.
2. **Modes:** TERMINAL (default; only the escape chord `C-\ C-n` is taken), NORMAL, COMMAND. Every key, click and `:` line resolves to one closed `Action` list.
3. **Grid chrome:** three rows (tabline, statusline, command line) and floating overlays, all on the font's cell grid with box-drawing borders. No sidebar, menu, toolbar, icon set or UI framework.
4. **The UI shows the command it ran** and `y` copies it.
5. **Config is one text file**, read-only to the app after first creation, reloaded on save. It binds keys to action names only and cannot start a program. No settings dialog; the app does not start `$EDITOR`.
6. **Separate binary.** The CLI crate gets no GUI dependency; `rein-app window [DIR]` starts `rein-desktop`.
7. **Web view is untrusted:** bundled UI only, strict CSP, a closed IPC table, no `shell`/`fs`/`http`/`opener` plugin, no IPC command that takes a program or argv.
8. **Health, release and profiles** go through the Phase 1 library seams; the verdict stays rein's (D-029, D-032); the three-key allow-list is unchanged.
9. **This revision covers Linux and macOS.** The Windows window, bundled Node and signing/notarization are later revisions; no task is registered for them.
10. Phase 2 work starts while E-6 (first tag) still waits on rein and the operator; no release artifact carries the window until a later task says so.

### Alternatives considered
1. **tmux control mode (`-CC`) with app-drawn panes** — rejected for now: the app would re-implement layout, copy mode and the operator's bindings, and it is Phase 3 territory (`architect.md` §6).
2. **A conventional GUI (sidebar, buttons, settings dialog)** — rejected: it hides the terminal and the CLI from the audience that prefers them, and needs a mouse.
3. **A native Rust UI (egui/iced + a terminal emulator crate)** — rejected: `architect.md` §5 fixes Tauri + xterm.js; a second emulator is a large surface to own.
4. **`ratatui` TUI only** — kept as the fallback in `architect.md` §5; the grid design means most of the model (modes, actions, config) would carry over.
5. **Keymaps that can run shell commands** — rejected: the config file would become a way to execute programs through the app.
6. **Chosen: one pty + modal grid chrome** — smallest surface, tmux stays the multiplexer (contract §4), familiar to the audience on first sight.

### Constraints driving this decision
- Contract §3.5–3.6 and §4: `rein pane` is the only role binding; tmux stays through Phase 2; only the stable entry points are invoked.
- D-001: no Rein logic in the app — it does not read `.ai/` files.

### Impact
- Unlocks: E-34..E-39. `.ai/blueprints/desktop.md` written.
- Open questions sent to rein (none blocking): a machine-readable "session running" signal; `rein start` behaviour in a bare pty; a one-line project status source.
- Risk if wrong: if `rein start` cannot run cleanly as a pty child, E-35 needs a contract answer from rein before it can finish.

### Rollback
Remove `crates/rein-desktop` and the `window` subcommand; nothing in `$HOME/.rein`, a profile or a project is changed by the window itself.

---

## D-034 — The window is click-first by default and modal by choice; every action is reachable both ways

**Date**: 2026-11-11
**Task**: operator request (amends D-033); E-40, E-41
**Decision**: The window ships two input styles over one action list — `simple` (default: no modes, common terminal shortcuts, everything clickable) and `modal` (the D-033 keymap) — and every action must be reachable by mouse alone and by keyboard alone.

### Why needed
The operator pointed out that D-033 served only power users. People who are used to clicking must be able to open a project, check health and update without learning a mode or a command.

### Rulings
1. **Two input styles, one `modes` function, two key tables.** `simple` is the default; the first-run screen offers the choice; Settings changes it. The `simple` shortcuts also work in `modal`. This replaces D-033 ruling 2's "TERMINAL default with only the escape chord taken" for the `simple` style, where the listed `Ctrl+Shift` shortcuts are taken instead.
2. **All chrome is clickable** — tabs, `×`, `[+]`, version, `[≡]` menu, statusline segments, overlay rows — still drawn on the cell grid, with hover, tooltip (action + shortcut) and accessible name.
3. **One menu lists every action in plain words** with its shortcut. It is the single place a newcomer needs.
4. **Machine-changing actions end in two labelled buttons**, the safe one focused.
5. **Plain words first, the command beneath.** rein's own error text is never reworded.
6. **A Settings overlay exists** (amends D-033 ruling 5, "no settings dialog"). It writes the app's own `ui.json`. `config.toml` is still never rewritten and wins when it sets a key; Settings shows such a row read-only.
7. **One plugin permission is added:** the system folder-open dialog. Its result passes the `tab_open` validation. A multi-line paste asks once before it is sent.
8. A test enforces parity: an action with no menu entry, or no binding in either table, fails the build.

### Alternatives considered
1. **Keep modal as the default with a "beginner" switch** — rejected: the first minute decides whether a clicker stays; a vim user finds the switch at once.
2. **Two separate UIs (a GUI and a "pro" skin)** — rejected: two code paths and two sets of bugs; one action list with two input tables gives both.
3. **Let Settings write `config.toml`** — rejected: rewriting a hand-edited file loses comments and ordering, and breaks the promise that the file is the operator's.
4. **Chosen: same grid design, click-first default, modal opt-in.**

### Constraints driving this decision
- D-033 rulings 3, 5 and 7 (closed action list, config cannot run programs, untrusted web view) stay in force.

### Impact
- Unlocks: E-40, E-41. E-34..E-39 as registered stay valid (E-36 must take the key table as input; E-34's capability test is updated by E-40).
- Risk if wrong: the `simple` shortcuts collide with a user's tmux or shell bindings — every one is rebindable in `config.toml`.

### Rollback
Set the default style back to `modal`; the click layer and Settings are additive and can be removed without touching the action list.

---

## D-035 — Phase 2 window dependencies for E-34 (dependency_gate, Engineer-recorded)

**Date**: 2026-11-11
**Task**: E-34 (rein-desktop crate, web build)
**Decision**: ADOPT the set below (operator, 2026-11-11). Every item is named in `.ai/blueprints/desktop.md` §Execution Constraints. Recorded by the Engineer; the Architect may revise.

| Package | Version | License | Use | Kind |
|---|---|---|---|---|
| tauri (crate) | 2.11.6 | Apache-2.0 OR MIT | window, webview, IPC, capabilities | runtime (`rein-desktop` only) |
| tauri-build (crate) | 2.6.3 | Apache-2.0 OR MIT | build script: capability/ACL codegen, context | build (`rein-desktop` only) |
| esbuild (npm) | 0.28.2 | MIT | bundle the TypeScript UI (one file, no framework) | build-time |
| typescript (npm) | 7.0.2 | Apache-2.0 | type-check the UI (`tsc --noEmit`) | build-time |

Also used directly (Linux/FreeBSD only): `glib` 0.18.5 (MIT), already in tauri's tree at that exact version, so no new code enters the build — `glib::set_prgname("rein-app")` gives X11 WM_CLASS and the Wayland app id `rein-app` (tauri sets a GTK app id only from a dotted identifier).

Not added: `tauri-plugin-single-instance` — on Linux it uses D-Bus, while the blueprint puts the lock in `$XDG_RUNTIME_DIR` (state dir fallback); a std `UnixListener` on a socket there does the handoff in ~80 lines with no dependency. No `@tauri-apps/api` npm package: the UI calls the core through `window.__TAURI_INTERNALS__.invoke` behind one small typed module, so no plugin JS enters the bundle. The web test runner is Node's built-in `node:test` (no Jest/Vitest). The xterm packages are E-35's gate.

### Why needed
Tauri 2 is the stack fixed by `architect.md` §5 and D-033; esbuild and TypeScript are the blueprint's web build.

### Alternatives considered
1. **Implement ourselves** — not for a webview host. Accepted for the single-instance lock (above).
2. **tauri 3.0.0-alpha** — rejected: alpha; the blueprint says Tauri 2.
3. **Vite** instead of esbuild — rejected: larger tree; the blueprint names esbuild.

### Size / weight
tauri pulls wry, tao, webkit2gtk bindings (Linux) / WKWebView (macOS), muda, serde, tokio — a large tree, isolated to `rein-desktop`. The `rein-app` CLI crate gains nothing (checked by `cargo tree -p rein-app`). esbuild ships one native binary per platform; typescript is pure JS. Neither is in any shipped artifact.

### Security track record
Tauri has a public security policy, an external audit (Radically Open Security, v1 and v2) and published GHSAs that were fixed promptly; we turn off every plugin and use a closed capability file. No RustSec advisory against tauri 2.11.x known to the Engineer at recording time (cargo-audit not installed locally). esbuild/typescript run only at build time.

### Maintenance
tauri 2.x is actively released (stable line beside the 3.0 alphas); esbuild and typescript are released monthly.

### System requirement
Linux builds need WebKitGTK 4.1, GTK 3, libsoup 3 development packages (installed on the operator's machine 2026-11-11; the hosted Linux job installs them via apt).

### Rollback
Remove `crates/rein-desktop` from the workspace members and the `window` subcommand (blueprint §Rollback Plan). `Cargo.lock` and `crates/rein-desktop/ui/package-lock.json` are committed; CI runs `--locked` / `npm ci`.

---

## D-036 — Terminal stage dependencies for E-35 (dependency_gate, Engineer-recorded)

**Date**: 2026-11-11
**Task**: E-35 (PtyHost, xterm.js stage)
**Decision**: ADOPT the set below (operator, 2026-11-11). Every item is named in `.ai/blueprints/desktop.md` §Execution Constraints. Recorded by the Engineer; the Architect may revise.

| Package | Version | License | Use | Kind |
|---|---|---|---|---|
| portable-pty (crate) | 0.9.0 | MIT | open a pty, spawn `rein start` in it, resize, reap (`rein-desktop` only) | runtime |
| @xterm/xterm (npm) | 6.0.0 | MIT | the terminal emulator in the stage | bundled into the UI |
| @xterm/addon-fit (npm) | 0.11.0 | MIT | size the terminal to the stage in whole cells | bundled |
| @xterm/addon-webgl (npm) | 0.19.0 | MIT | GPU renderer; falls back to the DOM renderer when WebGL is unavailable | bundled |

### Why needed
The stage is a real terminal running `rein start` (D-033 ruling 1). A pty needs platform code (openpty/forkpty on Unix, ConPTY later); xterm.js is the terminal the architecture fixes (`architect.md` §5).

### Alternatives considered
1. **Implement the pty ourselves** with `libc`/`nix` — rejected: `unsafe_code = "forbid"` in the workspace, and portable-pty is the pty layer of WezTerm, maintained with it.
2. **`pty-process`** — Unix-only and async (tokio); portable-pty also keeps the later Windows (ConPTY) revision open.
3. **xterm.js DOM renderer only** (skip addon-webgl) — kept as the runtime fallback, not the default: the WebGL renderer keeps a key under the 5 ms budget on large outputs.

### Size / weight
portable-pty 0.9.0 pulls `nix` 0.28, `filedescriptor`, `serial2`, `shell-words`, `downcast-rs`, `anyhow`, `log` (all MIT/Apache-2.0, small); we never use its serial-port or shell-words paths. Also used directly: `nix` 0.28 with only the `signal` feature (the same crate portable-pty builds; `Cargo.lock` keeps one `nix`) — a safe `killpg(SIGKILL)` for reaping, because portable-pty's `ChildKiller` sends only SIGHUP and the workspace forbids `unsafe`. The three npm packages are bundled by esbuild into `app.js` (xterm ~ 300 KB minified, webgl addon ~ 100 KB); nothing is fetched at run time (CSP `default-src 'self'`). xterm's CSS is bundled into `app.css` (no inline `<style>`).

### Security track record
xterm.js: widely deployed (VS Code's terminal); past advisories concerned escape-sequence handling that apps exposed to the clipboard — we keep OSC 52 off and open no link (desktop.md §Security 3). portable-pty: no RustSec advisory known to the Engineer at recording time (cargo-audit not installed locally).

### Maintenance
xterm.js 6.0.0 (2025-12) is the current major; portable-pty 0.9.0 is the current release from the WezTerm repository.

### Rollback
The pty sits behind `PtyHost`; the terminal behind the UI's `term` module. Remove both and the stage falls back to the E-34 placeholder.

### Addendum (2026-11-11, E-35 security review)
- `filedescriptor` 0.8.3 (MIT) is now a direct dependency: the master fd is duplicated with `FileDescriptor::dup` (close-on-exec) for writing, because portable-pty's `take_writer` types a newline and EOF into the pty when dropped. It is the same crate and version portable-pty already builds.
- `nix` 0.28 also enables the `fs` feature: `fcntl::Flock` serialises the single-instance takeover (`single_instance.rs`). No new crate.
- `COLORTERM=truecolor` is set for the child beside `TERM=xterm-256color`: xterm.js renders 24-bit colour, and tmux uses it to pass true colour through.

---

## D-037 — The window's click layer uses shadcn/ui (React + Radix + Tailwind), restyled to the grid

**Date**: 2026-11-11
**Task**: operator request, confirmed; amends D-033 ruling 3 ("no UI framework"); E-42. Numbered D-035 until 2026-11-11 20:20 UTC — renumbered because D-035 and D-036 were already the Engineer's dependency records; E-42's task text still says D-035 and means this entry
**Decision**: The web UI moves to React. shadcn/ui components are copied into the repo for the menu, dialogs, tooltips, context menu, Settings and first-run screen, restyled to the terminal look; the tabline, statusline, command line and terminal stay our own grid components.

### Why needed
The operator asked for shadcn/ui. D-034 added a click layer (menu, dialogs, tooltips, context menu, Settings) whose keyboard and screen-reader behaviour is costly to write by hand and is what these components already provide.

### Rulings
1. React + Tailwind (compiled at build time) + the Radix primitives of the components actually used + `cmdk`. Each passes `dependency_gate`. No icon set, no second component library.
2. Only the listed components are copied in; they live in the repo and are ours to change.
3. **The look stays the grid:** monospace font, cell-multiple sizes, square or box-drawing borders, no shadows, colours only from the theme variables. shadcn's default look is not shipped.
4. **The CSP does not loosen.** A component that needs an inline style or script element is not used.
5. `modes` stays pure TypeScript with no React import, so the input logic is testable without a DOM.
6. Web bundle ≤ 1.5 MiB uncompressed.
7. E-34's static shell is rebuilt in React by E-42, before E-36.

### Alternatives considered
1. **Stay framework-free and hand-write the click layer** — rejected on the operator's choice; it also means owning focus traps, roving focus and ARIA for six widget kinds.
2. **shadcn/ui with its default look** — rejected: the window would read as a generic web app and lose the D-033 design.
3. **Radix primitives alone, no Tailwind** — workable, but shadcn's copied components are the faster starting point and the operator asked for it.
4. **Chosen: shadcn/ui for the click layer, restyled.**

### Constraints driving this decision
- D-033 ruling 7 (untrusted web view, strict CSP, closed IPC table) is unchanged.

### Impact
- Unlocks: E-42; E-36..E-41 build on it. Task texts that say "no framework" are superseded here.
- Risk if wrong: bundle size and first-paint budget (400 ms) — measured in E-42; a Radix component that conflicts with the CSP is dropped and replaced by our own.

### Rollback
The `modes` function, the IPC table and the core do not depend on React; the components directory can be replaced by hand-written equivalents without touching them.

## D-039 — React + Tailwind + shadcn/ui base dependencies for E-42 (dependency_gate, Engineer-recorded)

> Numbered D-038 until 2026-11-11 21:45 UTC; renumbered because the Architect's Windows-window decision took D-038 in the same commit (d14c209).

**Date**: 2026-11-11
**Task**: E-42 (React shell, theme variables, restyled shadcn/ui components) — D-037
**Decision**: ADOPT the set below (operator, 2026-11-11: "finish everything … don't ask me for permissions, just build it"). Every item is named in `.ai/blueprints/desktop.md` §Execution Constraints. Recorded by the Engineer; the Architect may revise.

| Package | Version | Published | License | Use | Kind |
|---|---|---|---|---|---|
| react, react-dom | 19.3.1 | 2026-10-14 | MIT | UI components | bundled |
| @types/react, @types/react-dom | 19.3.0 | 2026-09-09 | MIT | types | dev |
| tailwindcss, @tailwindcss/cli | 4.3.3 | 2026-07-16 | MIT | compile one static stylesheet at build time | dev (build) |
| @radix-ui/react-dialog | 1.3.1 | 2026-10-12 | MIT | Dialog, Command dialog | bundled |
| @radix-ui/react-tooltip | 1.4.1 | 2026-10-12 | MIT | Tooltip | bundled |
| @radix-ui/react-context-menu | 2.4.1 | 2026-10-12 | MIT | terminal Copy / Paste menu | bundled |
| @radix-ui/react-select | 2.4.1 | 2026-10-12 | MIT | Settings selects | bundled |
| @radix-ui/react-switch | 1.4.1 | 2026-10-12 | MIT | Settings toggles | bundled |
| @radix-ui/react-scroll-area | 1.4.0 | 2026-10-12 | MIT | overlay scrolling | bundled |
| @radix-ui/react-slot | 1.5.0 | 2026-10-12 | MIT | Button `asChild` | bundled |
| cmdk | 1.1.1 | 2025-03-14 | MIT | menu (Command) | bundled |
| class-variance-authority | 0.7.1 | 2024-11-26 | Apache-2.0 | component variants | bundled |
| clsx | 2.1.1 | 2024-04-23 | MIT | class joining | bundled |
| tailwind-merge | 3.7.0 | 2026-09-12 | MIT | class merging (`cn`) | bundled |

Tailwind 4.3.4 / 4.3.5 are days old at recording; 4.3.3 is pinned until a newer release has aged two weeks.

### Why needed
D-037: the click layer (menu, dialogs, tooltips, context menu, Settings, first-run screen) uses shadcn/ui on Radix primitives, which are React components; Tailwind is how shadcn components are styled.

### Alternatives considered
1. **Hand-written components in plain TypeScript** (E-34's approach) — rejected by D-037 (operator): focus trapping, keyboard navigation and accessible names for menus and dialogs are what Radix provides.
2. **Preact + compat** — smaller, but Radix and cmdk target React 19; a compat layer is a second source of bugs.
3. **Tailwind at run time (Play CDN)** — impossible under CSP `default-src 'self'` and not wanted: the stylesheet is compiled at build time.

### Size / weight
React + react-dom ~ 190 KB minified; Radix primitives used ~ 60–90 KB together; cmdk ~ 15 KB; cva/clsx/tailwind-merge ~ 25 KB. The E-42 acceptance caps the whole bundle (xterm.js included) at 1.5 MiB uncompressed; a test asserts it. Tailwind runs only at build time; its output is one static CSS file.

### Security track record
No advisory known to the Engineer for these versions. Radix components that inject a `<style>` element (Scroll Area's and Select's scrollbar hiding, `react-remove-scroll`) are checked against the CSP in E-42; per D-037 a component that needs inline style or script elements is dropped, the CSP is never loosened.

### Maintenance
React 19 is current; Radix primitives and Tailwind 4 had releases in the last month; cmdk, cva and clsx are small and stable.

### Rollback
`modes`, the IPC table and the core have no React import; the `ui/components/` directory can be replaced by hand-written equivalents without touching them. Removing React means restoring E-34's `grid.ts` rendering in `main.ts`.

## D-038 — The Windows window is a native WebView2 build whose every process is `wsl.exe`; installer as a CI artifact

**Date**: 2026-11-11
**Task**: operator wants to install the window on Windows; Engineer finding that the Linux build cannot paint under WSLg (MESA "ZINK: failed to choose pdev"; three WebKit/GL switches do not help; the same binary renders on Xvfb). Revises `desktop.md` ("Windows window" was a later revision). E-43, E-44, E-45.
**Decision**: `rein-desktop` is built for Windows (Tauri 2 / WebView2). Rein, the projects and the profiles stay inside WSL2; the Windows program starts only `wsl.exe`, with fixed argv shapes, and learns everything about a project by asking inside WSL.

### Why needed
On Windows the operator's only way to see the window was WSLg, and WebKitGTK does not paint there. Contract non-goal "native Windows without WSL2" is unchanged — this is a Windows window over WSL2, as Phase 1's launcher is.

### Rulings
1. **`Host` seam.** Project validation, the pty child, doctor, release / profile actions and directories sit behind one trait: `LocalHost` (today's behaviour) and `WslHost`.
2. **Pty child (ConPTY):** `wsl.exe -d D --cd <linux path> -e <user's shell> -lic 'exec ~/.rein/bin/rein start'`. Login + interactive so `PATH` matches the operator's terminal. The script is a constant; the path is only the `--cd` argument.
3. **Distro:** `wsl.distro` in `config.toml`, else WSL's default; name, home and shell learned once by a fixed probe and validated; one distro per window. State detection reuses the Phase 1 `wsl.rs` logic (shared, not copied).
4. **Project path on Windows = the Linux path.** Accepted: a POSIX absolute path, or `\\wsl.localhost\<distro>\…` / `\\wsl$\<distro>\…` mapped to it. Drive paths (`/mnt/c`) are refused in this revision. The `.ai/` check, the "rein installed" check and canonicalisation are one fixed call inside WSL. Paths with `"` or `\` are refused on Windows rather than escaped.
5. **Environment:** `WSLENV` is removed from the child, so nothing crosses; WSL supplies `TERM=xterm-256color`.
6. **Single instance:** an exclusively held lock file plus a watched handoff directory under `%LOCALAPPDATA%\rein-app` — no named pipe.
7. **Directories:** config in `%APPDATA%\rein-app`, state in `%LOCALAPPDATA%\rein-app`.
8. **Health / update / rollback / profiles run the Linux programs inside WSL** (`rein doctor --json`, `rein-app update --check | update --yes | rollback | profiles apply …`). The Windows binary never touches a Rein tree or a settings file. The CLI gains the additive `update --check`.
9. **Distribution now:** a separate hosted workflow builds an unsigned per-user NSIS installer and uploads it as a workflow artifact (14 days). It is not a release asset; `release.yml` and the dist configuration are untouched (D-033's "no release artifact carries the window" still holds).
10. The window never installs WSL and never elevates; it shows the command to run.

### Alternatives considered
1. **Fix WSLg rendering** — rejected: three switches tried, the fault is in the WSLg GL stack, not ours.
2. **Named pipe for single instance** — rejected: the default pipe ACL lets other users connect and a pipe name can be squatted; securing it needs raw Win32 security attributes (`unsafe` or a new dependency). A lock file in the user's private profile directory needs neither.
3. **Run the Windows `rein_release` / `rein_profiles` against `\\wsl.localhost` paths** — rejected: POSIX modes, locks and the owner guard do not hold over the 9P share; it would be a second settings writer outside the Linux security model.
4. **`wsl.exe -e <home>/.rein/bin/rein start` with no shell** — rejected: `PATH` would lack nvm / Homebrew, which the operator's terminal has.
5. **Allow `/mnt/c` projects** — deferred: slow, and Windows processes can rewrite files the Rein gate trusts.
6. **MSI or a release asset now** — rejected: the CLI owns the `.msi`; nothing unsigned goes on a release.

### Constraints driving this decision
- `architect.md` §2 non-goal (no native Windows Rein); D-033 ruling 7 (closed process list, no interpolated command line); the Tier 3 settings boundary (`distribution.md`).
- No Windows Rust toolchain on the operator's machine — the build must come from hosted CI.

### Impact
- Unlocks: E-43 → E-44 give the operator an installable window (opens the DIR from the command line until E-38); E-45 completes parity.
- Risk if wrong: `wsl.exe` re-parses its command line — mitigated by constant scripts without double quotes and by refusing `"` and `\` in paths; the interactive login shell may print rc-file noise before tmux starts (harmless, tmux repaints); an unsigned installer triggers SmartScreen; any signed-in GitHub user can download a public repository's workflow artifacts (it is a test build of public source).

### Rollback
Remove `desktop-windows.yml` and `WslHost`; `LocalHost` behind the seam is the previous behaviour. Nothing inside WSL is changed by the window.

## D-040 — Object.prototype is frozen by the UI after its libraries load, not by Tauri's `freezePrototype` (Engineer-recorded)

**Date**: 2026-11-11
**Task**: E-35 (security review P2: `freezePrototype` removed from `tauri.conf.json`)
**Decision**: Keep Tauri's `freezePrototype` off and freeze `Object.prototype` in `ui/src/harden.ts`, called from `main.tsx` right after the imports, before any pty output, IPC reply or event is handled. The Architect may revise.

### Why
With `freezePrototype: true` the window fails at start: `TypeError: Attempted to assign to readonly property` (seen under Xvfb, E-35). xterm.js 6.0.0 bundles VS Code's `KeyCodeUtils` namespace, which assigns `toString` on a plain object while its module loads; under a frozen `Object.prototype` that assignment throws in strict mode (the "override mistake").

### Alternatives considered
1. **Patch xterm's bundle at build time** — rejected: a string rewrite of a dependency breaks silently on the next upgrade.
2. **Leave the prototype unfrozen** — rejected: the review's point stands; the IPC bridge and the pty data path should not be pollutable.

### Effect
Same scope as Tauri's option (`Object.prototype` only). The difference is when: Tauri freezes before any script; we freeze after the bundled modules have evaluated and before the first untrusted input. Tests: `ui/test/harden.test.mjs` (a `obj["__proto__"][k] = v` gadget throws and pollutes nothing), `tests/capabilities.rs` (no `freezePrototype` in the config, `main.tsx` calls `hardenPrototypes()`).

### Rollback
Re-enable `freezePrototype` once xterm.js no longer assigns over prototype properties at load; delete `harden.ts`.


## D-041 — Tauri CLI for the Windows installer build (E-44, dependency_gate, Engineer-recorded)

**Date**: 2026-11-11
**Task**: E-44 (`desktop-windows.yml`, NSIS installer as a workflow artifact) — D-038 ruling 9
**Decision**: ADOPT `@tauri-apps/cli` 2.11.5 (published 2026-09-20; Apache-2.0 OR MIT) as a dev dependency of `crates/rein-desktop/ui`, pinned exactly in the committed `package-lock.json` (operator, 2026-11-11: "don't ask me for permissions, just build it"). The Architect may revise.

### Why needed
Only the Tauri CLI produces the NSIS bundle (installer script, WebView2 bootstrapper, per-user install) from `tauri.conf.json`; `cargo build` makes the bare executable.

### Alternatives considered
1. **`cargo install tauri-cli --locked`** in the workflow — compiles the CLI from source on every run (minutes) and its version lives in the workflow, not in a lockfile.
2. **Hand-written NSIS script** — re-implements what the bundler maintains (WebView2 bootstrap, uninstaller, per-user registry keys).

### Size / weight
The npm package downloads one prebuilt native binary for the running platform (an optional dependency per OS, ~10 MB). It runs only at build time; nothing of it ships in the installer.

### Security track record
Maintained by the Tauri project; no advisory known to the Engineer for 2.11.x. Installed with `npm ci --ignore-scripts` from the lockfile (integrity hashes).

### Maintenance
Same minor (2.11) as the `tauri` crate (2.11.6), as Tauri recommends.

### Rollback
Remove the dev dependency and `desktop-windows.yml`; nothing else uses it.

### D-038 addendum 1 (2026-11-11) — `wsl.exe --cd` never fails; validation guards against the `/` fallback
E-43's interop smoke showed `wsl.exe --cd <missing | file | unreadable>` prints `chdir(...) failed`, runs the command in `/` and exits 0. Rulings: (1) the validation script starts with `case $PWD in /) exit 5;; esac` (constant, no quotes); exit 5 means "not a directory"; (2) `/` is refused as a project path before any call; (3) the pty start script is unchanged — it runs in the user's shell (possibly fish), and a directory that vanishes after validation ends in `rein start`'s own error shown in the stage; (4) passing the path as a positional parameter was rejected (needs double quotes through `wsl.exe`'s re-parsing). Also accepted from E-43: the shared crate `rein-platform`; platform `cfg` in `pty_sys` and one `platform_host()`; the handoff directory is polled every 200 ms (no watcher — final, not a stopgap). Smoke confirmed: `$SHELL` is set under `-e`, `WSLENV` is the only crossing path, `TERM=xterm-256color`.

### D-038 addendum 2 (2026-11-11) — E-43 (dbc9908) and E-44 (8988d48) accepted; two residual risks recorded
`security_engineer` PASS, no P0/P1. Accepted fixes: `%SystemRoot%` only when drive-absolute, UNC `%LOCALAPPDATA%` refused, bounded handoff read. Accepted residual risks, written into `desktop.md` §Security (Windows): (1) the pty start script keeps no `/` guard — reaching a session that way needs `/.ai`, i.e. root in the distro; a guard cannot be a constant that every user shell (fish) accepts; (2) WSL's `appendWindowsPath` puts Windows directories on the Linux `PATH` — WSL's own default, mitigated by the operator in `/etc/wsl.conf`, to be stated in the Windows operator docs. `desktop-windows.yml` also triggers on `crates/rein-platform/**`.

## D-042 — Clipboard for `clipboard_write` (E-36, dependency_gate, Engineer-recorded)

**Date**: 2026-11-12
**Task**: E-36 (`cmd.yank`: `y` copies the last shown command through `clipboard_write`, ≤ 64 KiB — desktop.md §API)
**Decision**: ADOPT `arboard` 3.7.0 (published 2026-10-15; MIT OR Apache-2.0) with **default features off** (text only — no image decoding, no `image` crate) as a dependency of `rein-desktop` (operator, 2026-11-11: "don't ask me for permissions, just build it"). The Architect may revise.

### Why needed
`clipboard_write` is an IPC command (§API): the core writes the clipboard. Tauri core has no clipboard API without the clipboard-manager plugin, and the capability file admits no plugin (§Security 1).

### Alternatives considered
1. **`tauri-plugin-clipboard-manager`** — a plugin; the closed capability file and the "no Tauri plugin in Cargo.lock" test forbid it.
2. **`navigator.clipboard.writeText` in the web view** — no IPC, but availability differs across WebKitGTK, WKWebView and WebView2, and it would put a clipboard path outside the validated IPC table.
3. **Hand-rolled per-OS clipboard code** — X11 selections, Wayland data-control, Win32 and NSPasteboard each need platform bindings and `unsafe`, which the workspace forbids.

### Size / weight
Text-only arboard pulls `x11rb` on Linux (pure Rust X11), `clipboard-win` on Windows and `objc2-app-kit` on macOS — several of these are already in Tauri's tree. On Wayland it works through XWayland; the `wayland-data-control` feature is left off.

### Security track record
Maintained by 1Password; no RustSec advisory known to the Engineer. The IPC input is bounded (64 KiB) and only ever *written*; reading the clipboard (`clipboard_read`, E-40) is a separate, user-gesture-only command.

### Rollback
Remove `clipboard_write` and the dependency; `y` then reports that copying is unavailable.

## D-043 — `toml` for config.toml (E-37, dependency_gate, Engineer-recorded)

**Date**: 2026-11-12
**Task**: E-37 (`Config` service — desktop.md §Components 4, §Data Model `ConfigFile`, §Security 4)
**Decision**: ADOPT `toml` 1.2.1 (MIT OR Apache-2.0) as a direct dependency of `rein-desktop` — the version Tauri's own build already pulls into `Cargo.lock`, so no new code enters the tree. Named in desktop.md §Execution Constraints. `notify` is **not** added: config.toml is re-read when its modification time or size changes, polled every 500 ms, as the Windows handoff directory is (D-038 addendum, Architect: "keep the 200 ms polling"). Recorded by the Engineer; the Architect may revise.

### Why needed
The config file is TOML (§Core Concept rule 5). Validation must report file, line and reason; `toml`'s errors carry a byte span, which gives the line.

### Alternatives considered
A hand-written TOML subset parser — a second parser of a format with string escapes, tables and arrays is where bugs and divergence come from.

### Rollback
`Config` is one module; removing it returns the window to its built-in defaults.

## D-044 — Rulings on the E-42 / E-36 report: `dir_complete`, `arboard`, the prototype freeze, refused Radix styles, polling instead of `notify`

**Date**: 2026-11-12
**Task**: E-42 (b3b40e6) and E-36 (0bf9edd) accepted on the Engineer's report; amends `desktop.md` §API, §Security 1, §Components 4, §Execution Constraints, §Windows window W4
**Decision**: Six items the Engineer raised are ruled as follows.

### Rulings
1. **`dir_complete` is added to the closed IPC table** as built (absolute checked prefix, ≤ 50, directories only, through the `Host` seam). On Windows it gets one more fixed `wsl.exe` call shape (`ls -1Ap` in `--cd <parent>`, with the `/` guard except when the parent is `/`), built in E-45.
2. **`arboard` (D-042) is ratified** for `clipboard_write`, text only. It keeps the clipboard out of the web view's capabilities.
3. **D-040 is ratified**: the UI freezes `Object.prototype` after its modules load and before the first untrusted input; Tauri's `freezePrototype` stays off.
4. **Refused runtime `<style>` from Radix is acceptable** while nothing the user sees or does depends on it. The CSP is not loosened; a component that breaks without the style is dropped (D-037 ruling 4 stands). E-40 verifies each mounted component with the CSP active.
5. **No `notify` dependency.** `config.toml` reload polls the modification time every 500 ms, as the Windows handoff directory polls every 200 ms. E-37's task text naming `notify` is superseded.
6. Hints for actions not yet wired show the real CLI command; own class names must not collide with Tailwind utilities (tested).

### Alternatives considered
1. **Let the web view list directories through an fs plugin** — rejected: D-033 ruling 7, no `fs` capability.
2. **`notify` for config reload** — rejected: a native watcher dependency on three platforms for one small file; half a second of latency is not noticeable after a save.
3. **`style-src 'unsafe-inline'` for Radix** — rejected: it would also allow injected styles from terminal-derived text.
4. **No directory completion on Windows** — rejected: `:open` with a typed Linux path is the keyboard way to open a project there.

### Constraints driving this decision
- D-033 ruling 7 (closed IPC table, strict CSP), D-038 (fixed `wsl.exe` argv shapes, constant scripts).

### Impact
- Unlocks: E-37 without `notify`; E-45 gains directory completion on Windows.
- Risk if wrong: a `wsl.exe` call per Tab press may feel slow on a cold VM (5-second limit, empty list on failure); a file name containing a newline can yield a completion that does not exist — it fails validation when opened.

### Rollback
Remove the `dir_complete` row and command (completion falls back to command and theme names); swap polling for a watcher behind the same `Config` interface.

## D-045 — The window runs the `rein-app` CLI for update, rollback and profiles on every platform

**Date**: 2026-11-12
**Task**: Engineer question before E-39; amends `desktop.md` §Components 5, §API, §Security 5, §Test Plan and D-033 ruling 8 ("over the Phase 1 library seams")
**Decision**: Option A. Release and profile actions are one `Host` method that runs the `rein-app` binary with a constant argv per action and streams its lines; the window decides only on the exit code. Health keeps calling `rein doctor --json` and parses it with the Phase 1 parser, moved into `rein-platform`.

### Why needed
The update / rollback flow (check, notes, lock, owner guard, snapshot swap, install-then-verdict, exit codes, refusal texts) lives in the `rein-app` bin crate, not in a library. D-033 assumed library seams that exist only for parts of it.

### Rulings
1. `LocalHost` starts `rein-app` found beside `rein-desktop`, else on `PATH`, resolved once to an absolute path; stdin closed. `WslHost` runs the same argv inside WSL (D-038 W4). One code path, three platforms.
2. Argv are constants: `update --check`, `update --yes`, `rollback`, `profiles list`, `profiles apply [--dry-run] --profile N…`. Profile names match `[A-Za-z0-9._-]{1,64}` and must appear in `profiles list`.
3. `rein-app update --check` moves from E-45 into **E-39**. Its first two lines are `installed: <version>` and `newest: <version>`; the window may display them. Nothing else in CLI text is read.
4. The confirm step is the window's (two labelled buttons); the CLI is then run with `--yes`.
5. The doctor parser is moved to `rein-platform`, not copied.
6. One CLI run at a time per window.

### Alternatives considered
1. **(B) Move the orchestration into a library and call it in-process** — rejected: a large refactor of tested Tier 3 code for no user-visible gain, and Windows would still need the CLI route, leaving two paths.
2. **Structured (JSON) CLI output for the window** — deferred: the overlays only need to show what the CLI says; add it when a screen needs fields.

### Constraints driving this decision
- "No second implementation" (D-033), the Tier 3 settings boundary, D-038 ruling 8 (Windows already takes this route).

### Impact
- Unlocks: E-39 as one path; E-45 shrinks to the WSL transport, the WSL-absent screen, `wsl.distro`, the folder-dialog start directory and Windows `dir_complete`.
- Risk if wrong: the window needs the CLI installed beside it (true in every package; reported plainly when not); a `PATH` fallback runs whatever `rein-app` the user's `PATH` names — the same trust as typing the command.

### Rollback
`run_cli` is one method; an in-process implementation can replace `LocalHost`'s later without touching the IPC table.

### D-045 addendum 1 (2026-11-12) — window health is the install scope, run from the home directory
Ratified from E-39: `health_get` runs `rein doctor --scope install --json` with cwd = home (on Windows `cd;` before the constant `exec`). Measured: a full `rein doctor --json` from `$HOME` is `fail` on a healthy install because project-scope and host checks judge the cwd; the statusline claims "install ok" and the install scope is the document D-029 already uses for that. The window still recomputes nothing. Per-project health per tab is deferred. Also ratified: profile names must start with a letter or digit (`[A-Za-z0-9][A-Za-z0-9._-]{0,63}`); `profiles list` is read as table rows only; no doctor run while a release or profile run is in progress.

## D-046 — Folder dialog for `project_choose` without a Tauri plugin (E-40, dependency_gate, Engineer-recorded)

**Date**: 2026-11-12
**Task**: E-40 (`[+]` / Ctrl+Shift+O open a project through the system folder dialog — desktop.md §API `project_choose`, §Security 6)
**Decision**: ADOPT `rfd` 0.17.2 (2026-01-12; MIT) as a direct dependency of `rein-desktop`, default features off, `gtk3` on (Linux: the GTK 3 Tauri already links; Windows: the native COM dialog; macOS: AppKit). The core opens the dialog itself — from the IPC worker thread on Linux and Windows (rfd's GTK backend posts the dialog to the window's own GLib main loop and waits, so calling it *on* the main thread deadlocks the window; found live under Xvfb), on the main thread only on macOS (AppKit requires it) — and passes the chosen path through the same `Host::project` validation as `tab_open`. **No Tauri plugin is added and the capability file gains no plugin permission** — stricter than §Security 6, which allowed exactly one (the dialog plugin's folder-open). Operator, 2026-11-11: "don't ask me for permissions, just build it". The Architect may revise.

### Why
`tauri-plugin-dialog` is a wrapper around `rfd`; it also pulls `tauri-plugin-fs` into the build and needs a web-side permission. Calling `rfd` from the core keeps the IPC table the only door (`project_choose` takes no input) and the "no Tauri plugin in Cargo.lock" test unchanged.

### Alternatives considered
1. **`tauri-plugin-dialog`** — the blueprint's choice; adds two plugins to the lockfile and one capability permission.
2. **`rfd` 0.18.0** — published 2026-11-07, under two weeks old.
3. **`xdg-portal` backend on Linux** — rfd's default; pulls an async D-Bus stack. GTK 3 is already in the process.

### Rollback
`project_choose` is one IPC command; removing it leaves `:open DIR` and the picker.

## D-047 — Rulings on the E-45 / E-40 report: `rfd` folder dialog, added key bindings, profile names in the WSL script, no branch or profiles segment

**Date**: 2026-11-12
**Task**: E-45 (c166a5a) and E-40 (ecad6ed) accepted on the Engineer's report; amends `desktop.md` §Core Concept (screen, keymap, statusline), §Security 6, §Security (Windows), §Execution Constraints
**Decision**: Four rulings.

### Rulings
1. **D-046 ratified:** the core opens the folder dialog with `rfd`; no Tauri plugin and no capability permission. Off the main thread on Linux and Windows, on it on macOS.
2. **Key bindings added** so every action has a key: `simple` — `Ctrl+,` settings, `Ctrl+Shift+Q` quit; `modal` leader — `S` profiles.apply, `c` config.open, `R` config.reload, `t` theme.set, `,` settings.show, `i` input.style, `Space` menu.show. `project.open` with no directory opens the folder dialog.
3. **Profile names may be placed in the WSL profile script** — the single exception to "scripts are constants" — with every word re-checked against `[A-Za-z0-9._-]` and each name against the profile-name rule.
4. **The statusline has no branch and no profiles segment.** The drawing in the blueprint is corrected. `ui.json` / `ui_get` / `ui_set` having been built in E-40 instead of E-41 is accepted.

### Alternatives considered
1. **`tauri-plugin-dialog` with one permission** — rejected: it brings `tauri-plugin-fs` into the lockfile.
2. **A branch segment** — rejected: it needs `git` in the process list or a read inside the project, and one more `wsl.exe` call per tab on Windows; tmux already shows it.
3. **A profiles-count segment** — rejected: a CLI run at every window start for a number; Profiles is one click away in the menu.
4. **Pass profile names as positional parameters to a constant script** — rejected for the same quoting reason as D-038 addendum 1.

### Constraints driving this decision
- D-033 ruling 7 (closed process list and capabilities), D-038 (fixed `wsl.exe` shapes).

### Impact
- Unlocks: E-41 with no statusline additions.
- Risk if wrong: the profile-script exception relies on the character check being applied to every word; a test asserts refusal of each shell metacharacter (`security_engineer`: 21 of 21 payloads refused on bash, zsh, dash).

### Rollback
Segments can be added later behind the `Host` seam; the dialog call is one function.

## D-048 — A task is not DONE while hosted CI on its commit is red

**Date**: 2026-11-12
**Task**: review of E-38..E-41; process ruling
**Decision**: From E-46 on, an E-task is marked DONE only after the hosted `ci` run (and `desktop-windows`, when the change touches the window) on its commit is green. "Local CI PASS" alone is not acceptance.

### Why needed
A recent-projects test assumed POSIX temp paths. It failed on Windows from E-38 on, so the Windows CI job and the installer workflow were red through E-38, E-39, E-45, E-40 and E-41 while each was marked DONE on a local pass. The operator's installer could not be rebuilt during that time. Fixed in 9804f26 (test only; production behaviour unchanged).

### Rulings
1. Hosted CI green on the task's commit is part of every task's acceptance.
2. A red hosted run found after DONE is fixed before the next task starts.
3. The layout-equality test stays local (D-032); this ruling does not change that.

### Alternatives considered
1. **Keep local-only acceptance** — rejected: this machine has no Windows or macOS toolchain, so local cannot see two of three platforms.
2. **Block the DONE mark with a hook** — deferred: a rule is enough for one Engineer; revisit if it recurs.

### Constraints driving this decision
- Three supported platforms, one development machine (Linux under WSL2).

### Impact
- Unlocks: nothing; slows each task by one CI wait.
- Risk if wrong: none beyond the wait.

### Rollback
Drop the rule; acceptance returns to the task text.

## D-049 — The window gets a modern look: ordinary shadcn/ui chrome, bundled fonts, a `rein` default theme, no console window

**Date**: 2026-11-12
**Task**: operator feedback with a screenshot of the first Windows build ("so unpolished — use the UI package, I want a modern look and feel"; a console window opens beside the app and closing it closes the app; "tmux should be themed to look like part of that window"; "icons are not rendering, seems like a font issue"). Supersedes D-033 ruling on the grid look (design rule 1) and D-037 ruling 3 ("shadcn's default look is not shipped", "no icon set") for the chrome. E-47..E-50.
**Decision**: The chrome becomes a normal modern desktop UI built from shadcn/ui as it comes; only the terminal stays monospace. The app bundles its terminal fonts, matches rein's tmux colours by default, and is a Windows GUI program.

### Why needed
The grid restyle was the Architect's choice in D-033 / D-037, not the operator's; the operator asked for shadcn/ui and got a terminal imitation of it. The other three items are defects of the first build.

### Rulings
1. **Look:** proportional UI font, `lucide-react` icons, 8 px radius, shadows, short transitions; tokens derived from the theme by one tested function. Tabs move into a custom title bar (own window buttons on Windows, overlay on macOS, native decorations on Linux by default). Status bar replaces the statusline and the permanent command-line row; messages become toasts. Detail: `desktop.md` §Look and feel v2.
2. **Behaviour is frozen:** `Action` list, both key tables, IPC table, `modes`, Host seam and all security boundaries are unchanged. The capability file gains exactly four `core:window` permissions for the custom title bar.
3. **Fonts:** JetBrains Mono is bundled and is the default; Symbols Nerd Font Mono is bundled and always appended as a fallback. Vendored with licences and checksums.
4. **Theme:** a built-in `rein` theme copies rein's tmux colours and is the default; the terminal background is the window background.
5. **tmux itself is not configured by the app** (contract §3.5). Making tmux follow other themes and drop the duplicated session name is requested from rein (`plans/rein-window-theme-request.md`).
6. **Windows:** GUI subsystem; errors with no window go to a message box; helper processes start hidden.
7. Bundle limit: JS + CSS 2 MiB, fonts 3 MiB.

### Alternatives considered
1. **Polish the grid look** — rejected: the operator asked for a modern look twice.
2. **Rely on an installed Nerd Font** — rejected: the default must render tmux's glyphs on a clean Windows machine.
3. **Have the app run `tmux set` to restyle or hide the status line** — rejected: contract §3.5; it would also change the session for a plain terminal attached to it.
4. **Keep native Windows decorations with a tab row beneath** — rejected for Windows: two stacked bars is the look the operator called unpolished; kept as the Linux default where window managers differ.
5. **Bundle Inter as the UI font** — rejected: the system UI font looks native on each platform and costs nothing.

### Constraints driving this decision
- D-033 ruling 7 (CSP, closed IPC and capabilities), D-044 ruling 4, contract §3.5, D-048.

### Impact
- Unlocks: E-47 → E-48 → E-49 → E-50, a new installer after E-47 and after E-50.
- Risk if wrong: custom window buttons lose Windows 11's snap-layout flyout on the maximise button; the `rein` theme copies colours that rein can change later; the redesign touches every UI component, so the parity and accessibility tests are the guard; installer size grows by the fonts.

### Rollback
Presentation only — revert the UI commits to return to the grid chrome; fonts, the `rein` theme and the GUI-subsystem fix are independent and can stay.

### D-049 addendum 1 (2026-11-12) — there is no rein tmux theme; the default theme is `catppuccin-mocha`
E-48 finding: rein at 3994640 sets no tmux colours; the blue / mauve status line in the operator's screenshot is the operator's own `~/.tmux.conf` (catppuccin plugin, flavour mocha). Ruling 4 and ruling 5's request were built on the Architect's unverified reading of a screenshot and are corrected: (1) no `rein` theme is added; (2) `catppuccin-mocha` replaces `gruvbox-dark` as the default; (3) the terminal background equals the window background in every theme; (4) the theme choice (first run, Settings) carries one line telling the user to pick the theme their tmux uses; (5) `plans/rein-window-theme-request.md` is withdrawn — nothing is asked of rein. Rejected: an alias named `rein` (it would present the operator's dotfile as rein's look). E-48's task text on the `rein` theme is superseded.

## D-050 — The window applies its own tmux look to the session it is attached to, and tmux undoes it on detach

**Date**: 2026-11-12
**Task**: operator direction ("I don't want to touch my existing tmux config, but on top of it rein-app should have its own setup that overrides when the app is open, so the app feels natural"); amends `desktop.md` §Core Concept ("the app never configures tmux"), §Look and feel v2 V4; adds V6. E-51.
**Decision**: While a tab's tmux client is attached, the app sets a closed list of session-scoped style options coloured from the window theme. A `client-detached` hook set at the same time unsets them when the app's client goes away.

### Why needed
D-049 addendum 1 left the look of tmux to the user's own configuration; the operator wants the app to bring its own look without editing that configuration.

### Rulings
1. **Scope:** session-scoped options only (window-scoped for border styles), from the closed list in V6. No global option, no key binding, no file, no `pane-border-format`.
2. **The app never creates, names, renames or kills a session and never mints a role.** The session is found by the tty of the app's own client.
3. **One constant `sh` script on stdin**, values as validated positional words — the same script on Linux, macOS and through `wsl.exe`. This avoids every command-line quoting problem recorded in D-038.
4. **tmux restores itself** through the `client-detached` hook, so a crash cannot leave the overlay behind.
5. **Settings:** `[tmux] overlay` (default on) and `status` = `hidden` (default) | `app` | `keep`.
6. Requires tmux ≥ 3.2; otherwise, or on any failure, nothing is styled and the health view says why.
7. **Contract:** this narrows our reading of rein contract §3.5 by the operator's direction; rein's Architect is to be notified (`plans/rein-window-theme-request.md`), and the overlay is switched off by default again if rein objects on a ground the operator accepts.

### Alternatives considered
1. **Ask rein to apply a host theme in `rein start`** — rejected for now: rein has no tmux look of its own to extend, and the operator wants the look owned by the app.
2. **Write an include file and have the user source it from `~/.tmux.conf`** — rejected: the operator does not want their config touched.
3. **Start tmux with the app's own config (`tmux -f`)** — rejected: the app does not start tmux, `rein start` does; and it would drop the user's bindings.
4. **Recolour the user's status-line plugin through its own variables** — rejected: plugin-specific.
5. **The app restores the options itself on close** — rejected: a crash would leave them; the hook is safer.

### Constraints driving this decision
- D-033 ruling 7 (closed process list, validated arguments), D-038 (no interpolation through `wsl.exe`), rein contract §3.5.

### Impact
- Unlocks: E-51.
- Risk if wrong: a plain terminal attached to the same session sees the overlay while the app is attached; `status = hidden` hides a status line the user relies on (one setting changes it); rein may use a private tmux socket (E-51 checks first and stops if so); `#{hook_client}` behaviour differs across tmux versions (tested in CI on the runner's tmux, version recorded).

### Rollback
Set `overlay = false` (or ship that as the default); delete the script and the Host method. Detaching always restores the user's look.

## D-051 — Bundled terminal fonts: JetBrains Mono 2.304 and Symbols Nerd Font Mono 3.5.1 (E-48, dependency_gate, Engineer-recorded)

**Date**: 2026-11-12
**Task**: E-48 (desktop.md §Look and feel v2 V3; D-049: Nerd Font glyphs showed as empty boxes)
**Decision**: ADOPT, as vendored font files (assets, no code), the upstream release files byte for byte:
- **JetBrains Mono 2.304** (GitHub `JetBrains/JetBrainsMono` release v2.304, 2023-01-14; the latest release): `fonts/webfonts/JetBrainsMono-{Regular,Bold,Italic,BoldItalic}.woff2`, 380 768 B total; licence OFL-1.1 (`OFL.txt`).
- **Symbols Nerd Font Mono 3.5.1** (GitHub `ryanoasis/nerd-fonts` release v3.5.1, 2026-08-21, `NerdFontsSymbolsOnly.zip`): `SymbolsNerdFontMono-Regular.ttf`, 2 610 012 B; licence MIT (`LICENSE`), the bundled icon sets under their own permissive licences as listed by the project.
Total 2 990 780 B (2.85 MiB), within V3's 3 MiB budget. D-049 already rules that these fonts are bundled; this entry records versions, sources, sizes and sums. Operator, 2026-11-11: "don't ask me for permissions, just build it".

### Why
Default terminal fonts differ per machine, and Windows has no Nerd Font, so Powerline and icon glyphs from tmux, prompts and Claude Code render as empty boxes. A bundled primary font plus an always-appended symbols font draws them everywhere, whatever `font.family` says.

### Alternatives considered
1. **Ask the operator to install a Nerd Font** — the report is exactly that it is not installed; a default must work.
2. **A patched "JetBrainsMono Nerd Font" only** — larger (≈ 2.4 MB per style, ≈ 9.5 MB for four) and only one family gets the icons; the symbols-only font serves any family.
3. **Convert the symbols TTF to woff2** — about half the size, but needs a converter in the build and the sums would no longer equal upstream's file; the TTF fits the budget.

### Security / maintenance
Fonts are parsed by the WebView's font engine (WebView2 / WebKit), the same engine that parses any web font; they are served from the app itself (`default-src 'self'` covers `font-src`, CSP unchanged) and never fetched at run time. SHA-256 sums are kept in `ui/fonts/MANIFEST.sha256` and checked by a test. Both projects are maintained (Nerd Fonts released 2026-08; JetBrains Mono is stable and its 2.304 is the current release).

### Rollback
Delete `ui/fonts/` and the `@font-face` rules; the font stack falls back to the platform monospace fonts as before.

## D-052 — No build goes to the operator before a real project tab has been looked at; the Architect reviews the screenshots

**Date**: 2026-11-12
**Task**: operator's second screenshot (15:13): in the build with the bundled fonts the terminal is about 80 × 24 cells in the corner of a 2560 × 1440 window. E-52; amends the acceptance of E-49, E-50, E-51.
**Decision**: A window build is handed to the operator only after an end-to-end check under Xvfb with a real `rein start` tab, whose screenshot the Architect has opened and accepted.

### Why needed
Two builds in a row reached the operator with a defect visible at a glance (a console window; then a terminal that does not fill the window). The second was introduced by E-48 and passed every test, critic and CI run, because nothing looks at a real project tab at a real window size.

### Rulings
1. **E-52 first:** fix the terminal size and add the regression tests (unit + one end-to-end size check).
2. **Screens gate:** for every task that changes the window, the Engineer saves screenshots of a real project tab (and of every screen the task touches) at 2560 × 1440 and 1280 × 800 under `.ai/screens/<task>/` and names them in the handoff. The task is not DONE, and no installer is handed over, until the Architect has opened them and replied.
3. The end-to-end size check from E-52 runs in hosted CI on Linux from then on.
4. The operator's own check on Windows stays the last step; the gate does not replace it.

### Alternatives considered
1. **Rely on `ux_reviewer` alone** — rejected: it audits contrast and layout metrics; it passed while the terminal was a quarter of the window.
2. **Send every build to the operator and let them find defects** — rejected: that is what happened twice.

### Constraints driving this decision
- No Windows desktop is observable from the development machine; Xvfb on Linux is the nearest real rendering.

### Impact
- Unlocks: E-52; slows each window task by one review round.
- Risk if wrong: Xvfb rendering differs from WebView2 on Windows, so a Windows-only defect can still pass.

### Rollback
Drop ruling 2; the size check in CI stays.

### D-050 addendum 1 (2026-11-12) — as built in E-51, ratified
Ruling 4 is amended: the overlay **puts the user's own session- and window-level values back** (saved in `@rein-saved-<option>`) instead of unsetting them — unsetting deleted values the user had set on the session by hand (security review F2). Also ratified: IPC `tab_tmux` / `allow-tab-tmux`; the internal mode word `off`; `mode-style` and `window-status-*` set per window; the detach hook compares the client inside tmux (F1); fixed hook index `[4271]`. Accepted known limit F3: after `switch-client` the overlay stays on the session the app left. Detail: `desktop.md` V6 "As built". Screens accepted under D-052 (`.ai/screens/E-51/`).

## D-053 — Icons: lucide-react 1.62.0 (E-49, dependency_gate, Engineer-recorded)

**Date**: 2026-11-12
**Task**: E-49 (desktop.md §Look and feel v2 V1 "Icons"; D-049 withdraws D-037's "no icon set" and names lucide-react)
**Decision**: ADOPT `lucide-react` **1.62.0**, a runtime dependency of the window UI, pinned exactly.
- Released 2026-10-27. The newest is 1.72.0 (2026-11-11). This is the latest release at least two weeks old, so a compromised new release has time to be noticed before it can reach us.
- D-049 already rules that lucide-react is used. This entry records the version and the checks; the operator, 2026-11-11: "don't ask me for permissions, just build it" (as for D-051).

### Why needed
The modern chrome (V2) needs icons:
- tab close, `+`, menu, minimise / maximise / restore / close;
- the health badge (check / warning / cross / question);
- copy, and the exited-tab dot.

Drawing them as Unicode glyphs depends on the UI font and is what D-049 replaces.

### Alternatives considered
1. **Hand-written inline SVG components.** No dependency, but about 15 icons drawn and kept by us, and every later icon added by hand. lucide is the icon set shadcn/ui's new-york style is built around, so the shadcn components (E-50) expect it.
2. **@radix-ui/react-icons.** Smaller set, 15 × 15 grid, does not match new-york's lucide icons; would need a second set later.
3. **An icon font.** Rejected: a font load and glyph-box failure mode, the problem V3 just fixed for the terminal.

### Size / weight
- The package is 36 MB unpacked: one ES module per icon, plus types.
- Only named imports are bundled: esbuild tree-shakes the per-icon modules. About 15 icons add roughly 10–15 KB minified to `app.js`.
- The bundle test keeps JS + CSS ≤ 2 MiB (V3).
- No runtime dependencies. Peer dependency: react ^16.5.1 … ^19.

### Security track record
- No published advisories for lucide-react at the time of writing.
- `npm audit` is run at install; its result is recorded in the E-49 handoff.
- The icons are static SVG elements built with React (no `dangerouslySetInnerHTML`). They render inside the existing CSP (`default-src 'self'`, no inline style needed).

### Maintenance status
Actively maintained: releases are roughly weekly (1.58 → 1.72 between 2026-10-16 and 2026-11-11).

### License
ISC (compatible with the project's Apache-2.0).

### Rollback
Remove the dependency and the `lucide-react` imports in `ui/src/` (the icon components are named in one place, `ui/src/icons.ts`). Replace them with inline SVG, or with text labels in the buttons' existing accessible names.

## D-054 — Every wait in the window is shown; motion is small, CSS-only and never delays an action

**Date**: 2026-11-12
**Task**: operator direction after the E-53 report ("add some loader animations; everything should be cool, minimal, modern"). `desktop.md` V7; E-54.
**Decision**: The window gets three loading primitives (spinner, 2 px progress line, skeleton), a `starting` state for a tab until tmux has drawn, busy states on the health badge, dialogs, start screen and picker, and short enter / exit transitions — all from one set of motion tokens, in the bundled stylesheet, with no animation library.

### Why needed
A new tab shows one line and a bare cursor for a few seconds before tmux draws; the operator read it as a blank, broken tab (E-53). The health check and the update dialogs are silent while they run, and the window opens with a flash.

### Rulings
1. The terminal's content is never covered, dimmed or animated; the tab loader is a 2 px line above the stage, a spinner in the tab pill and a status-bar label.
2. An animation never delays an action, and a loader is not shown for a wait under 150 ms.
3. CSS only: classes and keyframes in the bundled stylesheet. No new dependency, no inline or injected style; the CSP is unchanged.
4. Under `prefers-reduced-motion` nothing moves; text labels carry the meaning. No setting is added for this.
5. An idle window runs no animation.
6. E-54 runs after E-50, so it animates the final components, and passes the screens gate (D-052).

### Alternatives considered
1. **A full-stage "Starting…" overlay over the terminal** — rejected: it would hide a real error printed by `rein start`.
2. **An animation library (`motion`)** — rejected: a dependency and runtime styles the CSP refuses, for effects CSS covers.
3. **Fold it into E-49 / E-50** — rejected: E-49 is built and awaiting its screens; mixing motion into the restyle makes both harder to review.

### Constraints driving this decision
- CSP refuses runtime styles (D-044 ruling 4); bundle ≤ 2 MiB (E-49).

### Impact
- Unlocks: E-54. If E-53 finds the wait is `rein start`'s own start-up, the tab start state is the answer to it.
- Risk if wrong: motion that reads as decoration; the reduced-motion path and ruling 5 bound it.

### Rollback
Revert the E-54 UI commit; presentation only.

## D-055 — Sessions opened in the window run on rein-app's own tmux server; the user's tmux config and plugins do not apply

**Date**: 2026-11-12
**Task**: operator report on the E-54 build ("it also picks up my tmux plugins … this should be in the scope of rein-app if we are using rein-app"), and their three answers: own server with a clean config; plugins later, from rein's own list only; isolation now. `desktop.md` V8; E-57.
**Decision**: A tab's `rein start` reaches a separate tmux server (socket `rein-app`, rein-app's own constant config) through a constant `tmux` shim placed first on the tab's `PATH`. The user's server, config, plugins and sessions are untouched.

### Why needed
tmux reads config and starts plugins once per server. The V6 overlay sets session options and can hide what a plugin draws, but cannot stop its scripts, bindings and hooks. Only a server that never read the user's config is free of them.

### Rulings
1. Socket name `rein-app`; config is a closed list of options with no command-running line.
2. The shim, because rein 0.5.0 calls plain `tmux` and tmux has no environment variable for the config file. A native option is requested from rein (`plans/rein-tmux-server-request.md`); the shim goes when rein ships it.
3. `tmux.server = "app" | "user"`, default `app`; `user` is the previous behaviour and the rollback.
4. The V6 overlay stays and talks to the app's server.
5. Tier 3: a directory on `PATH` is a hijack surface — ownership and mode checks, constant contents rewritten on every start, `security_engineer` review.
6. Plugins (rein's own list, on/off in Settings) are a later revision; nothing here downloads or loads one.
7. This narrows our reading of rein contract §3.5 further than D-050. rein's Architect is told in the request file.

### Alternatives considered
1. **`TMUX_TMPDIR` only** — rejected: it moves the socket, but the new server still reads `~/.tmux.conf`.
2. **Change `HOME` or `XDG_CONFIG_HOME` for the tab** — rejected: it would change the environment of Claude and every tool in the panes.
3. **Wait for a rein option** — rejected as the only path: it needs a signed rein release; requested in parallel.
4. **Copy a list of the user's settings into the app's config** — not chosen by the operator.
5. **Override more options on the shared server** — rejected: cannot stop plugin code.

### Constraints driving this decision
- rein-app holds no Rein logic and does not edit rein; `rein start` stays the tab's only command.
- On Windows the app starts only `wsl.exe` with constant scripts.

### Impact
- Unlocks: E-57; the later plugin revision.
- Risk if wrong: a project with a live session on the user's own server gets a second session (second Architect and Engineer) when opened in the window; a program calling tmux by absolute path bypasses the shim. Both are documented known limits and questions to rein.

### Rollback
`tmux.server = "user"`, or revert E-57. Sessions on the app's server stay reachable with `tmux -L rein-app`.

## D-056 — The window gets a side menu of data-only panels; first Dashboard, Communication, Tasks and "Needs you"

**Date**: 2026-11-13
**Task**: operator direction ("we should have side menu with those plugins … tasks, communication, issue tracking"), and their answer to my question: "data-only is fine, start with dashboard, communication, tasks and needs you".
**Decision**: A rail on the window's left edge opens one panel beside the terminal. A panel is a descriptor naming a source and a view from closed lists compiled into the app; it holds no code. The four first panels are compiled in. Blueprint: `.ai/blueprints/panels.md`.

### Why needed
The operator follows the Triad by reading two panes. Tasks, handoffs and "whose turn" already exist as files in each project's `.ai/`; nothing shows them.

### Rulings
1. Data-only: no panel code, HTML, URL or path comes from a descriptor. File text is drawn as text nodes only.
2. Sources read two constant paths (`.ai/state.json`, the tail of `.ai/COMM.md`), read-only, bounded, and answer "not available" on anything unexpected. Both formats are outside the contract, so the parsers are provisional; a stable interface is requested from rein (`plans/rein-panels-request.md`).
3. Descriptors from `rein-plugins` are deferred until its Architect and rein's answer. A panel from outside may only combine existing sources and views; a new source is always an app release.
4. "Needs you" shows only what the app can know. A permission prompt in a pane is not visible without a signal from rein; this gap is stated in the panel's design and to the operator.
5. The unit is called a "panel", to keep it apart from Claude Code plugins and tmux plugins.

### Alternatives rejected
- **Panels that ship their own UI code**: every panel would need a sandbox, pinning and a signature check. The operator chose data-only.
- **Waiting for rein's read interface before building anything**: no rein session is reachable; the four panels are useful now and the parsers fail soft.
- **A Markdown renderer for handoffs**: turns untrusted file text into markup and links. Plain text with visible structure is enough.
- **Reading rein's sqlite files or MCP servers from the app**: carries rein's internals into the shell (contract non-goal).

## D-057 — The window offers set-up, update and close for a project, running only rein's own commands on a click

**Date**: 2026-11-13
**Task**: operator reports on the E-58 build: `/exit` in a Claude pane leaves a shell and the project stays open ("should be smart enough to close whole project"); a folder without `.ai/` should list what is needed and offer to install ("npm init rein init"); a stale `.ai/` should prompt for `rein sync`.
**Decision**: Three prompts in the tab — a set-up card, an out-of-date bar, an agent-exit bar — each offering one rein entry point (`rein init`, `rein sync`, `rein start --kill`). Blueprint: `desktop.md` V9.

### Rulings
1. The window runs only rein's entry points, as constant commands, each after a click. It does not run `npm init`, `git init` or any other tool in a project; such prerequisites are listed with the command to copy. Reason: contract §3.3 (rein-app writes no file inside a project) and the non-goal of carrying Rein logic. This differs from the operator's wording ("installs everything npm init"); they have been told, and rein is asked to make `rein init` cover what it can.
2. What is missing or out of date is rein's verdict, never the app's own comparison of versions or files.
3. One agent exited: a bar with Close project / Keep open. All agents exited: the project closes by default (`close_when_agents_exit`, default true). Closing with an agent still running asks first.
4. Close project is `rein start --kill --yes` on the tab's own tmux server. Closing a tab the ordinary way still leaves the session running (D-034).
5. `rein init` runs in the tab's own terminal so its output and questions are visible.

### Alternatives rejected
- **Closing the project as soon as any one agent exits**: kills the other agent's running work on a mistaken `/exit`.
- **The app comparing `state.json`'s `rein_version` with the installed version**: reimplements a rein check.
- **Running set-up automatically on open**: writes into a folder the user may have opened by mistake.
- **A tmux hook in the app's config to detect the exit**: the config is a closed list with no command-running line (V8).
