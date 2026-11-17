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

// ── V12 component 7 (D-061, E-71): the countdown after an agent's exit ──────────────────────
import { COUNTDOWN_S, counted, countdownStep } from "../src/countdown.ts";

const two = (e, a) => [
  { role: "engineer", state: e },
  { role: "architect", state: a },
];
const cdOn = (over = {}) => ({ closeWhenAgentsExit: true, project: "rein-app", shown: true, now: 0, ...over });

test("an exit starts a 5 s countdown on the active tab of a visible window; zero fires", () => {
  const t = { agents: two("ended", "running"), exited: false };
  let s = countdownStep(t, cdOn({ now: 1000 }));
  assert.deepEqual(s.view, { kind: "countdown", text: "The Engineer exited. Closing rein-app in 5 s.", key: "engineer", left: 5 });
  assert.equal(s.fire, false);
  s = countdownStep(t, cdOn({ now: 3500 }));
  assert.equal(s.view.left, 3, "a number, counted down by the clock");
  s = countdownStep(t, cdOn({ now: 1000 + COUNTDOWN_S * 1000 }));
  assert.equal(s.view.left, 0);
  assert.equal(s.fire, true, "at zero: V9's close without its confirm");
});

test("Keep open stops it until the set of ended roles changes; a changed set starts again from 5", () => {
  const t = { agents: two("ended", "running"), exited: false };
  countdownStep(t, cdOn({ now: 0 }));
  t.kept = "engineer"; // Keep open
  assert.deepEqual(countdownStep(t, cdOn({ now: 2000 })).view, { kind: "none" });
  assert.equal(t.countdown, null);
  t.agents = two("ended", "ended"); // both ended now: all ended is V9's close, not a countdown
  assert.deepEqual(countdownStep(t, cdOn({ now: 3000 })).view, { kind: "close" });
  // Three roles: a second exit changes the set and starts from 5 again.
  const u = { agents: [...two("ended", "running"), { role: "reviewer", state: "running" }], exited: false };
  countdownStep(u, cdOn({ now: 0 }));
  assert.equal(countdownStep(u, cdOn({ now: 3000 })).view.left, 2);
  u.agents[1] = { role: "architect", state: "ended" };
  const s = countdownStep(u, cdOn({ now: 3000 }));
  assert.equal(s.view.left, 5);
  assert.equal(s.view.text, "The Engineer and the Architect exited. Closing rein-app in 5 s.");
});

test("a background tab or a hidden window never counts down: the tab is marked and starts from 5 when shown", () => {
  const t = { agents: two("ended", "running"), exited: false };
  countdownStep(t, cdOn({ now: 0 }));
  let s = countdownStep(t, cdOn({ now: 2000, shown: false }));
  assert.deepEqual(s, { view: { kind: "none" }, fire: false });
  assert.equal(t.marked, true);
  assert.equal(t.countdown, null);
  s = countdownStep(t, cdOn({ now: 60000, shown: false }));
  assert.equal(s.fire, false, "never fires out of sight");
  s = countdownStep(t, cdOn({ now: 61000 }));
  assert.equal(s.view.left, 5, "from 5 when the user comes to it");
  assert.equal(t.marked, false);
});

test("the setting off keeps V9's bar; a restored tab counts a role not running as ended", () => {
  const t = { agents: two("ended", "running"), exited: false };
  const s = countdownStep(t, cdOn({ closeWhenAgentsExit: false }));
  assert.equal(s.view.kind, "bar");
  assert.equal(s.fire, false);
  // Restored: the engineer was not running at the first read — "unknown" to the core.
  const r = { agents: two("unknown", "running"), endedAtRestore: ["engineer"], exited: false };
  assert.equal(countdownStep(r, cdOn()).view.kind, "countdown");
  assert.deepEqual(counted(two("unknown", "running"), ["engineer"]).map((a) => a.state), ["ended", "running"]);
  assert.deepEqual(counted(two("running", "running"), ["engineer"]).map((a) => a.state), ["running", "running"], "running again: running");
  // Nothing ended clears the mark; an exited or closing tab has no countdown.
  const q = { agents: two("running", "running"), exited: false, marked: true };
  countdownStep(q, cdOn());
  assert.equal(q.marked, false);
  assert.equal(countdownStep({ ...t, exited: true }, cdOn()).view.kind, "none");
});
