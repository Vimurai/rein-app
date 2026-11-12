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

// E-49 replaces the grid tabline/statusline version of this test (the frame is now
// chrome.ts → frame.tsx, desktop.md V2): every control of the title bar and the status bar
// runs an action from the closed list, which also names its tooltip and accessible name.
test("every title-bar and status-bar control carries its action", async () => {
  const { titleBar, statusBar, windowButtons, titleInset, chromeOf } = await import("../src/chrome.ts");
  const { ACTIONS } = await import("../src/actions.ts");
  const names = new Set(ACTIONS.map((a) => a.name));
  const bar = titleBar(
    [
      { title: "pa", current: false, exited: false },
      { title: "pb", current: true, exited: true },
    ],
    "0.5.0",
  );
  const calls = (c) => [c.name, c.arg ?? null];
  assert.deepEqual(
    bar.tabs.map((t) => [t.label, t.current, t.exited, calls(t.select), calls(t.close)]),
    [
      ["pa", false, false, ["tab.goto", "1"], ["tab.detach", "1"]],
      ["pb", true, true, ["tab.goto", "2"], ["tab.detach", "2"]],
    ],
  );
  assert.deepEqual(calls(bar.add), ["project.open", null]);
  assert.deepEqual([bar.version.label, ...calls(bar.version.action)], ["0.5.0", "release.update", null]);
  assert.deepEqual(calls(bar.menu), ["menu.show", null]);
  const status = statusBar("/home/me/p", { status: "ok", checks: [], reason: null, setup: null, taken_at: 0 }, "cd /home/me/p && rein start", null);
  assert.deepEqual(calls(status.project.action), ["project.pick", null]);
  assert.deepEqual([status.health.icon, ...calls(status.health.action)], ["ok", "health.show", null]);
  assert.deepEqual(calls(status.command.copy), ["cmd.yank", null]);
  // E-50: the home directory is shown as `~` (V2: `~/Dev/schraubr_laravel`); nothing else is.
  const { tildePath } = await import("../src/chrome.ts");
  assert.equal(status.project.text, "~/p");
  assert.equal(tildePath("/home/emir/Dev/schraubr_laravel"), "~/Dev/schraubr_laravel");
  assert.equal(tildePath("/Users/me/code/x"), "~/code/x");
  assert.equal(tildePath("/root"), "~");
  assert.equal(tildePath("/homework/x"), "/homework/x");
  assert.equal(tildePath("/srv/home/me/x"), "/srv/home/me/x");
  assert.equal(tildePath("/home"), "/home");
  assert.equal(status.mode, null, "no mode badge in the simple style");
  assert.equal(statusBar(null, null, null, "NORMAL").mode, "NORMAL");
  assert.equal(statusBar(null, null, null, null).health.icon, "checking");
  for (const c of [bar.add, bar.version.action, bar.menu, status.project.action, status.health.action, status.command.copy, ...bar.tabs.flatMap((t) => [t.select, t.close])]) {
    assert.ok(names.has(c.name), `${c.name} is not an action`);
  }
  // Our window buttons only where there is no native title bar; macOS leaves room for its own.
  assert.deepEqual(windowButtons("windows"), ["minimize", "maximize", "close"]);
  assert.deepEqual(windowButtons("linux"), []);
  assert.deepEqual(windowButtons("macos"), []);
  assert.equal(titleInset("macos"), 80);
  assert.equal(titleInset("windows"), 0);
  assert.equal(chromeOf("windows"), "windows");
  assert.equal(chromeOf("<script>"), "linux");
});

test("a paste with a newline asks first; one line does not", async () => {
  const { pasteLines, pasteNeedsQuestion } = await import("../src/paste.ts");
  assert.equal(pasteNeedsQuestion("ls -la"), false);
  assert.equal(pasteNeedsQuestion("make\nmake install"), true);
  assert.equal(pasteLines("ls -la"), 1);
  assert.equal(pasteLines("make\nmake install\n"), 3);
  assert.equal(pasteLines("a\r\nb"), 2);
});
