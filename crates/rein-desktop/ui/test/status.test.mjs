// A project's status as the window draws it (status.md §Components 2 and 3, D-062): the mark,
// the accessible name, the tooltip lines and the status-bar segment per state — pure, from what
// the core sent. The core's table (which state) is status.rs's test; the drawn tab bar is in
// motion.test.mjs ("status marks do not animate").
import { test } from "node:test";
import assert from "node:assert/strict";
import { MARKS, accessibleName, quietSpan, sentence, statusMark, statusSegment, tooltipLines } from "../src/status.ts";
import { DEFAULT_CONFIG } from "../src/config.ts";
import { statusBar } from "../src/chrome.ts";

const item = { key: "b", kind: "bell", index: 0, tab: 4, text: "t", quote: null, list: [], dismissable: true };
const st = (state, over = {}) => ({ tab: 4, state, quiet_s: 0, worked_s: 0, waiting: null, now: [], info: [], ...over });

test("each state has its mark, sentence and accessible name", () => {
  const want = {
    setup: ["wrench", "Needs set-up."],
    ended: ["square", "Ended."],
    starting: ["spinner", "Starting…"],
    needs_you: ["bell-dot", "Needs you."],
    working: ["circle-dot", "Working."],
    quiet: ["circle", "Quiet for 45 s."],
    unknown: [null, "—"],
  };
  assert.deepEqual(Object.keys(MARKS).sort(), Object.keys(want).sort(), "the closed list of states");
  for (const [state, [mark, text]] of Object.entries(want)) {
    const s = st(state, { quiet_s: 50 });
    assert.equal(MARKS[state], mark, state);
    assert.equal(sentence(s), text, state);
    assert.equal(accessibleName("schraubr", s), `schraubr: ${text}`, state);
    const m = statusMark("schraubr", s, true);
    assert.equal(m?.mark ?? null, mark, `${state}: drawn mark`);
  }
});

test("the bell's count shows only over one", () => {
  assert.equal(statusMark("p", st("needs_you", { now: [item] }), true).count, null);
  assert.equal(statusMark("p", st("needs_you", { now: [item, item, item] }), true).count, 3);
  assert.equal(statusMark("p", st("working", { now: [item, item] }), true).count, null);
});

test("quiet time in the core's steps: 15 s under a minute, then minutes, then hours", () => {
  assert.deepEqual([15, 29, 30, 59, 60, 119, 3599, 3600, 7300].map(quietSpan), ["15 s", "15 s", "30 s", "45 s", "1 min", "1 min", "59 min", "1 h", "2 h"]);
  assert.equal(sentence(st("quiet", { quiet_s: null })), "Quiet.");
});

test("the tooltip: at most three lines, the task and the turn only when known", () => {
  // Per state, with nothing else known: the sentence alone.
  const alone = { setup: "Needs set-up.", ended: "Ended.", starting: "Starting…", needs_you: "Needs you.", working: "Working.", quiet: "Quiet for 1 min.", unknown: "—" };
  for (const [state, line] of Object.entries(alone)) assert.deepEqual(tooltipLines(st(state, { quiet_s: 75 })), [line], state);
  // Per state, with the task and the turn known: the same three lines under each sentence.
  const known = { in_progress: { id: "E-80", title: "Build Home." }, turn: "Architect", last_handoff_s: 90 };
  for (const [state, line] of Object.entries(alone)) {
    assert.deepEqual(tooltipLines(st(state, { quiet_s: 75, ...known })), [line, "In progress: E-80 — Build Home.", "Architect's turn · last handoff 1 min ago"], state);
  }
  // A drawn tab's tooltip is these lines (the mark carries them); unknown draws no mark.
  for (const state of Object.keys(alone)) {
    const m = statusMark("p", st(state, { quiet_s: 75 }), true);
    assert.deepEqual(m?.lines ?? null, state === "unknown" ? null : [alone[state]], state);
  }
  const full = st("quiet", {
    quiet_s: 600,
    in_progress: { id: "E-77", title: "Show each project's status in the tab bar. Operator direction." },
    turn: "Engineer",
    last_handoff_s: 7200,
  });
  assert.deepEqual(tooltipLines(full), [
    "Quiet for 10 min.",
    "In progress: E-77 — Show each project's status in the tab bar.",
    "Engineer's turn · last handoff 2 h ago",
  ]);
  assert.deepEqual(tooltipLines({ ...full, last_handoff_s: null }).at(-1), "Engineer's turn");
  for (const id of ["e-77", "E-1234567", "E-77 ", "<b>", "EE-1"]) {
    const lines = tooltipLines({ ...full, in_progress: { id, title: "x" } });
    assert.ok(!lines.some((l) => l.startsWith("In progress")), `a task id ${JSON.stringify(id)} is not shown`);
    assert.equal(statusSegment({ ...full, in_progress: { id, title: "x" } }), "Quiet for 10 min. · Engineer's turn");
  }
  assert.ok(tooltipLines(full).length <= 3);
});

test("the status-bar segment: sentence, task id, whose turn", () => {
  assert.equal(statusSegment(st("working", { in_progress: { id: "E-77", title: "t" }, turn: "Architect" })), "Working. · E-77 · Architect's turn");
  assert.equal(statusSegment(st("ended")), "Ended.");
  assert.equal(statusSegment(st("unknown")), null);
  const quiet = statusMark("p", st("quiet", { quiet_s: 20 }), true);
  assert.equal(statusBar(null, null, null, null, { status: quiet }).status, quiet);
  assert.equal(statusBar(null, null, null, null, { status: quiet, starting: true }).status, null, "the bar already says Starting rein…");
});

test("[tabs] status: shown by default; hidden or not yet known draws no mark", () => {
  assert.equal(DEFAULT_CONFIG.tabs.status, "shown");
  assert.equal(statusMark("p", st("working"), false), null, "hidden");
  assert.equal(statusMark("p", undefined, true), null, "no status from the core yet");
  assert.equal(statusMark("p", st("unknown"), true), null, "unknown has no mark");
});
