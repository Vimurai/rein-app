// Open tabs remembered and restored (desktop.md §Look and feel v2 V12 components 4–5, D-061;
// E-70): the web side's half. The shell's tabs are written to the record as core ids on every
// open, close, reorder and switch; at start the record is opened in order, the recorded tab
// first; a restored tab whose session ended shows the "not running" card (Start / Close
// project), a gone folder the "missing" card, and an attached tab whose agents are all gone
// the bar (Start again / Close project). Kept out of `shell.tsx` (D-056 addendum 2 item 6).

import { useEffect, useRef } from "react";
import { Button } from "../components/button.tsx";
import { Card } from "../components/card.tsx";
import { Spinner } from "../components/loaders.tsx";
import { agentsGoneText, cardText, recordOf, type RecordTab } from "./restore.ts";
import { projectClose, tabsRecord, tabsRestore, type CardView } from "./ipc.ts";
export { agentsGone } from "./restore.ts";

/** Write the record whenever the tabs (or the current one) change — never before the
 *  restore at start is done, so an empty window does not wipe it. */
export function useTabsRecord(ready: boolean, tabs: () => readonly RecordTab[], current: () => number) {
  const last = useRef("");
  useEffect(() => {
    if (!ready) return;
    const r = recordOf(tabs(), current());
    const key = JSON.stringify(r);
    if (key === last.current) return;
    last.current = key;
    void tabsRecord(r.ids, r.active).catch(() => {});
  });
}

/** At start: the recorded tabs, the recorded current one opened first, then the others in
 *  order, one setup process at a time (V12 "Execution constraints": at most two). `open`
 *  makes the tab in place (in record order) and returns a start function. Then the
 *  command-line project, opened in addition or focused if listed. */
export async function restoreAtStart(
  open: (project: string, restore: boolean) => (() => Promise<void>) | null,
  show: (index: number) => void,
  extra: string | null,
): Promise<number> {
  const rec = await tabsRestore().catch(() => ({ tabs: [] as string[], active: null, reloaded: false }));
  const starts = rec.tabs.map((p) => open(p, true));
  const first = rec.active ?? 0;
  if (rec.tabs.length) show(first);
  const order = [first, ...rec.tabs.map((_, i) => i).filter((i) => i !== first)];
  for (const i of order) {
    const start = starts[i];
    if (start) await start().catch(() => {});
  }
  if (rec.tabs.length) show(first);
  // After a reload (V12 10) the command-line project was handled at the first load already.
  if (extra !== null && !rec.reloaded) open(extra, false)?.();
  return rec.tabs.length;
}

/** The restored tab as the card and the bar act on it (the shell's own tab entry). */
export interface RestoredTab {
  title: string;
  project: string;
  card?: CardView | undefined;
  gone?: boolean | undefined;
  closing?: boolean | undefined;
  term: { tab: { id: number } | null; start: (setupId?: number, cardId?: number) => Promise<void>; restart: () => Promise<void> };
}

/** The card of a restored tab with no session (or no folder): Start runs `rein start` in
 *  place of the card; Close project drops the tab (there is no session to stop). */
export function RestoreCard({ t, rerender, onClose }: { t: RestoredTab; rerender: () => void; onClose: () => void }) {
  const card = t.card;
  // The card has the keyboard whenever its tab is shown: a tab switch gives it to the
  // (hidden) terminal after this renders, so take it back on the next frame.
  const first = useRef<HTMLButtonElement>(null);
  useEffect(() => {
    const id = requestAnimationFrame(() => requestAnimationFrame(() => first.current?.focus()));
    return () => cancelAnimationFrame(id);
  }, [card?.id]);
  if (!card) return null;
  return (
    <div id="restore-card" data-own-keys="" className="absolute inset-0 z-10 flex items-center justify-center bg-background p-6">
      <Card className="grid w-[min(560px,100%)] gap-4 p-5">
        <div className="grid gap-1">
          <h2 className="m-0 text-[15px] font-semibold [overflow-wrap:anywhere] text-foreground">
            {cardText(card.kind, t.title)}
          </h2>
          <code className="font-mono text-[12px] [overflow-wrap:anywhere] text-muted-foreground">{t.project}</code>
        </div>
        <div className="flex justify-end gap-2">
          <Button ref={card.kind !== "not_running" ? first : undefined} onClick={onClose}>
            Close project
          </Button>
          {card.kind === "not_running" && (
            <Button
              ref={first}
              variant="primary"
              onClick={() => {
                t.card = undefined;
                rerender();
                void t.term.start(undefined, card.id);
              }}
            >
              Start
            </Button>
          )}
        </div>
      </Card>
    </div>
  );
}

/** One line above a restored tab whose agents are all gone: Start again is V9's close, then
 *  a fresh open in the same tab (the session is gone, so it runs `rein start`). */
export function AgentsGoneBar({ t, rerender, say, onClose }: { t: RestoredTab; rerender: () => void; say: (text: string, error?: boolean) => void; onClose: () => void }) {
  const busy = t.closing === true;
  const startAgain = () => {
    t.closing = true;
    rerender();
    void projectClose(t.term.tab?.id ?? -1, true)
      .then(
        () => t.term.restart(),
        (e: { message?: string }) => say(e.message ?? String(e), true),
      )
      .finally(() => {
        t.closing = false;
        t.gone = false;
        rerender();
      });
  };
  return (
    <div
      id="agents-gone-bar"
      data-own-keys=""
      role="status"
      className="fade-in flex h-[36px] shrink-0 items-center gap-[8px] border-b border-border bg-muted px-[12px] font-ui text-[13px] text-foreground"
    >
      <span className="min-w-0 flex-1 truncate">{agentsGoneText(t.title)}</span>
      <Button size="sm" onClick={onClose} disabled={busy}>
        Close project
      </Button>
      <Button size="sm" variant="primary" onClick={startAgain} disabled={busy} aria-busy={busy || undefined}>
        {busy && <Spinner size={12} />}
        Start again
      </Button>
    </div>
  );
}
