// The three panel views (panels.md §Components 5, D-056; E-60): summary, list, timeline.
// Every string here came from a project's `.ai/` files through the core, which already cut
// it to its caps and made control and bidi characters visible. It is put on the page as a
// React text child only — never HTML, never Markdown, never a link.

import { useState } from "react";
import { Button } from "../components/button.tsx";
import { KNOWN_GAP, type AttentionItem } from "./attention.ts";
import { ChevronDown, ChevronRight } from "./icons.ts";
import type { Entry, Group, Row, Task } from "./panels.ts";

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

/** One task row, opening its detail in place (the drawer). */
function TaskRow({ t }: { t: Task }) {
  const [open, setOpen] = useState(false);
  return (
    <li className="border-b border-border/50 last:border-b-0">
      <button
        type="button"
        aria-expanded={open}
        onClick={() => setOpen(!open)}
        className="flex w-full items-start gap-2 rounded-md px-1 py-1.5 text-left text-[13px] text-foreground transition-colors duration-[var(--motion-fast)] hover:bg-muted focus-visible:ring-2 focus-visible:ring-ring focus-visible:outline-none"
      >
        {open ? (
          <ChevronDown size={14} aria-hidden="true" className="mt-0.5 shrink-0 text-muted-foreground" />
        ) : (
          <ChevronRight size={14} aria-hidden="true" className="mt-0.5 shrink-0 text-muted-foreground" />
        )}
        <span className="shrink-0 font-mono text-[12px] text-muted-foreground">{t.id}</span>
        <span className={open ? "[overflow-wrap:anywhere]" : "truncate"}>{t.description || "—"}</span>
      </button>
      {open && (
        <dl className="m-0 mb-2 ml-6 grid grid-cols-[auto_1fr] gap-x-3 gap-y-1 text-[12px]">
          <dt className="text-muted-foreground">Status</dt>
          <dd className="m-0 text-foreground">{t.status || "—"}</dd>
          <dt className="text-muted-foreground">Owner</dt>
          <dd className="m-0 text-foreground">{t.owner || "—"}</dd>
          <dt className="text-muted-foreground">Tier</dt>
          <dd className="m-0 text-foreground">{t.tier || "—"}</dd>
          {t.blocked_by.length > 0 && (
            <>
              <dt className="text-muted-foreground">Blocked by</dt>
              <dd className="m-0 [overflow-wrap:anywhere] text-foreground">{t.blocked_by.join(", ")}</dd>
            </>
          )}
          <dt className="text-muted-foreground">Updated</dt>
          <dd className="m-0 text-foreground">{t.updated_at || "—"}</dd>
        </dl>
      )}
    </li>
  );
}

/** Tasks: grouped rows, each opening its detail. */
export function ListView({ groups, cut }: { groups: Group[]; cut: boolean }) {
  return (
    <div className="grid gap-4">
      {groups.map((g) => (
        <section key={g.key} aria-label={g.label} className="grid gap-1">
          <h3 className="m-0 flex items-baseline justify-between text-[12px] font-medium text-muted-foreground">
            {g.label}
            <span className="tabular-nums">{g.rows.length + g.more}</span>
          </h3>
          {g.rows.length === 0 ? (
            <p className="m-0 text-[12px] text-muted-foreground">None</p>
          ) : (
            <ul className="m-0 list-none p-0">
              {g.rows.map((t) => (
                <TaskRow key={t.id} t={t} />
              ))}
            </ul>
          )}
          {g.more > 0 && <p className="m-0 text-[12px] text-muted-foreground">… and {g.more} more</p>}
        </section>
      ))}
      {cut && <p className="m-0 text-[12px] text-muted-foreground">Older tasks are left out.</p>}
    </div>
  );
}

/** One handoff, collapsible; the newest starts open. */
function EntryItem({ e, first }: { e: Entry; first: boolean }) {
  const [open, setOpen] = useState(first);
  return (
    <li className="border-b border-border/50 pb-2 last:border-b-0">
      <button
        type="button"
        aria-expanded={open}
        onClick={() => setOpen(!open)}
        className="grid w-full gap-0.5 rounded-md px-1 py-1.5 text-left transition-colors duration-[var(--motion-fast)] hover:bg-muted focus-visible:ring-2 focus-visible:ring-ring focus-visible:outline-none"
      >
        <span className="flex items-center gap-1.5 text-[13px] font-medium text-foreground">
          {open ? (
            <ChevronDown size={14} aria-hidden="true" className="shrink-0 text-muted-foreground" />
          ) : (
            <ChevronRight size={14} aria-hidden="true" className="shrink-0 text-muted-foreground" />
          )}
          <span className="[overflow-wrap:anywhere]">{e.title}</span>
        </span>
        <span className="pl-5 text-[12px] text-muted-foreground">{e.when}</span>
        {e.trigger && <span className={`pl-5 text-[12px] text-foreground ${open ? "[overflow-wrap:anywhere]" : "truncate"}`}>{e.trigger}</span>}
      </button>
      {open && (
        <div className="grid gap-2 pt-1 pl-6">
          {e.sections.map((s, i) => (
            <section key={i} aria-label={s.name || "Section"} className="grid gap-0.5">
              {s.name && <h4 className="m-0 text-[12px] font-medium text-muted-foreground">{s.name}</h4>}
              <p className="m-0 text-[12px] [overflow-wrap:anywhere] whitespace-pre-wrap text-foreground">{s.text}</p>
            </section>
          ))}
          {e.cut && <p className="m-0 text-[12px] text-muted-foreground">The rest of this handoff is left out.</p>}
        </div>
      )}
    </li>
  );
}

/** Communication: whose turn it is, then the handoffs newest first. */
export function TimelineView({ whoseTurn, entries, cut }: { whoseTurn: string | null; entries: Entry[]; cut: boolean }) {
  return (
    <div className="grid gap-3">
      {whoseTurn !== null && (
        <p className="m-0 rounded-md border border-border px-3 py-2 text-[13px] text-foreground">
          <span className="text-muted-foreground">Whose turn: </span>
          <span className="font-medium [overflow-wrap:anywhere]">{whoseTurn}</span>
        </p>
      )}
      {entries.length === 0 ? (
        <p className="m-0 text-[12px] text-muted-foreground">No handoffs yet.</p>
      ) : (
        <ol aria-label="Handoffs, newest first" className="m-0 grid list-none gap-1 p-0">
          {entries.map((e, i) => (
            <EntryItem key={e.key} e={e} first={i === 0} />
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
