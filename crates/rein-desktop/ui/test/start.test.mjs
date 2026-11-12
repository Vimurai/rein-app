// E-38 → E-50 (desktop.md §Look and feel v2 V2): the start screen — open and check buttons,
// recent projects 1–9 (missing ones marked) with when each was last opened, the WSL card, the
// five key hints — and its keys.
import { test } from "node:test";
import assert from "node:assert/strict";
import { DEFAULT_MODAL, DEFAULT_SIMPLE } from "../src/keymap.ts";
import { step } from "../src/modes.ts";
import { HINTS, ago, startScreen, withStartKeys } from "../src/start.ts";

const NOW = 1_800_000_000;
const r = (i, exists = true) => ({ path: `/home/me/p${i}`, name: `p${i}`, last_opened: NOW - i * 3600, exists });

test("open and check buttons, numbered recent projects, missing marked, five hints", () => {
  const s = startScreen([r(1), r(2, false), ...Array.from({ length: 12 }, (_, i) => r(i + 3))], null, NOW);
  assert.equal(s.recent.length, 9, "at most nine recent projects");
  assert.deepEqual(
    s.recent.slice(0, 2).map((x) => [x.n, x.name, x.path, x.ago, x.missing]),
    [
      [1, "p1", "/home/me/p1", "1 h ago", false],
      [2, "p2", "/home/me/p2", "2 h ago", true],
    ],
  );
  assert.equal(HINTS.length, 5);
  assert.equal(s.hints, HINTS);
  assert.equal(s.setup, null);
  assert.deepEqual(startScreen([], null, NOW).recent, []);
  // Windows without WSL: the reason, the command and "check again" (E-45).
  const blocked = startScreen([], { reason: "WSL is not installed", command: "wsl --install" }, NOW);
  assert.deepEqual(blocked.setup, { reason: "WSL is not installed", command: "wsl --install", again: { name: "health.show" } });
  // Newcomer path (D-034): opening a project and checking health are buttons as well as keys.
  const plain = startScreen([r(1)], null, NOW);
  assert.deepEqual(
    [plain.open, plain.check, ...plain.recent].map((x) => [x.label ?? x.name, x.action.name, x.action.arg ?? null]),
    [
      ["Open a project…", "project.open", null],
      ["Check installation", "health.show", null],
      ["p1", "project.open", "/home/me/p1"],
    ],
  );
});

test("last opened in a few plain words", () => {
  assert.equal(ago(NOW, NOW), "just now");
  assert.equal(ago(NOW + 30, NOW), "just now", "a clock that moved back is not the future");
  assert.equal(ago(NOW - 59, NOW), "just now");
  assert.equal(ago(NOW - 60, NOW), "1 min ago");
  assert.equal(ago(NOW - 3599, NOW), "59 min ago");
  assert.equal(ago(NOW - 3600, NOW), "1 h ago");
  assert.equal(ago(NOW - 86400, NOW), "yesterday");
  assert.equal(ago(NOW - 29 * 86400, NOW), "29 days ago");
  assert.equal(ago(0, NOW), "1970-01-01");
});

test("start-screen keys: p o h u q, digits open recent N", () => {
  const t = withStartKeys(DEFAULT_MODAL);
  const act = (k) => step(t, { mode: "NORMAL", pending: [] }, k).action;
  assert.deepEqual(act("p"), { name: "project.pick" });
  assert.deepEqual(act("o"), { name: "project.open" });
  assert.deepEqual(act("h"), { name: "health.show" });
  assert.deepEqual(act("u"), { name: "release.update" });
  assert.deepEqual(act("q"), { name: "app.quit" });
  assert.deepEqual(act("c"), { name: "health.show" });
  assert.deepEqual(act("3"), { name: "tab.goto", arg: "3" });
  assert.equal(DEFAULT_MODAL.normal.has("p"), false, "the project keymap is unchanged");
  // The simple style has no digit keys of its own; the start screen gives them.
  const simple = withStartKeys(DEFAULT_SIMPLE);
  assert.deepEqual(step(simple, { mode: "NORMAL", pending: [] }, "2").action, { name: "tab.goto", arg: "2" });
  assert.deepEqual(step(simple, { mode: "NORMAL", pending: [] }, "o").action, { name: "project.open" });
});

test("the picker's fuzzy filter keeps recency order", async () => {
  const { fuzzy } = await import("../src/start.ts");
  const items = [
    { path: "/home/me/Dev/energy-api", name: "energy-api", last_opened: 3, exists: true },
    { path: "/home/me/Dev/energy/docs", name: "docs", last_opened: 2, exists: true },
    { path: "/home/me/Dev/rein-app", name: "rein-app", last_opened: 1, exists: true },
  ];
  assert.deepEqual(fuzzy("ene", items).map((r) => r.name), ["energy-api", "docs"]);
  assert.deepEqual(fuzzy("REIN", items).map((r) => r.name), ["rein-app"], "case-insensitive");
  assert.deepEqual(fuzzy("", items).length, 3);
  assert.deepEqual(fuzzy("zzz", items), []);
});
