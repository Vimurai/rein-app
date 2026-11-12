// The terminal's fonts (desktop.md §Look and feel v2 V3, D-051): JetBrains Mono is bundled
// and is the default on every platform; Symbols Nerd Font Mono is bundled and always in the
// stack, so Powerline and Nerd Font glyphs (from tmux, a prompt, Claude Code) are drawn
// whatever font.family names, instead of empty boxes.

export const BUNDLED_MONO = "JetBrains Mono";
export const SYMBOLS = "Symbols Nerd Font Mono";

/** CSS generic families. The symbols font goes before them: a browser falls back glyph by
 *  glyph through the whole list, and the system font a generic family maps to may carry its
 *  own (wrong) glyph for a Nerd Font code point, e.g. Unifont's, which would win. */
const GENERIC = new Set(["monospace", "ui-monospace", "serif", "sans-serif", "system-ui", "cursive", "fantasy"]);

/** The terminal font stack for config.toml's `font.family` (empty: the bundled default):
 *  the operator's families, then the symbols font, then the generic family. */
export function terminalStack(family: string): string {
  const names = family
    .split(",")
    .map((n) => n.trim().replace(/^["']|["']$/g, ""))
    .filter((n) => n !== "");
  const own = names.filter((n) => !GENERIC.has(n) && n !== SYMBOLS);
  const generic = names.filter((n) => GENERIC.has(n));
  const head = own.length > 0 ? own : [BUNDLED_MONO];
  return [...head, SYMBOLS, ...(generic.length > 0 ? generic : ["monospace"])]
    .map((n) => (GENERIC.has(n) ? n : `"${n}"`))
    .join(", ");
}

/** Characters that make the browser load each face: text, and two icons (a Powerline
 *  separator U+E0B0 and a folder U+F07B) for the symbols font. */
const SAMPLE = "Mm\ue0b0\uf07b";

/** Load the bundled faces before the first terminal opens, so xterm measures the right cell
 *  and draws its atlas with them. Never blocks start-up for more than `timeoutMs`. */
export async function loadFonts(size = 16, timeoutMs = 3000): Promise<void> {
  if (typeof document === "undefined" || !document.fonts) return;
  const faces = [
    `${size}px "${BUNDLED_MONO}"`,
    `bold ${size}px "${BUNDLED_MONO}"`,
    `italic ${size}px "${BUNDLED_MONO}"`,
    `italic bold ${size}px "${BUNDLED_MONO}"`,
    `${size}px "${SYMBOLS}"`,
  ];
  const all = Promise.all(faces.map((f) => document.fonts.load(f, SAMPLE))).then(() => {});
  await Promise.race([all.catch(() => {}), new Promise<void>((r) => setTimeout(r, timeoutMs))]);
}
