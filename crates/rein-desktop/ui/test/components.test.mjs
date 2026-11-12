// E-42 → E-50 (D-037, superseded for the look by D-049 / desktop.md §Look and feel v2 V1–V2):
// the shadcn/ui components in the new-york look — colours only from the theme's shadcn tokens,
// radii only from the scale (4 / 6 / 8 px, full), one shadow token, transitions of 150 ms at
// most and no animation (motion is E-54's), the UI font — and the whole bundle, xterm.js
// included, stays within 2 MiB.
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
/** Each component's source without its comments: prose may say "rounded" or "shadow". */
const sources = readdirSync(componentsDir).map((f) => [
  f,
  readFileSync(join(componentsDir, f), "utf8")
    .replace(/\/\*[\s\S]*?\*\//g, "")
    .replace(/^\s*\/\/.*$/gm, ""),
]);

/** The theme's shadcn tokens (tailwind.css `@theme inline`, theme.ts `shadcnVariables`): the
 *  only colours a component class may name. */
const TOKENS = new Set([
  "background", "foreground", "card", "card-foreground", "popover", "popover-foreground",
  "muted", "muted-foreground", "primary", "primary-foreground", "ring", "destructive", "border",
]);
// A variant prefix may carry a bracketed condition: `group-data-[selected=true]:text-…`.
const COLOR_UTIL = /^(?:[a-z0-9-]*(?:\[[^\]]*\])?:)*(bg|text|border(?:-[trblxy])?|ring|outline|fill|stroke|from|via|to|placeholder|decoration|caret|accent|divide)-([a-z0-9-]+)(?:\/\d+)?$/;
const NOT_COLOR = /^(offset-\d+|left|right|center|justify|start|end|wrap|nowrap|ellipsis|clip|none|solid|dashed|dotted|double|hidden|collapse|separate|0|2|4|8|x|y|t|b|l|r|base|xs|sm|lg|xl|2xl|auto|inherit|transparent|current)$/;

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

test("the components name no hard-coded colour, no shadow or radius off the scale, no animation", () => {
  for (const [file, src] of sources) {
    assert.doesNotMatch(src, /#[0-9a-fA-F]{3,8}\b|\b(?:rgb|rgba|hsl|hsla|oklch|oklab)\(/, `${file}: literal colour`);
    assert.doesNotMatch(src, /\bshadow-(?!md\b)|\bdrop-shadow-/, `${file}: a shadow other than the token`);
    for (const m of src.matchAll(/\brounded(-[a-z0-9[\]-]+)?/g)) {
      assert.match(m[1] ?? "", /^-(?:sm|md|lg|full)$/, `${file}: ${m[0]} is off the radius scale`);
    }
    assert.doesNotMatch(src, /\banimate-(?!none\b)/, `${file}: animation`);
    for (const m of src.matchAll(/\bduration-(\d+)/g)) {
      assert.ok(Number(m[1]) <= 150, `${file}: duration-${m[1]} is over 150 ms`);
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
    ["dialog", rendered.dialog(), ["Update Rein", "Update to 0.5.1", "rein-app update --yes"]],
    ["tooltip", rendered.tooltip(), ["Menu  Ctrl+Shift+P"]],
    ["command", rendered.command(), ["Open a project…", "Ctrl+Shift+O", "Check installation health"]],
  ];
  for (const [name, html, texts] of cases) {
    for (const t of texts) assert.ok(html.includes(t), `${name}: shows "${t}"`);
    const classes = classesOf(html);
    assert.ok(classes.some((c) => c === "font-ui"), `${name}: the UI font`);
    assert.deepEqual(badColourClasses(classes), [], `${name}: colours outside the theme tokens`);
    assert.doesNotMatch(html, /style="[^"]*(?:color|background)[^"]*#/, `${name}: an inline colour`);
    // Every token the render uses has a gruvbox-dark value (tailwind.css maps --color-<t> to --<t>).
    for (const t of new Set(classes.map(tokenOf).filter(Boolean))) {
      assert.match(vars[`--${t}`] ?? "", /^#[0-9a-f]{6}$/, `${name}: gruvbox-dark sets --${t}`);
    }
  }
  // The pairs the components draw text with read at 4.5:1 in every theme: themes.test.mjs.
});

// E-49: the limit is 2 MiB (desktop.md V3); the icons of the modern frame are in it.
test("the built bundle is at most 2 MiB uncompressed, xterm.js included", async () => {
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
  // Our grid's own classes (style.css) must not also be Tailwind utilities: `.border` was,
  // and drew a box around every frame glyph.
  const tw = readFileSync(join(dir, "tw.css"), "utf8");
  const ours = new Set([...readFileSync(join(ui, "src/style.css"), "utf8").matchAll(/\.([a-z][\w-]*)/g)].map((m) => m[1]));
  const clashes = [...ours].filter((c) => new RegExp(`\\.${c}[{,:\\s]`).test(tw) && c !== "hidden");
  assert.deepEqual(clashes, [], "classes defined by both style.css and Tailwind");
  const total = readdirSync(dir).reduce((n, f) => n + statSync(join(dir, f)).size, 0);
  console.log(`bundle: ${(total / 1024).toFixed(0)} KiB of 2048`);
  assert.ok(total <= 2 * 1024 * 1024, `${(total / 1024).toFixed(0)} KiB`);
  assert.ok(total > 300 * 1024, "the bundle includes xterm.js and React");
});
