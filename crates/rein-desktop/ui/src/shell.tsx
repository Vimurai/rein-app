// The window shell as React components (E-42, D-037): tabline, stage, statusline and command
// line — our own components on the cell grid, drawn from the pure `grid.ts` layout — and the
// terminal stage (xterm.js, E-35) over the stage rows. E-36 adds the modes: keys go through
// the pure `modes` function with the key table as input; NORMAL and COMMAND keys drive the
// app, TERMINAL keys go to the pty except the escape chord. Text goes in as React text nodes
// only, never as HTML.

import { useCallback, useEffect, useLayoutEffect, useRef, useState } from "react";
import type { ActionName } from "./actions.ts";
import { commonPrefix, complete, parse } from "./commands.ts";
import { cells, fit, layout, type Row, type ShellData } from "./grid.ts";
import { clipboardWrite, dirComplete, listen } from "./ipc.ts";
import { keyBytes, keyName } from "./keys.ts";
import type { ActionCall, KeyTable } from "./keymap.ts";
import { WHICH_KEY_DELAY_MS, continuations, step, type ModeState } from "./modes.ts";
import { WhichKey, label } from "./overlays.tsx";
import { TermTab } from "./term.ts";
import { THEME_NAMES, cssVariables, type Theme } from "./theme.ts";

/** What an action handler supplied by the app may do on the command line. */
export interface Ui {
  say: (text: string, error?: boolean) => void;
  showCommand: (cmd: string, note?: string) => void;
}

export type Handlers = Partial<Record<ActionName, (call: ActionCall, ui: Ui) => void>>;

export interface ShellProps {
  theme: Theme;
  keys: KeyTable;
  fontFamily: string;
  fontSize: number;
  lineHeight: number;
  /** Window opacity (0.6–1.0, config.toml); applies where the window is transparent. */
  opacity: number;
  /** Actions the app owns (config, theme — E-37); everything else the shell runs. */
  handlers: Handlers;
  /** A line to show on the command line (a config.toml reload or refusal); `seq` changes
   *  each time there is a new one. */
  notice: { text: string; error: boolean; seq: number };
  /** The DIR the window was started with (set by the core before any script ran). */
  startDir: string | null;
}

const INITIAL: ShellData = {
  tabs: [],
  version: "0.5.0",
  mode: "NORMAL",
  segments: [],
  clock: "",
  stageLines: ["no project open — :open DIR, or start the window with one: rein-app window DIR"],
  commandLine: "",
};

/** Actions whose backend arrives with a later task: they say so in one line (E-36). */
const LATER: Partial<Record<ActionName, string>> = {
  "project.pick": "E-38",
  "tab.next": "E-38",
  "tab.prev": "E-38",
  "tab.goto": "E-38",
  "app.quit": "E-38",
  "health.show": "E-39",
  "release.update": "E-39",
  "release.rollback": "E-39",
  "profiles.show": "E-39",
  "profiles.apply": "E-39",
  "menu.show": "E-40",
  "help.show": "E-40",
  "input.style": "E-40",
  "term.copy": "E-40",
  "term.paste": "E-40",
  "settings.show": "E-41",
  "font.bigger": "E-41",
  "font.smaller": "E-41",
  "font.reset": "E-41",
};

/** The CLI command an action stands for (rule 4: the UI teaches the CLI). */
const CLI: Partial<Record<ActionName, string>> = {
  "health.show": "rein-app doctor",
  "release.update": "rein-app update",
  "release.rollback": "rein-app rollback",
  "profiles.show": "rein-app profiles list",
};

function clockNow(): string {
  const now = new Date();
  return `${String(now.getHours()).padStart(2, "0")}:${String(now.getMinutes()).padStart(2, "0")}`;
}

