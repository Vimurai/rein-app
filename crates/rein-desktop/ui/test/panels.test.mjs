// panels.md (D-056; E-60): the descriptor schema and registry, the panel keys, the view
// models, and the views' markup. E-61 extends it with "Needs you": the attention rules as a
// pure function, the bell mark, the badge, and the one action the panel has. New suite: the side menu is new; the text-only rule is the
// one test/components.test.mjs states for the chrome, here for text from a project's files.
import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtempSync, readFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { build } from "esbuild";
import {
  DESCRIPTORS,
  OVERLAY_BELOW,
  PANEL_IDS,
  POLL_MS,
  age,
  checkDescriptor,
  clampWidth,
  enabledPanels,
  handoffSeconds,
  overlays,
  panelDigit,
  shouldRead,
  summaryModel,
  taskGroups,
  timelineModel,
  updatedAgo,
} from "../src/panels.ts";
import { DEFAULT_MODAL, DEFAULT_SIMPLE, expanded } from "../src/keymap.ts";
import { KNOWN_GAP, MAX_ITEMS, QUIET_S, attentionItems, badgeText, bellClears, bellMarks, goToTab, span } from "../src/attention.ts";
import { describe as describeAction, menuEntries } from "../src/menu.ts";

const ui = fileURLToPath(new URL("..", import.meta.url));

test("the compiled-in descriptors pass the closed schema; anything unknown is refused", () => {
  for (const d of DESCRIPTORS) assert.ok("ok" in checkDescriptor(d), d.id);
  const good = DESCRIPTORS[0];
  const rows = [
    ["an unknown source", { ...good, source: "shell" }, /source/],
    ["an unknown view", { ...good, view: "html" }, /view/],
    ["an unknown icon", { ...good, icon: "skull" }, /icon/],
    ["an extra key", { ...good, url: "https://example.com" }, /keys/],
    ["a missing key", (({ options, ...rest }) => rest)(good), /keys/],
    ["schema 2", { ...good, schema: 2 }, /schema/],
    ["an id with a capital", { ...good, id: "Dash" }, /id/],
    ["a long title", { ...good, title: "x".repeat(41) }, /title/],
    ["summary with options", { ...good, options: { limit: 5 } }, /no options/],
    ["a list with an unknown group", { ...DESCRIPTORS[2], options: { groups: ["ARCHIVED"], limit: 10, done: 10 } }, /group/],
    ["a list limit over 200", { ...DESCRIPTORS[2], options: { groups: ["OPEN"], limit: 201, done: 10 } }, /1–200/],
    ["a timeline pin it does not know", { ...DESCRIPTORS[1], options: { pin: "everyone" } }, /pin/],
    ["attention drawn by another view", { ...DESCRIPTORS[3], view: "summary", options: {} }, /list view/],
    ["attention with task groups", { ...DESCRIPTORS[3], options: { groups: ["OPEN"], limit: 10, done: 10 } }, /\{limit\}/],
    ["attention limit over 200", { ...DESCRIPTORS[3], options: { limit: 500 } }, /1–200/],
    ["not an object", "dashboard", /object/],
  ];
  for (const [label, raw, why] of rows) {
    const r = checkDescriptor(raw);
    assert.ok("refused" in r, label);
    assert.match(r.refused, why, label);
  }
});

test("the panel ids match the core's list, and each has a digit and a key in both styles", () => {
  // config.rs PANEL_IDS is the core's list: the web side's must be the same, in the same order.
  const rs = readFileSync(join(ui, "../src/config.rs"), "utf8");
  const core = /pub const PANEL_IDS: &\[&str\] = &\[([^\]]*)\]/.exec(rs)?.[1].match(/"([a-z-]+)"/g)?.map((s) => s.slice(1, -1));
  assert.deepEqual(core, [...PANEL_IDS]);
  assert.deepEqual(DESCRIPTORS.map((d) => d.id), [...PANEL_IDS]);
  for (const id of PANEL_IDS) {
    const n = panelDigit(id);
    assert.equal(DEFAULT_SIMPLE.shortcuts.get(`C-S-${n}`)?.arg, id, `simple Ctrl+Shift+${n}`);
    assert.equal(expanded(DEFAULT_MODAL).get(`Space P ${n}`)?.arg, id, `modal Space P ${n}`);
  }
  for (const name of ["panel.close", "panel.refresh", "panel.focus"]) {
    assert.ok([...DEFAULT_SIMPLE.shortcuts.values()].some((c) => c.name === name), `simple key for ${name}`);
    assert.ok([...expanded(DEFAULT_MODAL).values()].some((c) => c.name === name), `modal key for ${name}`);
  }
  assert.equal(describeAction({ name: "panel.toggle", arg: "tasks" }, DEFAULT_SIMPLE, DEFAULT_MODAL), "Tasks panel  (Ctrl+Shift+3)");
  assert.equal(describeAction({ name: "panel.toggle", arg: "attention" }, DEFAULT_SIMPLE, DEFAULT_MODAL), "Needs you panel  (Ctrl+Shift+4)");
});

