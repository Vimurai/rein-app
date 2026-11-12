// The window frame in the modern look (desktop.md §Look and feel v2 V1–V2, E-49): the title
// bar that is the tab bar, the status bar, the toast and the command palette. What each one
// shows and runs comes from the pure `chrome.ts`; this file only draws it. Sizes are in px
// (the 4 px scale of V1). Text goes in as React text nodes only.

import { useContext, useEffect, useLayoutEffect, useRef, useState, type MouseEvent, type ReactNode } from "react";
import { ClosingContext } from "../components/dialog.tsx";
import { Spinner } from "../components/loaders.tsx";
import { Tooltip, TooltipContent, TooltipTrigger } from "../components/tooltip.tsx";
import type { Chrome, HealthIcon, StatusBar as StatusBarData, TabControl, TitleBar as TitleBarData } from "./chrome.ts";
import { titleInset, windowButtons } from "./chrome.ts";
import {
  Check,
  CircleCheck,
  CircleHelp,
  CircleX,
  Copy,
  Menu,
  Minus,
  Plus,
  Square,
  TriangleAlert,
  X,
} from "./icons.ts";
import { minimizeWindow, startDragging, toggleMaximize } from "./ipc.ts";
import { MOTION, reducedMotion } from "./motion.ts";
import type { ActionCall } from "./keymap.ts";

export interface Runner {
  run: (call: ActionCall) => void;
  /** The action's tooltip: plain words and its shortcut. */
  tip: (call: ActionCall) => string;
}

/** A button with a tooltip; its accessible name is the tooltip's text. */
function TipButton({
  tip,
  className,
  onClick,
  children,
}: {
  tip: string;
  className: string;
  onClick: () => void;
  children: ReactNode;
}) {
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <button type="button" aria-label={tip} className={className} onClick={onClick}>
          {children}
        </button>
      </TooltipTrigger>
      <TooltipContent>{tip}</TooltipContent>
    </Tooltip>
  );
}

/** Text that loses its START when it does not fit, so the end stays visible: a path keeps the
 *  project's own directory, a command keeps `&& rein start` (V2, E-50). The ellipsis is
 *  drawn by the browser at the left of a right-to-left box; the text itself is an isolated
 *  left-to-right run, so its slashes and punctuation keep their places. */
export function TruncStart({ text, className = "" }: { text: string; className?: string }) {
  return (
    <span className={`block min-w-0 truncate text-left [direction:rtl] ${className}`.trim()}>
      <span className="[direction:ltr] [unicode-bidi:isolate]">{text}</span>
    </span>
  );
}

const ICON_BUTTON =
  "flex size-[28px] shrink-0 items-center justify-center rounded-md text-muted-foreground transition-colors duration-[var(--motion-base)] hover:bg-muted hover:text-foreground focus-visible:outline-2 focus-visible:outline-ring";

/** Dragging the empty part of the bar moves the window; a double click maximises (V2). */
function onDragRegion(e: MouseEvent<HTMLDivElement>) {
  if (e.button !== 0 || e.target !== e.currentTarget) return;
  if (e.detail === 2) void toggleMaximize().catch(() => {});
  else void startDragging().catch(() => {});
}

/** The accent underline slides from the tab that was current to the new one (V7). Web
 *  Animations on the new underline — a transform from the old one's place to its own — so no
 *  style attribute and no stylesheet is written at run time. */
function useUnderlineSlide(row: React.RefObject<HTMLDivElement | null>, current: number) {
  const last = useRef<DOMRect | null>(null);
  useLayoutEffect(() => {
    const line = row.current?.querySelector<HTMLElement>("[data-underline]");
    const now = line?.getBoundingClientRect() ?? null;
    const from = last.current;
    last.current = now;
    if (!line || !now || !from || !now.width || reducedMotion()) return;
    const dx = from.left - now.left;
    if (Math.abs(dx) < 1) return;
    line.animate(
      [{ transform: `translateX(${dx}px) scaleX(${from.width / now.width})` }, { transform: "none" }],
      { duration: MOTION.base, easing: MOTION.ease },
    );
  }, [row, current]);
}

/** Tabs that were just closed (V7): each stays for --motion-base where it was, as an inert
 *  pill that shrinks out. None under reduced motion. */
