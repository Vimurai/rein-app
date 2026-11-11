// Entry point: measure the font's cell, draw the shell on the grid, put the terminal over
// the stage rows, redraw on resize. Text goes in through `textContent` only — never HTML.

import "@xterm/xterm/css/xterm.css";
import "./style.css";
import { cells, layout, type Row, type ShellData } from "./grid.ts";
import { listen } from "./ipc.ts";
import { TermTab } from "./term.ts";
import { GRUVBOX_DARK, cssVariables } from "./theme.ts";
import { hardenPrototypes } from "./harden.ts";

// Windows has neither JetBrains Mono nor DejaVu by default (desktop.md §Windows window W6).
const FONT_FAMILY = navigator.userAgent.includes("Windows")
  ? `"Cascadia Mono", Consolas, monospace`
  : `"JetBrains Mono", "DejaVu Sans Mono", Menlo, monospace`;
const FONT_SIZE_PX = 14;
const LINE_HEIGHT = 1.25;

/** Chrome data. Health, profiles and the version arrive with E-39; tabs with E-38. */
const data: ShellData = {
  tabs: [],
  version: "0.5.0",
  mode: null,
  segments: [],
  clock: "",
  stageLines: ["no project open — start the window with a project: rein-app window DIR"],
  commandLine: "",
};

let tab: TermTab | null = null;

function applyTheme(): void {
  const root = document.documentElement.style;
  for (const [k, v] of Object.entries(cssVariables(GRUVBOX_DARK))) root.setProperty(k, v);
  root.setProperty("--font-family", FONT_FAMILY);
  root.setProperty("--font-size", `${FONT_SIZE_PX}px`);
  root.setProperty("--line-height", String(LINE_HEIGHT));
}

function cellSize(): { w: number; h: number } {
  const probe = document.getElementById("probe");
  if (!probe) return { w: 0, h: 0 };
  const r = probe.getBoundingClientRect();
  return { w: r.width / 10, h: r.height };
}

function rowElement(r: Row): HTMLDivElement {
  const div = document.createElement("div");
  div.className = "row";
  for (const s of r) {
    const span = document.createElement("span");
    if (s.cls) span.className = s.cls;
    span.textContent = s.text;
    div.append(span);
  }
  return div;
}

function draw(): void {
  const grid = document.getElementById("grid");
  const termEl = document.getElementById("term");
  if (!grid || !termEl) return;
  const { w, h } = cellSize();
  const root = document.documentElement.style;
  root.setProperty("--cell-h", `${h}px`);
  const now = new Date();
  data.clock = `${String(now.getHours()).padStart(2, "0")}:${String(now.getMinutes()).padStart(2, "0")}`;
  const cols = cells(window.innerWidth, w);
  const rows = cells(window.innerHeight, h);
  // With a terminal open the stage rows are left blank and the terminal covers them.
  const shell = layout({ ...data, stageLines: tab?.tab ? [] : data.stageLines }, cols, rows);
  if (tab?.tab) shell.stage = shell.stage.map(() => [{ text: " ".repeat(cols), cls: "" }]);
  const tabline = rowElement(shell.tabline);
  tabline.classList.add("bar");
  const status = rowElement(shell.statusline);
  status.classList.add("bar");
  grid.replaceChildren(tabline, ...shell.stage.map(rowElement), status, rowElement(shell.commandLine));

  root.setProperty("--stage-top", `${h}px`);
  root.setProperty("--stage-w", `${cols * w}px`);
  root.setProperty("--stage-h", `${shell.stage.length * h}px`);
  termEl.classList.toggle("hidden", !tab);
  tab?.fit();
}

function say(line: string): void {
  data.commandLine = line;
  draw();
}

/** Open `project` in the stage: `rein start` in a pty, drawn by xterm.js. */
function open(project: string): void {
  if (tab) {
    say(`one project per window until tabs arrive (E-38) — not opening ${project}`);
    return;
  }
  const termEl = document.getElementById("term");
  if (!termEl) return;
  termEl.classList.remove("hidden");
  draw();
  const t = new TermTab(termEl, project, {
    fontFamily: FONT_FAMILY,
    fontSize: FONT_SIZE_PX,
    lineHeight: LINE_HEIGHT,
    theme: GRUVBOX_DARK,
  });
  tab = t;
  t.onOpen = (opened) => {
    data.tabs = [{ name: opened.title, current: true }];
    data.segments = [opened.title];
    say("");
  };
  t.onError = (message) => {
    t.detach();
    tab = null;
    data.stageLines = [`cannot open ${project}`, message];
    say(`E: ${message}`);
  };
  void t.start();
}

// Every import above has run; nothing untrusted has been handled yet.
hardenPrototypes();
applyTheme();
await document.fonts.ready;
draw();
new ResizeObserver(draw).observe(document.documentElement);
setInterval(draw, 30_000);

await listen<{ id: number; code: number }>("pty_exit", ({ id, code }) => {
  if (tab?.tab?.pty === id) tab.exited(code);
});
// A DIR handed over by a second start (the core validated it with `project_path::check`).
await listen<string>("open_dir", open);

// The DIR `rein-desktop DIR` was started with, set by the core before this script ran.
const startDir = (globalThis as { __REIN_START_DIR__?: string | null }).__REIN_START_DIR__;
if (typeof startDir === "string") open(startDir);
