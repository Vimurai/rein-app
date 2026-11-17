// The reader (status.md §Components 7, D-063; E-74): a task or a handoff read whole, in a
// sheet over the terminal beside the panel. It lies over the stage, so the terminal is never
// resized. Esc or Close hands focus back to the row; Up / Down (and k / j in the modal input
// style) move to the neighbouring item without closing. Its text is structured text only.

import { useEffect, useRef } from "react";
import { Button } from "../components/button.tsx";
import { ChevronDown, ChevronUp, X } from "./icons.ts";
import { labelOf, readerBody, readerKey, step, taskTitle } from "./panel-text.ts";
import { age, type Entry, type Task } from "./panels.ts";
import { InlineText, StructuredText } from "./structured-text.tsx";

export type ReaderItem = { key: string; kind: "task"; task: Task } | { key: string; kind: "handoff"; entry: Entry };

/** A task's chips: tier, owner, what blocks it, and how long since it changed. */
export function taskChips(t: Task, nowMs: number): string[] {
  const chips: string[] = [];
  if (t.tier) chips.push(/^\d+$/.test(t.tier) ? `Tier ${t.tier}` : t.tier);
  if (t.owner) chips.push(t.owner);
  if (t.blocked_by.length) chips.push(`blocked by ${t.blocked_by.join(", ")}`);
  const at = Date.parse(t.updated_at);
  if (!Number.isNaN(at)) chips.push(age(Math.max(0, Math.floor((nowMs - at) / 1000))));
  return chips;
}

export function Chips({ chips }: { chips: string[] }) {
  if (!chips.length) return null;
  return (
    <span className="flex flex-wrap gap-1">
      {chips.map((c) => (
        <span key={c} className="rounded-full border border-border px-2 py-px text-[11px] leading-normal text-muted-foreground [overflow-wrap:anywhere]">
          {c}
        </span>
      ))}
    </span>
  );
}

function Header({ item, nowMs }: { item: ReaderItem; nowMs: number }) {
  if (item.kind === "task") {
    const t = item.task;
    return (
      <div className="grid gap-1.5">
        <span className="font-mono text-[12px] text-muted-foreground">
          {t.id}
          {t.status ? ` · ${t.status}` : ""}
        </span>
        <h3 className="m-0 text-[15px] leading-snug font-semibold [overflow-wrap:anywhere]">{taskTitle(t.description) || "—"}</h3>
        <Chips chips={taskChips(t, nowMs)} />
      </div>
    );
  }
  const e = item.entry;
  return (
    <div className="grid gap-1.5">
      <span className="text-[12px] text-muted-foreground [overflow-wrap:anywhere]">
        {e.title} · {e.when}
      </span>
      <h3 className="m-0 text-[15px] leading-snug font-semibold [overflow-wrap:anywhere]">{e.trigger ? <InlineText text={e.trigger} /> : "Handoff"}</h3>
    </div>
  );
}

/** A task's description (E-82): without the sentence the title already shows, each long
 *  paragraph in pieces of a few sentences, a leading label bold. Only the source's text. */
function TaskBody({ description }: { description: string }) {
  const { pieces } = readerBody(description, taskTitle(description));
  // Nothing beyond the title: nothing more to show; no description at all: a dash.
  if (pieces.length === 0) return description ? null : <p className="m-0 text-muted-foreground">—</p>;
  return (
    <div className="grid gap-3 [overflow-wrap:anywhere]">
      {pieces.map((piece, i) => {
        const text = piece.trim();
        if (text.includes("\n")) return <StructuredText key={i} text={text} />;
        const label = labelOf(text);
        return (
          <p key={i} className="m-0">
            {label && <strong className="font-semibold">{label}</strong>}
            <InlineText text={label ? text.slice(label.length) : text} />
          </p>
        );
      })}
    </div>
  );
}

function Body({ item }: { item: ReaderItem }) {
  if (item.kind === "task") return <TaskBody description={item.task.description} />;
  const e = item.entry;
  return (
    <div className="grid gap-4">
      {e.sections.map((s, i) => (
        <section key={i} aria-label={s.name || "Section"} className="grid gap-1.5">
          {s.name && <h4 className="m-0 text-[12px] font-medium tracking-wide text-muted-foreground uppercase">{s.name}</h4>}
          <StructuredText text={s.text} />
        </section>
      ))}
      {e.cut && <p className="m-0 text-[12px] text-muted-foreground">The rest of this handoff is left out.</p>}
    </div>
  );
}

/** The sheet. `left`: where the panel's right edge is, so the sheet starts there. */
export function Reader({
  items,
  index,
  modal,
  nowMs,
  left,
  onIndex,
  onClose,
}: {
  items: ReaderItem[];
  index: number;
  modal: boolean;
  nowMs: number;
  left: number;
  onIndex: (i: number) => void;
  onClose: () => void;
}) {
  const root = useRef<HTMLElement>(null);
  const item = items[index];
  useEffect(() => {
    root.current?.focus();
    root.current?.querySelector("[data-reader-scroll]")?.scrollTo?.(0, 0);
  }, [index]);
  if (!item) return null;
  const go = (by: -1 | 1) => onIndex(step(index, items.length, by));
  const what = item.kind === "task" ? "Task" : "Handoff";
  return (
    <section
      ref={root}
      role="dialog"
      aria-modal="false"
      aria-label={`${what} ${index + 1} of ${items.length}`}
      data-reader=""
      tabIndex={-1}
      onKeyDown={(e) => {
        const k = readerKey(e.key, modal);
        if (!k) return;
        e.preventDefault();
        e.stopPropagation();
        if (k === "close") onClose();
        else go(k === "prev" ? -1 : 1);
      }}
      style={{ width: `min(720px, calc(100vw - ${Math.round(left)}px - 8px))` }}
      className="absolute inset-y-0 left-full z-30 flex flex-col border-r border-border bg-background text-[13px] leading-[1.5] text-foreground shadow-lg outline-none"
    >
      <header className="flex items-center gap-1 border-b border-border px-4 py-2">
        <span className="flex-1 text-[12px] text-muted-foreground tabular-nums">
          {what} {index + 1} of {items.length}
        </span>
        <Button variant="ghost" size="icon" aria-label="Previous" disabled={index === 0} onClick={() => go(-1)}>
          <ChevronUp size={14} aria-hidden="true" />
        </Button>
        <Button variant="ghost" size="icon" aria-label="Next" disabled={index === items.length - 1} onClick={() => go(1)}>
          <ChevronDown size={14} aria-hidden="true" />
        </Button>
        <Button variant="ghost" size="icon" aria-label="Close the reader" onClick={onClose}>
          <X size={14} aria-hidden="true" />
        </Button>
      </header>
      <div data-reader-scroll="" className="min-h-0 flex-1 overflow-y-auto px-5 py-4">
        <article className="mx-auto grid max-w-[72ch] gap-4">
          <Header item={item} nowMs={nowMs} />
          <Body item={item} />
        </article>
      </div>
    </section>
  );
}
