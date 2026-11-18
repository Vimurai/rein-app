// A start that fails (desktop.md V14 components 3–5, D-068): the bar's three forms, one bar per
// tab, "Don't ask" not hiding it, rein's answer mapped, the set-up's failure not offered twice,
// and rein's rows passed through uncut (the core caps them at 200 — project_setup.rs's
// rows_are_capped_made_visible_and_counted). Extends project.test.mjs's out-of-date bar tests.
import { test } from "node:test";
import assert from "node:assert/strict";
import { afterSetUp, failedBar, failedFrom, whichBar, FAILED_LINE } from "../src/start-failed.ts";
import { INIT_FAILED } from "../src/project.ts";

const form = (f) => {
  const v = failedBar(f);
  return { lines: v.lines, buttons: v.buttons.map((b) => (b.primary ? `*${b.label}` : b.label)) };
};

test("the bar's three forms: checking, needs_init, no_fix", () => {
  assert.deepEqual(form({ at: "needs_init", rows: ["refresh CLAUDE.md"] }), {
    lines: [FAILED_LINE, "rein init --check reports changes for this folder."],
    buttons: ["*Set up this project", "Try again", "Close tab"],
  });
  assert.deepEqual(form({ at: "no_fix" }), {
    lines: [FAILED_LINE, "rein's message is in the terminal above."],
    buttons: ["*Try again", "Close tab"],
  });
  assert.deepEqual(form({ at: "checking" }), {
    lines: [FAILED_LINE, "Asking rein…"],
    buttons: ["Try again", "Close tab"],
  });
  // The bar shows no row of rein's.
  assert.ok(!JSON.stringify(failedBar({ at: "needs_init", rows: ["refresh CLAUDE.md"] })).includes("CLAUDE"));
});

test("never two bars, and Don't ask does not hide a failed start", () => {
  const base = { exited: true, update: ["refresh x"], updateHidden: false };
  assert.equal(whichBar({ ...base, failed: { at: "no_fix" } }), "failed", "failed wins over out of date");
  assert.equal(whichBar({ ...base, updateHidden: true, failed: { at: "checking" } }), "failed", "Don't ask hides only the update bar");
  assert.equal(whichBar({ exited: false, update: ["refresh x"], updateHidden: false }), "update");
  assert.equal(whichBar({ exited: false, update: ["refresh x"], updateHidden: true }), null);
  assert.equal(whichBar({ exited: true, update: ["refresh x"] }), null, "an exited tab without a failed start: no bar");
  assert.equal(whichBar({ exited: false }), null);
});

test("rein's answer: needs_init only on its word; anything else offers nothing", () => {
  const rows = Array.from({ length: 200 }, (_, i) => `row ${i + 1}`).concat(["… and 3 more"]);
  assert.deepEqual(failedFrom({ start_failed: { needs_init: rows } }), { at: "needs_init", rows });
  assert.equal(failedFrom({ start_failed: { needs_init: rows } }).rows.length, 201, "nothing cut on this side");
  assert.deepEqual(failedFrom({ start_failed: { no_fix: true } }), { at: "no_fix" });
  assert.deepEqual(failedFrom({ unavailable: "rein init --check did not give an answer" }), { at: "no_fix" });
  assert.deepEqual(failedFrom({ out_of_date: ["x"] }), { at: "no_fix" });
  assert.deepEqual(failedFrom(null), { at: "no_fix" }, "the call failed");
});

test("a set-up from the bar that fails brings the bar back once, with nothing to offer", () => {
  assert.deepEqual(afterSetUp(INIT_FAILED, true), { at: "no_fix" });
  assert.equal(afterSetUp(INIT_FAILED, false), undefined, "a set-up tab's own card handles its failure");
  assert.equal(afterSetUp(2, true), undefined, "the start after a good init: the core decides");
  assert.equal(afterSetUp(0, true), undefined);
});
