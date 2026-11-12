// The window shell as React components (E-42, D-037): tabline, stage, statusline and command
// line — our own components on the cell grid, drawn from the pure `grid.ts` layout so they look
// exactly as E-34's — and the terminal stage (xterm.js, E-35) over the stage rows. Text goes in
// as React text nodes only, never as HTML.

import { useCallback, useEffect, useRef, useState } from "react";
import { cells, layout, type Row, type ShellData } from "./grid.ts";
import { listen } from "./ipc.ts";
import { TermTab } from "./term.ts";
import type { Theme } from "./theme.ts";

export interface ShellProps {
  theme: Theme;
  fontFamily: string;
  fontSize: number;
  lineHeight: number;
  /** The DIR the window was started with (set by the core before any script ran). */
  startDir: string | null;
}

const INITIAL: ShellData = {
  tabs: [],
  version: "0.5.0",
  mode: null,
  segments: [],
  clock: "",
  stageLines: ["no project open — start the window with a project: rein-app window DIR"],
  commandLine: "",
};

function clockNow(): string {
  const now = new Date();
  return `${String(now.getHours()).padStart(2, "0")}:${String(now.getMinutes()).padStart(2, "0")}`;
}

/** One grid row: spans of text with their style class. */
export function GridRow({ row, className = "" }: { row: Row; className?: string }) {
  return (
    <div className={`row ${className}`.trim()}>
      {row.map((s, i) => (
        <span key={i} className={s.cls || undefined}>
          {s.text}
        </span>
      ))}
    </div>
  );
}

/** The cell size of the configured font, measured from the hidden probe. */
function useCell(): { w: number; h: number } {
  const [cell, setCell] = useState({ w: 0, h: 0 });
  useEffect(() => {
    const probe = document.getElementById("probe");
    if (!probe) return;
    const measure = () => {
      const r = probe.getBoundingClientRect();
      setCell({ w: r.width / 10, h: r.height });
    };
    measure();
    void document.fonts.ready.then(measure);
  }, []);
  return cell;
}

function useViewport(): { w: number; h: number } {
  const [size, setSize] = useState({ w: window.innerWidth, h: window.innerHeight });
  useEffect(() => {
    const o = new ResizeObserver(() => setSize({ w: window.innerWidth, h: window.innerHeight }));
    o.observe(document.documentElement);
    return () => o.disconnect();
  }, []);
  return size;
}

export function Shell({ theme, fontFamily, fontSize, lineHeight, startDir }: ShellProps) {
  const cell = useCell();
  const view = useViewport();
  const [data, setData] = useState<ShellData>(INITIAL);
  const [clock, setClock] = useState(clockNow);
  const [termShown, setTermShown] = useState(false);
  const [termOpen, setTermOpen] = useState(false);
  const termEl = useRef<HTMLDivElement>(null);
  const tab = useRef<TermTab | null>(null);

  useEffect(() => {
    const t = setInterval(() => setClock(clockNow()), 30_000);
    return () => clearInterval(t);
  }, []);

  const say = useCallback((line: string) => setData((d) => ({ ...d, commandLine: line })), []);

  /** Open `project` in the stage: `rein start` in a pty, drawn by xterm.js. */
  const open = useCallback(
    (project: string) => {
      if (tab.current) {
        say(`one project per window until tabs arrive (E-38) — not opening ${project}`);
        return;
      }
      const el = termEl.current;
      if (!el) return;
      setTermShown(true);
      const t = new TermTab(el, project, { fontFamily, fontSize, lineHeight, theme });
      tab.current = t;
      t.onOpen = (opened) => {
        setTermOpen(true);
        setData((d) => ({
          ...d,
          tabs: [{ name: opened.title, current: true }],
          segments: [opened.title],
          commandLine: "",
        }));
      };
      t.onError = (message) => {
        void t.detach();
        tab.current = null;
        setTermShown(false);
        setTermOpen(false);
        setData((d) => ({ ...d, stageLines: [`cannot open ${project}`, message], commandLine: `E: ${message}` }));
      };
      void t.start();
    },
    [fontFamily, fontSize, lineHeight, theme, say],
  );

  // Core events: a child exited; a second start handed over a DIR.
  useEffect(() => {
    void listen<{ id: number; code: number }>("pty_exit", ({ id, code }) => {
      if (tab.current?.tab?.pty === id) tab.current.exited(code);
    });
    void listen<string>("open_dir", open);
    if (typeof startDir === "string") open(startDir);
    // Listeners live as long as the window; `open` is stable for a given font and theme.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const cols = cells(view.w, cell.w);
  const rows = cells(view.h, cell.h);
  const shell = layout({ ...data, clock, stageLines: termOpen ? [] : data.stageLines }, cols, rows);
  const stage = termOpen ? shell.stage.map(() => [{ text: " ".repeat(cols), cls: "" }]) : shell.stage;

  // The terminal sits over the stage rows, sized in whole cells (style.css reads these).
  useEffect(() => {
    const root = document.documentElement.style;
    root.setProperty("--cell-h", `${cell.h}px`);
    root.setProperty("--stage-top", `${cell.h}px`);
    root.setProperty("--stage-w", `${cols * cell.w}px`);
    root.setProperty("--stage-h", `${stage.length * cell.h}px`);
    tab.current?.fit();
  }, [cell, cols, stage.length]);

  return (
    <>
      <div id="grid" role="application" aria-label="rein-app">
        <GridRow row={shell.tabline} className="bar" />
        {stage.map((r, i) => (
          <GridRow key={i} row={r} />
        ))}
        <GridRow row={shell.statusline} className="bar" />
        <GridRow row={shell.commandLine} />
      </div>
      <div id="term" ref={termEl} className={termShown ? undefined : "hidden"} />
    </>
  );
}
