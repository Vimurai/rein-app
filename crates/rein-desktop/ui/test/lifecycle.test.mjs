// desktop.md §Look and feel v2 V11 (D-059, E-66): the words of the tab-close confirm, and what
// the toast says after a close on each server. Quit asks nothing since V12 (D-061, E-69). New suite: V11 is new; the agent-exit
// bar's confirm keeps test/agents.test.mjs.
import { test } from "node:test";
import assert from "node:assert/strict";
import * as lifecycle from "../src/lifecycle.ts";
import { agentsGone, agentsGoneText, missingText, notRunningText, recordOf } from "../src/restore.ts";
const { closeTabText, closedNote } = lifecycle;

test("the confirm names who stops; quit has no confirm text (V12)", () => {
  assert.equal(closeTabText(["architect", "engineer"], "rein-app"), "Closing this tab stops the Architect and the Engineer in rein-app.");
  assert.equal(closeTabText(["engineer"], "p"), "Closing this tab stops the Engineer in p.");
  assert.equal(closeTabText([], "p"), "Closing this tab stops the agents in p.");
  assert.equal("quitText" in lifecycle, false);
});

test("after a close: the app's server stopped the agents; the user's keeps the session", () => {
  assert.equal(closedNote("p", "app"), "p closed — its agents were stopped");
  assert.equal(closedNote("p", "user"), "p closed — its tmux session keeps running");
});

// V12 components 4–5 (D-061, E-70): the restored tab's words, "agents gone" on the first
// status read, and the record taken from the tabs (core ids, never paths).
test("a restored tab's card and bar say what happened, in the blueprint's words", () => {
  assert.equal(notRunningText("rein-app"), "rein-app is not running. Its session ended while the app was closed.");
  assert.equal(agentsGoneText("rein-app"), "The agents in rein-app are no longer running.");
  assert.match(missingText("p"), /folder is gone/);
});

test("on the first status read, no role running means the agents are gone", () => {
  assert.equal(agentsGone([]), true, "no role pane at all");
  assert.equal(agentsGone([{ role: "engineer", state: "ended" }, { role: "architect", state: "unknown" }]), true);
  assert.equal(agentsGone([{ role: "engineer", state: "running" }, { role: "architect", state: "ended" }]), false);
});

test("the record holds the tabs' core ids in tab order and where the current one is", () => {
  const t = (coreId) => ({ term: { coreId } });
  assert.deepEqual(recordOf([t(5), t(null), t(9), t(2)], 2), { ids: [5, 9, 2], active: 1 }, "a tab with no core id is left out");
  assert.deepEqual(recordOf([t(5), t(null)], 1), { ids: [5], active: null }, "the current tab has no core id");
  assert.deepEqual(recordOf([], 0), { ids: [], active: null });
});
