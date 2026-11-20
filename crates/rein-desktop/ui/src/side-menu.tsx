// The side menu's wiring into the shell (panels.md §Components, E-60; moved out of
// `shell.tsx` by E-72 with no behaviour change): which panel is open and how wide, the four
// `panel.*` actions, the key rule inside a panel, and the rail and panel elements the shell
// places beside (or, in a narrow window, over) the terminal stage. E-61 adds "Needs you"; E-75
// (status.md §4) moves its rules into the core: one `attention_items` call for every open tab
// every 5 s while the window is visible, the bell mark and count, and a badge of Now items.

import { useCallback, useEffect, useRef, useState, type MutableRefObject, type ReactNode, type RefObject } from "react";
import { NO_ITEMS, bellClears, bellMarks, goToTab, type Lists, type TabState } from "./attention.ts";
import type { Runner } from "./frame.tsx";
import { attentionDismiss, attentionItems, tabBell, tabCurrent, type HealthView } from "./ipc.ts";
import type { ActionCall } from "./keymap.ts";
import { PanelHost, useVisible, type AttentionFeed } from "./panel-host.tsx";
import type { StatusMark } from "./status.ts";
import { HOME_PANELS, HOME_PANEL_TIP } from "./home.ts";
import { PANEL_DEFAULT, POLL_MS, clampWidth, descriptor, enabledPanels, maxWidth, overlays as panelOverlays } from "./panels.ts";
import { Rail } from "./rail.tsx";
import type { TermTab } from "./term.ts";

/** The shell's props for the side menu. */
export interface SideMenuProps {
  /** The side menu (panels.md, E-60): the rail, the panels on offer, the panel last open and
   *  its width as ui.json had them (null until ui.json is read), and how to keep them. */
  panelsRail: "shown" | "hidden";
  panelsEnabled: readonly string[];
  panelInit: { open: string | null; width: number } | null;
  onPanelState: (field: "panel_open" | "panel_width", value: string | number | null) => void;
}

/** An open tab as the side menu sees it (the shell's own tab entries fit this shape). The
 *  bell mark is the one field the side menu sets. */
export interface SideTab {
  term: { tab: { id: number } | null };
  title: string;
  exited: boolean;
  refused?: boolean | undefined;
  bell?: boolean | undefined;
  /** Bells rung while marked (E-75): a new one brings a dismissed bell row back. */
  bellCount?: number | undefined;
  update?: string[] | undefined;
  updateHidden?: boolean | undefined;
  setup?: unknown;
  /** A failed start (V14): in `needs_init` the core's status is `setup` (E-95's flag). */
  failed?: { at: string } | undefined;
}

/** What the shell knows at render time that the side menu shows. */
export interface SideMenuView {
  termOpen: boolean;
  current: { title: string; term: TermTab } | undefined;
  health: HealthView | null;
  runner: Runner;
  /** The modal input style (the reader then also takes k / j). */
  modal: boolean;
  /** The current tab's status for the Dashboard's first row (status.md §3, E-77). */
  status: StatusMark | null;
  /** Home is shown (status.md §Home H4, E-80): only "Needs you" has something to show. */
  home?: boolean;
}

export interface SideMenu {
  /** While the panel's edge is dragged the terminal is not fitted (one pty resize at the end). */
  resizing: MutableRefObject<boolean>;
  /** Runs a `panel.*` action; false for any other action. */
  act: (call: ActionCall) => boolean;
  /** The rail (or null), the open panel (or null), whether it lies over the stage, and the
   *  Now items with their Dismiss (Home, E-80). */
  view: (v: SideMenuView) => { rail: ReactNode; panel: ReactNode; over: boolean; feed: AttentionFeed };
  /** A tab's terminal rang its bell (E-61): marked unless it is in front of the user. */
  bell: (t: SideTab, isCurrent: boolean) => void;
}

const HEALTH_WORDS = { ok: "OK", warn: "Warnings", fail: "Failing", unknown: "Not checked yet" } as const;

/** The panel (E-60) keeps its own keys (Tab, Enter, arrows, Esc back to the terminal); only
 *  the shortcuts still act there, as in every mode. True when the key was the panel's. The
 *  restored tab's card and bar (E-70, `data-own-keys`) keep theirs the same way. */
export function panelOwnsKey(e: KeyboardEvent, shortcut: ActionCall | undefined, run: (c: ActionCall) => void): boolean {
  if (!(e.target instanceof Element && e.target.closest("[data-panel-host], [data-own-keys]"))) return false;
  if (shortcut) {
    e.preventDefault();
    e.stopPropagation();
    run(shortcut);
  }
  return true;
}

