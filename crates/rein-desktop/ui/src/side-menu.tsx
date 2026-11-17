// The side menu's wiring into the shell (panels.md §Components, E-60; moved out of
// `shell.tsx` by E-72 with no behaviour change): which panel is open and how wide, the four
// `panel.*` actions, the key rule inside a panel, and the rail and panel elements the shell
// places beside (or, in a narrow window, over) the terminal stage. E-61 adds "Needs you":
// every open tab's attention facts read every 5 s while the window is visible (the core
// stats the two files and parses only on a change), the bell mark, the items and the badge.

import { useCallback, useEffect, useRef, useState, type MutableRefObject, type ReactNode, type RefObject } from "react";
import { attentionItems, bellClears, bellMarks, goToTab, type AttentionFacts } from "./attention.ts";
import type { Runner } from "./frame.tsx";
import { panelData, type HealthView } from "./ipc.ts";
import type { ActionCall } from "./keymap.ts";
import { PanelHost, useVisible, type AttentionFeed } from "./panel-host.tsx";
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
  update?: string[] | undefined;
  updateHidden?: boolean | undefined;
  setup?: unknown;
}

/** What the shell knows at render time that the side menu shows. */
export interface SideMenuView {
  termOpen: boolean;
  current: { title: string; term: TermTab } | undefined;
  health: HealthView | null;
  runner: Runner;
  /** The modal input style (the reader then also takes k / j). */
  modal: boolean;
}

export interface SideMenu {
  /** While the panel's edge is dragged the terminal is not fitted (one pty resize at the end). */
  resizing: MutableRefObject<boolean>;
  /** Runs a `panel.*` action; false for any other action. */
  act: (call: ActionCall) => boolean;
  /** The rail (or null), the open panel (or null), and whether it lies over the stage. */
  view: (v: SideMenuView) => { rail: ReactNode; panel: ReactNode; over: boolean };
  /** A tab's terminal rang its bell (E-61): marked unless it is in front of the user. */
  bell: (t: SideTab, isCurrent: boolean) => void;
}

const HEALTH_WORDS = { ok: "OK", warn: "Warnings", fail: "Failing", unknown: "Not checked yet" } as const;

/** The panel (E-60) keeps its own keys (Tab, Enter, arrows, Esc back to the terminal); only
 *  the shortcuts still act there, as in every mode. True when the key was the panel's. */
export function panelOwnsKey(e: KeyboardEvent, shortcut: ActionCall | undefined, run: (c: ActionCall) => void): boolean {
  if (!(e.target instanceof Element && e.target.closest("[data-panel-host]"))) return false;
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

  // "Needs you" (E-61): each open tab's facts, read every POLL_MS while the window can be
  // seen; nothing at all while it is hidden or the panel is turned off. The shell is drawn
  // again only when a fact changed, or while the panel is open (its "updated" age).
  const visible = useVisible();
  const watching = panelsEnabled.includes("attention");
  const [facts, setFacts] = useState<{ byTab: ReadonlyMap<number, AttentionFacts | null>; at: number | null }>({ byTab: new Map(), at: null });
  const [, bump] = useState(0);
  const tabsRef = useRef(openTabs);
  tabsRef.current = openTabs;
  const idsKey = openTabs()
    .tabs.map((t) => t.term.tab?.id ?? "-")
    .join(",");
  useEffect(() => {
    if (!watching || !visible) return;
    let alive = true;
    const load = () => {
      const ids = tabsRef.current().tabs.flatMap((t) => (t.term.tab ? [t.term.tab.id] : []));
      void Promise.all(
        ids.map((id) =>
          panelData<AttentionFacts>(id, "attention").then(
            (r) => [id, "ok" in r ? r.ok : null] as const,
            () => [id, null] as const,
          ),
        ),
      ).then((rows) => {
        if (!alive) return;
        const at = Math.floor(Date.now() / 1000);
        const byTab = new Map(rows);
        setFacts((prev) => {
          const same = JSON.stringify([...prev.byTab]) === JSON.stringify([...byTab]);
          if (same && panelRef.current !== "attention") return prev;
          return { byTab: same ? prev.byTab : byTab, at };
        });
      });
    };
    load();
    const t = setInterval(load, POLL_MS);
    return () => {
      alive = false;
      clearInterval(t);
    };
  }, [watching, visible, idsKey, panelRefresh]);

  // The bell mark goes once its tab is the current tab of a focused window.
  useEffect(() => {
    const { tabs, current } = tabsRef.current();
    const t = tabs[current];
    if (t?.bell && bellClears(true, document.hasFocus())) {
      t.bell = false;
      bump((n) => n + 1);
    }
  });
  useEffect(() => {
    const on = () => bump((n) => n + 1);
    window.addEventListener("focus", on);
    return () => window.removeEventListener("focus", on);
  }, []);
  const bell = useCallback((t: SideTab, isCurrent: boolean) => {
    if (t.bell || !bellMarks(isCurrent, document.hasFocus())) return;
    t.bell = true;
    bump((n) => n + 1);
  }, []);

  // panels.md §API (E-60): one panel at a time; the same toggle again closes it.
  const act = useCallback(
    (call: ActionCall): boolean => {
      switch (call.name) {
        case "panel.toggle": {
          const id = call.arg ?? "";
          const d = descriptor(id);
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
  const view = ({ termOpen, current, health, runner, modal }: SideMenuView) => {
    const offered = enabledPanels(panelsEnabled);
    const coreTab = current?.term.tab;
    const openPanel = panel !== null && panelsEnabled.includes(panel) ? descriptor(panel) : null;
    const items = watching
      ? attentionItems(
          openTabs().tabs.flatMap((t, index) =>
            t.setup
              ? []
              : [
                  {
                    index,
                    title: t.title,
                    exited: t.exited && !t.refused,
                    refused: t.refused === true,
                    bell: t.bell === true,
                    update: (t.update?.length ?? 0) > 0 && t.updateHidden !== true,
                    facts: t.term.tab ? (facts.byTab.get(t.term.tab.id) ?? null) : null,
                  },
                ],
          ),
          health?.status ?? "unknown",
          Date.now(),
        )
      : [];
    const feed: AttentionFeed = {
      items,
      readAt: facts.at,
      goTo: goToTab(runner.run),
      refresh: () => setPanelRefresh((n) => n + 1),
    };
    const rail =
      termOpen && panelsRail === "shown" && offered.length > 0 ? (
        <Rail panels={offered} open={openPanel ? openPanel.id : null} r={runner} badge={items.length} />
      ) : null;
    const el =
      termOpen && openPanel && current && coreTab ? (
        <PanelHost
          d={openPanel}
          tab={coreTab.id}
          project={current.title}
          width={clampWidth(liveWidth ?? panelWidth, maxWidth(winWidth))}
          maxWidth={maxWidth(winWidth)}
          modal={modal}
          overlay={panelOverlays(winWidth)}
          refreshSeq={panelRefresh}
          focusSeq={panelFocus}
          attention={feed}
          app={{
            health: HEALTH_WORDS[health?.status ?? "unknown"],
            server: coreTab.server === "app" ? "rein-app's own" : "your own",
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
    return { rail, panel: el, over: panelOverlays(winWidth) };
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
