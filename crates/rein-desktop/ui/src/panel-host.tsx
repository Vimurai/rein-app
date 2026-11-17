// The panel host (panels.md §Components 2, D-056; E-60): one panel at a time, left of the
// terminal. It reads its source only while it is open and the window can be seen — at once,
// then every POLL_MS (the core stats the file and parses only on a change) — and draws the
// descriptor's view. Dragging its right edge changes its width; the terminal is fitted once
// when the drag ends, not per pixel. Esc in the panel gives the terminal the keyboard back.
// E-74 (status.md §Components 7): the body is 13 px / 1.5, the width 320 px to the smaller of
// 720 px and half the window, and a task or handoff opens in the reader over the terminal.

import {
  Component,
  useEffect,
  useRef,
  useState,
  type KeyboardEvent as ReactKeyboardEvent,
  type PointerEvent as ReactPointerEvent,
  type ReactNode,
} from "react";
import { Button } from "../components/button.tsx";
import { Skeleton } from "../components/loaders.tsx";
import { ScrollArea } from "../components/scroll-area.tsx";
import { RefreshCw, TriangleAlert, X } from "./icons.ts";
import { panelData } from "./ipc.ts";
import type { AttentionItem } from "./attention.ts";
import { Reader, type ReaderItem } from "./panel-reader.tsx";
import { AttentionView, ListView, SummaryView, TimelineView, type OnRead } from "./panel-views.tsx";
import {
  PANEL_MIN,
  POLL_MS,
  clampWidth,
  shouldRead,
  summaryModel,
  timelineModel,
  updatedAgo,
  type Descriptor,
  type Handoffs,
  type ListOptions,
  type Reply,
  type Summary,
  type Tasks,
} from "./panels.ts";

/** `document.visibilityState`, as state: no read while the window is hidden or minimised. */
export function useVisible(): boolean {
  const [visible, setVisible] = useState(() => typeof document === "undefined" || document.visibilityState !== "hidden");
  useEffect(() => {
    const on = () => setVisible(document.visibilityState !== "hidden");
    document.addEventListener("visibilitychange", on);
    return () => document.removeEventListener("visibilitychange", on);
  }, []);
  return visible;
}

/** The fixed sentence when the core could not be asked at all (never text from a file). */
const NO_ANSWER = "The panel could not ask the app for its data.";
/** The fixed sentence when a view could not draw an answer: the panel says so, and the rest of
 *  the window — the terminal above all — keeps running. */
const NO_DRAW = "The panel could not draw this answer.";

/** A fault inside a panel stays inside it. */
class PanelGuard extends Component<{ children: ReactNode; resetKey: string }, { failed: boolean; key: string }> {
  override state = { failed: false, key: this.props.resetKey };
  static getDerivedStateFromError() {
    return { failed: true };
  }
  static getDerivedStateFromProps(p: { resetKey: string }, s: { failed: boolean; key: string }) {
    return p.resetKey === s.key ? null : { failed: false, key: p.resetKey };
  }
  override render() {
    return this.state.failed ? <Unavailable text={NO_DRAW} /> : this.props.children;
  }
}

function Unavailable({ text }: { text: string }) {
  return (
    <div role="status" className="grid gap-1 rounded-md border border-border p-3 text-[13px]">
      <span className="flex items-center gap-1.5 font-medium text-foreground">
        <TriangleAlert size={14} aria-hidden="true" className="text-muted-foreground" />
        Not available
      </span>
      <span className="[overflow-wrap:anywhere] text-muted-foreground">{text}</span>
    </div>
  );
}

/** "Needs you" is not read by the host: the side menu reads every open tab's facts for the
 *  badge and hands the panel the items, the time of the last read, and "go to tab". */
export interface AttentionFeed {
  items: AttentionItem[];
  readAt: number | null;
  goTo: (tab: number) => void;
  refresh: () => void;
}

