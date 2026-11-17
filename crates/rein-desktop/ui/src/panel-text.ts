// Readable panels (status.md §Components 7, D-063; E-74): the pure half. Text from a
// project's `.ai/` files becomes a closed set of blocks — paragraphs, bullet and numbered
// lists, heading lines, fenced code — with inline `code` and **bold**. Nothing here makes a
// link, an image or HTML: a URL, a Markdown link or a tag stays literal text, and an unclosed
// marker is literal. Every pass is linear in the input (the core has already capped it).

export type Inline = { kind: "text" | "code" | "bold"; text: string };
export type Block =
  | { kind: "p"; inl: Inline[] }
  | { kind: "heading"; inl: Inline[] }
  | { kind: "ul"; items: Inline[][] }
  | { kind: "ol"; start: number; items: Inline[][] }
  | { kind: "pre"; text: string };

/** `code` spans and **bold** runs; anything unclosed stays as written. Linear: once a marker
 *  has no later partner, it is never searched for again. */
export function inline(s: string): Inline[] {
  const out: Inline[] = [];
  let text = "";
  let tickGone = false;
  let starsGone = false;
  const flush = () => {
    if (text) out.push({ kind: "text", text });
    text = "";
  };
  let i = 0;
  while (i < s.length) {
    if (s[i] === "`" && !tickGone) {
      const j = s.indexOf("`", i + 1);
      if (j < 0) tickGone = true;
      else if (j > i + 1) {
        flush();
        out.push({ kind: "code", text: s.slice(i + 1, j) });
        i = j + 1;
        continue;
      }
    } else if (s.startsWith("**", i) && !starsGone) {
      const j = s.indexOf("**", i + 2);
      if (j < 0) starsGone = true;
      else if (j > i + 2) {
        flush();
        out.push({ kind: "bold", text: s.slice(i + 2, j) });
        i = j + 2;
        continue;
      }
    }
    text += s[i];
    i += 1;
  }
  flush();
  return out;
}

const FENCE = /^\s*```/;
const HEADING = /^#{1,6}\s+(.*)$/;
const BULLET = /^\s*[-*+]\s+(.*)$/;
const NUMBERED = /^\s*(\d{1,4})[.)]\s+(.*)$/;

/** The blocks of `text`. A fence without its closing line is a line of text. */
export function blocks(text: string): Block[] {
  const lines = text.replace(/\r\n?/g, "\n").split("\n");
  // Where each fence closes, found in one backward pass (so an unclosed one costs nothing).
  const closes = new Array<number>(lines.length).fill(-1);
  let next = -1;
  for (let k = lines.length - 1; k >= 0; k--) {
    if (FENCE.test(lines[k] as string)) {
      closes[k] = next;
      next = k;
    }
  }
  const out: Block[] = [];
  let para: string[] = [];
  let list: { kind: "ul" | "ol"; start: number; items: string[] } | null = null;
  const endPara = () => {
    if (para.length) out.push({ kind: "p", inl: inline(para.join(" ")) });
    para = [];
  };
  const endList = () => {
    if (list) {
      const items = list.items.map(inline);
      out.push(list.kind === "ul" ? { kind: "ul", items } : { kind: "ol", start: list.start, items });
    }
    list = null;
  };
  let i = 0;
  while (i < lines.length) {
    const line = lines[i] as string;
    const close = FENCE.test(line) ? (closes[i] as number) : -1;
    if (close > i) {
      endPara();
      endList();
      out.push({ kind: "pre", text: lines.slice(i + 1, close).join("\n") });
      i = close + 1;
      continue;
    }
    if (line.trim() === "") {
      endPara();
      endList();
    } else if (HEADING.test(line)) {
      endPara();
      endList();
      out.push({ kind: "heading", inl: inline((HEADING.exec(line) as RegExpExecArray)[1] as string) });
    } else if (BULLET.test(line) || NUMBERED.test(line)) {
      endPara();
      const b = BULLET.exec(line);
      const kind = b ? "ul" : "ol";
      const m = b ?? (NUMBERED.exec(line) as RegExpExecArray);
      const item = (b ? m[1] : m[2]) as string;
      if (!list || list.kind !== kind) {
        endList();
        list = { kind, start: b ? 1 : Number(m[1]), items: [] };
      }
      list.items.push(item);
    } else if (list && /^\s+\S/.test(line)) {
      // An indented line under an item continues it.
      list.items[list.items.length - 1] += ` ${line.trim()}`;
    } else {
      endList();
      para.push(line.trim());
    }
    i += 1;
  }
  endPara();
  endList();
  return out;
}

/** The first `n` lines of `text` (a Communication entry's "Next action needed"), and whether
 *  any were left out. */
export function firstLines(text: string, n: number): { text: string; more: boolean } {
  const lines = text.replace(/\r\n?/g, "\n").split("\n");
  while (lines.length && (lines[lines.length - 1] as string).trim() === "") lines.pop();
  return { text: lines.slice(0, n).join("\n"), more: lines.length > n };
}

export const TITLE_MAX = 90;

/** A task's title: its description up to the first sentence end or line break, at most 90
 *  characters, cut at a word. */
export function taskTitle(description: string): string {
  const line = (description.split("\n")[0] ?? "").trim();
  const end = /[.!?](?=\s|$)/.exec(line);
  const t = end ? line.slice(0, end.index + 1) : line;
  if (t.length <= TITLE_MAX) return t;
  const room = t.slice(0, TITLE_MAX - 1);
  const space = room.lastIndexOf(" ");
  return `${(space >= TITLE_MAX / 2 ? room.slice(0, space) : room).replace(/[\s,;:(—-]+$/, "")}…`;
}

/** The rows a filter keeps: the query in the id or the description, case ignored. */
export function filterTasks<T extends { id: string; description: string }>(rows: readonly T[], query: string): T[] {
  const q = query.trim().toLowerCase();
  if (!q) return [...rows];
  return rows.filter((t) => t.id.toLowerCase().includes(q) || t.description.toLowerCase().includes(q));
}

/** The reader's Up / Down: the neighbouring item, stopping at either end. */
export function step(index: number, count: number, by: -1 | 1): number {
  return Math.min(Math.max(0, index + by), Math.max(0, count - 1));
}

/** The reader's keys: Up / Down always, `k` / `j` in the modal input style. */
export function readerKey(key: string, modal: boolean): "prev" | "next" | "close" | null {
  if (key === "Escape") return "close";
  if (key === "ArrowUp" || (modal && key === "k")) return "prev";
  if (key === "ArrowDown" || (modal && key === "j")) return "next";
  return null;
}
