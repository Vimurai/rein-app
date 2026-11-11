// Grid shell (desktop.md "Screen"): every row is exactly `cols` cells, the three chrome rows
// are present, the stage is box-drawn, and the chrome leaves room for ×, [+] and [≡] (D-034).
import { test } from "node:test";
import assert from "node:assert/strict";
import { layout, rowWidth, width } from "../src/grid.ts";
import { GRUVBOX_DARK, contrast } from "../src/theme.ts";

const data = {
  tabs: [
    { name: "rein-app", current: true },
    { name: "energy-api", current: false },
  ],
  version: "0.5.0",
  mode: null,
  segments: ["rein-app", "master", "✓ install ok", "2 profiles"],
  clock: "14:02",
  stageLines: ["placeholder"],
  commandLine: ":",
};
const text = (row) => row.map((s) => s.text).join("");

test("every row is exactly cols wide, at 80x24 and 200x60", () => {
  for (const [cols, rows] of [[80, 24], [200, 60], [20, 5]]) {
    const s = layout(data, cols, rows);
    const all = [s.tabline, ...s.stage, s.statusline, s.commandLine];
    assert.equal(all.length, rows, `${cols}x${rows} row count`);
    for (const r of all) assert.equal(rowWidth(r), cols, `${cols}x${rows}: ${text(r)}`);
  }
});

test("the tabline has tabs with ×, [+], the version and [≡]", () => {
  const t = text(layout(data, 80, 24).tabline);
  assert.match(t, /^ 1:rein-app\* × {2}2:energy-api × {3}\[\+\]/);
  assert.match(t, /rein 0\.5\.0 {2}\[≡\] $/);
});

test("the stage is a box-drawn frame with the placeholder centred", () => {
  const st = layout(data, 40, 10).stage.map(text);
  assert.equal(st.length, 7);
  assert.equal(st[0], `┌${"─".repeat(38)}┐`);
  assert.equal(st.at(-1), `└${"─".repeat(38)}┘`);
  assert.ok(st.slice(1, -1).every((r) => r.startsWith("│") && r.endsWith("│")));
  assert.ok(st.some((r) => r.includes("placeholder")));
});

test("statusline: no mode segment in the simple style, a mode segment when set", () => {
  const simple = text(layout(data, 80, 24).statusline);
  assert.match(simple, /^ rein-app │ master │ ✓ install ok │ 2 profiles +14:02 $/);
  const modal = layout({ ...data, mode: "TERMINAL" }, 80, 24).statusline;
  assert.equal(modal[0].text, " TERMINAL ");
  assert.equal(modal[0].cls, "mode-terminal");
});

test("a narrow window keeps the right-hand items and never overflows", () => {
  const t = layout(data, 12, 4);
  assert.equal(rowWidth(t.tabline), 12);
  assert.match(text(t.tabline), /\[≡\] $/);
  assert.equal(layout(data, 0, 0).stage.length, 0);
});

test("chrome glyphs are one cell wide", () => {
  for (const g of ["┌", "─", "│", "×", "≡", "✓"]) assert.equal(width(g), 1);
});

test("gruvbox-dark chrome text has contrast >= 4.5:1", () => {
  const t = GRUVBOX_DARK;
  for (const [fg, bg] of [[t.fg, t.bg], [t.barFg, t.barBg], [t.dim, t.bg], [t.barDim, t.barBg], [t.bg, t.ansi[10]]]) {
    assert.ok(contrast(fg, bg) >= 4.5, `${fg} on ${bg}: ${contrast(fg, bg).toFixed(2)}`);
  }
});
