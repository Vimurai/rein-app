// The window shell (E-42, D-037; frame rebuilt in E-49 per desktop.md §Look and feel v2 V2):
// the title bar / tab bar, the borderless terminal stage (xterm.js, E-35), the status bar,
// the toast and — in the modal style — the command palette. The frame is drawn by `frame.tsx`
// from the pure `chrome.ts`; the start screen (no tab) by `start-screen.tsx` (E-50).
// E-36 adds the modes: keys go through
// the pure `modes` function with the key table as input; NORMAL and COMMAND keys drive the
// app, TERMINAL keys go to the pty except the escape chord. Text goes in as React text nodes
// only, never as HTML.

import { useCallback, useEffect, useLayoutEffect, useReducer, useRef, useState } from "react";
import {
  ContextMenu,
  ContextMenuContent,
  ContextMenuItem,
  ContextMenuSeparator,
  ContextMenuShortcut,
  ContextMenuTrigger,
} from "../components/context-menu.tsx";
import { DEFAULT_MODAL, DEFAULT_SIMPLE } from "./keymap.ts";
import { describe as describeAction, menuEntries } from "./menu.ts";
import { pasteNeedsQuestion } from "./paste.ts";
import type { ActionName } from "./actions.ts";
import { commonPrefix, complete, parse } from "./commands.ts";
import { COPIED_MS, TOAST_MS, chromeOf, overlayNotice, statusBar, titleBar } from "./chrome.ts";
import { ProgressLine } from "../components/loaders.tsx";
import { Exit } from "../components/dialog.tsx";
import { MOTION } from "./motion.ts";
import { CommandPalette, StatusBar, TitleBar, Toast } from "./frame.tsx";
import {
  clipboardRead,
  clipboardWrite,
  projectChoose,
  dirComplete,
  healthGet,
  tabTmux,
  listen,
  appQuit,
  projectAgents,
  projectCheck,
  projectClose,
  quitCancelled,
  tabClose,
  projectsForget,
  projectsList,
  type HealthView,
} from "./ipc.ts";
import { HealthOverlay, ProfilesOverlay, ReleaseOverlay } from "./release.tsx";
import { keyBytes, keyName } from "./keys.ts";
import type { ActionCall, KeyTable } from "./keymap.ts";
import { WHICH_KEY_DELAY_MS, continuations, step, type ModeState } from "./modes.ts";
import { MenuOverlay, PasteConfirm, ProjectPicker, WhichKey, label } from "./overlays.tsx";
import { startScreen, withStartKeys, type RecentView } from "./start.ts";
import { StartScreen } from "./start-screen.tsx";
import { TermTab } from "./term.ts";
import { POLL_MS, agentBar, shouldPoll, type Agent } from "./agents.ts";
import { AgentBar, CloseConfirm, StopConfirm } from "./agent-bar.tsx";
import { closeTabText, closedNote, quitText } from "./lifecycle.ts";
import { shouldCheck, type CheckReply } from "./project.ts";
import { SetupCard, UpdateBar, UpdateSheet } from "./project-ui.tsx";
import { THEME_NAMES, cssVariables, tmuxColours, type Theme } from "./theme.ts";

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
  /** The tmux overlay settings (V6) as one key: a change re-applies the overlay to every
   *  open tab, as a theme change does. */
  tmuxKey: string;
  /** Actions the app owns (config, theme — E-37); everything else the shell runs. */
  handlers: Handlers;
  /** A line to show on the command line (a config.toml reload or refusal); `seq` changes
   *  each time there is a new one. */
  notice: { text: string; error: boolean; seq: number };
  /** `window.start = "last"` (config.toml): reopen the most recent project at start. */
  startLast: boolean;
  /** The DIR the window was started with (set by the core before any script ran). */
  startDir: string | null;
  /** V9 component 5: every agent exited — close the project without asking. */
  closeWhenAgentsExit: boolean;
  /** V9 component 3: projects whose out-of-date bar is not shown, and how to add one. */
  dontAskUpdate: readonly string[];
  onDontAskUpdate: (project: string) => void;
  /** V11: closing a project or quitting does not ask; and how to set it. */
  closeDontAsk: boolean;
  onCloseDontAsk: () => void;
}

const VERSION = "0.5.0";

/** The frame the core asked for (V2): set before any script ran. */
const CHROME = chromeOf((globalThis as { __REIN_CHROME__?: unknown }).__REIN_CHROME__);

/** Actions whose backend arrives with a later task: they say so in one line (E-36). */
const LATER: Partial<Record<ActionName, string>> = {
};

/** The CLI command an action stands for (rule 4: the UI teaches the CLI). */
const CLI: Partial<Record<ActionName, string>> = {};