/** A path as one shell word: quoted only when it must be. */
function shellWord(s: string): string {
  return /^[\w@%+=:,./-]+$/.test(s) ? s : `'${s.replaceAll("'", `'\\''`)}'`;
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

function useViewport(): { w: number; h: number } {
  const [size, setSize] = useState({ w: window.innerWidth, h: window.innerHeight });
  useEffect(() => {
    const o = new ResizeObserver(() => setSize({ w: window.innerWidth, h: window.innerHeight }));
    o.observe(document.documentElement);
    return () => o.disconnect();
  }, []);
  return size;
}

export function Shell(props: ShellProps) {
  const { theme, keys, fontFamily, fontSize, lineHeight, opacity, handlers, notice, startDir } = props;
  const [cell, setCell] = useState({ w: 0, h: 0 });
  const view = useViewport();
  const [data, setData] = useState<ShellData>(INITIAL);
  const [clock, setClock] = useState(clockNow);
  const [termShown, setTermShown] = useState(false);
  const [termOpen, setTermOpen] = useState(false);
  const [modeView, setModeView] = useState<ModeState>({ mode: "NORMAL", pending: [] });
  const [cmdText, setCmdText] = useState("");
  const [message, setMessage] = useState<{ text: string; error: boolean }>({ text: "", error: false });
  const [whichKey, setWhichKey] = useState(false);
  const termEl = useRef<HTMLDivElement>(null);
  const tab = useRef<TermTab | null>(null);
  const modeRef = useRef<ModeState>(modeView);
  const cmdRef = useRef("");
  const lastCommand = useRef<string | null>(null);
  const completion = useRef<{ base: string; items: string[]; index: number } | null>(null);
  /** A TERMINAL key the modes layer took (a chord key): its keypress is kept from xterm too. */
  const held = useRef(false);
  /** The latest TERMINAL key handler, so a tab opened earlier never runs a stale one. */
  const terminalKeyRef = useRef<(e: KeyboardEvent) => boolean>(() => true);

  useEffect(() => {
    const t = setInterval(() => setClock(clockNow()), 30_000);
    return () => clearInterval(t);
  }, []);

  const say = useCallback((text: string, error = false) => setMessage({ text, error }), []);

  // Theme and font (config.toml, `:theme`): the CSS variables every element reads, the cell
  // size of the font, and the open terminal — restyled before paint.
  useLayoutEffect(() => {
    const root = document.documentElement.style;
    for (const [k, v] of Object.entries(cssVariables(theme))) root.setProperty(k, v);
    root.setProperty("--font-family", fontFamily);
    root.setProperty("--font-size", `${fontSize}px`);
    root.setProperty("--line-height", String(lineHeight));
    root.setProperty("--opacity", String(opacity));
    const probe = document.getElementById("probe");
    const measure = () => {
      const r = probe?.getBoundingClientRect();
      if (r) setCell({ w: r.width / 10, h: r.height });
    };
    measure();
    void document.fonts.ready.then(measure);
    tab.current?.restyle({ fontFamily, fontSize, lineHeight, theme });
  }, [theme, fontFamily, fontSize, lineHeight, opacity]);

  useEffect(() => {
    if (notice.text) say(notice.text, notice.error);
  }, [notice, say]);

  /** Show the exact command an action ran or stands for; `y` copies it. */
  const showCommand = useCallback(
    (cmd: string, note = "") => {
      lastCommand.current = cmd;
      say(note ? `${cmd}   — ${note}` : cmd);
    },
    [say],
  );

  const setMode = useCallback((next: ModeState) => {
    const prev = modeRef.current.mode;
    modeRef.current = next;
    setModeView(next);
    if (next.mode !== prev) {
      if (next.mode === "TERMINAL") tab.current?.focus();
      else tab.current?.blur();
      if (next.mode === "COMMAND") {
        cmdRef.current = "";
        setCmdText("");
        completion.current = null;
      }
    }
  }, []);

  const setCmd = useCallback((text: string) => {
    cmdRef.current = text;
    setCmdText(text);
  }, []);

  /** Open `project` in the stage: `rein start` in a pty, drawn by xterm.js. */
  const open = useCallback(
    (project: string) => {
      if (tab.current) {
        say(`one project per window until tabs arrive (E-38) — not opening ${project}`, true);
        return;
      }
      const el = termEl.current;
      if (!el) return;
      setTermShown(true);
      const t = new TermTab(el, project, { fontFamily, fontSize, lineHeight, theme });
      tab.current = t;
      t.keyFilter = (e) => terminalKeyRef.current(e);
      t.onOpen = (opened) => {
        setTermOpen(true);
        setData((d) => ({ ...d, tabs: [{ name: opened.title, current: true }], segments: [opened.title] }));
        showCommand(`cd ${shellWord(opened.project)} && rein start`);
        setMode({ mode: "TERMINAL", pending: [] });
      };
      t.onError = (msg) => {
        void t.detach();
        tab.current = null;
        setTermShown(false);
        setTermOpen(false);
        setData((d) => ({ ...d, stageLines: [`cannot open ${project}`, msg] }));
        say(`E: ${msg}`, true);
      };
      void t.start();
    },
    [fontFamily, fontSize, lineHeight, theme, say, showCommand, setMode],
  );

  const detach = useCallback(() => {
    const t = tab.current;
    if (!t) {
      say("no tab is open");
      return;
    }
    tab.current = null;
    void t.detach();
    setTermShown(false);
    setTermOpen(false);
    setData((d) => ({ ...d, tabs: [], segments: [], stageLines: INITIAL.stageLines }));
    setMode({ mode: "NORMAL", pending: [] });
    say("tab closed — the tmux session keeps running");
  }, [say, setMode]);

  /** Run one action — from a key, the command line or (E-40) a click: the same closed list. */
  const run = useCallback(
    (call: ActionCall) => {
      const own = handlers[call.name];
      if (own) {
        own(call, { say, showCommand });
        return;
      }
      switch (call.name) {
        case "mode.normal":
          setMode({ mode: "NORMAL", pending: [] });
          return;
        case "mode.terminal":
          if (!tab.current) {
            say("no project open — :open DIR");
            setMode({ mode: "NORMAL", pending: [] });
          } else setMode({ mode: "TERMINAL", pending: [] });
          return;
        case "mode.command":
          setMode({ mode: "COMMAND", pending: [] });
          return;
        case "project.open":
          if (call.arg) open(call.arg);
          else {
            setMode({ mode: "COMMAND", pending: [] });
            setCmd("open ");
          }
          return;
        case "tab.detach":
          detach();
          return;
        case "cmd.yank": {
          const cmd = lastCommand.current;
          if (!cmd) {
            say("no command to copy yet");
            return;
          }
          clipboardWrite(cmd).then(
            () => say(`copied: ${cmd}`),
            (e: { message?: string }) => say(`E: cannot copy: ${e.message ?? String(e)}`, true),
          );
          return;
        }
        default: {
          const task = LATER[call.name] ?? "a later task";
          const cli = CLI[call.name];
          const text = `${label(call)} arrives in the window with ${task}`;
          if (cli) showCommand(cli, `${text}; in a terminal it works now`);
          else say(text);
        }
      }
    },
    [open, detach, say, setMode, setCmd, showCommand, handlers],
  );

  /** TERMINAL keys, seen by xterm first: hold a chord prefix, take the chord, pass the rest. */
  const terminalKey = (e: KeyboardEvent): boolean => {
    if (modeRef.current.mode !== "TERMINAL") return false;
    if (e.type !== "keydown") return !held.current;
    const k = keyName(e);
    if (!k) return true;
    const s = step(keys, modeRef.current, k);
    const consumed = s.action !== undefined || s.pending.length > 0;
    // Keys held earlier and now released go to the pty first, in order.
    const released = consumed ? s.forward : s.forward.slice(0, -1);
    const bytes = released.map((r) => keyBytes(r) ?? "").join("");
    if (bytes) tab.current?.send(bytes);
    held.current = consumed;
    modeRef.current = { mode: "TERMINAL", pending: s.action ? [] : s.pending };
    if (s.action) run(s.action);
    else setModeView(modeRef.current);
    if (consumed) e.preventDefault();
    return !consumed;
  };
  terminalKeyRef.current = terminalKey;

  const execute = useCallback(
    (line: string) => {
      setMode({ mode: "NORMAL", pending: [] });
      const p = parse(line);
      if ("error" in p) say(p.error, true);
      else if ("action" in p) run(p.action);
    },
    [run, say, setMode],
  );

  const tabComplete = useCallback(async () => {
    const c = completion.current;
    if (c && c.items.length > 1) {
      // Repeated Tab cycles through the candidates.
      c.index = (c.index + 1) % c.items.length;
      setCmd(c.items[c.index] ?? c.base);
      return;
    }
    const base = cmdRef.current;
    const items = await complete(base, {
      themes: THEME_NAMES,
      dirs: async (prefix) => (prefix.startsWith("/") ? dirComplete(prefix).catch(() => []) : []),
    });
    if (!items.length) return;
    const common = commonPrefix(items);
    completion.current = { base, items, index: items.length === 1 ? 0 : -1 };
    setCmd(items.length === 1 ? (items[0] ?? base) : common.length > base.length ? common : base);
    if (items.length > 1) say(items.map((i) => i.slice(i.lastIndexOf(" ") + 1)).join("  "));
  }, [say, setCmd]);

  // NORMAL and COMMAND keys: the window handles them before anything else sees them.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const k = keyName(e);
      if (!k) return;
      if (modeRef.current.mode === "TERMINAL") {
        // A TERMINAL key that missed the terminal (focus was on the page): give the terminal
        // focus and pass the key through the same chord logic, so none is lost.
        const inTerm = e.target instanceof Node && termEl.current?.contains(e.target);
        if (!inTerm && tab.current) {
          e.preventDefault();
          tab.current.focus();
          if (terminalKeyRef.current(e)) {
            const bytes = keyBytes(k) ?? (k === "Enter" ? "\r" : k === "Tab" ? "\t" : k === "BS" ? "\x7f" : k === "Esc" ? "\x1b" : "");
            if (bytes) tab.current.send(bytes);
          }
        }
        return;
      }
      e.preventDefault();
      e.stopPropagation();
      const s = step(keys, modeRef.current, k);
      if (s.edit !== undefined) {
        const t = cmdRef.current;
        if (s.edit === "Enter") execute(t);
        else if (s.edit === "Tab") void tabComplete();
        else if (s.edit === "BS") {
          if (t === "") setMode({ mode: "NORMAL", pending: [] });
          else setCmd(t.slice(0, -1));
        } else if (s.edit === "Space") setCmd(`${t} `);
        else if (s.edit.length === 1) setCmd(t + s.edit);
        if (s.edit !== "Tab") completion.current = null;
        return;
      }
      // A completed action changes the mode through `run` (it knows, e.g., that TERMINAL
      // needs an open tab); otherwise only the pending keys change.
      modeRef.current = { mode: modeRef.current.mode, pending: s.action ? [] : s.pending };
      if (s.action) run(s.action);
      else setModeView(modeRef.current);
    };
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  }, [keys, run, execute, tabComplete, setMode, setCmd]);

  // TERMINAL: the terminal takes the keyboard whenever the window does.
  useEffect(() => {
    const onFocus = () => {
      if (modeRef.current.mode === "TERMINAL") tab.current?.focus();
    };
    window.addEventListener("focus", onFocus);
    return () => window.removeEventListener("focus", onFocus);
  }, []);

  // The which-key overlay appears 400 ms after the leader, while it is still pending.
  useEffect(() => {
    setWhichKey(false);
    if (modeView.mode !== "NORMAL" || modeView.pending[0] !== keys.leader) return;
    const t = setTimeout(() => setWhichKey(true), WHICH_KEY_DELAY_MS);
    return () => clearTimeout(t);
  }, [modeView, keys.leader]);

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
  const shell = layout(
    { ...data, mode: modeView.mode, clock, commandLine: "", stageLines: termOpen ? [] : data.stageLines },
    cols,
    rows,
  );
  const stage = termOpen ? shell.stage.map(() => [{ text: " ".repeat(cols), cls: "" }]) : shell.stage;
  const commandRow: Row =
    modeView.mode === "COMMAND"
      ? fit([{ text: `:${cmdText}`, cls: "" }, { text: " ", cls: "cursor" }], cols)
      : fit([{ text: message.text, cls: message.error ? "error" : "" }], cols);

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
        <GridRow row={commandRow} />
      </div>
      <div id="term" ref={termEl} className={termShown ? undefined : "hidden"} />
      {whichKey && <WhichKey leader={keys.leader} next={continuations(keys, modeView.pending)} />}
    </>
  );
}