export function useSideMenu(
  props: SideMenuProps,
  say: (text: string, error?: boolean) => void,
  terminal: () => TermTab | null,
  openTabs: () => { tabs: readonly SideTab[]; current: number },
): SideMenu {
  const { panelsRail, panelsEnabled, panelInit, onPanelState } = props;
  /** The open panel (one at a time) and its width; `liveWidth` follows a drag of its edge. */
  const [panel, setPanel] = useState<string | null>(null);
  const [panelWidth, setPanelWidth] = useState(PANEL_DEFAULT);
  const [liveWidth, setLiveWidth] = useState<number | null>(null);
  const resizing = useRef(false);
  const [panelRefresh, setPanelRefresh] = useState(0);
  const [panelFocus, setPanelFocus] = useState(0);
  const [winWidth, setWinWidth] = useState(() => (typeof window === "undefined" ? 1280 : window.innerWidth));
  const panelSeeded = useRef(false);
  useEffect(() => {
    if (!panelInit || panelSeeded.current) return;
    panelSeeded.current = true;
    setPanel(panelInit.open);
    setPanelWidth(panelInit.width);
  }, [panelInit]);
  useEffect(() => {
    const on = () => setWinWidth(window.innerWidth);
    window.addEventListener("resize", on);
    return () => window.removeEventListener("resize", on);
  }, []);
  const panelRef = useRef<string | null>(null);
  panelRef.current = panel;
  const enabledRef = useRef(panelsEnabled);
  enabledRef.current = panelsEnabled;
  const terminalRef = useRef(terminal);
  terminalRef.current = terminal;

  // "Needs you" (E-61, E-75): one `attention_items` call for every open tab, every POLL_MS
  // while the window can be seen and at once when a tab's own state changes; nothing at all
  // while it is hidden or the panel is turned off.
  const visible = useVisible();
  const watching = panelsEnabled.includes("attention");
  const [lists, setLists] = useState<{ lists: Lists; at: number | null }>({ lists: NO_ITEMS, at: null });
  const [, bump] = useState(0);
  const tabsRef = useRef(openTabs);
  tabsRef.current = openTabs;
  const healthRef = useRef<"ok" | "warn" | "fail" | "unknown">("unknown");
  const tabStates = (): TabState[] =>
    tabsRef.current().tabs.flatMap((t, index) =>
      t.setup
        ? []
        : [
            {
              index,
              tab: t.term.tab?.id ?? null,
              refused: t.refused === true,
              update: (t.update?.length ?? 0) > 0 && t.updateHidden !== true,
              setup: t.failed?.at === "needs_init",
            },
          ],
    );
  const statesKey = JSON.stringify(tabStates());
  useEffect(() => {
    if (!watching || !visible) return;
    let alive = true;
    const load = () => {
      attentionItems(tabStates(), healthRef.current).then(
        (l) => {
          if (!alive) return;
          const at = Math.floor(Date.now() / 1000);
          setLists((prev) => (JSON.stringify(prev.lists) === JSON.stringify(l) && panelRef.current !== "attention" ? prev : { lists: l, at }));
        },
        () => {},
      );
    };
    load();
    const t = setInterval(load, POLL_MS);
    return () => {
      alive = false;
      clearInterval(t);
    };
    // tabStates reads the tabs through a ref; statesKey is what changes.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [watching, visible, statesKey, panelRefresh]);

  // The bell mark goes once its tab is the current tab of a focused window.
  useEffect(() => {
    const { tabs, current } = tabsRef.current();
    const t = tabs[current];
    if (t?.bell && bellClears(true, document.hasFocus())) {
      t.bell = false;
      t.bellCount = 0;
      bump((n) => n + 1);
    }
  });
  useEffect(() => {
    const on = () => bump((n) => n + 1);
    window.addEventListener("focus", on);
    return () => window.removeEventListener("focus", on);
  }, []);
  // The core keeps the bells and the current tab for the status (E-76); the mark drawn here
  // is the side menu's own.
  const currentId = (() => {
    const { tabs, current } = tabsRef.current();
    return tabs[current]?.term.tab?.id ?? null;
  })();
  useEffect(() => {
    void tabCurrent(currentId).catch(() => {});
  }, [currentId]);
  const bell = useCallback((t: SideTab, isCurrent: boolean) => {
    const id = t.term.tab?.id;
    if (id !== undefined) void tabBell(id).catch(() => {});
    if (!bellMarks(isCurrent, document.hasFocus())) return;
    t.bell = true;
    t.bellCount = (t.bellCount ?? 0) + 1;
    bump((n) => n + 1);
  }, []);

  // panels.md §API (E-60): one panel at a time; the same toggle again closes it.
  const act = useCallback(
    (call: ActionCall): boolean => {
      switch (call.name) {
        case "panel.toggle": {
          const id = call.arg ?? "";
          const d = descriptor(id);
          // Home (H4): a per-project panel has no project to show.
          if (tabsRef.current().current < 0 && !HOME_PANELS.includes(id)) {
            say(HOME_PANEL_TIP);
            return true;
          }
          if (!d || !enabledRef.current.includes(id)) {
            say(`the ${d?.title ?? id} panel is turned off — Settings → Panel: ${d?.title ?? id}`, true);
            return true;
          }
          const next = panelRef.current === id ? null : id;
          setPanel(next);
          onPanelState("panel_open", next);
          if (next === null) terminalRef.current()?.focus();
          return true;
        }
        case "panel.close":
          if (panelRef.current !== null) {
            setPanel(null);
            onPanelState("panel_open", null);
          }
          terminalRef.current()?.focus();
          return true;
        case "panel.refresh":
          setPanelRefresh((n) => n + 1);
          return true;
        case "panel.focus": {
          // No panel open: the first one on offer opens, then takes the keyboard.
          if (panelRef.current === null) {
            const first = enabledPanels(enabledRef.current)[0];
            if (!first) {
              say("every panel is turned off — Settings → Panel", true);
              return true;
            }
            setPanel(first.id);
            onPanelState("panel_open", first.id);
          }
          setPanelFocus((n) => n + 1);
          return true;
        }
        default:
          return false;
      }
    },
    [say, onPanelState],
  );

  // The rail while a project is open, and the open panel for the current tab — beside the
  // terminal, or over it in a narrow window.
  const view = ({ termOpen, current, health, runner, modal, status, home = false }: SideMenuView) => {
    const offered = enabledPanels(panelsEnabled);
    // Home (H4): "Needs you" reads every open tab; the others wait for a project tab.
    const homeTab = home ? openTabs().tabs.find((t) => t.term.tab)?.term.tab : undefined;
    const coreTab = home ? homeTab : current?.term.tab;
    const openPanel = panel !== null && panelsEnabled.includes(panel) && (!home || HOME_PANELS.includes(panel)) ? descriptor(panel) : null;
    // The health word goes with the next ask (a change shows within POLL_MS).
    healthRef.current = health?.status ?? "unknown";
    const shown = watching ? lists.lists : NO_ITEMS;
    const titles = openTabs().tabs;
    const feed: AttentionFeed = {
      lists: shown,
      title: (index) => titles[index]?.title ?? `Tab ${index + 1}`,
      readAt: lists.at,
      goTo: goToTab(runner.run),
      refresh: () => setPanelRefresh((n) => n + 1),
      dismiss: (tab, key) =>
        attentionDismiss(tab, key).then(
          () => setPanelRefresh((n) => n + 1),
          (e: { message?: string }) => say(e.message ?? String(e), true),
        ),
      panel,
    };
    const rail =
      termOpen && panelsRail === "shown" && offered.length > 0 ? (
        <Rail panels={offered} open={openPanel ? openPanel.id : null} r={runner} badge={shown.now.length} home={home ? { panels: HOME_PANELS, tip: HOME_PANEL_TIP } : null} />
      ) : null;
    const el =
      termOpen && openPanel && (current || home) && coreTab ? (
        <PanelHost
          d={openPanel}
          tab={coreTab.id}
          project={current?.title ?? ""}
          width={clampWidth(liveWidth ?? panelWidth, maxWidth(winWidth))}
          maxWidth={maxWidth(winWidth)}
          modal={modal}
          overlay={panelOverlays(winWidth)}
          refreshSeq={panelRefresh}
          focusSeq={panelFocus}
          attention={feed}
          app={{
            health: HEALTH_WORDS[health?.status ?? "unknown"],
            server: current?.term.tab?.server === "app" ? "rein-app's own" : "your own",
            status,
          }}
          onClose={() => runner.run({ name: "panel.close" })}
          onEscape={() => terminalRef.current()?.focus()}
          onDrag={(phase, w) => {
            if (phase === "start") resizing.current = true;
            else if (phase === "move") setLiveWidth(w);
            else {
              resizing.current = false;
              setLiveWidth(null);
              setPanelWidth(clampWidth(w, maxWidth(winWidth)));
              onPanelState("panel_width", clampWidth(w, maxWidth(winWidth)));
              // The terminal is fitted once, after the layout has its final width.
              requestAnimationFrame(() => terminalRef.current()?.fit());
            }
          }}
        />
      ) : null;
    return { rail, panel: el, over: panelOverlays(winWidth), feed };
  };

  return { resizing, act, view, bell };
}

/** The row under the bars: the rail, the panel beside the stage (when it is not over it), and
 *  the stage itself as `children` (which places the panel inside it when it lies over it). */
export function SideRow({ v, children }: { v: ReturnType<SideMenu["view"]>; children: ReactNode }) {
  return (
    <div className="flex min-h-0 flex-1">
      {v.rail}
      {!v.over && v.panel}
      {children}
    </div>
  );
}

/** The stage is borderless and fills what the bars leave (V2): the terminal follows its size
 *  in whole cells, centred; the window itself is never snapped to cells. Not while the panel's
 *  edge is dragged: one pty resize after the drag ends. */
export function useStageFit(stage: RefObject<HTMLDivElement | null>, terminal: () => TermTab | null, side: SideMenu) {
  useEffect(() => {
    const el = stage.current;
    if (!el) return;
    const o = new ResizeObserver(() => {
      if (!side.resizing.current) terminal()?.fit();
    });
    o.observe(el);
    return () => o.disconnect();
    // `terminal` reads the current tab when the observer fires.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);
}
