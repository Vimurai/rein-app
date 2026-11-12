// The bundled terminal fonts (desktop.md §Look and feel v2 V3, D-051). Extends the font case
// in config.test.mjs ("the font"): the stack rules as a table, and the vendored files.
import { test } from "node:test";
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { readFileSync, readdirSync, statSync } from "node:fs";
import { BUNDLED_MONO, SYMBOLS, terminalStack } from "../src/fonts.ts";

const SYM = `"${SYMBOLS}"`;

test("the terminal stack always holds the symbols font, before any generic family", () => {
  const cases = [
    ["", `"${BUNDLED_MONO}", ${SYM}, monospace`],
    ["   ", `"${BUNDLED_MONO}", ${SYM}, monospace`],
    ["Iosevka", `"Iosevka", ${SYM}, monospace`],
    ['"Fira Code", monospace', `"Fira Code", ${SYM}, monospace`],
    ["Cascadia Mono, Consolas, ui-monospace", `"Cascadia Mono", "Consolas", ${SYM}, ui-monospace`],
    // Named already, or placed after a generic family: moved to its one place.
    [`monospace, ${SYMBOLS}, Iosevka`, `"Iosevka", ${SYM}, monospace`],
    ["monospace", `"${BUNDLED_MONO}", ${SYM}, monospace`],
  ];
  for (const [family, stack] of cases) assert.equal(terminalStack(family), stack, `font.family = ${family}`);
});

const FONTS = new URL("../fonts/", import.meta.url);

test("the vendored fonts match their manifest, fit the budget and carry their licences", () => {
  const manifest = readFileSync(new URL("MANIFEST.sha256", FONTS), "utf8")
    .split("\n")
    .filter((l) => l.trim() !== "")
    .map((l) => l.split(/\s+\*?/));
  const files = readdirSync(FONTS).filter((f) => /\.(woff2|ttf)$/.test(f)).sort();
  assert.deepEqual(manifest.map(([, f]) => f).sort(), files, "every font file is in the manifest, and only those");
  let total = 0;
  for (const [sum, f] of manifest) {
    const bytes = readFileSync(new URL(f, FONTS));
    assert.equal(createHash("sha256").update(bytes).digest("hex"), sum, f);
    total += statSync(new URL(f, FONTS)).size;
  }
  assert.ok(total <= 3 * 1024 * 1024, `fonts are ${total} bytes, over 3 MiB`);
  assert.deepEqual(
    files,
    [
      "JetBrainsMono-Bold.woff2",
      "JetBrainsMono-BoldItalic.woff2",
      "JetBrainsMono-Italic.woff2",
      "JetBrainsMono-Regular.woff2",
      "SymbolsNerdFontMono-Regular.ttf",
    ],
  );
  assert.match(readFileSync(new URL("LICENSE-JetBrainsMono-OFL.txt", FONTS), "utf8"), /SIL OPEN FONT LICENSE Version 1\.1/);
  assert.match(readFileSync(new URL("LICENSE-NerdFontsSymbols-MIT.txt", FONTS), "utf8"), /The MIT License/);
});

test("every face the stylesheet names is a vendored file", () => {
  const css = readFileSync(new URL("fonts.css", FONTS), "utf8");
  const urls = [...css.matchAll(/url\("fonts\/([^"]+)"\)/g)].map((m) => m[1]).sort();
  assert.deepEqual(urls, readdirSync(FONTS).filter((f) => /\.(woff2|ttf)$/.test(f)).sort());
  assert.match(css, new RegExp(`font-family: "${BUNDLED_MONO}"`));
  assert.match(css, new RegExp(`font-family: "${SYMBOLS}"`));
});
