// Each project's status in the window (status.md §Components 3, D-062; E-77): the mark on its
// tab, the current tab's status-bar segment and the Dashboard's first row. The core sends the
// status (`project_status`, `project_status_all()`, E-76); this file keeps the last one per
// tab and draws it. It decides nothing: what to say is `status.ts`.

import { useEffect, useRef } from "react";
import { Spinner } from "../components/loaders.tsx";
import { BellDot, Circle, CircleDot, Square, Wrench } from "./icons.ts";
import { listen, projectStatusAll } from "./ipc.ts";
import { coreId, statusMark, type ProjectStatus, type StatusMark } from "./status.ts";

/** `[tabs] status` (config.toml, ui.json, Settings — E-77). */
export interface TabStatusProps {
  tabsStatus: "shown" | "hidden";
}

/** An open tab as this file sees it (the shell's own tab entries fit this shape). */
export interface StatusTab {
  title: string;
  term: { tab: { id: number } | null };
  /** A set-up tab has no terminal; its core id is here (V9 component 1). */
  setup?: { id: number } | undefined;
}

export interface TabStatus {
  /** The tab's mark; null when hidden, not known yet, or `unknown`. */
  mark: (t: StatusTab | undefined) => StatusMark | null;
  /** The same, whatever the `[tabs] status` setting (the Dashboard's first row). */
  always: (t: StatusTab | undefined) => StatusMark | null;
}

/** The last status the core sent for every tab, by core id. `shown`: `[tabs] status`. */
export function useTabStatus(shown: boolean, rerender: () => void): TabStatus {
  const byTab = useRef(new Map<number, ProjectStatus>());
  useEffect(() => {
    let live = true;
    void listen<ProjectStatus>("project_status", (s) => {
      if (!live) return;
      byTab.current.set(s.tab, s);
      rerender();
    });
    // A web view that has just loaded asks once; an event that came first is newer.
    projectStatusAll().then(
      (all) => {
        if (!live) return;
        for (const s of all) if (!byTab.current.has(s.tab)) byTab.current.set(s.tab, s);
        rerender();
      },
      () => {},
    );
    return () => {
      live = false;
    };
    // The listener lives as long as the window.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);
  const of = (t: StatusTab | undefined) => {
    const id = coreId(t);
    return id === undefined ? undefined : byTab.current.get(id);
  };
  return {
    mark: (t) => statusMark(t?.title ?? "", of(t), shown),
    always: (t) => statusMark(t?.title ?? "", of(t), true),
  };
}

const TONE: Record<StatusMark["state"], string> = {
  setup: "text-ansi-11",
  ended: "text-muted-foreground",
  starting: "text-muted-foreground",
  needs_you: "text-primary",
  working: "text-primary",
  quiet: "text-muted-foreground",
  unknown: "",
};

/** The mark of one status, 12 px, decorative (the name is on the element it marks). Only the
 *  spinner moves (D-054). */
export function StatusGlyph({ s }: { s: StatusMark }) {
  const p = { size: 12, "aria-hidden": true, className: `shrink-0 ${TONE[s.state]}` } as const;
  const icon = (() => {
    switch (s.mark) {
      case "wrench":
        return <Wrench {...p} />;
      case "square":
        return <Square {...p} />;
      case "spinner":
        return <Spinner size={12} className="shrink-0 text-muted-foreground" />;
      case "bell-dot":
        return <BellDot {...p} />;
      case "circle-dot":
        return <CircleDot {...p} />;
      default:
        return <Circle {...p} />;
    }
  })();
  return (
    <span data-status={s.state} className="flex shrink-0 items-center gap-[2px]">
      {icon}
      {s.count !== null && <span className="text-[11px] tabular-nums text-primary">{s.count}</span>}
    </span>
  );
}

/** The tooltip of a tab with a status: one line each, at most three. */
export function StatusLines({ s }: { s: StatusMark }) {
  return (
    <span className="grid gap-[2px]">
      {s.lines.map((l, i) => (
        <span key={i} className={i === 0 ? "font-medium" : undefined}>
          {l}
        </span>
      ))}
    </span>
  );
}
