// desktop.md §Look and feel v2 V7 "Tests (V7)" (D-054, E-54): the loader delay, everything
// still under reduced motion, and an idle window that runs no animation. New suite: motion is
// new with E-54; components.test.mjs keeps the token rules (it builds the render fixture this
// suite reuses). The tab phase rule is test/phase.test.mjs.
import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtempSync, readFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { build } from "esbuild";

const ui = fileURLToPath(new URL("..", import.meta.url));
const css = readFileSync(join(ui, "src/style.css"), "utf8").replace(/\/\*[\s\S]*?\*\//g, "");

/** `selector { body }` pairs of the stylesheet, outside @media. */
function rules(src) {
  const out = [];
  for (const m of src.replace(/@media[^{]*\{(?:[^{}]*\{[^}]*\})*[^}]*\}/g, "").matchAll(/([^{}]+)\{([^}]*)\}/g)) {
    out.push([m[1].trim(), m[2]]);
  }
  return out;
}
const top = rules(css);
const token = (name) => new RegExp(`--${name}:\\s*([^;]+);`).exec(css)?.[1]?.trim();

test("a loader shows nothing for a wait under --loader-delay (150 ms)", () => {
  assert.equal(token("loader-delay"), "150ms");
  // The hold keeps the loader invisible for its whole run.
  assert.match(css, /@keyframes rein-hold\s*\{\s*from,\s*to\s*\{\s*opacity:\s*0;\s*\}\s*\}/);
  for (const cls of [".spinner", ".progress-line", ".skeleton"]) {
    const body = top.find(([sel]) => sel === cls)?.[1] ?? "";
    assert.match(body, /animation:\s*rein-hold var\(--loader-delay\) linear/, `${cls} starts with the hold`);
    // Its own motion begins only once the hold is over.
    for (const a of body.matchAll(/rein-(spin|pulse)[^,;]*/g)) {
      assert.match(a[0], /var\(--loader-delay\) infinite/, `${cls}: ${a[0]}`);
    }
  }
  const sweep = top.find(([sel]) => sel === ".progress-line > span")?.[1] ?? "";
  assert.match(sweep, /rein-sweep [^;]*var\(--loader-delay\) infinite/);
  // An operation that ends in 100 ms unmounts its loader while the hold still hides it.
  assert.ok(parseInt(token("loader-delay"), 10) > 100);
});

test("under reduced motion nothing moves and the progress line stands still", () => {
  const reduced = /@media \(prefers-reduced-motion: reduce\)\s*\{([\s\S]*?)\n\}/.exec(css)?.[1] ?? "";
  assert.match(reduced, /\*,\s*\*::before,\s*\*::after\s*\{\s*transition:\s*none !important;\s*animation:\s*none !important;\s*\}/);
  assert.match(reduced, /\.progress-line > span\s*\{\s*width:\s*100%;\s*transform:\s*none;\s*\}/);
  // Every animation and transition in the sheet is a declaration that rule overrides: none is
  // set with !important or in an inline style the stylesheet cannot reach.
  assert.doesNotMatch(css.replace(reduced, ""), /(animation|transition)[^;]*!important/);
  // The token values (V7 table).
  assert.deepEqual(
    ["motion-fast", "motion-base", "motion-slow", "motion-spin"].map(token),
    ["120ms", "150ms", "220ms", "800ms"],
  );
  assert.equal(token("motion-ease"), "cubic-bezier(0.2, 0, 0, 1)");
});

const rendered = await (async () => {
  const out = join(mkdtempSync(join(tmpdir(), "rein-motion-")), "render.mjs");
  await build({
    entryPoints: [join(ui, "test/fixtures/render.tsx")],
    bundle: true,
    platform: "node",
    format: "esm",
    jsx: "automatic",
    outfile: out,
    banner: { js: 'import { createRequire } from "node:module"; const require = createRequire(import.meta.url);' },
    logLevel: "error",
    define: { "process.env.NODE_ENV": '"production"' },
  });
  return import(pathToFileURL(out).href);
})();

/** The classes whose animation never ends (read from the stylesheet, not listed here). */
const endless = [
  ...new Set(
    top
      .filter(([, body]) => /animation:[^;]*infinite/.test(body))
      .flatMap(([sel]) => [...sel.matchAll(/\.([a-z][\w-]*)/g)].map((m) => m[1])),
  ),
];
const classesOf = (html) => [...html.matchAll(/class="([^"]*)"/g)].flatMap((m) => m[1].split(/\s+/));

test("an idle start screen and an idle project tab run no animation", () => {
  assert.deepEqual(endless.sort(), ["progress-line", "skeleton", "spinner"]);
  for (const [name, idle, busy] of [
    ["start screen", rendered.start(false), rendered.start(true)],
    ["project tab", rendered.tab(false), rendered.tab(true)],
  ]) {
    const running = classesOf(idle).filter((c) => endless.includes(c));
    assert.deepEqual(running, [], `${name}: idle, yet ${running.join(", ")}`);
    // The check can see a loader when there is one: the same screen, busy.
    assert.ok(classesOf(busy).some((c) => endless.includes(c)), `${name}: busy shows a loader`);
  }
  assert.match(rendered.tab(true), /Starting rein…/);
  assert.match(rendered.tab(true), /Checking…/);
});

test("the script's motion values are the stylesheet's tokens", async () => {
  const { MOTION } = await import("../src/motion.ts");
  assert.deepEqual(
    [MOTION.fast, MOTION.base, MOTION.slow].map((n) => `${n}ms`),
    ["motion-fast", "motion-base", "motion-slow"].map(token),
  );
  assert.equal(MOTION.ease, token("motion-ease"));
});

test("dialogs, their backdrop and the palette leave with an exit animation", () => {
  const exit = (sel) => top.find(([s]) => s === sel)?.[1] ?? "";
  assert.match(exit('[data-slot="dialog-content"][data-state="closed"]'), /rein-pop-out var\(--motion-fast\)[^;]*forwards/);
  assert.match(exit('[data-slot="dialog-overlay"][data-state="closed"]'), /rein-fade-out var\(--motion-fast\)[^;]*forwards/);
  assert.match(exit(".palette-out"), /rein-pop-out var\(--motion-fast\)[^;]*forwards/);
  assert.match(css, /@keyframes rein-pop-out\s*\{\s*to\s*\{\s*opacity:\s*0;\s*transform:\s*scale\(0\.98\);\s*\}\s*\}/);
});

test("a tab keeps its id when a neighbour closes (its pill is not drawn again)", async () => {
  const { titleBar } = await import("../src/chrome.ts");
  const before = titleBar([{ id: 7, title: "a", current: false, exited: false }, { id: 9, title: "b", current: true, exited: false }], "0.5.0");
  const after = titleBar([{ id: 9, title: "b", current: true, exited: false }], "0.5.0");
  assert.deepEqual(before.tabs.map((t) => t.id), [7, 9]);
  assert.equal(after.tabs[0].id, 9, "same id, though its position and its Alt+N changed");
  assert.equal(after.tabs[0].select.arg, "1");
});
