// updates.md §Components 1 and 4 for Rein, §Tests (D-065): what the web side draws from the
// update watch's state. New suite: the watch is new; the schedule itself is the core's test
// (crates/rein-desktop/src/updates.rs, the_schedule_follows_the_clock).
import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtempSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { build } from "esbuild";
import {
  STEP_SENTENCES,
  appAskText,
  appCard,
  appCardText,
  percent,
  runningNotes,
  updatedTo,
  appNewer,
  compareVersions,
  installedChanged,
  recheckOrder,
  reinAsk,
  reinAskText,
  reinNewer,
  showUpdateButton,
  updateRows,
} from "../src/updates.ts";

const state = (rein = {}, asked = {}, app = {}) => ({
  rein: { installed: "0.5.0", newest: null, checked_at: null, offline: false, ...rein },
  app: { running: "0.1.0", newest: null, notes: null, mode: "none", command: null, updated_from: null, ...app },
  job: "idle",
  asked: { rein: null, app: null, ...asked },
});

test("versions compare as semver, a pre-release below its release, junk is no answer", () => {
  assert.equal(compareVersions("0.5.1", "0.5.0"), 1);
  assert.equal(compareVersions("0.10.0", "0.9.9"), 1, "numbers, not text");
  assert.equal(compareVersions("v0.5.0", "0.5.0"), 0);
  assert.equal(compareVersions("0.6.0-rc.1", "0.6.0"), -1);
  assert.equal(compareVersions("0.6.0", "0.6.0-rc.1"), 1);
  assert.equal(compareVersions("0.6.0+g1", "0.6.0"), 0, "build metadata is ignored");
  assert.equal(compareVersions("latest", "0.5.0"), null);
});

test("the button and the card show only for a newer Rein; Later quiets the card, not the button", () => {
  assert.equal(reinNewer(null), null);
  assert.equal(showUpdateButton(state()), false, "nothing known yet");
  assert.equal(showUpdateButton(state({ newest: "0.5.0" })), false, "current");
  assert.equal(showUpdateButton(state({ newest: "0.4.9" })), false, "never an older one");
  assert.equal(showUpdateButton(state({ installed: null, newest: "0.5.1" })), false, "nothing installed to update");
  const newer = state({ newest: "0.5.1" });
  assert.equal(reinNewer(newer), "0.5.1");
  assert.equal(showUpdateButton(newer), true);
  assert.equal(reinAsk(newer), "0.5.1");
  const later = state({ newest: "0.5.1" }, { rein: "0.5.1" });
  assert.equal(reinAsk(later), null, "Later: this version does not ask again");
  assert.equal(showUpdateButton(later), true, "the button stays");
  assert.equal(reinAsk(state({ newest: "0.5.2" }, { rein: "0.5.1" })), "0.5.2", "a newer version asks once more");
  assert.equal(reinAskText("0.5.1"), "Rein 0.5.1 is available. Your projects keep running during the update.");
});

test("the window's row offers nothing until its own update exists", () => {
  assert.equal(appNewer(state({}, {}, { newest: "0.2.0" })), null, "mode none: no offer");
  assert.equal(appNewer(state({}, {}, { newest: "0.2.0", mode: "in_place" })), "0.2.0");
  assert.equal(appNewer(state({}, {}, { newest: "0.1.0", mode: "in_place" })), null, "no same version");
});

test("the Update dialog has two rows: Rein and the window", () => {
  assert.deepEqual(updateRows(state({ newest: "0.5.1", checked_at: 1 })), [
    { name: "Rein", from: "0.5.0", to: "0.5.1", note: "newer release" },
    { name: "rein-app window", from: "0.1.0", to: null, note: "no newer window release" },
  ]);
  assert.equal(updateRows(state({ newest: "0.5.0", checked_at: 1 }))[0].note, "up to date");
  assert.equal(updateRows(state({ offline: true }))[0].note, "the release server cannot be reached");
  assert.equal(updateRows(state())[0].note, "not checked yet");
  assert.deepEqual(
    updateRows(null).map((r) => r.from),
    ["not installed", "unknown"],
    "before the first state",
  );
});

