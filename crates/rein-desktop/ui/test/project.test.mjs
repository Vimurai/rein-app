// desktop.md §Look and feel v2 V9 "Tests" (D-057, E-63; ruling 2026-11-13 14:10): the set-up
// card's rows from each check answer, when the out-of-date check runs, and what Update project
// leaves behind for each way it can end. New suite: set-up and out-of-date are new; the
// agent-exit bar keeps test/agents.test.mjs.
import { test } from "node:test";
import assert from "node:assert/strict";
import { SETUP_CARD, cardRows, fixOutcome, shouldCheck } from "../src/project.ts";

test("the card shows rein's own rows, or says why it has none", () => {
  const line = "rein init --check: no .ai/ here — rein init would provision a new project.";
  assert.deepEqual(cardRows({ not_set_up: [line] }), { rows: [line], note: null });
  assert.deepEqual(cardRows(null), { rows: [], note: SETUP_CARD.checking });
  assert.deepEqual(cardRows({ unavailable: "rein init --check did not give an answer" }), { rows: [], note: SETUP_CARD.noAnswer });
});

test("the out-of-date check runs once, after the start, unless not to be asked", () => {
  const t = { project: "/p", started: true, asked: false };
  assert.equal(shouldCheck(t, []), true);
  assert.equal(shouldCheck({ ...t, started: false }, []), false, "never delays the start");
  assert.equal(shouldCheck({ ...t, asked: true }, []), false, "once per tab open");
  assert.equal(shouldCheck(t, ["/p"]), false, "Don't ask for this project");
  assert.equal(shouldCheck(t, ["/q"]), true);
});

test("each way Update project can end, as a table", () => {
  const ok = { code: 0, stopped: null };
  const rows = [
    ["exit 0, current on the re-check: the bar goes", ok, { ready: true }, "ok", false, /^Project updated\. Running agents keep/],
    ["exit 0, still pending: one line, no second offer", ok, { out_of_date: ["x"] }, "warn", false, /still reports changes pending/],
    ["exit 0, the re-check unanswered", ok, { unavailable: "x" }, "warn", false, /did not answer/],
    ["exit 1: the bar stays", { code: 1, stopped: null }, null, "fail", true, /exit 1/],
    ["cancelled", { code: null, stopped: "cancelled" }, null, "warn", true, /^Cancelled/],
    ["10 minutes", { code: null, stopped: "timed_out" }, null, "fail", true, /10 minutes/],
  ];
  for (const [label, end, recheck, status, keepBar, text] of rows) {
    const o = fixOutcome(end, recheck);
    assert.equal(o.status, status, label);
    assert.equal(o.keepBar, keepBar, label);
    assert.match(o.text, text, label);
  }
});
