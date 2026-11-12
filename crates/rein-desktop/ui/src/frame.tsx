// The window frame in the modern look (desktop.md §Look and feel v2 V1–V2, E-49): the title
// bar that is the tab bar, the status bar, the toast and the command palette. What each one
// shows and runs comes from the pure `chrome.ts`; this file only draws it. Sizes are in px
// (the 4 px scale of V1). Text goes in as React text nodes only.

import type { MouseEvent, ReactNode } from "react";
import { Tooltip, TooltipContent, TooltipTrigger } from "../components/tooltip.tsx";
import type { Chrome, HealthIcon, StatusBar as StatusBarData, TitleBar as TitleBarData } from "./chrome.ts";
import { titleInset, windowButtons } from "./chrome.ts";
import {
  CircleCheck,
  CircleHelp,
  CircleX,
  Copy,
  LoaderCircle,
  Menu,
  Minus,
  Plus,
  Square,
  TriangleAlert,
  X,
} from "./icons.ts";
import { minimizeWindow, startDragging, toggleMaximize } from "./ipc.ts";
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
  "flex size-[28px] shrink-0 items-center justify-center rounded-md text-muted-foreground transition-colors duration-150 hover:bg-muted hover:text-foreground focus-visible:outline-2 focus-visible:outline-ring";

/** Dragging the empty part of the bar moves the window; a double click maximises (V2). */
function onDragRegion(e: MouseEvent<HTMLDivElement>) {
  if (e.button !== 0 || e.target !== e.currentTarget) return;
  if (e.detail === 2) void toggleMaximize().catch(() => {});
  else void startDragging().catch(() => {});
}

export function TitleBar({ bar, chrome, r }: { bar: TitleBarData; chrome: Chrome; r: Runner }) {
  const inset = titleInset(chrome);
  return (
    <div
      id="titlebar"
      className="flex h-[40px] shrink-0 items-center gap-[4px] border-b border-border bg-background pl-[8px] font-ui text-[13px] text-foreground select-none"
      style={inset ? { paddingLeft: `${inset}px` } : undefined}
    >
      <div role="tablist" aria-label="Projects" className="flex min-w-0 items-center gap-[4px]">
        {bar.tabs.map((t) => (
          <Tooltip key={t.select.arg}>
            <TooltipTrigger asChild>
              <div
                role="tab"
                aria-selected={t.current}
                aria-label={`${t.label} — ${r.tip(t.select)}`}
                className={
                  "group relative flex h-[28px] min-w-0 max-w-[220px] cursor-pointer items-center gap-[6px] rounded-md pr-[4px] pl-[10px] transition-colors duration-150 " +
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
                <button
                  type="button"
                  aria-label={r.tip(t.close)}
                  className={
                    "flex size-[18px] shrink-0 items-center justify-center rounded-md hover:bg-muted " +
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
                {t.current && <span aria-hidden="true" className="absolute right-[8px] bottom-0 left-[8px] h-[2px] rounded-full bg-primary" />}
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
        className="flex h-[22px] shrink-0 items-center rounded-md border border-border px-[8px] text-[12px] text-muted-foreground transition-colors duration-150 hover:bg-muted hover:text-foreground"
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
  "flex h-[40px] w-[46px] shrink-0 items-center justify-center text-muted-foreground transition-colors duration-150 hover:bg-muted hover:text-foreground";

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
      return <LoaderCircle {...p} />;
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
        <HealthGlyph icon={bar.health.icon} />
        <span>{bar.health.text}</span>
      </TipButton>
      <div className="flex-1" />
      {bar.command && (
        <div className="flex min-w-0 items-center gap-[2px] rounded-md border border-border pl-[8px] font-mono">
          <TruncStart text={bar.command.text} />
          <TipButton tip={r.tip(bar.command.copy)} className="flex size-[20px] shrink-0 items-center justify-center rounded-md hover:bg-muted hover:text-foreground" onClick={() => r.run(bar.command!.copy)}>
            <Copy size={12} aria-hidden="true" />
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
export function Toast({ message }: { message: { text: string; error: boolean; seq: number } | null }) {
  return (
    <div aria-live="polite" role="status" className="pointer-events-none fixed right-[12px] bottom-[38px] z-40 flex max-w-[min(70ch,80vw)] justify-end">
      {message && (
        <div
          key={message.seq}
          className={
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
export function CommandPalette({ text, items }: { text: string; items: readonly string[] }) {
  return (
    <div className="fixed bottom-[38px] left-1/2 z-40 w-[min(640px,90vw)] -translate-x-1/2 overflow-hidden rounded-lg border border-border bg-popover font-mono text-[13px] text-popover-foreground shadow-md">
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
      </div>
    </div>
  );
}
