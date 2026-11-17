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
  PANEL_DEFAULT,
  PANEL_IDS,
  POLL_MS,
  age,
  checkDescriptor,
  clampWidth,
  enabledPanels,
  handoffSeconds,
  maxWidth,
  overlays,
  panelDigit,
  shouldRead,
  summaryModel,
  taskGroups,
  timelineModel,
  updatedAgo,
} from "../src/panels.ts";
import { DEFAULT_MODAL, DEFAULT_SIMPLE, expanded } from "../src/keymap.ts";
import { KNOWN_GAP, badgeText, bellClears, bellMarks, byProject, goToTab } from "../src/attention.ts";
import { describe as describeAction, menuEntries } from "../src/menu.ts";
import { TITLE_MAX, blocks, filterTasks, firstLines, inline, readerKey, step, taskTitle } from "../src/panel-text.ts";

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
  // E-74 (status.md §7): 320 px to the smaller of 720 px and half the window; 400 by default.
  assert.equal(PANEL_DEFAULT, 400);
  assert.equal(clampWidth(100), 320);
  assert.equal(clampWidth(999), 720);
  assert.equal(clampWidth(400.4), 400);
  assert.equal(maxWidth(1280), 640);
  assert.equal(maxWidth(2000), 720);
  assert.equal(maxWidth(500), 320, "never below the minimum");
  assert.equal(clampWidth(700, maxWidth(1280)), 640);
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

// ── Needs you (E-61; triaged in the core by E-75 — the rules' table test is attention.rs) ──

const item = (over) => ({ key: "k", kind: "bell", index: 0, tab: 10, text: "t", quote: null, list: [], dismissable: false, ...over });

test("Needs you groups the core's items by project in first-seen order; the badge counts what it is given", () => {
  const titles = ["alpha", "beta"];
  const groups = byProject(
    [item({ index: null, key: "health", kind: "health_fail" }), item({ index: 1, key: "bell" }), item({ index: 0, key: "ended" }), item({ index: 1, key: "mention:0" })],
    (i) => titles[i],
  );
  assert.deepEqual(groups.map((g) => [g.project, g.index, g.items.map((x) => x.key)]), [
    ["Every project", null, ["health"]],
    ["beta", 1, ["bell", "mention:0"]],
    ["alpha", 0, ["ended"]],
  ]);
  assert.deepEqual(byProject([], () => "x"), []);
  assert.equal(badgeText(3), "3");
  assert.equal(badgeText(200), "99+");
  // The web side sends no title, path or sentence to the core: only booleans, ids and a count.
  const ipc = readFileSync(join(ui, "src/ipc.ts"), "utf8");
  assert.match(ipc, /invoke<import\("\.\/attention\.ts"\)\.Lists>\("attention_items", \{ tabs, health \}\)/);
  const side = readFileSync(join(ui, "src/side-menu.tsx"), "utf8");
  const sent = /const tabStates = \(\): TabState\[\] =>[\s\S]*?\];\n/.exec(side)?.[0] ?? "";
  assert.ok(sent.includes("index,") && !/title|project|path/.test(sent), "tab state holds no title or path");
});

test("the bell marks only a tab out of sight and clears when it is in front of the user", () => {
  assert.equal(bellMarks(false, true), true, "another tab");
  assert.equal(bellMarks(true, false), true, "this tab, window not focused");
  assert.equal(bellMarks(true, true), false, "the tab the user is looking at");
  assert.equal(bellClears(true, true), true);
  assert.equal(bellClears(true, false), false);
  assert.equal(bellClears(false, true), false);
});

