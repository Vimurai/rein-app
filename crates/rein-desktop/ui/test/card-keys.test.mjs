// Keys on a set-up card (desktop.md §V9 "Keys on a set-up card", D-070 ruling 1; E-98): the
// shortcut chords run their action in both input styles, the plain keys stay with the card,
// and a real overlay takes every key. Extends modes.test.mjs (the key tables themselves).
import { test } from "node:test";
import assert from "node:assert/strict";
import { cardKey } from "../src/card-keys.ts";
import { DEFAULT_MODAL, DEFAULT_SIMPLE } from "../src/keymap.ts";

const CHORDS = {
  "C-Tab": "tab.next",
  "C-S-Tab": "tab.prev",
  "M-2": "tab.goto",
  "C-S-h": "home.show",
  "C-S-p": "menu.show",
  "C-S-o": "project.open",
};

test("on a set-up card the shortcut chords run their action, in both input styles", () => {
  for (const table of [DEFAULT_SIMPLE, DEFAULT_MODAL]) {
    for (const [k, name] of Object.entries(CHORDS)) {
      const r = cardKey(k, table.shortcuts, false);
      assert.equal(typeof r, "object", `${table.style} ${k}`);
      assert.equal(r.action.name, name, `${table.style} ${k}`);
    }
    assert.deepEqual(cardKey("M-2", table.shortcuts, false), { action: { name: "tab.goto", arg: "2" } });
  }
});

test("the plain keys stay with the card", () => {
  for (const k of ["Tab", "S-Tab", "Enter", "Space", "Esc", "a", "q", "1"]) {
    assert.equal(cardKey(k, DEFAULT_SIMPLE.shortcuts, false), "card", k);
    assert.equal(cardKey(k, DEFAULT_MODAL.shortcuts, false), "card", `modal ${k}`);
  }
});

test("with a real overlay open (the picker) the same chords give no action", () => {
  for (const k of [...Object.keys(CHORDS), "Tab", "Enter", "a"]) {
    assert.equal(cardKey(k, DEFAULT_SIMPLE.shortcuts, true), "overlay", k);
  }
});
