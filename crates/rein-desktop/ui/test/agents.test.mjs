// desktop.md §Look and feel v2 V9 "Tests" (D-057, E-62): the agent-exit bar's states from each
// answer — "one ended" vs "all ended", the setting, Keep open — the confirm rule's wording, and
// when the window polls. New suite: the bar is new; nothing tested agent state before.
import { test } from "node:test";
import assert from "node:assert/strict";
import { agentBar, confirmText, roleName, shouldPoll } from "../src/agents.ts";

const a = (role, state) => ({ role, state });
const on = { closeWhenAllExit: true, kept: null };

test("the bar from each answer, as a table", () => {
  const rows = [
    ["nothing known yet", [], on, { kind: "none" }],
    ["both running", [a("architect", "running"), a("engineer", "running")], on, { kind: "none" }],
    ["still starting (unknown)", [a("architect", "unknown"), a("engineer", "unknown")], on, { kind: "none" }],
    [
      "one of two ended: the bar, nothing by itself",
      [a("architect", "running"), a("engineer", "ended")],
      on,
      { kind: "bar", text: "The Engineer has exited.", key: "engineer" },
    ],
    [
      "one ended, one unknown: not all ended — the bar",
      [a("architect", "unknown"), a("engineer", "ended")],
      on,
      { kind: "bar", text: "The Engineer has exited.", key: "engineer" },
    ],
    ["all ended, setting on: close without asking", [a("architect", "ended"), a("engineer", "ended")], on, { kind: "close" }],
    [
      "all ended, setting off: the bar",
      [a("architect", "ended"), a("engineer", "ended")],
      { closeWhenAllExit: false, kept: null },
      { kind: "bar", text: "The Architect and the Engineer have exited.", key: "architect,engineer" },
    ],
    [
      "Keep open hides the bar for that set",
      [a("architect", "running"), a("engineer", "ended")],
      { closeWhenAllExit: true, kept: "engineer" },
      { kind: "none" },
    ],
    [
      "…until another role ends",
      [a("architect", "ended"), a("engineer", "ended")],
      { closeWhenAllExit: false, kept: "engineer" },
      { kind: "bar", text: "The Architect and the Engineer have exited.", key: "architect,engineer" },
    ],
  ];
  for (const [label, agents, opts, want] of rows) assert.deepEqual(agentBar(agents, opts), want, label);
});

test("the confirm names who is still running; default No is the dialog's", () => {
  assert.equal(confirmText(["architect"]), "The Architect is still running. Close it too?");
  assert.equal(confirmText(["architect", "engineer"]), "The Architect and the Engineer are still running. Close them too?");
  assert.equal(confirmText([]), "An agent may still be running. Close it too?");
  assert.equal(roleName("code-reviewer"), "The Code Reviewer");
});

test("only the active tab of a visible window is polled, while its process runs", () => {
  const live = { active: true, visible: true, open: true, exited: false };
  assert.equal(shouldPoll(live), true);
  assert.equal(shouldPoll({ ...live, active: false }), false);
  assert.equal(shouldPoll({ ...live, visible: false }), false);
  assert.equal(shouldPoll({ ...live, open: false }), false);
  assert.equal(shouldPoll({ ...live, exited: true }), false);
});
