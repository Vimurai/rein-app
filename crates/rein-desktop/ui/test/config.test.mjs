// E-37: config.toml's keys and font as the UI uses them (the core validates the file).
import { test } from "node:test";
import assert from "node:assert/strict";
import { DEFAULT_CONFIG, fontFamily, keyTable } from "../src/config.ts";
import { DEFAULT_MODAL } from "../src/keymap.ts";
import { step } from "../src/modes.ts";

test("the default config gives the default keymap", () => {
  const t = keyTable(DEFAULT_CONFIG);
  assert.deepEqual(t.escape, DEFAULT_MODAL.escape);
  assert.equal(t.leader, DEFAULT_MODAL.leader);
  assert.deepEqual([...t.normal], [...DEFAULT_MODAL.normal]);
});

test("config bindings, leader and escape chord reach the modes function", () => {
  const cfg = structuredClone(DEFAULT_CONFIG);
  cfg.keys.leader = ",";
  cfg.keys.escape = ["C-g"];
  cfg.keys.normal = { "<leader> 1": "tab.goto 1", "g t": "tab.prev" };
  const t = keyTable(cfg);
  let s = step(t, { mode: "NORMAL", pending: [] }, ",");
  s = step(t, s, "1");
  assert.deepEqual(s.action, { name: "tab.goto", arg: "1" });
  s = step(t, { mode: "NORMAL", pending: [] }, "g");
  assert.deepEqual(step(t, s, "t").action, { name: "tab.prev" }, "a config binding replaces the default");
  assert.deepEqual(step(t, { mode: "TERMINAL", pending: [] }, "C-g").action, { name: "mode.normal" });
});

test("font: config.toml's family, else the platform's", () => {
  // The bundled font is the default everywhere; the symbols font is always in the stack.
  assert.equal(fontFamily(DEFAULT_CONFIG), '"JetBrains Mono", "Symbols Nerd Font Mono", monospace');
  assert.equal(
    fontFamily({ ...DEFAULT_CONFIG, font: { family: "Iosevka", size: 14 } }),
    '"Iosevka", "Symbols Nerd Font Mono", monospace',
  );
});