test("a changed installed Rein asks every open tab again, the active tab first", () => {
  assert.equal(installedChanged(null, state()), false, "the first state is no change");
  assert.equal(installedChanged(state(), state()), false);
  assert.equal(installedChanged(state(), state({ newest: "0.5.1" })), false, "only the installed version counts");
  assert.equal(installedChanged(state(), state({ installed: "0.5.1" })), true);
  assert.deepEqual(recheckOrder(4, 2), [2, 0, 1, 3]);
  assert.deepEqual(recheckOrder(3, 0), [0, 1, 2]);
  assert.deepEqual(recheckOrder(2, -1), [0, 1], "no current tab");
  assert.deepEqual(recheckOrder(0, 0), []);
});

// ── The window's own update drawn (updates.md §Components 4, D-066; E-87) ──

const offer = (job = "idle", over = {}, asked = {}) =>
  ({ ...state({}, asked, { newest: "0.2.0", mode: "in_place", notes: "- Faster tabs.", ...over }), job });

test("the window's card: asked once per version, each job state its sentence", () => {
  assert.deepEqual(appCard(offer()), { at: "ask", version: "0.2.0", mode: "in_place", command: null });
  assert.equal(appCardText(appCard(offer())), "rein-app 0.2.0 is available. Your projects keep running during the update.");
  assert.equal(appAskText("0.2.0"), appCardText(appCard(offer())));
  assert.equal(appCard(offer("idle", {}, { app: "0.2.0" })), null, "Later: this version does not ask again");
  assert.equal(appCard(offer("idle", {}, { app: "0.1.5" })).version, "0.2.0", "a newer version asks once more");
  assert.equal(appCard(offer("idle", { newest: "0.1.0" })), null, "no same version");
  assert.equal(appCard(offer("idle", { mode: "none" })), null);
  const dl = appCard(offer({ downloading: { done: 41, total: 100 } }, {}, { app: "0.2.0" }));
  assert.deepEqual(dl, { at: "downloading", version: "0.2.0", percent: 41 }, "a running job shows after Later too");
  assert.equal(appCardText(dl), "Downloading 0.2.0 — 41 %");
  assert.equal(appCardText(appCard(offer("verifying"))), "Checking the signature…");
  assert.equal(appCardText(appCard(offer("restarting"))), "Restarting…");
  for (const [step, sentence] of Object.entries(STEP_SENTENCES)) {
    const f = appCard(offer({ failed: step }));
    assert.equal(f.at, "failed");
    assert.equal(appCardText(f), sentence);
  }
  assert.equal(STEP_SENTENCES.signature, "The update's signature did not verify. Nothing was installed.", "the core's sentence");
});

test("the download's percent counts in whole numbers and never passes 100", () => {
  assert.equal(percent(0, 0), 0);
  assert.equal(percent(5, 0), 0, "no size known");
  assert.equal(percent(1, 3), 33);
  assert.equal(percent(999, 1000), 99, "100 only when complete");
  assert.equal(percent(1000, 1000), 100);
  assert.equal(percent(2000, 1000), 100);
});

test("Updated to: once after a restart; What's new only with the running version's notes", () => {
  assert.equal(updatedTo(offer()), null);
  const after = state({}, {}, { running: "0.2.0", updated_from: "0.1.0", notes: "- Faster tabs." });
  assert.equal(updatedTo(after), "0.2.0");
  assert.equal(runningNotes(after), "- Faster tabs.");
  assert.equal(runningNotes(offer()), null, "the notes of a newer offer are not this version's");
});

const render = await (async () => {
  const ui = fileURLToPath(new URL("..", import.meta.url));
  const out = join(mkdtempSync(join(tmpdir(), "rein-upd-")), "update.mjs");
  await build({
    entryPoints: [join(ui, "test/fixtures/update-render.tsx")],
    bundle: true,
    platform: "node",
    format: "esm",
    jsx: "automatic",
    outfile: out,
    banner: { js: 'import { createRequire } from "node:module"; const require = createRequire(import.meta.url);' },
    logLevel: "error",
    define: { "process.env.NODE_ENV": '"production"' },
  });
  return import(pathToFileURL(out).href);
})();