function useLeavingTabs(tabs: readonly TabControl[]): { tab: TabControl; at: number }[] {
  const prev = useRef<readonly TabControl[]>(tabs);
  const [leaving, setLeaving] = useState<{ tab: TabControl; at: number }[]>([]);
  const timers = useRef<ReturnType<typeof setTimeout>[]>([]);
  const ids = tabs.map((t) => t.id).join(",");
  useLayoutEffect(() => {
    const now = new Set(tabs.map((t) => t.id));
    const gone = prev.current.flatMap((tab, at) => (now.has(tab.id) ? [] : [{ tab, at }]));
    prev.current = tabs;
    if (!gone.length || reducedMotion()) return;
    setLeaving((l) => [...l, ...gone]);
    timers.current.push(setTimeout(() => setLeaving((l) => l.filter((x) => !gone.includes(x))), MOTION.base));
    // Only the tab ids decide; the timers outlive later renders.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [ids]);
  useEffect(() => () => timers.current.forEach(clearTimeout), []);
  return leaving;
}

/** The open tabs with the ones leaving put back where they were. */
function withLeaving(tabs: readonly TabControl[], leaving: readonly { tab: TabControl; at: number }[]) {
  const out: { tab: TabControl; gone: boolean }[] = tabs.map((tab) => ({ tab, gone: false }));
  for (const l of [...leaving].sort((a, b) => a.at - b.at)) out.splice(Math.min(l.at, out.length), 0, { tab: l.tab, gone: true });
  return out;
}

/** A closed tab on its way out: its label, shrinking from its own width to none (Web
 *  Animations — no style attribute or stylesheet is written). */
function LeavingTab({ label }: { label: string }) {
  const el = useRef<HTMLDivElement>(null);
  useLayoutEffect(() => {
    const e = el.current;
    if (!e) return;
    e.animate(
      [
        { maxWidth: `${e.getBoundingClientRect().width}px`, opacity: 1 },
        { maxWidth: "0px", opacity: 0, paddingLeft: "0px", paddingRight: "0px" },
      ],
      { duration: MOTION.base, easing: MOTION.ease, fill: "forwards" },
    );
  }, []);
  return (
    <div
      ref={el}
      aria-hidden="true"
      className="pointer-events-none flex h-[28px] max-w-[220px] shrink-0 items-center overflow-hidden rounded-md pr-[4px] pl-[10px] whitespace-nowrap text-muted-foreground"
    >
      {label}
    </div>
  );
}

export function TitleBar({ bar, chrome, r }: { bar: TitleBarData; chrome: Chrome; r: Runner }) {
  const inset = titleInset(chrome);
  const row = useRef<HTMLDivElement>(null);
  useUnderlineSlide(row, bar.tabs.findIndex((t) => t.current));
  const leaving = useLeavingTabs(bar.tabs);
  return (
    <div
      id="titlebar"
      className="flex h-[40px] shrink-0 items-center gap-[4px] border-b border-border bg-background pl-[8px] font-ui text-[13px] text-foreground select-none"
      style={inset ? { paddingLeft: `${inset}px` } : undefined}
    >
      <div ref={row} role="tablist" aria-label="Projects" className="flex min-w-0 items-center gap-[4px]">
        {withLeaving(bar.tabs, leaving).map(({ tab: t, gone }) =>
          gone ? (
            <LeavingTab key={`gone-${t.id}`} label={t.label} />
          ) : (
          <Tooltip key={t.id}>
            <TooltipTrigger asChild>
              <div
                role="tab"
                aria-selected={t.current}
                aria-label={`${t.label} — ${r.tip(t.select)}`}
                aria-busy={t.starting || undefined}
                className={
                  "tab-in group relative flex h-[28px] min-w-0 max-w-[220px] cursor-pointer items-center gap-[6px] overflow-hidden rounded-md pr-[4px] pl-[10px] transition-colors duration-[var(--motion-base)] " +
                  (t.current ? "bg-card text-card-foreground" : "text-muted-foreground hover:bg-muted/60 hover:text-foreground")
                }
                onClick={() => r.run(t.select)}
                onMouseDown={(e) => {
                  if (e.button === 1) e.preventDefault(); // no autoscroll on a middle click
                }}
                onAuxClick={(e) => {
                  if (e.button === 1) r.run(t.close);
                }}
              >
                {t.exited && <span aria-label="exited" className="size-[6px] shrink-0 rounded-full bg-destructive" />}
                <span className="truncate">{t.label}</span>
                {/* Starting (V7): a spinner in place of the close icon, which hover still shows. */}
                {t.starting && (
                  <span className="flex size-[18px] shrink-0 items-center justify-center text-muted-foreground group-hover:hidden group-focus-within:hidden">
                    <Spinner size={12} />
                  </span>
                )}
                <button
                  type="button"
                  aria-label={r.tip(t.close)}
                  className={
                    "size-[18px] shrink-0 items-center justify-center rounded-md hover:bg-muted " +
                    (t.starting ? "hidden group-hover:flex group-focus-within:flex " : "flex ") +
                    "opacity-0 group-hover:opacity-70 group-focus-within:opacity-70 hover:opacity-100 focus-visible:opacity-100"
                  }
                  onClick={(e) => {
                    e.stopPropagation();
                    r.run(t.close);
                  }}
                >
                  <X size={14} aria-hidden="true" />
                </button>
                {/* The active tab: a 2 px accent underline (V2). */}
                {t.current && (
                  <span data-underline aria-hidden="true" className="absolute right-[8px] bottom-0 left-[8px] h-[2px] origin-left rounded-full bg-primary" />
                )}
              </div>
            </TooltipTrigger>
            <TooltipContent>{r.tip(t.select)}</TooltipContent>
          </Tooltip>
        ))}
        <TipButton tip={r.tip(bar.add)} className={ICON_BUTTON} onClick={() => r.run(bar.add)}>
          <Plus size={16} aria-hidden="true" />
        </TipButton>
      </div>
      <div className="h-full min-w-[24px] flex-1" onMouseDown={onDragRegion} aria-hidden="true" />
      <TipButton
        tip={r.tip(bar.version.action)}
        className="flex h-[22px] shrink-0 items-center rounded-md border border-border px-[8px] text-[12px] text-muted-foreground transition-colors duration-[var(--motion-base)] hover:bg-muted hover:text-foreground"
        onClick={() => r.run(bar.version.action)}
      >
        {bar.version.label}
      </TipButton>
      <TipButton tip={r.tip(bar.menu)} className={ICON_BUTTON} onClick={() => r.run(bar.menu)}>
        <Menu size={16} aria-hidden="true" />
      </TipButton>
      <WindowButtons chrome={chrome} onClose={() => r.run({ name: "app.quit" })} />
      {windowButtons(chrome).length === 0 && <div className="w-[4px]" />}
    </div>
  );
}

const WINDOW_BUTTON =
  "flex h-[40px] w-[46px] shrink-0 items-center justify-center text-muted-foreground transition-colors duration-[var(--motion-base)] hover:bg-muted hover:text-foreground";

/** Windows only (V2): minimise, maximise, close — 46 × 40 px, close turns red on hover. */
function WindowButtons({ chrome, onClose }: { chrome: Chrome; onClose: () => void }) {
  const kinds = windowButtons(chrome);
  if (!kinds.length) return null;
  return (
    <div className="ml-[4px] flex shrink-0">
      {kinds.map((k) =>
        k === "minimize" ? (
          <button key={k} type="button" aria-label="Minimise" title="Minimise" className={WINDOW_BUTTON} onClick={() => void minimizeWindow().catch(() => {})}>
            <Minus size={16} aria-hidden="true" />
          </button>
        ) : k === "maximize" ? (
          <button key={k} type="button" aria-label="Maximise" title="Maximise" className={WINDOW_BUTTON} onClick={() => void toggleMaximize().catch(() => {})}>
            <Square size={13} aria-hidden="true" />
          </button>
        ) : (
          <button
            key={k}
            type="button"
            aria-label="Close the window"
            title="Close the window"
            className={`${WINDOW_BUTTON} hover:bg-destructive hover:text-background`}
            onClick={onClose}
          >
            <X size={16} aria-hidden="true" />
          </button>
        ),
      )}
    </div>
  );
}

function HealthGlyph({ icon }: { icon: HealthIcon }) {
  const p = { size: 13, "aria-hidden": true } as const;
  switch (icon) {
    case "ok":
      return <CircleCheck {...p} className="text-ansi-10" />;
    case "warn":
      return <TriangleAlert {...p} className="text-ansi-11" />;
    case "fail":
      return <CircleX {...p} className="text-destructive" />;
    case "unknown":
      return <CircleHelp {...p} />;
    default:
      return <Spinner size={12} />;
  }
}

const MODE_CLASS: Record<string, string> = {
  TERMINAL: "mode-terminal",
  NORMAL: "mode-normal",
  COMMAND: "mode-command",
};

export function StatusBar({ bar, r }: { bar: StatusBarData; r: Runner }) {
  return (
    <div
      id="statusbar"
      className="flex h-[26px] shrink-0 items-center gap-[12px] border-t border-border bg-background px-[8px] font-ui text-[12px] text-muted-foreground select-none"
    >
      {bar.project && (
        <TipButton tip={r.tip(bar.project.action)} className="min-w-0 rounded-md px-[4px] font-mono hover:bg-muted hover:text-foreground" onClick={() => r.run(bar.project!.action)}>
          <TruncStart text={bar.project.text} />
        </TipButton>
      )}
      <TipButton
        tip={r.tip(bar.health.action)}
        className="flex shrink-0 items-center gap-[6px] rounded-md px-[6px] py-[2px] hover:bg-muted hover:text-foreground"
        onClick={() => r.run(bar.health.action)}
      >
        <span aria-busy={bar.health.icon === "checking" || undefined} className="flex items-center gap-[6px]">
          {/* Keyed by state: the new icon and words fade in (V7). */}
          <span key={bar.health.icon} className="fade-in flex items-center gap-[6px]">
            <HealthGlyph icon={bar.health.icon} />
            <span role="status">{bar.health.text}</span>
          </span>
        </span>
      </TipButton>
      <div className="flex-1" />
      {bar.starting && (
        <span aria-busy="true" className="flex shrink-0 items-center gap-[6px]">
          <Spinner size={12} />
          <span role="status">Starting rein…</span>
        </span>
      )}
      {!bar.starting && bar.command && (
        <div className="flex min-w-0 items-center gap-[2px] rounded-md border border-border pl-[8px] font-mono">
          <TruncStart text={bar.command.text} />
          <TipButton tip={r.tip(bar.command.copy)} className="flex size-[20px] shrink-0 items-center justify-center rounded-md hover:bg-muted hover:text-foreground" onClick={() => r.run(bar.command!.copy)}>
            {bar.command.copied ? (
              <Check size={12} aria-hidden="true" className="fade-in text-ansi-10" />
            ) : (
              <Copy size={12} aria-hidden="true" />
            )}
          </TipButton>
        </div>
      )}
      {bar.mode && (
        <span className={`shrink-0 rounded-md px-[8px] text-[11px] font-semibold ${MODE_CLASS[bar.mode] ?? ""}`}>{bar.mode}</span>
      )}
    </div>
  );
}

/** Messages (errors, "copied", …): bottom right, 4 s, read out politely (V2). The live region
 *  stays mounted so a screen reader hears each new message. */
export function Toast({ message }: { message: { text: string; error: boolean; seq: number; leaving?: boolean } | null }) {
  return (
    <div aria-live="polite" role="status" className="pointer-events-none fixed right-[12px] bottom-[38px] z-40 flex max-w-[min(70ch,80vw)] justify-end">
      {message && (
        <div
          key={message.seq}
          className={
            (message.leaving ? "toast-out " : "toast-in ") +
            "pointer-events-auto flex items-start gap-[8px] rounded-lg border bg-popover px-[12px] py-[8px] font-ui text-[13px] text-popover-foreground shadow-md whitespace-pre-wrap " +
            (message.error ? "border-destructive" : "border-border")
          }
        >
          {/* An error is marked by the red border and icon; its text stays at full contrast
              (the theme's red on a popover is under 4.5:1 in some themes). */}
          {message.error && <CircleX size={14} aria-hidden="true" className="mt-[2px] shrink-0 text-destructive" />}
          <span>{message.text}</span>
        </div>
      )}
    </div>
  );
}

/** `modal` style, COMMAND mode: the command line as a palette anchored at the bottom, with the
 *  current completions above it. Keys are still handled by the shell (`modes`). */
export function CommandPalette({ text, items, pending = false }: { text: string; items: readonly string[]; pending?: boolean }) {
  const closing = useContext(ClosingContext); // leaving: `Exit` keeps it for its exit animation
  return (
    <div className={(closing ? "palette-out" : "palette-in") + " fixed bottom-[38px] left-1/2 z-40 w-[min(640px,90vw)] -translate-x-1/2 overflow-hidden rounded-lg border border-border bg-popover font-mono text-[13px] text-popover-foreground shadow-md"}>
      {items.length > 1 && (
        <ul aria-label="Completions" className="max-h-[200px] overflow-hidden border-b border-border py-[4px]">
          {items.slice(0, 8).map((i) => (
            <li key={i} className="truncate px-[12px] py-[2px] text-muted-foreground">
              {i}
            </li>
          ))}
        </ul>
      )}
      <div className="flex items-center px-[12px] py-[8px]" aria-label="Command line">
        <span className="text-primary">:</span>
        <span className="whitespace-pre">{text}</span>
        <span className="cursor ml-px inline-block h-[1.1em] w-[0.55em]" aria-hidden="true" />
        {/* `:open` Tab completion asks the core for directories (V7): a spinner while it does. */}
        {pending && (
          <span aria-busy="true" className="ml-auto flex items-center gap-[6px] font-ui text-[12px] text-muted-foreground">
            <Spinner size={12} />
            <span role="status">Completing…</span>
          </span>
        )}
      </div>
    </div>
  );
}
