// The three panel views (panels.md §Components 5, D-056; E-60): summary, list, timeline;
// made readable by E-74 (status.md §Components 7, D-063). Every string here came from a
// project's `.ai/` files through the core, which already cut it to its caps and made control
// and bidi characters visible. It is put on the page as a React text child, or through the
// closed structured text (`structured-text.tsx`) — never HTML, never a link, never an image.

import { useState } from "react";
import { Button } from "../components/button.tsx";
import { Input } from "../components/input.tsx";
import { KNOWN_GAP, type AttentionItem } from "./attention.ts";
import { ChevronDown, ChevronRight } from "./icons.ts";
import { Chips, taskChips, type ReaderItem } from "./panel-reader.tsx";
import { filterTasks, firstLines, taskTitle } from "./panel-text.ts";
import { taskGroups, type Entry, type Group, type ListOptions, type Row, type Task, type Tasks } from "./panels.ts";
import { InlineText, StructuredText } from "./structured-text.tsx";

/** Dashboard: key facts as rows, counts by status as chips. */
export function SummaryView({ rows, chips }: { rows: Row[]; chips: { label: string; n: number }[] }) {
  return (
    <div className="grid gap-4">
      {chips.length > 0 && (
        <ul aria-label="Tasks by status" className="m-0 flex list-none flex-wrap gap-2 p-0">
          {chips.map((c) => (
            <li key={c.label} className="rounded-full border border-border px-2.5 py-0.5 text-[12px] text-foreground">
              {c.label} <span className="font-medium tabular-nums">{c.n}</span>
            </li>
          ))}
        </ul>
      )}
      <dl className="m-0 grid gap-2">
        {rows.map((r) => (
          <div key={r.label} className="grid gap-0.5">
            <dt className="text-[12px] text-muted-foreground">{r.label}</dt>
            <dd className="m-0 text-[13px] [overflow-wrap:anywhere] text-foreground">{r.value}</dd>
          </div>
        ))}
      </dl>
    </div>
  );
}

/** Opens the reader on `items[index]`; `from` is the row, which gets focus back. */
export type OnRead = (items: ReaderItem[], index: number) => void;

/** One task row: id, title and chips; activating it opens the reader (status.md §7). */
function TaskRow({ t, nowMs, onOpen }: { t: Task; nowMs: number; onOpen: () => void }) {
  return (
    <li className="border-b border-border/50 last:border-b-0">
      <button
        type="button"
        data-reader-key={`task:${t.id}`}
        onClick={onOpen}
        className="grid w-full gap-1 rounded-md px-1 py-1.5 text-left text-foreground transition-colors duration-[var(--motion-fast)] hover:bg-muted focus-visible:ring-2 focus-visible:ring-ring focus-visible:outline-none"
      >
        <span className="flex min-w-0 items-baseline gap-2">
          <span className="shrink-0 font-mono text-[12px] text-muted-foreground">{t.id}</span>
          <span className="min-w-0 flex-1 [overflow-wrap:anywhere]">{taskTitle(t.description) || "—"}</span>
        </span>
        <span className="pl-[calc(4ch+0.5rem)]">
          <Chips chips={taskChips(t, nowMs)} />
        </span>
      </button>
    </li>
  );
}

/** Tasks: a filter over id and description (the rows already read), groups that collapse —
 *  Done starts collapsed — and rows that open the reader. */
export function ListView({ tasks, options, cut, nowMs, onRead }: { tasks: Tasks; options: ListOptions; cut: boolean; nowMs: number; onRead?: OnRead | undefined }) {
  const [query, setQuery] = useState("");
  const [closed, setClosed] = useState<Record<string, boolean>>({ DONE: true });
  const filtering = query.trim() !== "";
  const groups = taskGroups({ ...tasks, tasks: filterTasks(tasks.tasks, query) }, options);
  const isOpen = (g: Group) => filtering || !closed[g.key];
  // Up / Down in the reader walk the rows as shown: open groups, in order.
  const shown = groups.filter(isOpen).flatMap((g) => g.rows);
  const items: ReaderItem[] = shown.map((t) => ({ key: `task:${t.id}`, kind: "task", task: t }));
  return (
    <div className="grid gap-4">
      <Input
        type="search"
        aria-label="Filter tasks by id or text"
        placeholder="Filter tasks"
        value={query}
        onChange={(e) => setQuery(e.target.value)}
      />
      {groups.map((g) => {
        const open = isOpen(g);
        const n = g.rows.length + g.more;
        if (filtering && n === 0) return null;
        return (
          <section key={g.key} aria-label={g.label} className="grid gap-1">
            <button
              type="button"
              aria-expanded={open}
              onClick={() => setClosed({ ...closed, [g.key]: open })}
              disabled={filtering}
              className="flex items-center gap-1 rounded-md px-1 py-0.5 text-left text-[12px] font-medium text-muted-foreground hover:bg-muted focus-visible:ring-2 focus-visible:ring-ring focus-visible:outline-none disabled:hover:bg-transparent"
            >
              {open ? <ChevronDown size={14} aria-hidden="true" /> : <ChevronRight size={14} aria-hidden="true" />}
              <span className="flex-1">{g.label}</span>
              <span className="tabular-nums">{n}</span>
            </button>
            {open &&
              (g.rows.length === 0 ? (
                <p className="m-0 pl-5 text-[12px] text-muted-foreground">None</p>
              ) : (
                <ul className="m-0 list-none p-0">
                  {g.rows.map((t) => (
                    <TaskRow key={t.id} t={t} nowMs={nowMs} onOpen={() => onRead?.(items, shown.indexOf(t))} />
                  ))}
                </ul>
              ))}
            {open && g.more > 0 && <p className="m-0 pl-5 text-[12px] text-muted-foreground">… and {g.more} more</p>}
          </section>
        );
      })}
      {filtering && groups.every((g) => g.rows.length === 0) && <p className="m-0 text-[12px] text-muted-foreground">No task matches.</p>}
      {cut && <p className="m-0 text-[12px] text-muted-foreground">Older tasks are left out.</p>}
    </div>
  );
}