const buttons = (html) => [...html.matchAll(/<button[^>]*>([\s\S]*?)<\/button>/g)].map((m) => m[1].replace(/<[^>]+>/g, "").replace(/&#x27;/g, "'"));

test("the card's buttons per state; none takes a tab stop (focus stays in the terminal)", () => {
  const cases = [
    [offer(), ["Later", "Update and restart"]],
    [offer("idle", { mode: "command", command: "brew upgrade rein-app" }), ["Copy", "Later"]],
    [offer("idle", { mode: "by_hand" }), ["Later"]],
    [offer({ downloading: { done: 1, total: 4 } }), ["Cancel"]],
    [offer("verifying"), []],
    [offer("restarting"), []],
    [offer({ failed: "checksum" }), ["Later", "Try again"]],
  ];
  for (const [s, want] of cases) {
    const html = render.card(s);
    assert.deepEqual(buttons(html), want, JSON.stringify(s.job) + " " + s.app.mode);
    for (const b of html.matchAll(/<button[^>]*>/g)) assert.match(b[0], /tabindex="-1"/i, b[0]);
    assert.match(html, /role="status"/);
  }
  assert.match(render.card(offer("idle", { mode: "command", command: "brew upgrade rein-app" })), /<code[^>]*>brew upgrade rein-app<\/code>/);
  const dl = render.card(offer({ downloading: { done: 41, total: 100 } }));
  assert.match(dl, /aria-valuenow="41"/);
  assert.match(dl, /width:41%/);
  assert.match(render.card(offer("restarting")), /class="check-draw"/, "the check mark before Restarting");
  assert.match(render.card(offer({ failed: "signature" })), /signature did not verify/);
  assert.equal(render.card(offer("idle", {}, { app: "0.2.0" })), "", "Later: no card");
});

test("the Updated toast and the dialog's window row", () => {
  assert.deepEqual(buttons(render.toast("0.2.0", true)), ["What's new", "Close"]);
  assert.deepEqual(buttons(render.toast("0.2.0", false)), ["Close"], "no notes yet: no link");
  assert.match(render.toast("0.2.0", true), /Updated to <!-- -->0.2.0/);
  const r = render.row(offer("idle", { notes: "<b>bold</b> **real**" }));
  assert.deepEqual(buttons(r), ["Update and restart"]);
  assert.doesNotMatch(r, /<b>bold/, "notes are text");
  assert.match(r, /<strong[^>]*>real<\/strong>/);
  assert.equal(render.row(offer("idle", { newest: null })), "", "nothing newer: no row");
});

// E-105 (D-073): the What's new dialog closes by a button, and the Update dialog — Rein and the
// window — is titled "Updates"; the action keeps "Update Rein".
test("What's new has a Close button; the Update dialog is titled Updates", async () => {
  const n = render.notes();
  assert.match(n, /What&#x27;s new in rein-app <!-- -->0.2.0|What&#x27;s new in rein-app 0.2.0/);
  const close = [...n.matchAll(/<button[^>]*>([\s\S]*?)<\/button>/g)].filter((m) => m[1].replace(/<[^>]+>/g, "") === "Close");
  assert.ok(close.length >= 1, "a Close button in the footer");
  assert.ok(n.lastIndexOf(">Close</button>") > n.indexOf("Faster tabs."), "Close sits under the notes, in the footer");
  const d = render.updateDialog();
  assert.match(d, />Updates</, "the dialog's title");
  assert.doesNotMatch(d, />Update Rein</, "not the old title");
  const { ACTIONS } = await import("../src/actions.ts");
  assert.equal(ACTIONS.find((a) => a.name === "release.update")?.label, "Update Rein", "the action keeps its label");
});
