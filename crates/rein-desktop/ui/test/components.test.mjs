// E-42 (D-037): the shadcn/ui components are restyled to the grid — colours only from the
// theme variables, no shadow, no rounded corner, no animation over 80 ms — and the whole
// bundle, xterm.js included, stays within 1.5 MiB.
import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtempSync, readFileSync, readdirSync, statSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { build } from "esbuild";
import { ESBUILD, buildTailwind } from "../build.mjs";
import { GRUVBOX_DARK, contrast, cssVariables } from "../src/theme.ts";

const ui = fileURLToPath(new URL("..", import.meta.url));
const componentsDir = join(ui, "components");
const sources = readdirSync(componentsDir).map((f) => [f, readFileSync(join(componentsDir, f), "utf8")]);

/** The theme's tokens (tailwind.css `@theme inline`): the only colours a class may name. */
const TOKENS = new Set(["fg", "bg", "accent", "bar-bg", "bar-fg", "dim", "bar-dim", "border", ...Array.from({ length: 16 }, (_, i) => `ansi-${i}`)]);
const COLOR_UTIL = /^(?:[a-z-]+:|\[[^\]]*\]:)*(bg|text|border(?:-[trblxy])?|ring|outline|fill|stroke|from|via|to|placeholder|decoration|caret|accent|divide)-([a-z0-9-]+)(?:\/\d+)?$/;
const NOT_COLOR = /^(left|right|center|justify|start|end|wrap|nowrap|ellipsis|clip|none|solid|dashed|dotted|double|hidden|collapse|separate|0|2|4|8|x|y|t|b|l|r|base|xs|sm|lg|xl|2xl|auto|inherit|transparent|current)$/;

function classesOf(html) {
  return [...html.matchAll(/class="([^"]*)"/g)].flatMap((m) => m[1].split(/\s+/)).filter(Boolean);
}

function badColourClasses(classes) {
  return classes.filter((c) => {
    const m = COLOR_UTIL.exec(c);
    if (!m) return false;
    const value = m[2];
    return !NOT_COLOR.test(value) && !TOKENS.has(value);
  });
}

test("the components name no hard-coded colour, shadow, radius or long animation", () => {
  for (const [file, src] of sources) {
    assert.doesNotMatch(src, /#[0-9a-fA-F]{3,8}\b|\b(?:rgb|rgba|hsl|hsla|oklch|oklab)\(/, `${file}: literal colour`);
    assert.doesNotMatch(src, /\bshadow-(?!none\b)|\bdrop-shadow-/, `${file}: shadow`);
    assert.doesNotMatch(src, /\brounded(?!-none\b)/, `${file}: rounded corner`);
    assert.doesNotMatch(src, /\banimate-(?!none\b)/, `${file}: animation`);
    for (const m of src.matchAll(/\bduration-(\d+)/g)) {
      assert.ok(Number(m[1]) <= 80, `${file}: duration-${m[1]} is over 80 ms`);
    }
    const literalClasses = [...src.matchAll(/"([^"]*)"/g)].flatMap((m) => m[1].split(/\s+/));
    assert.deepEqual(badColourClasses(literalClasses), [], `${file}: a colour outside the theme tokens`);
  }
});

const rendered = await (async () => {
  const out = join(mkdtempSync(join(tmpdir(), "rein-ui-")), "render.mjs");
  await build({
    entryPoints: [join(ui, "test/fixtures/render.tsx")],
    bundle: true,
    platform: "node",
    format: "esm",
    jsx: "automatic",
    outfile: out,
    // react-dom/server's CommonJS parts call require(); give the ES module one.
    banner: { js: 'import { createRequire } from "node:module"; const require = createRequire(import.meta.url);' },
    logLevel: "error",
    define: { "process.env.NODE_ENV": '"production"' },
  });
  return import(pathToFileURL(out).href);
})();

/** The colour token a utility class names (`bg-bar-bg` → `bar-bg`), if any. */
function tokenOf(c) {
  const m = COLOR_UTIL.exec(c);
  return m && TOKENS.has(m[2]) ? m[2] : null;
}

test("a Dialog, a Tooltip and the Command list render in gruvbox-dark using only theme tokens", () => {
  const vars = cssVariables(GRUVBOX_DARK);
  const cases = [
    ["dialog", rendered.dialog(), ["Update Rein", "[ Update to 0.5.1 ]", "rein-app update --yes"]],
    ["tooltip", rendered.tooltip(), ["Menu  Ctrl+Shift+P"]],
    ["command", rendered.command(), ["Open a project…", "Ctrl+Shift+O", "Check installation health"]],
  ];
  for (const [name, html, texts] of cases) {
    for (const t of texts) assert.ok(html.includes(t), `${name}: shows "${t}"`);
    const classes = classesOf(html);
    assert.ok(classes.some((c) => c === "font-mono"), `${name}: the grid's monospace font`);
    assert.deepEqual(badColourClasses(classes), [], `${name}: colours outside the theme tokens`);
    assert.doesNotMatch(html, /style="[^"]*(?:color|background)[^"]*#/, `${name}: an inline colour`);
    // Every token the render uses has a gruvbox-dark value (tailwind.css maps --color-<t> to --<t>).
    for (const t of new Set(classes.map(tokenOf).filter(Boolean))) {
      assert.match(vars[`--${t}`] ?? "", /^#[0-9a-f]{6}$/, `${name}: gruvbox-dark sets --${t}`);
    }
  }
  // The pairs the components draw text with are readable (WCAG AA, 4.5:1).
  for (const [fg, bg] of [["--fg", "--bg"], ["--bar-fg", "--bar-bg"], ["--accent", "--bg"], ["--accent", "--bar-bg"], ["--bg", "--accent"], ["--dim", "--bg"]]) {
    assert.ok(contrast(vars[fg], vars[bg]) >= 4.5, `${fg} on ${bg}: ${contrast(vars[fg], vars[bg]).toFixed(2)}`);
  }
});

test("the built bundle is at most 1.5 MiB uncompressed, xterm.js included", async () => {
  const dir = mkdtempSync(join(tmpdir(), "rein-ui-size-"));
  const prev = process.cwd();
  process.chdir(ui);
  try {
    // The shell plus all nine components: what ships once every overlay is mounted.
    await build({ ...ESBUILD, entryPoints: { app: "test/fixtures/all.tsx", boot: "src/boot.ts" }, outdir: dir });
    buildTailwind(join(dir, "tw.css"));
  } finally {
    process.chdir(prev);
  }
  const total = readdirSync(dir).reduce((n, f) => n + statSync(join(dir, f)).size, 0);
  assert.ok(total <= 1.5 * 1024 * 1024, `${(total / 1024).toFixed(0)} KiB`);
  assert.ok(total > 300 * 1024, "the bundle includes xterm.js and React");
});