test("the menu offers one row per enabled panel, in rail order", () => {
  const rows = (panels) =>
    menuEntries(DEFAULT_SIMPLE, DEFAULT_MODAL, { tabs: 1, panels })
      .filter((e) => e.call.name === "panel.toggle")
      .map((e) => [e.call.arg, e.label]);
  // Config order does not reorder: the rail's order (PANEL_IDS) wins.
  assert.deepEqual(rows(["tasks", "dashboard"]), [
    ["dashboard", "Panel: Dashboard"],
    ["tasks", "Panel: Tasks"],
  ]);
  assert.deepEqual(rows([]), []);
  assert.deepEqual(enabledPanels(["tasks", "dashboard"]).map((d) => d.id), ["dashboard", "tasks"]);
  assert.deepEqual(enabledPanels(["shell"]), []);
});

test("the host's rules: width, overlay, reading only while open and visible", () => {
  assert.equal(clampWidth(100), 320);
  assert.equal(clampWidth(999), 560);
  assert.equal(clampWidth(400.4), 400);
  assert.equal(OVERLAY_BELOW, 900);
  assert.equal(overlays(899), true);
  assert.equal(overlays(900), false);
  assert.equal(shouldRead(true, true), true);
  assert.equal(shouldRead(true, false), false, "a hidden window reads nothing");
  assert.equal(shouldRead(false, true), false);
  assert.ok(POLL_MS < 6000, "a change shows within 6 s");
});

test("ages and handoff timestamps; an odd timestamp is shown as written", () => {
  assert.equal(age(5), "5 s ago");
  assert.equal(age(125), "2 min ago");
  assert.equal(age(7300), "2 h ago");
  assert.equal(age(200000), "2 d ago");
  assert.equal(handoffSeconds("2026-11-16 11:45 UTC"), Date.UTC(2026, 10, 16, 11, 45) / 1000);
  assert.equal(handoffSeconds("yesterday"), null);
  assert.equal(handoffSeconds("2026-11-16 11:45"), null);
  assert.equal(updatedAgo(1000, 1012000), "updated 12 s ago");
});

const task = (id, status, d = "") => ({ id, status, owner: "engineer", tier: "T2", description: d, blocked_by: [], updated_at: "" });

test("the Tasks list: the descriptor's groups, newest first, Done capped, other statuses kept", () => {
  const done = Array.from({ length: 14 }, (_, i) => task(`E-${i + 1}`, "DONE"));
  const t = { cut: false, focus: "", tasks: [...done, task("E-20", "OPEN"), task("E-21", "IN_PROGRESS"), task("E-22", "WAITING"), task("E-23", "OPEN")] };
  const g = taskGroups(t, DESCRIPTORS[2].options);
  assert.deepEqual(g.map((x) => [x.key, x.rows.map((r) => r.id), x.more]), [
    ["IN_PROGRESS", ["E-21"], 0],
    ["OPEN", ["E-23", "E-20"], 0],
    ["BLOCKED", [], 0],
    ["DONE", ["E-14", "E-13", "E-12", "E-11", "E-10", "E-9", "E-8", "E-7", "E-6", "E-5"], 4],
    ["OTHER", ["E-22"], 0],
  ]);
});

test("the timeline is newest first and whose turn is the last handoff's to", () => {
  const h = (to, ts) => ({ timestamp: ts, from: "A", to, trigger: "t", sections: [], cut: false });
  const m = timelineModel({ cut: false, handoffs: [h("Engineer", "2026-11-16 09:00 UTC"), h("Architect", "2026-11-16 10:00 UTC")] }, Date.UTC(2026, 10, 16, 12, 0));
  assert.equal(m.whoseTurn, "Architect");
  assert.deepEqual(m.entries.map((e) => [e.title, e.when]), [
    ["A → Architect", "2026-11-16 10:00 UTC · 2 h ago"],
    ["A → Engineer", "2026-11-16 09:00 UTC · 3 h ago"],
  ]);
  assert.equal(timelineModel({ cut: false, handoffs: [] }, 0).whoseTurn, null);
  const s = summaryModel({ counts: [{ status: "OPEN", n: 2 }, { status: "WAITING", n: 1 }], in_progress: null, last_handoff: null, whose_turn: "", focus: "" }, { health: "OK", server: "x" }, 0);
  assert.deepEqual(s.chips, [{ label: "Open", n: 2 }, { label: "WAITING", n: 1 }]);
  assert.equal(s.rows.find((r) => r.label === "In progress")?.value, "—");
});

