// Home (status.md §Home H1–H5, D-064; E-80): the card order and the rule that a card never
// moves under the pointer or keyboard focus, the summary line with zero parts left out, the
// card model (never a guess), tab.next / tab.prev through Home, and the start-up rule. Extends
// status.test.mjs (the marks and sentences the cards reuse).
import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtempSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { build } from "esbuild";
import { ACTIVE_LABEL, ACTIVE_MAX, QUOTE_LINES, STATE_ORDER, cardModel, cardOrder, cycle, heldOrder, homeAtStart, nowRow, summaryLine, tasksCalls } from "../src/home.ts";
import { MARKS } from "../src/status.ts";

const st = (state, over = {}) => ({ tab: 4, state, quiet_s: 0, worked_s: 0, waiting: null, now: [], info: [], in_progress: null, turn: null, last_handoff_s: null, ...over });

test("cards are ordered by state, then by tab order", () => {
  assert.deepEqual([...STATE_ORDER].sort(), Object.keys(MARKS).sort(), "every state has a place");
  const cards = [
    { index: 0, state: "working" },
    { index: 1, state: "quiet" },
    { index: 2, state: "needs_you" },
    { index: 3, state: "unknown" },
    { index: 4, state: "ended" },
    { index: 5, state: "needs_you" },
    { index: 6, state: "setup" },
    { index: 7, state: "starting" },
  ];
  assert.deepEqual(cardOrder(cards), [2, 5, 4, 6, 1, 0, 7, 3]);
});

test("a held grid keeps its places; a closed card leaves, a new one goes last", () => {
  const before = ["a", "b", "c"];
  const after = ["c", "a", "d"]; // c moved up, b closed, d opened
  assert.deepEqual(heldOrder(before, after, false), after, "not held: the new order at once");
  assert.deepEqual(heldOrder(before, after, true), ["a", "c", "d"], "held: nothing moves under the pointer");
  assert.deepEqual(heldOrder([], after, true), after, "nothing drawn yet");
});

test("the summary line leaves out every part that is zero", () => {
  assert.equal(summaryLine([]), null);
  assert.equal(summaryLine(["working"]), "1 project open · 1 working");
  assert.equal(summaryLine(["setup", "unknown"]), "2 projects open");
  assert.equal(
    summaryLine(["working", "working", "needs_you", "quiet", "ended", "ended", "starting"]),
    "7 projects open · 2 working · 1 needs you · 1 quiet · 2 ended",
  );
  assert.equal(summaryLine(["needs_you", "needs_you"]), "2 projects open · 2 need you");
});

