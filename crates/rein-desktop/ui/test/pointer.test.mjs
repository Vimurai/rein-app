// desktop.md §Test Plan "Pointer parity" (E-40, D-034): every action has a key in the
// `simple` or the `modal` table; `simple` forwards every key that is not one of its
// shortcuts; the shortcuts work in every mode of both styles.
import { test } from "node:test";
import assert from "node:assert/strict";
import { ACTIONS } from "../src/actions.ts";
import { DEFAULT_MODAL, DEFAULT_SIMPLE, SIMPLE_SHORTCUTS, expanded } from "../src/keymap.ts";
import { step } from "../src/modes.ts";
import { keyName } from "../src/keys.ts";

const bound = (table) => new Set([...expanded(table).values(), ...table.shortcuts.values()].map((a) => a.name));

test("every action has a key in the simple or the modal table", () => {
  const keys = new Set([...bound(DEFAULT_SIMPLE), ...bound(DEFAULT_MODAL)]);
  // The escape chord is mode.normal's key in modal (it lives in `escape`, not the map).
  keys.add("mode.normal");
  const missing = ACTIONS.map((a) => a.name).filter((n) => !keys.has(n));
  assert.deepEqual(missing, []);
});

function rng(seed) {
  let x = seed >>> 0 || 1;
  return () => ((x ^= x << 13), (x ^= x >>> 17), (x ^= x << 5), (x >>> 0) / 2 ** 32);
}

test("property: simple forwards every key that is not a shortcut, in order", () => {
  const alphabet = ["a", "C-\\", "C-n", "Enter", "Esc", ":", "Space", "C-c", "C-S-p", "M-1", "F1", "C-Tab", "x"];
  for (let seed = 1; seed <= 1000; seed++) {
    const r = rng(seed);
    const keys = Array.from({ length: 1 + Math.floor(r() * 10) }, () => alphabet[Math.floor(r() * alphabet.length)]);
    let s = { mode: "TERMINAL", pending: [] };
    const sent = [];
    const acts = [];
    for (const k of keys) {
      s = step(DEFAULT_SIMPLE, s, k);
      sent.push(...s.forward);
      if (s.action) acts.push(s.action.name);
      assert.equal(s.mode, "TERMINAL", "simple never leaves TERMINAL");
    }
    assert.deepEqual(sent, keys.filter((k) => !SIMPLE_SHORTCUTS.has(k)), `seed ${seed}`);
    assert.deepEqual(acts, keys.filter((k) => SIMPLE_SHORTCUTS.has(k)).map((k) => SIMPLE_SHORTCUTS.get(k).name));
  }
});

test("the simple shortcuts work in every mode of both styles", () => {
  for (const [key, action] of SIMPLE_SHORTCUTS) {
    for (const [table, mode] of [[DEFAULT_SIMPLE, "TERMINAL"], [DEFAULT_MODAL, "TERMINAL"], [DEFAULT_MODAL, "NORMAL"]]) {
      assert.deepEqual(step(table, { mode, pending: [] }, key).action, action, `${table.style} ${mode} ${key}`);
    }
  }
  // A modal chord in progress is not interrupted by a shortcut key.
  const held = step(DEFAULT_MODAL, { mode: "TERMINAL", pending: ["C-\\"] }, "C-S-p");
  assert.equal(held.action, undefined);
});

test("shortcut key names", () => {
  const k = (key, m = {}) => keyName({ key, ctrlKey: false, altKey: false, shiftKey: false, metaKey: false, ...m });
  assert.equal(k("Tab", { ctrlKey: true }), "C-Tab");
  assert.equal(k("Tab", { ctrlKey: true, shiftKey: true }), "C-S-Tab");
  assert.equal(k("1", { altKey: true }), "M-1");
  assert.equal(k("=", { ctrlKey: true }), "C-=");
  assert.equal(k("-", { ctrlKey: true }), "C--");
  assert.equal(k("V", { ctrlKey: true, shiftKey: true }), "C-S-v");
  assert.equal(k("F1"), "F1");
});

test("the menu lists every action in plain words, with its shortcut", async () => {
  const { menuEntries, showKey } = await import("../src/menu.ts");
  const entries = menuEntries(DEFAULT_SIMPLE, DEFAULT_MODAL, { tabs: 2 });
  const names = new Set(entries.map((e) => e.call.name));
  assert.deepEqual(ACTIONS.map((a) => a.name).filter((n) => !names.has(n)), [], "an action without a menu entry");
  const find = (label) => entries.find((e) => e.label === label);
  assert.equal(find("Menu").shortcut, "Ctrl+Shift+P");
  assert.equal(find("Check installation health").shortcut, "modal: Space h", "the other style's key, named");
  assert.equal(find("Go to tab 2").shortcut, "Alt+2");
  assert.ok(find("Theme: nord"));
  assert.ok(find("Input style: modal (vim-style modes)"));
  assert.equal(entries.filter((e) => e.call.name === "tab.goto").length, 2, "one per open tab");
  assert.equal(showKey("C-S-Tab"), "Ctrl+Shift+Tab");
  assert.equal(showKey("<leader> h".replace("<leader>", "Space")), "Space h");
});

test("every chrome button carries its action (tooltip and accessible name come from it)", async () => {
  const { layout } = await import("../src/grid.ts");
  const shell = layout(
    {
      tabs: [{ name: "pa", current: false }, { name: "pb", current: true }],
      version: "0.5.0",
      mode: null,
      segments: ["pb", { text: "✓ install ok", cls: "", action: { name: "health.show" } }],
      clock: "10:00",
      stageLines: [],
      commandLine: "",
    },
    120,
    30,
  );
  const buttons = (row) => row.filter((s) => s.cls.includes("button") || s.action);
  const tl = buttons(shell.tabline).map((s) => [s.text.trim(), s.action?.name, s.action?.arg ?? null]);
  assert.deepEqual(tl, [
    ["1:pa", "tab.goto", "1"],
    ["×", "tab.detach", "1"],
    ["2:pb*", "tab.goto", "2"],
    ["×", "tab.detach", "2"],
    ["[+]", "project.open", null],
    ["rein 0.5.0", "release.update", null],
    ["[≡]", "menu.show", null],
  ]);
  assert.ok(shell.statusline.some((s) => s.action?.name === "health.show"));
  for (const s of [...shell.tabline, ...shell.statusline]) {
    if (s.cls.includes("button")) assert.ok(s.action, `button "${s.text}" has no action`);
  }
});

test("a paste with a newline asks first; one line does not", async () => {
  const { pasteLines, pasteNeedsQuestion } = await import("../src/paste.ts");
  assert.equal(pasteNeedsQuestion("ls -la"), false);
  assert.equal(pasteNeedsQuestion("make\nmake install"), true);
  assert.equal(pasteLines("ls -la"), 1);
  assert.equal(pasteLines("make\nmake install\n"), 3);
  assert.equal(pasteLines("a\r\nb"), 2);
});