/** A path as one shell word: quoted only when it must be. */
function shellWord(s: string): string {
  return /^[\w@%+=:,./-]+$/.test(s) ? s : `'${s.replaceAll("'", `'\\''`)}'`;
}

/** At most 16 tabs (desktop.md §Execution Constraints); the core enforces it too. */
const MAX_TABS = 16;

/** One open tab: its terminal, its pane, and what the chrome shows for it. */
interface OpenTab {
  /** For the tab's life (the title bar keys its pill by it). */
  id: number;
  term: TermTab;
  el: HTMLDivElement;
  /** As asked for (`:open`, a handoff), and canonical once open. */
  requested: string;
  project: string;
  title: string;
  open: boolean;
  /** `rein start` exited (Enter restarts it): a dot on the tab. */
  exited: boolean;
  /** A failed tmux overlay was told in a toast already (V6: once per tab). */
  overlayTold?: boolean;
  /** V9 (E-62): the last answer about the project's agents. */
  agents?: Agent[];
  /** The ended roles the operator chose Keep open for. */
  kept?: string | null;
  /** Close project is running. */
  closing?: boolean;
  /** Closing by itself failed once: from then on the bar asks instead. */
  autoFailed?: boolean;
  /** V9 (E-63): a folder without `.ai/` — the set-up card, no terminal yet. */
  setup?: { id: number; reply: CheckReply | null; inside: string | null } | undefined;
  /** The out-of-date check ran for this tab (once per open). */
  asked?: boolean;
  /** rein's rows when it reported the project out of date; the bar shows while set. */
  update?: string[] | undefined;
  /** Not now / Don't ask: the bar is hidden for this tab. */
  updateHidden?: boolean;
}

export function Shell(props: ShellProps) {
  const { theme, keys: projectKeys, fontFamily, fontSize, lineHeight, opacity, tmuxKey, handlers, notice, startDir, startLast, closeWhenAgentsExit, dontAskUpdate, onDontAskUpdate, closeDontAsk, onCloseDontAsk } =
    props;
  /** V11: a stop confirm — closing one project's tab, or quitting with `n` projects. */
  const [stop, setStop] = useState<{ kind: "tab"; entry: OpenTab; roles: string[] } | { kind: "quit"; n: number } | null>(null);
  const closeDontAskRef = useRef(closeDontAsk);
  closeDontAskRef.current = closeDontAsk;
  /** V9 3: Update project, running in its sheet for this tab. */
  const [updateSheet, setUpdateSheet] = useState<{ entry: OpenTab; id: number; rows: string[] } | null>(null);
  const dontAskRef = useRef(dontAskUpdate);
  dontAskRef.current = dontAskUpdate;
  /** V9: Close project with an agent still running asks first (default No). */
  const [confirmClose, setConfirmClose] = useState<{ entry: OpenTab; running: string[] } | null>(null);
  const [, rerender] = useReducer((n: number) => n + 1, 0);
  const [recent, setRecent] = useState<RecentView[]>([]);
  const [picker, setPicker] = useState(false);
  /** The health / update / rollback / profiles overlay, if one is open (E-39). */
  const [overlay, setOverlay] = useState<"health" | "update" | "rollback" | "profiles" | "menu" | null>(null);
  /** The menu's starting filter (`theme.set` without a name opens it at "Theme"). */
  const [menuQuery, setMenuQuery] = useState("");
  /** A paste with several lines, waiting for its question (D-034). */
  const [pasteText, setPasteText] = useState<string | null>(null);
  /** The last tab id handed out. */
  const tabSeq = useRef(0);
  const [health, setHealth] = useState<HealthView | null>(null);
  /** Health runs in flight (V7: the badge says `Checking…`); a count, as runs can overlap. */
  const [healthRuns, setHealthRuns] = useState(0);
  /** The recent projects have arrived once (V7: skeleton rows until then). */
  const [recentLoaded, setRecentLoaded] = useState(false);
  /** The copy icon shows a check for COPIED_MS after a copy (V7). */
  const [copied, setCopied] = useState(0);
  /** `:open` completion is asking the core for directories (V7). */
  const [completing, setCompleting] = useState(false);
  const [modeView, setModeView] = useState<ModeState>({ mode: "NORMAL", pending: [] });
  const [cmdText, setCmdText] = useState("");
  /** The toast (V2): the last message, gone after 4 s. */
  const [message, setMessage] = useState<{ text: string; error: boolean; seq: number; leaving?: boolean } | null>(null);
  /** The last command shown (rule 4), as the status bar's chip. */
  const [lastCmd, setLastCmd] = useState<string | null>(null);
  const [whichKey, setWhichKey] = useState(false);
  /** The panes of the open tabs live here, one element per tab (only the current one shown). */
  const termsEl = useRef<HTMLDivElement>(null);
  const tabs = useRef<OpenTab[]>([]);
  const cur = useRef(0);
  /** The current tab's terminal, read like a ref. */
  const tab = { get current(): TermTab | null { return tabs.current[cur.current]?.term ?? null; } };
  const pickerRef = useRef(false);
  // The set-up card (V9) takes the keyboard like an overlay: Tab and Enter work its buttons.
  pickerRef.current =
    picker ||
    stop !== null ||
    overlay !== null ||
    pasteText !== null ||
    confirmClose !== null ||
    updateSheet !== null ||
    tabs.current[cur.current]?.setup !== undefined;
  // No tab open: the start screen's keys (p o h u q, digits) join the keymap.
  const keys = tabs.current.length ? projectKeys : withStartKeys(projectKeys);
  /** The other input style's table: the menu and tooltips name its key when this one has none. */
  const otherTable = projectKeys.style === "simple" ? DEFAULT_MODAL : DEFAULT_SIMPLE;
  const modeRef = useRef<ModeState>(modeView);
  const cmdRef = useRef("");
  const lastCommand = useRef<string | null>(null);
  const completion = useRef<{ base: string; items: string[]; index: number } | null>(null);
  /** A TERMINAL key the modes layer took (a chord key): its keypress is kept from xterm too. */
  const held = useRef(false);
  /** The latest TERMINAL key handler, so a tab opened earlier never runs a stale one. */
  const terminalKeyRef = useRef<(e: KeyboardEvent) => boolean>(() => true);

  const say = useCallback(
    (text: string, error = false) => setMessage((m) => ({ text, error, seq: (m?.seq ?? 0) + 1 })),
    [],
  );
  useEffect(() => {
    if (!message) return;
    if (message.leaving) {
      const t = setTimeout(() => setMessage((m) => (m?.seq === message.seq ? null : m)), MOTION.base);
      return () => clearTimeout(t);
    }
    // Fades out over the last --motion-base (V7).
    const t = setTimeout(() => setMessage((m) => (m?.seq === message.seq ? { ...m, leaving: true } : m)), TOAST_MS - MOTION.base);
    return () => clearTimeout(t);
  }, [message]);
  useEffect(() => {
    if (!copied) return;
    const t = setTimeout(() => setCopied(0), COPIED_MS);
    return () => clearTimeout(t);
  }, [copied]);

  /** The tmux overlay (V6) for one open tab; a failure is told once for that tab (E-54), never
   *  for a tab that closed meanwhile. */
  const applyTmuxOverlay = useCallback(
    (entry: OpenTab, id: number, colours: ReturnType<typeof tmuxColours>) =>
      tabTmux(id, colours).then(
        (why) => {
          const open = tabs.current.includes(entry) && entry.term.tab?.id === id;
          const text = overlayNotice(why, open, entry.overlayTold === true);
          if (text === null) return;
          entry.overlayTold = true;
          say(text, true);
        },
        () => {},
      ),
    [say],
  );

  /** Run the install health check (fresh or cached), counting it as in flight while it runs. */
  const checkHealth = useCallback((fresh: boolean) => {
    setHealthRuns((n) => n + 1);
    return healthGet(fresh)
      .then(setHealth, () => {})
      .finally(() => setHealthRuns((n) => n - 1));
  }, []);

  // Theme and font (config.toml, `:theme`): the CSS variables every element reads and the open
  // terminal — restyled before paint.
  useLayoutEffect(() => {
    const root = document.documentElement.style;
    for (const [k, v] of Object.entries(cssVariables(theme))) root.setProperty(k, v);
    root.setProperty("--font-family", fontFamily);
    root.setProperty("--font-size", `${fontSize}px`);
    root.setProperty("--line-height", String(lineHeight));
    root.setProperty("--opacity", String(opacity));
    for (const t of tabs.current) t.term.restyle({ fontFamily, fontSize, lineHeight, theme });
  }, [theme, fontFamily, fontSize, lineHeight, opacity]);

  useEffect(() => {
    if (notice.text) say(notice.text, notice.error);
  }, [notice, say]);

  // The theme a tab opening later styles tmux with, and every open tab restyled when the
  // theme or the tmux settings change (V6).
  const themeRef = useRef(theme);
  themeRef.current = theme;
  useEffect(() => {
    for (const t of tabs.current) {
      const tab = t.term.tab;
      if (tab) void applyTmuxOverlay(t, tab.id, tmuxColours(theme));
    }
    // `applyTmuxOverlay` only reads refs and `say`, which are stable.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [theme, tmuxKey]);

  /** Show the exact command an action ran or stands for, as the status bar's chip; `y` and
   *  the chip's copy button copy it. A note goes to the toast. */
  const showCommand = useCallback(
    (cmd: string, note = "") => {
      lastCommand.current = cmd;
      setLastCmd(cmd);
      if (note) say(note);
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

  const refreshRecent = useCallback(() => {
    projectsList().then(setRecent, () => {});
  }, []);

  /** Show tab `i` (its pane only) and keep the chrome in step. */
  const showTab = useCallback((i: number) => {
    cur.current = Math.max(0, Math.min(i, tabs.current.length - 1));
    tabs.current.forEach((t, k) => t.el.classList.toggle("hidden", k !== cur.current));
    const t = tabs.current[cur.current];
    if (t) {
      requestAnimationFrame(() => t.term.shown());
      if (modeRef.current.mode === "TERMINAL") t.term.focus();
    }
    rerender();
  }, []);

  /** Open `project` in a new tab: `rein start` in a pty, drawn by xterm.js (at most 16). */
  const open = useCallback(
    (project: string) => {
      const same = tabs.current.findIndex((t) => t.project === project || t.requested === project);
      if (same >= 0) {
        showTab(same);
        return;
      }
      if (tabs.current.length >= MAX_TABS) {
        say(`at most ${MAX_TABS} tabs — close one first (ZZ or :q)`, true);
        return;
      }
      const host = termsEl.current;
      if (!host) return;
      const el = document.createElement("div");
      el.className = "term-pane";
      host.append(el);
      const term = new TermTab(el, project, { fontFamily, fontSize, lineHeight, theme });
      const entry: OpenTab = { id: ++tabSeq.current, term, el, requested: project, project, title: project.split(/[/\\]/).pop() || "/", open: false, exited: false };
      tabs.current.push(entry);
      term.keyFilter = (e) => terminalKeyRef.current(e);
      term.onPhase = () => {
        rerender(); // starting → running / exited (V7)
        // V9 3: once the tab has started (never delaying it), rein's verdict, once per open.
        const id = term.tab?.id;
        const started = entry.open && !entry.exited && term.phase(performance.now()) === "running";
        if (id === undefined || !shouldCheck({ project: entry.project, started, asked: entry.asked === true }, dontAskRef.current)) return;
        entry.asked = true;
        projectCheck(id).then(
          (r) => {
            if (!tabs.current.includes(entry) || term.tab?.id !== id || !("out_of_date" in r)) return;
            entry.update = r.out_of_date;
            rerender();
          },
          () => {},
        );
      };
      term.onSetup = (view) => {
        // V9 2: a folder without `.ai/` — the card in place of the terminal; rein is asked
        // what set-up will do.
        entry.setup = { id: view.id, reply: null, inside: view.inside };
        entry.project = view.project;
        entry.title = view.title;
        showTab(tabs.current.indexOf(entry));
        setMode({ mode: "NORMAL", pending: [] });
        // Inside a Rein project: nothing to ask rein — the card offers that project.
        if (view.inside !== null) return;
        projectCheck(view.id).then(
          (reply) => {
            if (entry.setup?.id !== view.id) return;
            entry.setup.reply = reply;
            rerender();
          },
          () => {
            if (entry.setup?.id !== view.id) return;
            entry.setup.reply = { unavailable: "" };
            rerender();
          },
        );
      };
      term.onRefused = () => {
        entry.exited = true; // V8: the tmux setup was refused; the tab says why
        rerender();
      };
      term.onOpen = (opened) => {
        entry.open = true;
        entry.exited = false;
        entry.project = opened.project;
        entry.title = opened.title;
        showTab(tabs.current.indexOf(entry));
        showCommand(`cd ${shellWord(opened.project)} && rein start`);
        setMode({ mode: "TERMINAL", pending: [] });
        refreshRecent();
        // The window's tmux look on the session the tab shows (V6, D-050), then health —
        // which is checked when a tab opens (desktop.md §Components 5) and carries the
        // overlay's failure, if any.
        void applyTmuxOverlay(entry, opened.id, tmuxColours(themeRef.current)).then(() => checkHealth(true));
      };
      term.onError = (msg) => {
        void term.detach();
        el.remove();
        tabs.current = tabs.current.filter((t) => t !== entry);
        showTab(cur.current);
        say(`E: cannot open ${project}: ${msg}`, true);
      };
      showTab(tabs.current.length - 1);
      void term.start();
    },
    [fontFamily, fontSize, lineHeight, theme, say, showCommand, setMode, showTab, refreshRecent],
  );

  /** The chip shows the current tab's own command again once a sheet closes (E-66: it kept
   *  `rein init` after Update project). */
  const restoreChip = useCallback(() => {
    const t = tabs.current[cur.current];
    if (t?.term.tab && !t.setup) showCommand(`cd ${shellWord(t.project)} && rein start`);
  }, [showCommand]);

  const closeOverlay = useCallback(() => {
    setOverlay(null);
    restoreChip();
    if (tabs.current.length) setMode({ mode: "TERMINAL", pending: [] });
  }, [setMode, restoreChip]);

  const openRef = useRef(open);
  openRef.current = open;
  const quitRef = useRef<() => void>(() => {});

  /** Close tab `i` (default: the current one); its tmux session keeps running — unless
   *  `note` says the project was closed (V9). Only removes the tab from the window: the core
   *  side is `tab_close` (V11, `closeTab`) or `tab_detach` (`term.detach`). */
  const detach = useCallback(
    (i = cur.current, note?: string) => {
      const t = tabs.current[i];
      if (!t) {
        say("no tab is open");
        return;
      }
      void t.term.detach();
      t.el.remove();
      tabs.current = tabs.current.filter((x) => x !== t);
      showTab(Math.min(i, tabs.current.length - 1));
      if (!tabs.current.length) {
        setMode({ mode: "NORMAL", pending: [] });
        // No tab: the chip's command belonged to a project that is no longer shown (E-63).
        lastCommand.current = null;
        setLastCmd(null);
      }
      // Another tab is current now: the chip shows its command, not the closed one's (E-66).
      else restoreChip();
      say(note ?? `${t.title} closed — its tmux session keeps running`);
    },
    [say, setMode, showTab, restoreChip],
  );

  /** V11 (E-66): close tab `i` — on the app's server its project closes too, with one confirm
   *  when an agent runs or is not known (unless "Don't ask again"); a set-up tab, an exited one
   *  and a tab on the user's server close as before. */
  const closeTab = useCallback(
    (i = cur.current, confirmed = false) => {
      const t = tabs.current[i];
      const tab = t?.term.tab;
      if (!t || !tab || t.setup) {
        detach(i);
        return;
      }
      tabClose(tab.id, confirmed || closeDontAskRef.current).then(
        (r) => {
          if (!tabs.current.includes(t)) return;
          if ("confirm" in r) {
            setStop({ kind: "tab", entry: t, roles: r.confirm });
            return;
          }
          detach(tabs.current.indexOf(t), closedNote(t.title, tab.server));
        },
        (e: { message?: string }) => {
          // The core closed the tab; the kill did not succeed — the next run's sweep ends it.
          if (tabs.current.includes(t)) detach(tabs.current.indexOf(t), `${t.title} closed — ${e.message ?? String(e)}`);
        },
      );
    },
    [detach],
  );

  /** V11: quit — the core counts the projects on its server, asks once (unless "Don't ask
   *  again"), ends its server and exits. */
  const quit = useCallback((confirmed = false) => {
    appQuit(confirmed || closeDontAskRef.current).then(
      (r) => {
        if (r && "confirm" in r) setStop({ kind: "quit", n: r.confirm });
      },
      () => {},
    );
  }, []);

  quitRef.current = () => quit();

  /** Close the project of `entry` (V9 component 4): `rein start --kill --yes` on its tmux
   *  server, then the tab closes. Without `confirmed`, a running agent asks first. */
  const closeProject = useCallback(
    (entry: OpenTab, confirmed: boolean, auto = false) => {
      const id = entry.term.tab?.id;
      if (id === undefined || entry.closing) return;
      entry.closing = true;
      rerender();
      showCommand(`cd ${shellWord(entry.project)} && rein start --kill --yes`);
      projectClose(id, confirmed)
        .then(
          (r) => {
            if (!tabs.current.includes(entry)) return;
            if ("confirm" in r) {
              // Closing by itself never asks: an agent that came back shows the bar instead.
              if (auto) entry.autoFailed = true;
              else setConfirmClose({ entry, running: r.confirm });
              return;
            }
            detach(tabs.current.indexOf(entry), `${entry.title} closed — its agents had exited`);
          },
          (e: { message?: string }) => {
            entry.autoFailed = true;
            say(`E: cannot close ${entry.title}: ${e.message ?? String(e)}`, true);
          },
        )
        .finally(() => {
          entry.closing = false;
          rerender();
        });
    },
    [detach, say, showCommand],
  );
  const closeProjectRef = useRef(closeProject);
  closeProjectRef.current = closeProject;
  const closeWhenRef = useRef(closeWhenAgentsExit);
  closeWhenRef.current = closeWhenAgentsExit;

  // V9: ask the core which agents of the active tab still run — every 5 s, only while the
  // window is visible and the tab's process runs. All ended (setting on): close by itself.
  useEffect(() => {
    const tick = () => {
      const t = tabs.current[cur.current];
      const id = t?.term.tab?.id;
      if (!t || id === undefined) return;
      const visible = document.visibilityState === "visible";
      if (!shouldPoll({ active: true, visible, open: t.open, exited: t.exited }) || t.closing) return;
      projectAgents(id).then(
        (r) => {
          if (!tabs.current.includes(t) || t.exited || !("agents" in r)) return;
          t.agents = r.agents;
          const view = agentBar(r.agents, { closeWhenAllExit: closeWhenRef.current && !t.autoFailed, kept: t.kept ?? null });
          if (view.kind === "close") closeProjectRef.current(t, false, true);
          rerender();
        },
        () => {},
      );
    };
    const timer = setInterval(tick, POLL_MS);
    return () => clearInterval(timer);
  }, []);

  /** Run one action — from a key, the command line or (E-40) a click: the same closed list. */
  const run = useCallback(
    (call: ActionCall) => {
      const own = handlers[call.name];
      if (own) {
        own(call, { say, showCommand });
        return;
      }
      if (keys.style === "simple" && (call.name === "mode.normal" || call.name === "mode.command")) {
        say("modes belong to the modal input style — menu → Input style: modal");
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
            // The system folder dialog; its answer is validated like `tab_open`.
            say("choose a project folder in the dialog…");
            projectChoose().then(
              (path) => {
                if (path) open(path);
                else say("no folder chosen");
              },
              (e: { message?: string }) => say(`E: ${e.message ?? String(e)}`, true),
            );
          }
          return;
        case "tab.detach":
          // `×` names its tab; the key form closes the current one (V11: its project too).
          closeTab(call.arg ? Number(call.arg) - 1 : cur.current);
          return;
        case "menu.show":
        case "help.show":
          setMode({ mode: "NORMAL", pending: [] });
          setMenuQuery("");
          setOverlay("menu");
          return;
        case "theme.set":
          if (!call.arg) {
            setMenuQuery("Theme");
            setOverlay("menu");
            return;
          }
          break;
        case "term.copy": {
          const text = tab.current?.selection() ?? "";
          if (!text) {
            say("nothing is selected");
            return;
          }
          clipboardWrite(text.slice(0, 64 * 1024)).then(
            () => say("copied"),
            (e: { message?: string }) => say(`E: cannot copy: ${e.message ?? String(e)}`, true),
          );
          return;
        }
        case "term.paste":
          if (!tab.current) {
            say("no terminal to paste into");
            return;
          }
          clipboardRead().then(
            (text) => {
              if (pasteNeedsQuestion(text)) setPasteText(text);
              else tab.current?.paste(text);
            },
            (e: { message?: string }) => say(`E: cannot paste: ${e.message ?? String(e)}`, true),
          );
          return;
        case "tab.next":
        case "tab.prev": {
          const n = tabs.current.length;
          if (n) showTab((cur.current + (call.name === "tab.next" ? 1 : n - 1)) % n);
          return;
        }
        case "tab.goto": {
          const n = Number(call.arg ?? "1");
          if (tabs.current.length) {
            if (n <= tabs.current.length) showTab(n - 1);
            else say(`no tab ${n}`);
          } else {
            // The start screen: digit N opens recent project N.
            const r = recent[n - 1];
            if (r) open(r.path);
            else say(`no recent project ${n}`);
          }
          return;
        }
        case "app.quit":
          // V11: the core ends its server (after one confirm) and exits; with no project on
          // it, it exits at once.
          quit();
          return;
        case "health.show":
          setMode({ mode: "NORMAL", pending: [] });
          showCommand("rein doctor --scope install --json");
          setOverlay("health");
          void checkHealth(true);
          return;
        case "release.update":
        case "release.rollback":
          setMode({ mode: "NORMAL", pending: [] });
          setOverlay(call.name === "release.update" ? "update" : "rollback");
          return;
        case "profiles.show":
        case "profiles.apply":
          setMode({ mode: "NORMAL", pending: [] });
          setOverlay("profiles");
          return;
        case "project.pick":
          refreshRecent();
          setMode({ mode: "NORMAL", pending: [] });
          setPicker(true);
          return;
        case "cmd.yank": {
          const cmd = lastCommand.current;
          if (!cmd) {
            say("no command to copy yet");
            return;
          }
          clipboardWrite(cmd).then(
            () => {
              setCopied(Date.now());
              say(`copied: ${cmd}`);
            },
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
    [open, detach, say, setMode, setCmd, showCommand, handlers, showTab, recent, refreshRecent, keys.style],
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
      dirs: async (prefix) => {
        if (!prefix.startsWith("/")) return [];
        setCompleting(true);
        try {
          return await dirComplete(prefix).catch(() => []);
        } finally {
          setCompleting(false);
        }
      },
    });
    if (!items.length) return;
    const common = commonPrefix(items);
    completion.current = { base, items, index: items.length === 1 ? 0 : -1 };
    setCmd(items.length === 1 ? (items[0] ?? base) : common.length > base.length ? common : base);
    // More than one: the palette lists them above the command line.
  }, [setCmd]);

  // NORMAL and COMMAND keys: the window handles them before anything else sees them.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (pickerRef.current) return; // the picker's own keys (j k C-n C-p Enter Esc Del)
      // Any open dialog or menu owns its keys (the first-run choice, Radix menus …).
      if (e.target instanceof Element && e.target.closest('[role="dialog"], [role="menu"], [role="listbox"]')) return;
      const k = keyName(e);
      if (!k) return;
      if (modeRef.current.mode === "TERMINAL") {
        // A TERMINAL key that missed the terminal (focus was on the page): give the terminal
        // focus and pass the key through the same chord logic, so none is lost.
        const inTerm = e.target instanceof Node && termsEl.current?.contains(e.target);
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
    void listen<HealthView>("health_changed", setHealth);
    void checkHealth(false);
    void listen<{ id: number; code: number }>("pty_exit", ({ id, code }) => {
      const t = tabs.current.find((x) => x.term.tab?.pty === id);
      if (!t) return;
      t.term.exited(code);
      t.exited = true;
      rerender();
    });
    void listen<string>("open_dir", (dir) => openRef.current(dir));
    // V11: a native close held for the confirm.
    void listen<null>("quit_requested", () => quitRef.current());
    projectsList().then(
      (list) => {
        setRecent(list);
        setRecentLoaded(true);
        if (typeof startDir === "string") openRef.current(startDir);
        else if (startLast && list[0]) openRef.current(list[0].path);
      },
      () => {
        setRecentLoaded(true);
        if (typeof startDir === "string") openRef.current(startDir);
      },
    );
    // Listeners live as long as the window; `open` is stable for a given font and theme.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const termOpen = tabs.current.length > 0;
  const current = tabs.current[cur.current];
  const agentView =
    current?.agents && !current.exited
      ? agentBar(current.agents, { closeWhenAllExit: closeWhenAgentsExit && !current.autoFailed, kept: current.kept ?? null })
      : ({ kind: "none" } as const);
  const runner = { run, tip: (c: ActionCall) => describeAction(c, keys, otherTable) };
  const now = performance.now();
  const starting = (t: OpenTab | undefined) => t !== undefined && !t.exited && t.term.phase(now) === "starting";
  const bar = titleBar(
    tabs.current.map((t, i) => ({ id: t.id, title: t.title, current: i === cur.current, exited: t.exited, starting: starting(t) })),
    VERSION,
  );
  // The `simple` style has no modes, so no mode badge (D-034).
  const status = statusBar(current ? current.project : null, health, lastCmd, keys.style === "modal" ? modeView.mode : null, {
    starting: starting(current),
    health: healthRuns > 0,
    copied: copied > 0,
  });

  // The stage is borderless and fills what the bars leave (V2): the terminal follows its size
  // in whole cells, centred; the window itself is never snapped to cells.
  useEffect(() => {
    const el = termsEl.current;
    if (!el) return;
    const o = new ResizeObserver(() => tab.current?.fit());
    o.observe(el);
    return () => o.disconnect();
    // `tab` reads the current tab when the observer fires.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  return (
    <>
      <div id="frame" role="application" aria-label="rein-app" className="flex h-full flex-col">
        <TitleBar bar={bar} chrome={CHROME} r={runner} />
        {current && current.update && !current.updateHidden && !current.exited && !current.setup && (
          <UpdateBar
            onUpdate={() => {
              const id = current.term.tab?.id;
              if (id !== undefined && current.update) setUpdateSheet({ entry: current, id, rows: current.update });
            }}
            onNotNow={() => {
              current.updateHidden = true;
              rerender();
            }}
            onDontAsk={() => {
              current.updateHidden = true;
              onDontAskUpdate(current.project);
              rerender();
            }}
          />
        )}
        {current && agentView.kind === "bar" && (
          <AgentBar
            text={agentView.text}
            closing={current.closing === true}
            onClose={() => closeProject(current, false)}
            onKeep={() => {
              current.kept = agentView.key;
              rerender();
            }}
          />
        )}
        <div id="stage" className="relative min-h-0 flex-1" aria-busy={status.starting || undefined}>
          {/* V7: the tab is starting — a 2 px line in the stage's own top padding, never over
              the terminal. */}
          {status.starting && !current?.setup && <ProgressLine className="absolute inset-x-0 top-0 z-10" />}
          {current?.setup && (
            <SetupCard
              project={current.project}
              reply={current.setup.reply}
              inside={current.setup.inside}
              onOpenInside={(parent) => {
                detach(cur.current, `${current.title} closed — opening the project it is inside`);
                open(parent);
              }}
              onSetUp={() => {
                const s = current.setup;
                if (!s) return;
                current.setup = undefined;
                rerender();
                void current.term.start(s.id);
              }}
              onOpenOther={() => run({ name: "project.open" })}
              onClose={() => detach(cur.current)}
            />
          )}
          {!termOpen && (
            <StartScreen
              model={startScreen(
                recent,
                health?.setup ? { reason: health.reason ?? "WSL is not ready", command: health.setup } : null,
                Math.floor(Date.now() / 1000),
                { loading: !recentLoaded, checking: healthRuns > 0 },
              )}
              r={runner}
            />
          )}
          <ContextMenu>
            <ContextMenuTrigger asChild>
              <div id="term" ref={termsEl} className={termOpen ? undefined : "hidden"} />
            </ContextMenuTrigger>
            <ContextMenuContent>
              <ContextMenuItem onSelect={() => run({ name: "term.copy" })}>
                Copy<ContextMenuShortcut>Ctrl+Shift+C</ContextMenuShortcut>
              </ContextMenuItem>
              <ContextMenuItem onSelect={() => run({ name: "term.paste" })}>
                Paste<ContextMenuShortcut>Ctrl+Shift+V</ContextMenuShortcut>
              </ContextMenuItem>
              <ContextMenuSeparator />
              <ContextMenuItem onSelect={() => tab.current?.selectAll()}>Select all</ContextMenuItem>
            </ContextMenuContent>
          </ContextMenu>
        </div>
        <StatusBar bar={status} r={runner} />
      </div>
      <Toast message={message} />
      {/* Each overlay leaves with its exit animation (V7): `Exit` keeps it for --motion-fast. */}
      <Exit
        show={modeView.mode === "COMMAND"}
        render={() => <CommandPalette text={cmdText} items={completion.current?.items ?? []} pending={completing} />}
      />
      <Exit
        show={overlay === "menu"}
        render={() => (
          <MenuOverlay
            entries={menuEntries(keys, otherTable, { tabs: tabs.current.length })}
            initialQuery={menuQuery}
            onRun={(c) => {
              closeOverlay();
              run(c);
            }}
            onClose={closeOverlay}
          />
        )}
      />
      <Exit
        show={pasteText !== null}
        render={() => (
          <PasteConfirm
            text={pasteText ?? ""}
            onPaste={() => {
              if (pasteText !== null) tab.current?.paste(pasteText);
              setPasteText(null);
              tab.current?.focus();
            }}
            onClose={() => {
              setPasteText(null);
              tab.current?.focus();
            }}
          />
        )}
      />
      <Exit
        show={confirmClose !== null}
        render={() => (
          <CloseConfirm
            running={confirmClose?.running ?? []}
            onYes={() => {
              const c = confirmClose;
              setConfirmClose(null);
              if (c) closeProject(c.entry, true);
            }}
            onNo={() => {
              setConfirmClose(null);
              tab.current?.focus();
            }}
          />
        )}
      />
      <Exit
        show={updateSheet !== null}
        render={() =>
          updateSheet && (
            <UpdateSheet
              tab={updateSheet.id}
              rows={updateSheet.rows}
              onCommand={showCommand}
              onDone={(outcome) => {
                if (!outcome.keepBar) updateSheet.entry.update = undefined;
                rerender();
              }}
              onClose={() => {
                setUpdateSheet(null);
                restoreChip();
                tab.current?.focus();
              }}
            />
          )
        }
      />
      <Exit
        show={stop !== null}
        render={() =>
          stop && (
            <StopConfirm
              title={stop.kind === "tab" ? "Close project?" : "Quit rein-app?"}
              text={stop.kind === "tab" ? closeTabText(stop.roles, stop.entry.title) : quitText(stop.n)}
              yes={stop.kind === "tab" ? "Close" : "Quit"}
              onYes={(dontAsk) => {
                const s = stop;
                setStop(null);
                if (dontAsk) onCloseDontAsk();
                if (s.kind === "tab") {
                  const i = tabs.current.indexOf(s.entry);
                  if (i >= 0) closeTab(i, true);
                } else quit(true);
              }}
              onNo={() => {
                if (stop.kind === "quit") void quitCancelled();
                setStop(null);
                tab.current?.focus();
              }}
            />
          )
        }
      />
      {whichKey && <WhichKey leader={keys.leader} next={continuations(keys, modeView.pending)} />}
      <Exit show={overlay === "health"} render={() => <HealthOverlay health={health} onClose={closeOverlay} />} />
      <Exit
        show={overlay === "update" || overlay === "rollback"}
        render={() => (
          <ReleaseOverlay kind={overlay === "rollback" ? "rollback" : "update"} onClose={closeOverlay} onCommand={showCommand} />
        )}
      />
      <Exit show={overlay === "profiles"} render={() => <ProfilesOverlay onClose={closeOverlay} onCommand={showCommand} />} />
      <Exit
        show={picker}
        render={() => (
          <ProjectPicker
            recent={recent}
            onPick={(path) => {
              setPicker(false);
              open(path);
            }}
            onForget={(path) => {
              projectsForget(path).then(setRecent, () => say(`E: cannot forget ${path}`, true));
            }}
            onClose={() => {
              setPicker(false);
              if (tabs.current.length) setMode({ mode: "TERMINAL", pending: [] });
            }}
          />
        )}
      />
    </>
  );
}