test("the panel's actions only switch tabs or dismiss; nothing in it runs or types", () => {
  const calls = [];
  goToTab((c) => calls.push(c))(2);
  assert.deepEqual(calls, [{ name: "tab.goto", arg: "3" }]);
  // The view reaches no IPC: the side menu asks for the lists and sends a dismiss (E-75).
  for (const f of ["src/attention.ts", "src/panel-views.tsx"]) {
    const src = readFileSync(join(ui, f), "utf8");
    assert.doesNotMatch(src, /from "\.\/ipc\.ts"/, `${f} imports no IPC`);
    assert.doesNotMatch(src, /\binvoke\(|\.write\(|\.paste\(|sendBytes/, `${f} writes nothing`);
  }
  const side = readFileSync(join(ui, "src/side-menu.tsx"), "utf8");
  assert.deepEqual(
    [...side.matchAll(/\b(panelData|attention[A-Z]\w*|[a-z][A-Za-z]*(?:Write|Close|Apply|Send))(?:<[^>(]*>)?\(/g)].map((m) => m[1]),
    ["attentionItems", "attentionDismiss"],
    "the side menu asks for the lists and dismisses, nothing else",
  );
});

test("the Needs you panel: Now grouped by project with one Go to tab, information collapsed, Dismiss, the empty state", () => {
  const html = rendered.attention();
  assert.equal((html.match(/>Go to tab</g) ?? []).length, 2, "one per project; health has none");
  assert.ok(html.includes(">alpha<") && html.includes(">beta<") && html.includes(">Every project<"));
  assert.ok(html.includes("a mention, not a fact") && html.includes("<blockquote"));
  assert.equal((html.match(/>Dismiss</g) ?? []).length, 2, "the bell and the mention");
  assert.ok(html.includes("For information (2)"));
  assert.ok(!html.includes("12 tasks are blocked"), "information starts collapsed");
  assert.match(html, /aria-expanded="false"[^>]*>.*For information/s);
  assert.ok(html.includes("About this list"));
  const info = rendered.attentionInfoOnly();
  assert.ok(info.includes("Nothing needs you.") && info.includes("For information (1)"));
  const empty = rendered.attentionEmpty();
  assert.ok(empty.includes("Nothing needs you.") && !empty.includes("For information"));
  assert.match(KNOWN_GAP, /permission prompt/);
  assert.match(KNOWN_GAP, /open or closed/, "D-061 addendum 1: the gap holds while the app is closed too");
  assert.equal(KNOWN_GAP.split(/\.\s/).length, 2, "two sentences");
  assert.match(rendered.attentionLoading(), /data-slot="skeleton"/);
});


// ── Readable panels (E-74, status.md §Components 7, D-063) ──────────────────────────────

test("structured text: each construct, and every marker left unclosed stays literal", () => {
  assert.deepEqual(inline("a `b` **c** d"), [
    { kind: "text", text: "a " },
    { kind: "code", text: "b" },
    { kind: "text", text: " " },
    { kind: "bold", text: "c" },
    { kind: "text", text: " d" },
  ]);
  assert.deepEqual(inline("an ` alone and ** alone"), [{ kind: "text", text: "an ` alone and ** alone" }]);
  assert.deepEqual(inline("empty `` and ****"), [{ kind: "text", text: "empty `` and ****" }]);
  assert.deepEqual(inline("[x](https://e.com) <b>"), [{ kind: "text", text: "[x](https://e.com) <b>" }], "a link and a tag are text");
  const b = blocks("# Head\n\nOne\ntwo\n\n- a\n  more\n- b\n\n2. x\n3. y\n\n```\ncode\n  kept\n```\n```\nloose");
  assert.deepEqual(b.map((x) => x.kind), ["heading", "p", "ul", "ol", "pre", "p"]);
  assert.deepEqual(b[1].inl, [{ kind: "text", text: "One two" }], "lines of a paragraph join");
  assert.deepEqual(b[2].items.map((i) => i[0].text), ["a more", "b"], "an indented line continues its item");
  assert.equal(b[3].start, 2);
  assert.equal(b[4].text, "code\n  kept");
  assert.deepEqual(b[5].inl, [{ kind: "text", text: "``` loose" }], "an unclosed fence is a line of text");
  // Linear: a capped-size input of unclosed markers parses at once.
  const t0 = performance.now();
  inline("`**".repeat(40000));
  blocks("```\n".repeat(20000).slice(0, -1) + "x");
  assert.ok(performance.now() - t0 < 1000, "no quadratic pass");
});

test("first lines of Next action needed, and the task title rule", () => {
  assert.deepEqual(firstLines("1\n2\n3\n\n", 6), { text: "1\n2\n3", more: false });
  assert.deepEqual(firstLines("1\n2\n3\n4", 2), { text: "1\n2", more: true });
  assert.equal(taskTitle("Fix the bar. Then the rest."), "Fix the bar.");
  assert.equal(taskTitle("First line\nsecond"), "First line");
  assert.equal(taskTitle("per .ai/blueprints/status.md §7 (D-063). Operator"), "per .ai/blueprints/status.md §7 (D-063).", "a dot inside a path is not an end");
  const long = "Make sessions on the app's own tmux server outlive the app and be attached to, not restarted, per the blueprint.";
  const t = taskTitle(long);
  assert.ok(t.length <= TITLE_MAX && t.endsWith("…"), t);
  assert.ok(long.startsWith(t.slice(0, -1)), "cut at a word, from the start");
  assert.ok(!/\s…$/.test(t) && !t.slice(0, -1).endsWith(","), t);
  assert.equal(taskTitle(""), "");
});

test("the Tasks filter matches id or text, case ignored, and keeps every row when empty", () => {
  const rows = [
    { id: "E-74", description: "Readable panels" },
    { id: "E-75", description: "Needs you triage" },
  ];
  assert.deepEqual(filterTasks(rows, "  ").map((r) => r.id), ["E-74", "E-75"]);
  assert.deepEqual(filterTasks(rows, "e-75").map((r) => r.id), ["E-75"]);
  assert.deepEqual(filterTasks(rows, "READABLE").map((r) => r.id), ["E-74"]);
  assert.deepEqual(filterTasks(rows, "nothing"), []);
});

test("the reader's keys: Up / Down always, k / j only in the modal style, Esc closes; stepping stops at the ends", () => {
  assert.equal(readerKey("Escape", false), "close");
  assert.equal(readerKey("ArrowDown", false), "next");
  assert.equal(readerKey("ArrowUp", true), "prev");
  assert.equal(readerKey("j", true), "next");
  assert.equal(readerKey("k", true), "prev");
  assert.equal(readerKey("j", false), null, "the simple style types nothing into the reader with j");
  assert.equal(readerKey("Enter", true), null);
  assert.equal(step(0, 3, -1), 0);
  assert.equal(step(2, 3, 1), 2);
  assert.equal(step(1, 3, 1), 2);
  assert.equal(step(0, 0, 1), 0);
});

const tagsIn = (html) => [...new Set([...html.matchAll(/<([a-zA-Z][a-zA-Z0-9]*)/g)].map((m) => m[1].toLowerCase()))].sort();

test("structured text renders only its closed set of elements: no link, image, src, href or event", () => {
  const { STRUCTURED_TAGS } = rendered;
  const html = rendered.structured();
  for (const t of tagsIn(html)) assert.ok(STRUCTURED_TAGS.includes(t), `<${t}> is not allowed`);
  assert.doesNotMatch(html, /<[^>]*\s(href|src|on[a-z]+)=/i, "an attribute inside a real tag");
  for (const t of ["h4", "p", "ul", "ol", "li", "pre", "code", "strong"]) assert.ok(tagsIn(html).includes(t), `${t} is drawn`);
  assert.ok(html.includes("&lt;script&gt;alert(1)&lt;/script&gt;"));
  assert.ok(html.includes("[the blueprint](https://example.com/x)"));
  assert.ok(html.includes("![img](https://example.com/i.png)"), "a Markdown image stays text");
  assert.ok(html.includes("an unclosed ` and ** marker"));
  assert.match(html, /<ol[^>]*start="3"/);
});

test("the reader: id, title and chips for a task; who → who, trigger and every section for a handoff; its own controls", () => {
  const { STRUCTURED_TAGS } = rendered;
  const task = rendered.readerTask();
  assert.ok(task.includes("E-74") && task.includes("Make the side panels readable, per status.md §Components 7 (D-063)."));
  assert.ok(task.includes("blocked by E-69") && task.includes("T2") && task.includes("engineer"));
  assert.match(task, /role="dialog"/);
  for (const name of ["Previous", "Next", "Close the reader"]) assert.ok(task.includes(`aria-label="${name}"`), `${name}: a pointer control`);
  assert.ok(task.includes("Task 1 of 1"));
  const ho = rendered.readerHandoff();
  assert.ok(ho.includes("Architect → Engineer") && ho.includes("Handoff 2 of 2"));
  assert.ok(ho.includes("The rest of this handoff is left out."));
  const allowed = new Set([...STRUCTURED_TAGS, "section", "header", "button", "svg", "path", "circle", "article", "h3", "h4", "span"]);
  for (const html of [task, ho]) {
    for (const t of tagsIn(html)) assert.ok(allowed.has(t), `<${t}> in the reader`);
    assert.doesNotMatch(html, /<a[\s>]|<img|<[^>]*\s(href|src)=/i);
    assert.ok(html.includes("&lt;script&gt;alert(1)&lt;/script&gt;"));
  }
  // The sheet lies over the stage beside the panel: nothing it does resizes the terminal.
  const src = readFileSync(join(ui, "src/panel-reader.tsx"), "utf8");
  assert.doesNotMatch(src, /\bfit\(|ptyResize|resize\(/);
  assert.doesNotMatch(src, /from "\.\/ipc\.ts"/, "the reader makes no IPC call");
});

test("Communication shows the trigger, six lines of Next action needed and Read all; Tasks a filter, titles and chips", () => {
  const c = rendered.communication();
  assert.ok(c.includes(">Read all<"));
  assert.ok(c.includes("Next action needed"));
  assert.ok(c.includes("six (left out)") === false && c.includes(">five<"), "six lines, the seventh left out");
  assert.ok(c.includes("…"));
  const t = rendered.tasks();
  assert.ok(t.includes('aria-label="Filter tasks by id or text"'));
  assert.ok(t.includes('data-reader-key="task:E-1"'));
  assert.ok(t.includes("T2") && t.includes("engineer"), "chips");
  assert.ok(rendered.readerTask().includes(">T2<"), "a tier that is not a bare number is shown as written");
  const NOW = Date.UTC(2026, 10, 16, 12, 0);
  assert.deepEqual(
    rendered.taskChips({ id: "E-1", status: "OPEN", owner: "Engineer (Claude)", tier: "2", description: "", blocked_by: ["E-69"], updated_at: "2026-11-16T11:40:00Z" }, NOW),
    ["Tier 2", "Engineer (Claude)", "blocked by E-69", "20 min ago"],
  );
  assert.deepEqual(rendered.taskChips({ id: "E-1", status: "", owner: "", tier: "", description: "", blocked_by: [], updated_at: "not a date" }, NOW), [], "no chip for what is not known");
  assert.match(t, /aria-expanded="true"[^>]*>.*Open/s);
});