/** What the descriptor's view draws for one answer. Exported for the render test. */
export function PanelBody({
  d,
  reply,
  app,
  nowMs,
  attention,
  onRead,
}: {
  d: Descriptor;
  reply: Reply<unknown> | null;
  app: { health: string; server: string };
  nowMs: number;
  attention?: AttentionFeed | undefined;
  /** Opens the reader (E-74); absent in the render test. */
  onRead?: OnRead | undefined;
}) {
  if (d.source === "attention") {
    if (attention?.readAt == null) reply = null;
    else return <AttentionView items={attention.items} goTo={attention.goTo} />;
  }
  if (reply === null) {
    return (
      <div className="grid gap-3" aria-hidden="true">
        <Skeleton className="h-4 w-2/3 rounded-md" />
        <Skeleton className="h-4 w-full rounded-md" />
        <Skeleton className="h-4 w-5/6 rounded-md" />
      </div>
    );
  }
  if ("unavailable" in reply) return <Unavailable text={reply.unavailable} />;
  switch (d.view) {
    case "summary": {
      const m = summaryModel(reply.ok as Summary, app, nowMs);
      return <SummaryView rows={m.rows} chips={m.chips} />;
    }
    case "list": {
      const t = reply.ok as Tasks;
      return <ListView tasks={t} options={d.options as ListOptions} cut={t.cut} nowMs={nowMs} onRead={onRead} />;
    }
    case "timeline": {
      const h = reply.ok as Handoffs;
      const m = timelineModel(h, nowMs);
      return <TimelineView whoseTurn={"pin" in d.options && d.options.pin ? m.whoseTurn : null} entries={m.entries} cut={h.cut} onRead={onRead} />;
    }
  }
}

