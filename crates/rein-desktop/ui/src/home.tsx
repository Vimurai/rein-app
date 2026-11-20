// Home — every project at a glance (status.md §Home H2–H3, D-064; E-80): the view that fills
// the stage while Home is selected, and with no project open. Top to bottom: the summary line,
// "Needs you now", one card per open project (ordered by state; a card never moves under the
// pointer or keyboard focus), then the start screen's recent projects and installation rows.
// It runs no command, writes nothing and types into no terminal: a card or a row only switches
// tabs (or dismisses a row the core sent as dismissable). No poll and no timer of its own.

import { useCallback, useReducer, useRef, type MutableRefObject } from "react";
import { Button } from "../components/button.tsx";
import { Tooltip, TooltipContent, TooltipTrigger } from "../components/tooltip.tsx";
import type { Item } from "./attention.ts";
import type { Runner } from "./frame.tsx";
import { cardModel, cardOrder, heldOrder, nowRow, summaryLine, type CardModel } from "./home.ts";
import { Bell } from "./icons.ts";
import { StartScreen } from "./start-screen.tsx";
import type { StartScreen as StartModel } from "./start.ts";
import { statusMark, type ProjectStatus } from "./status.ts";
import { StatusGlyph } from "./tab-status.tsx";

/** Whether Home is shown, and how to show it (H1, H2). It starts shown: with no project
 *  open the window shows Home, and start-up decides once the restore is done (H5). Showing a
 *  tab hides it (`shown.current = false` in the shell's showTab). Going Home takes the
 *  keyboard from the terminal and resizes and closes no pty: the panes are only hidden. */
export function useHome(rerender: () => void, terminal: () => { blur: () => void } | undefined): { shown: MutableRefObject<boolean>; show: () => void } {
  const shown = useRef(true);
  const term = useRef(terminal);
  term.current = terminal;
  const show = useCallback(() => {
    term.current()?.blur();
    shown.current = true;
    rerender();
  }, [rerender]);
  return { shown, show };
}

/** One open tab as Home sees it. */
export interface HomeTab {
  /** Stable for the tab's life (the card's key). */
  id: number;
  /** Its place in the tab bar. */
  index: number;
  title: string;
  status: ProjectStatus | undefined;
}

export interface HomeProps {
  tabs: readonly HomeTab[];
  /** The Now items of every project (the side menu's `attention_items` answer). */
  now: readonly Item[];
  /** A dismissable row's Dismiss; null when rows cannot be dismissed here. */
  dismiss: ((tab: number, key: string) => void) | null;
  goTo: (index: number) => void;
  start: StartModel;
  r: Runner;
}

export function HomeView({ tabs, now, dismiss, goTo, start, r }: HomeProps) {
  const summary = summaryLine(tabs.map((t) => t.status?.state ?? "unknown"));
  if (tabs.length === 0) return <StartScreen model={start} r={r} />;
  return (
    <div id="home" className="h-full overflow-y-auto px-6 font-ui text-[13px] text-foreground">
      <div className="mx-auto flex w-full max-w-[1240px] min-w-0 flex-col gap-6 py-8">
        <header className="grid gap-1">
          <h1 className="m-0 text-[18px] leading-6 font-semibold tracking-tight">Home</h1>
          {summary && (
            <p role="status" className="m-0 text-muted-foreground">
              {summary}
            </p>
          )}
        </header>
        {now.length > 0 && <NeedsYouNow items={now} tabs={tabs} dismiss={dismiss} goTo={goTo} />}
        <Cards tabs={tabs} goTo={goTo} />
        <StartScreen model={start} r={r} embedded />
      </div>
    </div>
  );
}

/** H2 "Needs you now": each Now item a row that goes to its tab, Dismiss where allowed. R3
 *  (status.md "Home, second revision", D-071): a row never widens Home — the name and the
 *  sentence on the first line, cut; the quote under it, wrapped to three lines; the whole row
 *  as its tooltip. The one column is minmax(0, 1fr), so no row's text sizes the track. */
