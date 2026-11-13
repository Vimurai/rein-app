// desktop.md §Look and feel v2 V11 (D-059, E-66): the words of the tab-close and quit confirms,
// and what the toast says after a close on each server. New suite: V11 is new; the agent-exit
// bar's confirm keeps test/agents.test.mjs.
import { test } from "node:test";
import assert from "node:assert/strict";
import { closeTabText, closedNote, quitText } from "../src/lifecycle.ts";

test("the confirms name who stops, or how many projects", () => {
  assert.equal(closeTabText(["architect", "engineer"], "rein-app"), "Closing this tab stops the Architect and the Engineer in rein-app.");
  assert.equal(closeTabText(["engineer"], "p"), "Closing this tab stops the Engineer in p.");
  assert.equal(closeTabText([], "p"), "Closing this tab stops the agents in p.");
  assert.equal(quitText(1), "Quitting stops the agents in 1 project.");
  assert.equal(quitText(3), "Quitting stops the agents in 3 projects.");
});

test("after a close: the app's server stopped the agents; the user's keeps the session", () => {
  assert.equal(closedNote("p", "app"), "p closed — its agents were stopped");
  assert.equal(closedNote("p", "user"), "p closed — its tmux session keeps running");
});
