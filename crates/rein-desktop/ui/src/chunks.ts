// Pure helpers for the pty stream (no DOM, tested under Node).

/** The core refuses a `pty_write` over 64 KiB (desktop.md §API). */
export const MAX_WRITE_BYTES = 64 * 1024;
/** UTF-16 units per piece: at most 3 UTF-8 bytes per unit (4 per surrogate pair). */
const PIECE_UNITS = Math.floor(MAX_WRITE_BYTES / 3);

/** Split `s` into pieces that each encode to at most 64 KiB, never inside a surrogate pair. */
export function splitInput(s: string): string[] {
  const out: string[] = [];
  let i = 0;
  while (i < s.length) {
    let end = Math.min(s.length, i + PIECE_UNITS);
    const last = s.charCodeAt(end - 1);
    if (end < s.length && last >= 0xd800 && last <= 0xdbff) end -= 1; // keep the pair whole
    out.push(s.slice(i, end));
    i = end;
  }
  return out;
}

/** Join queued output chunks into one buffer for a single terminal write. */
export function concat(chunks: Uint8Array[]): Uint8Array {
  const n = chunks.reduce((a, c) => a + c.length, 0);
  const out = new Uint8Array(n);
  let at = 0;
  for (const c of chunks) {
    out.set(c, at);
    at += c.length;
  }
  return out;
}
