// E-38: the start screen — wordmark, recent projects 1–9 (missing ones dimmed), five hints —
// and its keys.
import { test } from "node:test";
import assert from "node:assert/strict";
import { DEFAULT_MODAL } from "../src/keymap.ts";
import { step } from "../src/modes.ts";
import { HINTS, WORDMARK, startLines, withStartKeys } from "../src/start.ts";

const r = (i, exists = true) => ({ path: `/home/me/p${i}`, name: `p${i}`, last_opened: i, exists });

test("wordmark, numbered recent projects, missing dimmed, five hints", () => {
  const lines = startLines([r(1), r(2, false), ...Array.from({ length: 12 }, (_, i) => r(i + 3))]);
  const text = lines.map((l) => (typeof l === "string" ? l : l.text));
  assert.deepEqual(text.slice(0, 2), WORDMARK);
  const rows = lines.filter((l) => typeof l !== "string" && /^\d  /.test(l.text));
  assert.equal(rows.length, 9, "at most nine numbered rows");
  assert.match(rows[0].text, /^1  p1 +\/home\/me\/p1$/);
  assert.equal(rows[1].cls, "dim");
  assert.match(rows[1].text, /\(missing\)$/);
  assert.equal(text.at(-1), HINTS);
  assert.equal(HINTS.split(/ {3}/).length, 5);
  assert.ok(startLines([]).includes("no recent projects"));
});

test("start-screen keys: p o h u q, digits open recent N", () => {
  const t = withStartKeys(DEFAULT_MODAL);
  const act = (k) => step(t, { mode: "NORMAL", pending: [] }, k).action;
  assert.deepEqual(act("p"), { name: "project.pick" });
  assert.deepEqual(act("o"), { name: "project.open" });
  assert.deepEqual(act("h"), { name: "health.show" });
  assert.deepEqual(act("u"), { name: "release.update" });
  assert.deepEqual(act("q"), { name: "app.quit" });
  assert.deepEqual(act("3"), { name: "tab.goto", arg: "3" });
  assert.equal(DEFAULT_MODAL.normal.has("p"), false, "the project keymap is unchanged");
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