function NeedsYouNow({ items, tabs, dismiss, goTo }: { items: readonly Item[]; tabs: readonly HomeTab[]; dismiss: HomeProps["dismiss"]; goTo: (i: number) => void }) {
  const title = (index: number | null) => (index === null ? "Installation" : (tabs.find((t) => t.index === index)?.title ?? `Tab ${index + 1}`));
  return (
    <section aria-label="Needs you now" className="grid min-w-0 grid-cols-[minmax(0,1fr)] gap-2">
      <h2 className="m-0 text-[12px] font-medium text-muted-foreground">Needs you now</h2>
      <ul className="m-0 grid list-none grid-cols-[minmax(0,1fr)] gap-1 p-0">
        {items.map((it) => {
          const row = nowRow(title(it.index), it);
          return (
            <li key={`${it.tab ?? "x"}:${it.key}`} className="flex min-h-10 min-w-0 items-start gap-2 rounded-md border border-border bg-card px-2 py-1 text-card-foreground">
              <Tooltip>
                <TooltipTrigger asChild>
                  <button
                    type="button"
                    disabled={it.index === null}
                    onClick={() => it.index !== null && goTo(it.index)}
                    className="grid min-w-0 flex-1 cursor-pointer gap-0.5 rounded-md px-1 py-1 text-left outline-none hover:bg-muted focus-visible:ring-2 focus-visible:ring-ring disabled:cursor-default disabled:hover:bg-transparent"
                  >
                    <span className="flex min-w-0 items-center gap-2">
                      <Bell size={13} aria-hidden="true" className="shrink-0 text-primary" />
                      <span className="min-w-0 max-w-[45%] truncate font-medium">{row.name}</span>
                      <span className="min-w-0 truncate text-muted-foreground">{row.sentence}</span>
                    </span>
                    {row.quote && <span className="line-clamp-3 min-w-0 pl-[21px] [overflow-wrap:anywhere] text-muted-foreground">{row.quote}</span>}
                  </button>
                </TooltipTrigger>
                <TooltipContent className="max-w-[min(480px,calc(100vw-16px))] whitespace-normal [overflow-wrap:anywhere]">{row.tip}</TooltipContent>
              </Tooltip>
              {dismiss && it.dismissable && it.tab !== null && (
                <Button size="sm" variant="ghost" onClick={() => it.tab !== null && dismiss(it.tab, it.key)}>
                  Dismiss
                </Button>
              )}
            </li>
          );
        })}
      </ul>
    </section>
  );
}

/** H2 "Open projects": the cards in state order; held while the pointer is over the grid or a
 *  card has keyboard focus, so a card never moves under a click. */
function Cards({ tabs, goTo }: { tabs: readonly HomeTab[]; goTo: (i: number) => void }) {
  const [, rerender] = useReducer((n: number) => n + 1, 0);
  const held = useRef({ pointer: false, focus: false });
  const drawn = useRef<number[]>([]);
  const byIndex = new Map(tabs.map((t) => [t.index, t]));
  const next = cardOrder(tabs.map((t) => ({ index: t.index, state: t.status?.state ?? "unknown" }))).map((i) => byIndex.get(i)!.id);
  const order = heldOrder(drawn.current, next, held.current.pointer || held.current.focus);
  drawn.current = order;
  const byId = new Map(tabs.map((t) => [t.id, t]));
  const release = (k: "pointer" | "focus") => {
    held.current[k] = false;
    rerender();
  };
  return (
    <section aria-label="Open projects" className="grid min-w-0 grid-cols-[minmax(0,1fr)] gap-2">
      <h2 className="m-0 text-[12px] font-medium text-muted-foreground">Open projects</h2>
      <div
        data-home-grid
        className="grid grid-cols-[repeat(auto-fill,minmax(280px,1fr))] gap-3 max-[720px]:grid-cols-[minmax(0,1fr)]"
        onPointerEnter={() => (held.current.pointer = true)}
        onPointerLeave={() => release("pointer")}
        onFocus={() => (held.current.focus = true)}
        onBlur={(e) => {
          if (!e.currentTarget.contains(e.relatedTarget as Node | null)) release("focus");
        }}
      >
        {order.map((id) => {
          const t = byId.get(id);
          return t ? <Card key={id} t={t} m={cardModel(t.title, t.status)} onGo={() => goTo(t.index)} /> : null;
        })}
      </div>
    </section>
  );
}

const CHIP = "rounded-full border border-border px-2 text-[11px] leading-5 text-muted-foreground tabular-nums";

/** H3: one project — a button that goes to its tab. Every string is a text node. */
function Card({ t, m, onGo }: { t: HomeTab; m: CardModel; onGo: () => void }) {
  const mark = statusMark(t.title, t.status, true);
  return (
    <button
      type="button"
      onClick={onGo}
      aria-label={`${m.name}: ${m.sentence} — go to its tab`}
      data-state={m.state}
      className="grid min-w-0 cursor-pointer content-start gap-1.5 rounded-lg border border-border bg-card p-3 text-left text-card-foreground outline-none transition-colors duration-[var(--motion-base)] hover:bg-muted/60 focus-visible:ring-2 focus-visible:ring-ring"
    >
      <span className="flex min-w-0 items-center gap-2">
        {mark && <StatusGlyph s={mark} />}
        <span className="truncate font-semibold">{m.name}</span>
        <span className="ml-auto shrink-0 text-[12px] text-muted-foreground">{m.sentence}</span>
        {m.now > 0 && (
          <span className="shrink-0 rounded-full bg-primary px-1.5 text-[11px] leading-4 font-medium text-primary-foreground tabular-nums">{m.now}</span>
        )}
      </span>
      <span className="truncate text-muted-foreground">{m.task ?? "Tasks not available"}</span>
      {m.turn && <span className="truncate text-[12px] text-muted-foreground">{m.turn}</span>}
      {m.trigger && <span className="truncate text-[12px] text-muted-foreground">{m.trigger}</span>}
      {m.agents.length > 0 && <span className="truncate font-mono text-[11px] text-muted-foreground">{m.agents.join(" · ")}</span>}
      {m.chips && (
        <span className="flex flex-wrap gap-1 pt-0.5">
          <span className={CHIP}>{m.chips.open} open</span>
          <span className={CHIP}>{m.chips.in_progress} in progress</span>
          <span className={CHIP}>{m.chips.blocked} blocked</span>
        </span>
      )}
    </button>
  );
}