test("a card shows what the core sent and never guesses", () => {
  const full = cardModel(
    "rein-app",
    st("quiet", {
      quiet_s: 130,
      in_progress: { id: "E-80", title: "Build Home." },
      turn: "Engineer",
      last_handoff_s: 7200,
      counts: { open: 5, in_progress: 1, blocked: 4 },
      trigger: "Review of\nyour handoff " + "x".repeat(200),
      agents: [
        { role: "architect", state: "ended" },
        { role: "engineer", state: "running" },
        { role: "tester", state: "unknown" },
      ],
      now: [{}, {}],
    }),
  );
  assert.equal(full.sentence, "Quiet for 2 min.");
  assert.equal(full.task, "In progress: E-80 — Build Home.");
  assert.match(full.turn, /^Engineer's turn · last handoff /);
  assert.equal(full.trigger.length, 120, "cut at 120");
  assert.ok(!full.trigger.includes("\n"), "one line");
  assert.deepEqual(full.agents, ["architect: ended", "engineer: running"]);
  assert.deepEqual(full.chips, { open: 5, in_progress: 1, blocked: 4 });
  assert.equal(full.now, 2);

  const idle = cardModel("p", st("working", { counts: { open: 0, in_progress: 0, blocked: 0 } }));
  assert.equal(idle.task, "No task in progress");
  const odd = cardModel("p", st("working", { counts: { open: 1, in_progress: 1, blocked: 0 }, in_progress: { id: "<b>x</b>", title: "t" } }));
  assert.equal(odd.task, "No task in progress", "an id not in the task-id shape is not shown");

  // Files not readable (or team mode): the state, and "Tasks not available" (chips null).
  const none = cardModel("p", st("ended", { counts: null }));
  assert.deepEqual([none.sentence, none.task, none.chips, none.turn, none.trigger], ["Ended.", null, null, null, null]);
  const unsent = cardModel("p", undefined);
  assert.deepEqual([unsent.state, unsent.task, unsent.chips], ["unknown", null, null], "nothing sent yet");
});

test("tab.next and tab.prev cycle through Home and the tabs", () => {
  // Home is -1.
  assert.equal(cycle(-1, 3, true), 0);
  assert.equal(cycle(2, 3, true), -1, "after the last tab comes Home");
  assert.equal(cycle(-1, 3, false), 2, "before Home is the last tab");
  assert.equal(cycle(0, 3, false), -1);
  assert.equal(cycle(-1, 0, true), -1, "no tab: Home stays");
});

test("start-up: Home unless a project was given, or last restored one", () => {
  assert.equal(homeAtStart(false, false, 3), true, "dashboard (the default) opens Home");
  assert.equal(homeAtStart(false, true, 3), false, "last: the restored tab");
  assert.equal(homeAtStart(false, true, 0), true, "nothing to restore: Home either way");
  assert.equal(homeAtStart(false, false, 0), true);
  assert.equal(homeAtStart(true, false, 2), false, "a project on the command line is shown");
});

test("the Home button is first, not closable and not numbered", async () => {
  const { titleBar } = await import("../src/chrome.ts");
  const tabs = [
    { id: 7, title: "a", current: false, exited: false },
    { id: 8, title: "b", current: false, exited: false },
  ];
  const bar = titleBar(tabs, "0.5.0", false, { current: true, count: 3 });
  assert.deepEqual(bar.home, { current: true, count: 3, select: { name: "home.show" } });
  assert.equal("close" in bar.home, false, "no close");
  assert.deepEqual(bar.tabs[0].select, { name: "tab.goto", arg: "1" }, "tab.goto 1 is still the first project");
  assert.deepEqual(titleBar([], "0.5.0").home.current, true, "no project open: Home");
});

test("home.show has a key in both styles and a menu row (pointer and key parity)", async () => {
  const { DEFAULT_MODAL, DEFAULT_SIMPLE, expanded } = await import("../src/keymap.ts");
  const { menuEntries } = await import("../src/menu.ts");
  const bound = (m) => [...m].filter(([, c]) => c.name === "home.show").map(([k]) => k);
  assert.deepEqual(bound(DEFAULT_SIMPLE.shortcuts), ["C-S-h"]);
  assert.deepEqual(bound(expanded(DEFAULT_MODAL)), ["Space H"], "Space h is health.show");
  const rows = menuEntries(DEFAULT_SIMPLE, DEFAULT_MODAL, { tabs: 2 }).filter((e) => e.call.name === "home.show");
  assert.equal(rows.length, 1);
  assert.equal(rows[0].label, "Home");
  assert.match(rows[0].shortcut, /Ctrl\+Shift\+H/i);
});

test("on Home only Needs you has something to show (H4)", async () => {
  const { HOME_PANELS, HOME_PANEL_TIP } = await import("../src/home.ts");
  const { PANEL_IDS } = await import("../src/panels.ts");
  assert.deepEqual(HOME_PANELS, ["attention"]);
  assert.ok(HOME_PANELS.every((p) => PANEL_IDS.includes(p)));
  assert.equal(HOME_PANEL_TIP, "Open a project to see this");
});

test("a Needs you now row: name and sentence, the quote apart, the whole row as its tooltip (R3)", () => {
  const quote = "Operator: " + "merge #61–#64 after #60 ".repeat(12).slice(0, 290);
  const r = nowRow("rein-app", { text: "The last handoff asks the operator:", quote });
  assert.equal(r.name, "rein-app");
  assert.equal(r.sentence, "The last handoff asks the operator:");
  assert.equal(r.quote, `“${quote}”`, "the quote is whole; the row cuts it to QUOTE_LINES by style");
  assert.equal(r.tip, `rein-app: The last handoff asks the operator: “${quote}”`);
  assert.equal(QUOTE_LINES, 3);
  const bell = nowRow("Installation", { text: "The terminal rang its bell.", quote: null });
  assert.deepEqual(bell, { name: "Installation", sentence: "The terminal rang its bell.", quote: null, tip: "Installation: The terminal rang its bell." });
  assert.equal(nowRow("p", { text: "t", quote: "  " }).quote, null, "a blank quote is no quote");
});

test("a card lists the active tasks under the task line, the in-progress one not repeated (R4)", () => {
  const active = [
    { id: "E-80", title: "Build Home.", status: "in_progress" },
    { id: "E-79", title: "System\nnotifications. " + "x".repeat(200), status: "open" },
    { id: "E-81", title: "Read rein's waiting signal.", status: "blocked" },
  ];
  const m = cardModel("p", st("working", { in_progress: { id: "E-80", title: "Build Home." }, counts: { open: 1, in_progress: 1, blocked: 1 }, active }));
  assert.equal(m.task, "In progress: E-80 — Build Home.");
  assert.deepEqual(m.active.map((a) => [a.id, a.status, a.label]), [["E-79", "open", "Open"], ["E-81", "blocked", "Blocked"]]);
  assert.ok(m.active[0].title.length <= 120 && !m.active[0].title.includes("\n"), "one line, 120 at most");
  // No task in progress: the in-progress one of `active` is listed; never more than three.
  const m2 = cardModel("p", st("working", { counts: { open: 3, in_progress: 1, blocked: 0 }, active: [...active, { id: "E-82", title: "four", status: "open" }] }));
  assert.equal(m2.task, "No task in progress");
  assert.deepEqual(m2.active.map((a) => a.id), ["E-80", "E-79", "E-81"]);
  assert.equal(ACTIVE_MAX, 3);
  assert.deepEqual(Object.keys(ACTIVE_LABEL).sort(), ["blocked", "in_progress", "open"]);
  // Not in the task id shape: left out. Absent (an older core) or not read: none.
  assert.deepEqual(cardModel("p", st("working", { counts: { open: 1, in_progress: 0, blocked: 0 }, active: [{ id: "<x>", title: "t", status: "open" }] })).active, []);
  assert.deepEqual(cardModel("p", st("working")).active, []);
  assert.deepEqual(cardModel("p", undefined).active, []);
});

test("the Tasks button goes to the tab and opens the Tasks panel through the existing actions (R5)", () => {
  // Pointer and keyboard both press the same native button: one onClick, these calls.
  assert.deepEqual(tasksCalls(2, null), [{ name: "tab.goto", arg: "3" }, { name: "panel.toggle", arg: "tasks" }]);
  assert.deepEqual(tasksCalls(0, "dashboard"), [{ name: "tab.goto", arg: "1" }, { name: "panel.toggle", arg: "tasks" }], "another panel: Tasks replaces it");
  assert.deepEqual(tasksCalls(15, "tasks"), [{ name: "tab.goto", arg: "16" }], "Tasks already open: the toggle would close it");
});

const homeRender = await (async () => {
  const ui = fileURLToPath(new URL("..", import.meta.url));
  const out = join(mkdtempSync(join(tmpdir(), "rein-home-")), "home.mjs");
  await build({
    entryPoints: [join(ui, "test/fixtures/home-render.tsx")],
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

test("a card's tab stops: the card, then Tasks — both native buttons; its tasks are text (R4, R5)", () => {
  const html = homeRender.card();
  const from = html.indexOf("data-state=");
  const card = html.slice(from, html.indexOf("</section>", from)); // the card, to the end of "Open projects"
  const buttons = [...card.matchAll(/<button[^>]*>/g)].map((m) => m[0]);
  assert.equal(buttons.length, 2, `two tab stops on the card: ${buttons.map((b) => b.slice(0, 80)).join(" | ")}`);
  assert.match(buttons[0], /data-card-go/);
  assert.match(buttons[0], /aria-label="rein-app: .* — go to its tab"/);
  assert.match(buttons[1], /data-card-tasks/);
  assert.match(buttons[1], /type="button"/);
  assert.match(buttons[1], /aria-label="Tasks of rein-app"/);
  assert.ok(card.indexOf("data-card-active") > card.indexOf("In progress: E-80"), "under the task line");
  assert.ok(card.includes("E-79") && card.includes("E-81"), "the open and blocked tasks");
  assert.equal(card.split("E-80").length - 1, 1, "the in-progress task once");
  assert.match(card, /aria-label="Open"/);
  assert.match(card, /aria-label="Blocked"/);
  assert.doesNotMatch(card, /<b>System/, "a title is text");
  assert.match(card, /&lt;b&gt;System notifications\.&lt;\/b&gt;/);
});
