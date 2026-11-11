// Entry point: measure the font's cell, draw the shell on the grid, redraw on resize.
// Text goes in through `textContent` only — never HTML.

import { cells, layout, type Row, type ShellData } from "./grid.ts";
import { listen } from "./ipc.ts";
import { GRUVBOX_DARK, cssVariables } from "./theme.ts";

const FONT_FAMILY = `"JetBrains Mono", "DejaVu Sans Mono", Menlo, monospace`;
const FONT_SIZE_PX = 14;
const LINE_HEIGHT = 1.25;

/** Static chrome until the services arrive (E-35..E-39). */
const data: ShellData = {
  tabs: [{ name: "rein-app", current: true }],
  version: "0.5.0",
  mode: null,
  segments: ["rein-app", "master", "✓ install ok", "2 profiles"],
  clock: "",
  stageLines: ["the terminal opens here — `rein start` in the project (E-35)"],
  commandLine: "",
};

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
  if (!grid) return;
  const { w, h } = cellSize();
  document.documentElement.style.setProperty("--cell-h", `${h}px`);
  const now = new Date();
  data.clock = `${String(now.getHours()).padStart(2, "0")}:${String(now.getMinutes()).padStart(2, "0")}`;
  const shell = layout(data, cells(window.innerWidth, w), cells(window.innerHeight, h));
  const tabline = rowElement(shell.tabline);
  tabline.classList.add("bar");
  const status = rowElement(shell.statusline);
  status.classList.add("bar");
  grid.replaceChildren(tabline, ...shell.stage.map(rowElement), status, rowElement(shell.commandLine));
}

// The DIR `rein-desktop DIR` was started with, set by the core before this script ran.
const startDir = (globalThis as { __REIN_START_DIR__?: string | null }).__REIN_START_DIR__;
if (typeof startDir === "string") data.commandLine = `open: ${startDir}`;

applyTheme();
await document.fonts.ready;
draw();
window.addEventListener("resize", draw);
setInterval(draw, 30_000);

// A DIR handed over by a second start. Tabs arrive in E-38; until then
// the command line shows the request. The core sent it only after `project_path::check`.
await listen<string>("open_dir", (dir) => {
  data.commandLine = `open: ${dir}`;
  draw();
});