export function PanelHost({
  d,
  tab,
  project,
  width,
  maxWidth,
  modal,
  overlay,
  refreshSeq,
  focusSeq,
  app,
  attention,
  onClose,
  onEscape,
  onDrag,
}: {
  d: Descriptor;
  /** The current tab's core id: the core takes the project from it. */
  tab: number;
  /** The tab's project name, shown in the header. */
  project: string;
  width: number;
  /** The widest the edge may go now: the smaller of 720 px and half the window. */
  maxWidth: number;
  /** The modal input style: the reader also takes k / j. */
  modal: boolean;
  /** Below 900 px of window width the panel lies over the terminal. */
  overlay: boolean;
  /** Bumped by `panel.refresh` / `panel.focus`. */
  refreshSeq: number;
  focusSeq: number;
  app: { health: string; server: string };
  /** The "Needs you" items, for that panel only. */
  attention?: AttentionFeed | undefined;
  onClose: () => void;
  onEscape: () => void;
  /** A drag of the edge: `start`, then the width while it moves, then `end` with the width. */
  onDrag: (phase: "start" | "move" | "end", width: number) => void;
}) {
  const visible = useVisible();
  /** The last answer, with the source and tab it was read for: after a switch the old one is
   *  never drawn by the new panel's view (a Summary is not a list of handoffs). */
  const [answer, setAnswer] = useState<{ key: string; reply: Reply<unknown> } | null>(null);
  const key = `${d.source}:${tab}`;
  const reply = answer && answer.key === key ? answer.reply : null;
  const [busy, setBusy] = useState(false);
  /** The header's refresh button; `panel.refresh` comes in as `refreshSeq`. */
  const [refreshed, setRefreshed] = useState(0);
  const root = useRef<HTMLElement>(null);
  /** The reader (E-74): the items it walks, the one shown, where the panel's edge is. */
  const [reader, setReader] = useState<{ items: ReaderItem[]; index: number; left: number } | null>(null);
  const onRead: OnRead = (items, index) => setReader({ items, index, left: root.current?.getBoundingClientRect().right ?? 0 });
  /** Esc or Close: the row of the item last shown gets the keyboard, else the panel. */
  const closeReader = () => {
    const key = reader?.items[reader.index]?.key;
    setReader(null);
    requestAnimationFrame(() => {
      const row = key ? root.current?.querySelector<HTMLElement>(`[data-reader-key="${CSS.escape(key)}"]`) : null;
      (row ?? root.current)?.focus();
    });
  };
  // Another panel or another tab: the reader's items were someone else's.
  useEffect(() => setReader(null), [d.id, tab]);

  useEffect(() => {
    if (d.source === "attention" || !shouldRead(true, visible)) return;
    let alive = true;
    const load = () => {
      setBusy(true);
      panelData<unknown>(tab, d.source)
        .then(
          (r) => alive && setAnswer({ key, reply: r }),
          () => alive && setAnswer({ key, reply: { unavailable: NO_ANSWER } }),
        )
        .finally(() => alive && setBusy(false));
    };
    load();
    const t = setInterval(load, POLL_MS);
    return () => {
      alive = false;
      clearInterval(t);
    };
  }, [d.source, tab, key, visible, refreshSeq, refreshed]);

  useEffect(() => {
    if (focusSeq > 0) root.current?.focus();
  }, [focusSeq]);

  const drag = useRef<{ x: number; w: number } | null>(null);
  const onPointerDown = (e: ReactPointerEvent<HTMLDivElement>) => {
    e.currentTarget.setPointerCapture(e.pointerId);
    drag.current = { x: e.clientX, w: width };
    onDrag("start", width);
  };
  const onPointerMove = (e: ReactPointerEvent<HTMLDivElement>) => {
    if (drag.current) onDrag("move", clampWidth(drag.current.w + e.clientX - drag.current.x, maxWidth));
  };
  const onPointerUp = (e: ReactPointerEvent<HTMLDivElement>) => {
    if (!drag.current) return;
    const w = clampWidth(drag.current.w + e.clientX - drag.current.x, maxWidth);
    drag.current = null;
    onDrag("end", w);
  };
  // A drag the system cancels (pointer capture lost) still ends: the terminal is fitted again.
  const onPointerCancel = () => {
    if (!drag.current) return;
    const w = drag.current.w;
    drag.current = null;
    onDrag("end", w);
  };
  // The edge by keyboard too: arrows move it 16 px, each a finished resize.
  const onEdgeKey = (e: ReactKeyboardEvent<HTMLDivElement>) => {
    const step = e.key === "ArrowLeft" ? -16 : e.key === "ArrowRight" ? 16 : 0;
    if (!step) return;
    e.preventDefault();
    onDrag("end", clampWidth(width + step, maxWidth));
  };

  const nowMs = Date.now();
  const own = d.source === "attention" ? attention : undefined;
  const readAt = own ? own.readAt : reply && "ok" in reply ? reply.read_at : null;
  return (
    <aside
      ref={root}
      data-panel-host={d.id}
      aria-label={`${d.title} panel`}
      tabIndex={-1}
      onKeyDown={(e) => {
        if (e.key === "Escape") {
          e.preventDefault();
          e.stopPropagation();
          onEscape();
        }
      }}
      style={{ width: `${width}px` }}
      className={`panel-in relative flex h-full shrink-0 flex-col border-r border-border bg-background font-ui text-foreground outline-none ${
        overlay ? "absolute inset-y-0 left-0 z-20 shadow-md" : ""
      }`}
    >
      <header className="flex items-start gap-2 border-b border-border px-3 py-2">
        <div className="grid min-w-0 flex-1 gap-0.5">
          <h2 className="m-0 truncate text-[13px] font-medium">{d.title}</h2>
          <span className="flex min-w-0 gap-1 text-[12px] text-muted-foreground">
            <span className="truncate">{d.source === "attention" ? "Every open project" : project}</span>
            {readAt !== null && <span className="shrink-0">· {updatedAgo(readAt, nowMs)}</span>}
          </span>
        </div>
        <Button variant="ghost" size="icon" aria-label="Refresh the panel" aria-busy={busy || undefined} onClick={() => (own ? own.refresh() : setRefreshed((n) => n + 1))}>
          <RefreshCw size={14} aria-hidden="true" />
        </Button>
        <Button variant="ghost" size="icon" aria-label="Close the panel" onClick={onClose}>
          <X size={14} aria-hidden="true" />
        </Button>
      </header>
      <ScrollArea className="min-h-0 flex-1">
        <div className="p-3 text-[13px] leading-[1.5]" aria-busy={reply === null || undefined}>
          <PanelGuard resetKey={`${d.id}:${key}`}>
            <PanelBody d={d} reply={reply} app={app} nowMs={nowMs} attention={own} onRead={onRead} />
          </PanelGuard>
        </div>
      </ScrollArea>
      <div
        role="separator"
        aria-orientation="vertical"
        aria-label="Panel width"
        aria-valuemin={PANEL_MIN}
        aria-valuemax={maxWidth}
        aria-valuenow={width}
        tabIndex={0}
        onPointerDown={onPointerDown}
        onPointerMove={onPointerMove}
        onPointerUp={onPointerUp}
        onPointerCancel={onPointerCancel}
        onLostPointerCapture={onPointerCancel}
        onKeyDown={onEdgeKey}
        className="absolute inset-y-0 -right-1 w-2 cursor-col-resize focus-visible:bg-ring/40 focus-visible:outline-none"
      />
      {reader && (
        <Reader
          items={reader.items}
          index={reader.index}
          modal={modal}
          nowMs={nowMs}
          left={reader.left}
          onIndex={(index) => setReader({ ...reader, index })}
          onClose={closeReader}
        />
      )}
    </aside>
  );
}
