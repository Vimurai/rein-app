// The side menu's wiring into the shell (panels.md §Components, E-60; moved out of
// `shell.tsx` by E-72 with no behaviour change): which panel is open and how wide, the four
// `panel.*` actions, the key rule inside a panel, and the rail and panel elements the shell
// places beside (or, in a narrow window, over) the terminal stage.

import { useCallback, useEffect, useRef, useState, type MutableRefObject, type ReactNode, type RefObject } from "react";
import type { Runner } from "./frame.tsx";
import type { HealthView } from "./ipc.ts";
import type { ActionCall } from "./keymap.ts";
import { PanelHost } from "./panel-host.tsx";
import { clampWidth, descriptor, enabledPanels, overlays as panelOverlays } from "./panels.ts";
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

/** What the shell knows at render time that the side menu shows. */
export interface SideMenuView {
  termOpen: boolean;
  current: { title: string; term: TermTab } | undefined;
  health: HealthView | null;
  runner: Runner;
}

export interface SideMenu {
  /** While the panel's edge is dragged the terminal is not fitted (one pty resize at the end). */
  resizing: MutableRefObject<boolean>;
  /** Runs a `panel.*` action; false for any other action. */
  act: (call: ActionCall) => boolean;
  /** The rail (or null), the open panel (or null), and whether it lies over the stage. */
  view: (v: SideMenuView) => { rail: ReactNode; panel: ReactNode; over: boolean };
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
): SideMenu {
  const { panelsRail, panelsEnabled, panelInit, onPanelState } = props;
  /** The open panel (one at a time) and its width; `liveWidth` follows a drag of its edge. */
  const [panel, setPanel] = useState<string | null>(null);
  const [panelWidth, setPanelWidth] = useState(360);
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
  const view = ({ termOpen, current, health, runner }: SideMenuView) => {
    const offered = enabledPanels(panelsEnabled);
    const coreTab = current?.term.tab;
    const openPanel = panel !== null && panelsEnabled.includes(panel) ? descriptor(panel) : null;
    const rail =
      termOpen && panelsRail === "shown" && offered.length > 0 ? (
        <Rail panels={offered} open={openPanel ? openPanel.id : null} r={runner} />
      ) : null;
    const el =
      termOpen && openPanel && current && coreTab ? (
        <PanelHost
          d={openPanel}
          tab={coreTab.id}
          project={current.title}
          width={liveWidth ?? panelWidth}
          overlay={panelOverlays(winWidth)}
          refreshSeq={panelRefresh}
          focusSeq={panelFocus}
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
              setPanelWidth(clampWidth(w));
              onPanelState("panel_width", clampWidth(w));
              // The terminal is fitted once, after the layout has its final width.
              requestAnimationFrame(() => terminalRef.current()?.fit());
            }
          }}
        />
      ) : null;
    return { rail, panel: el, over: panelOverlays(winWidth) };
  };

  return { resizing, act, view };
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
