// The cell grid (desktop.md §Core Concept rule 1, "Screen"): every row is exactly `cols`
// cells; borders are box-drawing glyphs. Pure functions — no DOM — so they test under Node.

/** A run of cells with one style class ("" for the row's default). */
export interface Span {
  text: string;
  cls: string;
}
export type Row = Span[];

export interface Tab {
  name: string;
  current: boolean;
}

/** What the chrome shows. Static in E-34; derived from the core from E-35 on. */
export interface ShellData {
  tabs: Tab[];
  version: string;
  /** `null` in the `simple` input style: no mode segment (D-034). */
  mode: string | null;
  segments: string[];
  clock: string;
  stageLines: string[];
  commandLine: string;
}

export interface Shell {
  tabline: Row;
  stage: Row[];
  statusline: Row;
  commandLine: Row;
}

/** Cells a string occupies. Every glyph the chrome draws is one cell wide. */
export function width(s: string): number {
  return [...s].length;
}

/** `s` cut to `n` cells. */
export function clip(s: string, n: number): string {
  return [...s].slice(0, Math.max(0, n)).join("");
}

export function rowWidth(r: Row): number {
  return r.reduce((n, s) => n + width(s.text), 0);
}

/** Clip `r` to `cols` cells, then pad it with `fill` spans to exactly `cols`. */
export function fit(r: Row, cols: number, fillCls = ""): Row {
  const out: Row = [];
  let left = cols;
  for (const s of r) {
    if (left <= 0) break;
    const t = clip(s.text, left);
    out.push({ text: t, cls: s.cls });
    left -= width(t);
  }
  if (left > 0) out.push({ text: " ".repeat(left), cls: fillCls });
  return out;
}

/** `left`, a gap, then `right` flush to the edge; `right` wins when space runs out. */
export function split(left: Row, right: Row, cols: number, fillCls = ""): Row {
  const rw = rowWidth(right);
  if (rw >= cols) return fit(right, cols, fillCls);
  return [...fit(left, cols - rw, fillCls), ...right];
}

/** Tabline: ` 1:name* ×  2:name ×  [+]   …   rein 0.5.0  [≡] ` (D-034 leaves room for ×, [+], [≡]). */
export function tabline(d: ShellData, cols: number): Row {
  const left: Row = [];
  d.tabs.forEach((t, i) => {
    const cls = t.current ? "tab-current" : "";
    left.push({ text: ` ${i + 1}:${t.name}${t.current ? "*" : ""} `, cls });
    left.push({ text: "×", cls: t.current ? "tab-current button" : "button" });
    left.push({ text: " ", cls });
  });
  left.push({ text: "  ", cls: "" }, { text: "[+]", cls: "button" });
  const menu: Row = [
    { text: "[≡]", cls: "button" },
    { text: " ", cls: "" },
  ];
  const version: Row = [
    { text: `rein ${d.version}`, cls: "dim" },
    { text: "  ", cls: "" },
  ];
  // The menu is the newcomer's way in (D-034): on a narrow window the version goes first.
  const right = rowWidth(version) + rowWidth(menu) < cols ? [...version, ...menu] : menu;
  return split(left, right, cols);
}

/** Stage: a box-drawn frame with `lines` centred inside; `rows` may be < 2 on tiny windows. */
export function stage(lines: string[], cols: number, rows: number): Row[] {
  if (rows <= 0) return [];
  if (rows < 2 || cols < 2) return Array.from({ length: rows }, () => fit([], cols));
  const inner = cols - 2;
  const top = [{ text: `┌${"─".repeat(inner)}┐`, cls: "frame" }];
  const bottom = [{ text: `└${"─".repeat(inner)}┘`, cls: "frame" }];
  const body = rows - 2;
  const first = Math.max(0, Math.floor((body - lines.length) / 2));
  const out: Row[] = [top];
  for (let i = 0; i < body; i++) {
    const text = clip(lines[i - first] ?? "", inner);
    const pad = inner - width(text);
    const l = Math.floor(pad / 2);
    out.push([
      { text: "│", cls: "frame v" },
      { text: " ".repeat(l), cls: "" },
      { text, cls: "dim" },
      { text: " ".repeat(pad - l), cls: "" },
      { text: "│", cls: "frame v" },
    ]);
  }
  out.push(bottom);
  return out;
}

/** Statusline: `[ MODE ] seg │ seg │ seg` … `clock`. */
export function statusline(d: ShellData, cols: number): Row {
  const left: Row = [];
  if (d.mode !== null) left.push({ text: ` ${d.mode} `, cls: `mode-${d.mode.toLowerCase()}` });
  d.segments.forEach((s, i) => {
    if (i > 0 || d.mode !== null) left.push({ text: " │", cls: "bar dim" });
    left.push({ text: ` ${s}`, cls: "bar" });
  });
  return split(left, [{ text: ` ${d.clock} `, cls: "bar" }], cols, "bar");
}

/** The whole shell for a `cols` × `rows` grid: tabline, stage, statusline, command line. */
export function layout(d: ShellData, cols: number, rows: number): Shell {
  const c = Math.max(0, cols);
  const stageRows = Math.max(0, rows - 3);
  return {
    tabline: fit(tabline(d, c), c, "bar"),
    stage: stage(d.stageLines, c, stageRows),
    statusline: statusline(d, c),
    commandLine: fit([{ text: d.commandLine, cls: "" }], c),
  };
}

/** Whole cells that fit `px` pixels of `cell` pixels each (at least 0). */
export function cells(px: number, cell: number): number {
  return cell > 0 ? Math.max(0, Math.floor(px / cell)) : 0;
}