/** The lines of "Next action needed" a Communication entry shows before "Read all". */
export const NEXT_LINES = 6;

/** One handoff: who → who and when, the trigger as its title, the start of "Next action
 *  needed", and "Read all" for the reader. */
function EntryItem({ e, onOpen }: { e: Entry; onOpen: () => void }) {
  const next = e.sections.find((s) => s.name === "Next action needed");
  const head = next ? firstLines(next.text, NEXT_LINES) : null;
  return (
    <li className="grid gap-1.5 border-b border-border/50 pt-1 pb-3 last:border-b-0">
      <span className="text-[12px] text-muted-foreground [overflow-wrap:anywhere]">
        {e.title} · {e.when}
      </span>
      <span className="font-medium [overflow-wrap:anywhere]">{e.trigger ? <InlineText text={e.trigger} /> : "Handoff"}</span>
      {head && head.text.trim() !== "" && (
        <div className="grid gap-0.5">
          <span className="text-[12px] text-muted-foreground">Next action needed</span>
          <StructuredText text={head.text} />
          {head.more && <span className="text-[12px] text-muted-foreground">…</span>}
        </div>
      )}
      <span>
        <Button variant="default" size="sm" data-reader-key={`handoff:${e.key}`} onClick={onOpen}>
          Read all
        </Button>
      </span>
    </li>
  );
}

/** Communication: whose turn it is, then the handoffs newest first. */
export function TimelineView({ whoseTurn, entries, cut, onRead }: { whoseTurn: string | null; entries: Entry[]; cut: boolean; onRead?: OnRead | undefined }) {
  const items: ReaderItem[] = entries.map((e) => ({ key: `handoff:${e.key}`, kind: "handoff", entry: e }));
  return (
    <div className="grid gap-3">
      {whoseTurn !== null && (
        <p className="m-0 rounded-md border border-border px-3 py-2 text-foreground">
          <span className="text-muted-foreground">Whose turn: </span>
          <span className="font-medium [overflow-wrap:anywhere]">{whoseTurn}</span>
        </p>
      )}
      {entries.length === 0 ? (
        <p className="m-0 text-[12px] text-muted-foreground">No handoffs yet.</p>
      ) : (
        <ol aria-label="Handoffs, newest first" className="m-0 grid list-none gap-1 p-0">
          {entries.map((e, i) => (
            <EntryItem key={e.key} e={e} onOpen={() => onRead?.(items, i)} />
          ))}
        </ol>
      )}
      {cut && <p className="m-0 text-[12px] text-muted-foreground">Older handoffs are left out.</p>}
    </div>
  );
}

/** Needs you (E-61): one row per item across the open tabs, each naming its project; "Go to
 *  tab" only switches tabs. Text from a file is a quote, and a mention is labelled as one. */
export function AttentionView({ items, goTo }: { items: AttentionItem[]; goTo: (tab: number) => void }) {
  if (items.length === 0) {
    return (
      <div className="grid gap-2 text-[12px]">
        <p className="m-0 text-[13px] text-foreground">Nothing needs you right now.</p>
        <p className="m-0 text-muted-foreground">{KNOWN_GAP}</p>
      </div>
    );
  }
  return (
    <ul aria-label="Needs you" className="m-0 grid list-none gap-1 p-0">
      {items.map((it) => (
        <li key={it.key} className="grid gap-1 border-b border-border/50 py-1.5 last:border-b-0">
          <span className="flex items-start gap-2">
            <span className="grid min-w-0 flex-1 gap-0.5">
              <span className="truncate text-[12px] font-medium text-muted-foreground">{it.project}</span>
              <span className="text-[13px] [overflow-wrap:anywhere] text-foreground">{it.text}</span>
            </span>
            {it.tab !== null && (
              <Button variant="default" size="sm" className="shrink-0" aria-label={`Go to tab ${it.tab + 1}, ${it.project}`} onClick={() => goTo(it.tab as number)}>
                Go to tab
              </Button>
            )}
          </span>
          {it.quote !== null && (
            <figure className="m-0 grid gap-0.5">
              {it.kind === "mention" && <figcaption className="text-[11px] text-muted-foreground">Quoted from the handoff — a mention, not a fact</figcaption>}
              <blockquote className="m-0 border-l-2 border-border pl-2 text-[12px] [overflow-wrap:anywhere] whitespace-pre-wrap text-foreground">{it.quote}</blockquote>
            </figure>
          )}
        </li>
      ))}
    </ul>
  );
}