const rendered = await (async () => {
  const out = join(mkdtempSync(join(tmpdir(), "rein-panels-")), "render.mjs");
  await build({
    entryPoints: [join(ui, "test/fixtures/panels-render.tsx")],
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

test("text from a project's files is put on the page as text: no tag, no link, no image", () => {
  for (const name of ["communication", "tasks", "dashboard", "attention"]) {
    const html = rendered[name]();
    assert.doesNotMatch(html, /<script/i, `${name}: a script element`);
    assert.doesNotMatch(html, /<a[\s>]/i, `${name}: a link`);
    assert.doesNotMatch(html, /<img/i, `${name}: an image`);
    // An event handler renders as `onerror="…"`; the text (escaped, also in an aria-label) has none.
    assert.doesNotMatch(html, /\son[a-z]+="/i, `${name}: an event attribute`);
    assert.ok(html.includes("&lt;script&gt;alert(1)&lt;/script&gt;"), `${name}: the script tag shown as text`);
    assert.ok(html.includes("[the blueprint](https://example.com/x)"), `${name}: the Markdown link shown as written`);
  }
});

test("the not-available state and the first load", () => {
  const u = rendered.unavailable();
  assert.ok(u.includes("Not available") && u.includes("this project keeps tasks in a form this version does not read"));
  assert.match(u, /role="status"/);
  assert.match(rendered.loading(), /data-slot="skeleton"/, "a skeleton until the first answer");
});

// ── Needs you (E-61) ────────────────────────────────────────────────────────────────────

const NOW = Date.UTC(2026, 11 - 1, 17, 12, 0);
const noFacts = { tasks_read: true, blocked: [], in_progress: false, last_handoff: null, mentions: [] };
const tab = (index, over = {}) => ({ index, title: `p${index}`, exited: false, refused: false, bell: false, update: false, facts: noFacts, ...over });
const kinds = (tabs, health = "ok") => attentionItems(tabs, health, NOW).map((i) => `${i.project}:${i.kind}`);

test("each attention rule on and off", () => {
  assert.deepEqual(kinds([tab(0)]), [], "a quiet tab has nothing");
  assert.deepEqual(kinds([tab(0, { exited: true })]), ["p0:ended"]);
  assert.deepEqual(kinds([tab(0, { exited: true, refused: true })]), ["p0:refused"], "a refusal is one item, not also 'ended'");
  assert.deepEqual(kinds([tab(0, { update: true })]), ["p0:update"]);
  assert.deepEqual(kinds([tab(0, { bell: true })]), ["p0:bell"]);
  assert.deepEqual(kinds([tab(0)], "warn"), ["Every project:health"]);
  assert.deepEqual(kinds([tab(0)], "fail"), ["Every project:health"]);
  assert.deepEqual(kinds([tab(0)], "unknown"), []);
  const health = attentionItems([], "fail", NOW)[0];
  assert.equal(health.tab, null, "health is no one tab's: no go-to");
  const blocked = attentionItems([tab(0, { facts: { ...noFacts, blocked: [{ id: "E-2", description: "waits on rein" }, { id: "E-4", description: "" }] } })], "ok", NOW);
  assert.deepEqual(blocked.map((i) => [i.text, i.quote]), [["E-2 is blocked.", "waits on rein"], ["E-4 is blocked.", null]]);
  // No handoff for more than 30 minutes while a task is in progress.
  const at = (min) => new Date(NOW - min * 60000).toISOString().replace("T", " ").slice(0, 16) + " UTC";
  const quiet = (over) => kinds([tab(0, { facts: { ...noFacts, in_progress: true, last_handoff: at(31), ...over } })]);
  assert.deepEqual(quiet({}), ["p0:quiet"]);
  assert.match(attentionItems([tab(0, { facts: { ...noFacts, in_progress: true, last_handoff: at(31) } })], "ok", NOW)[0].text, /31 min/);
  assert.deepEqual(quiet({ last_handoff: at(29) }), [], "29 min is not yet");
  assert.deepEqual(quiet({ in_progress: false }), [], "nothing in progress");
  assert.deepEqual(quiet({ last_handoff: "yesterday" }), [], "an odd timestamp is not guessed at");
  assert.deepEqual(quiet({ last_handoff: null }), [], "no handoff at all: no time to measure");
  assert.ok(QUIET_S === 1800);
  assert.deepEqual([span(31 * 60), span(119 * 60), span(26 * 3600 + 40 * 60), span(3 * 86400)], ["31 min", "119 min", "26 h", "3 d"]);
  // The last handoff mentions the operator: a quote, labelled as a mention.
  const m = attentionItems([tab(0, { facts: { ...noFacts, mentions: ["- **Operator:** choose", "ask the operator"] } })], "ok", NOW);
  assert.deepEqual(m.map((i) => [i.kind, i.quote]), [["mention", "- **Operator:** choose"], ["mention", "ask the operator"]]);
  assert.match(m[0].text, /mentions the operator/);
  // Facts not read yet (or not available): only the app's own rules apply.
  assert.deepEqual(kinds([tab(0, { exited: true, facts: null })]), ["p0:ended"]);
});

test("attention across several tabs: tab order, each item names its project and its tab, capped", () => {
  const items = attentionItems(
    [tab(0, { bell: true }), tab(1), tab(2, { exited: true, facts: { ...noFacts, blocked: [{ id: "E-9", description: "x" }] } })],
    "warn",
    NOW,
  );
  assert.deepEqual(items.map((i) => [i.project, i.kind, i.tab]), [
    ["Every project", "health", null],
    ["p0", "bell", 0],
    ["p2", "ended", 2],
    ["p2", "blocked", 2],
  ]);
  assert.equal(new Set(items.map((i) => i.key)).size, items.length, "keys are unique");
  const many = Array.from({ length: 300 }, (_, i) => ({ id: `E-${i}`, description: "" }));
  assert.equal(attentionItems([tab(0, { facts: { ...noFacts, blocked: many } }), tab(1, { bell: true })], "fail", NOW).length, MAX_ITEMS);
  assert.equal(badgeText(3), "3");
  assert.equal(badgeText(200), "99+");
});

test("the bell marks only a tab out of sight and clears when it is in front of the user", () => {
  assert.equal(bellMarks(false, true), true, "another tab");
  assert.equal(bellMarks(true, false), true, "this tab, window not focused");
  assert.equal(bellMarks(true, true), false, "the tab the user is looking at");
  assert.equal(bellClears(true, true), true);
  assert.equal(bellClears(true, false), false);
  assert.equal(bellClears(false, true), false);
});

test("the panel's one action only switches tabs; nothing in it writes, runs or types", () => {
  const calls = [];
  goToTab((c) => calls.push(c))(2);
  assert.deepEqual(calls, [{ name: "tab.goto", arg: "3" }]);
  // The rules and the view reach no IPC at all: the side menu reads the facts, read-only.
  for (const f of ["src/attention.ts", "src/panel-views.tsx"]) {
    const src = readFileSync(join(ui, f), "utf8");
    assert.doesNotMatch(src, /from "\.\/ipc\.ts"/, `${f} imports no IPC`);
    assert.doesNotMatch(src, /\binvoke\(|\.write\(|\.paste\(|sendBytes/, `${f} writes nothing`);
  }
  const side = readFileSync(join(ui, "src/side-menu.tsx"), "utf8");
  assert.deepEqual([...side.matchAll(/\b(panelData|[a-z][A-Za-z]*(?:Write|Close|Apply|Send))(?:<[^>(]*>)?\(/g)].map((m) => m[1]), ["panelData"], "the side menu only reads");
});

test("the Needs you panel: items with their project, a quote labelled as a mention, the gap stated when empty", () => {
  const html = rendered.attention();
  assert.ok(html.includes("E-2 is blocked."));
  assert.equal((html.match(/>Go to tab</g) ?? []).length, 2, "health has no go-to");
  assert.ok(html.includes("a mention, not a fact"));
  assert.ok(html.includes("<blockquote"));
  const empty = rendered.attentionEmpty();
  assert.ok(empty.includes("Nothing needs you right now."));
  assert.ok(empty.includes(KNOWN_GAP.replace(/'/g, "&#x27;")));
  assert.match(KNOWN_GAP, /permission prompt/);
  assert.match(KNOWN_GAP, /while the app is closed/, "D-061 addendum 1: the gap holds while the app is closed too");
  assert.match(rendered.attentionLoading(), /data-slot="skeleton"/);
});
