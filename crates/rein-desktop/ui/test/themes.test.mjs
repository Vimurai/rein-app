// E-37 (desktop.md "For tiling window managers", §Test Plan "Screens"): six built-in themes,
// each sixteen colours plus fg, bg and accent, and every text pair the chrome draws reads at
// WCAG AA (4.5:1) in every one.
import { test } from "node:test";
import assert from "node:assert/strict";
import { DEFAULT_THEME_NAME, THEMES, THEME_NAMES, contrast, cssVariables, resolveTheme, xtermTheme } from "../src/theme.ts";
import { DEFAULT_CONFIG } from "../src/config.ts";

/** [text, background] CSS variable pairs the chrome uses (style.css). */
const PAIRS = [
  ["--fg", "--bg"], // stage, command line, tabs
  ["--dim", "--bg"], // placeholder and hints
  ["--bar-fg", "--bar-bg"], // tabline, statusline
  ["--bar-dim", "--bar-bg"], // version, separators
  ["--accent", "--bg"], // overlay titles
  ["--bg", "--mode-terminal"], // TERMINAL segment
  ["--bg", "--mode-normal"], // NORMAL segment
  ["--bg", "--mode-command"], // COMMAND segment
  ["--error", "--bg"], // error line
];

test("there are exactly the six named themes", () => {
  assert.equal(THEMES.size, 6);
  assert.deepEqual([...THEMES.keys()], [...THEME_NAMES]);
  for (const t of THEMES.values()) {
    assert.equal(t.ansi.length, 16, t.name);
    for (const c of [t.fg, t.bg, t.accent, ...t.ansi, t.barBg, t.barFg, t.dim, t.barDim, t.border]) {
      assert.match(c, /^#[0-9a-f]{6}$/, `${t.name}: ${c}`);
    }
  }
});

test("every chrome text pair reads at 4.5:1 or more in every theme", () => {
  const failures = [];
  for (const t of THEMES.values()) {
    const v = cssVariables(t);
    for (const [fg, bg] of PAIRS) {
      const r = contrast(v[fg], v[bg]);
      if (r < 4.5) failures.push(`${t.name}: ${fg} ${v[fg]} on ${bg} ${v[bg]} = ${r.toFixed(2)}`);
    }
  }
  assert.deepEqual(failures, []);
});

test("[theme.custom] overrides only the colours it names", () => {
  const t = resolveTheme("nord", { accent: "#ff0000", color4: "#00ff00" });
  assert.equal(t.accent, "#ff0000");
  assert.equal(t.ansi[4], "#00ff00");
  assert.equal(t.ansi[5], THEMES.get("nord").ansi[5]);
  assert.equal(t.fg, THEMES.get("nord").fg);
  assert.equal(resolveTheme("no-such").name, "catppuccin-mocha");
});

test("catppuccin-mocha is the default, and the terminal background is the window's in every theme", () => {
  assert.equal(DEFAULT_THEME_NAME, "catppuccin-mocha");
  assert.equal(DEFAULT_CONFIG.theme.name, DEFAULT_THEME_NAME);
  for (const name of THEME_NAMES) {
    const t = resolveTheme(name);
    assert.equal(xtermTheme(t).background, cssVariables(t)["--bg"], name);
    const custom = resolveTheme(name, { bg: "#123456" });
    assert.equal(xtermTheme(custom).background, "#123456", `${name} with [theme.custom] bg`);
  }
});

// E-51 (V6): the tmux overlay is painted from the theme. Extends the palette checks above:
// the core takes only `#rrggbb` in lower case, so a custom colour in upper case still passes.
test("the tmux overlay colours are the theme's, lower case, in every theme", async () => {
  const { tmuxColours } = await import("../src/theme.ts");
  for (const t of THEMES.values()) {
    const c = tmuxColours(t);
    assert.deepEqual(c, { bg: t.bg, fg: t.fg, muted: t.dim, border: t.border, accent: t.accent }, t.name);
  }
  const custom = tmuxColours(resolveTheme("nord", { accent: "#ABCDEF" }));
  assert.equal(custom.accent, "#abcdef");
  for (const v of Object.values(custom)) assert.match(v, /^#[0-9a-f]{6}$/);
});
