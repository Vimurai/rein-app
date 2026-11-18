// The update watch drawn (updates.md §Components 1 and 4 for Rein, D-065): the title bar's
// Update button, the ask card, the Update dialog's two rows, and the open tabs asked again
// when the installed Rein changes. The core decides when to look; this file draws what the
// `updates` event says and starts no timer of its own. Motion: CSS only, each effect once,
// none under reduced motion (style.css).

import { useContext, useEffect, useRef, useState } from "react";
import { Button } from "../components/button.tsx";
import { ClosingContext } from "../components/dialog.tsx";
import { ArrowUp, X } from "./icons.ts";
import { listen, projectCheck, updatesGet } from "./ipc.ts";
import { shouldCheck } from "./project.ts";
import { installedChanged, reinAskText, type UpdateRow, type UpdatesState } from "./updates.ts";

/** The watch's state for this web view: asked once at load, then every `updates` event.
 *  `onInstalled` runs when the installed Rein changed under the running window. */
export function useUpdates(onInstalled: () => void): UpdatesState | null {
  const [state, setState] = useState<UpdatesState | null>(null);
  const last = useRef<UpdatesState | null>(null);
  const cb = useRef(onInstalled);
  cb.current = onInstalled;
  useEffect(() => {
    const take = (s: UpdatesState) => {
      const changed = installedChanged(last.current, s);
      last.current = s;
      setState(s);
      if (changed) cb.current();
    };
    void listen<UpdatesState>("updates", take);
    updatesGet().then((s) => last.current === null && take(s), () => {});
  }, []);
  return state;
}

/** What asking rein about one tab needs from the shell's tab entry. */
export interface CheckedTab {
  project: string;
  open: boolean;
  exited: boolean;
  asked?: boolean;
  update?: string[] | undefined;
  term: { tab?: { id: number } | null | undefined; phase(now: number): string };
}

/** V9 3: once the tab has started (never delaying it), rein's verdict — once per open and
 *  once per installed Rein (updates.md component 1). `live`: the tab is still open. */
export function askOutOfDate(entry: CheckedTab, live: () => boolean, rerender: () => void, dontAsk: readonly string[]): Promise<void> {
  const id = entry.term.tab?.id;
  const started = entry.open && !entry.exited && entry.term.phase(performance.now()) === "running";
  if (id === undefined || !shouldCheck({ project: entry.project, started, asked: entry.asked === true }, dontAsk)) return Promise.resolve();
  entry.asked = true;
  return projectCheck(id).then(
    (r) => {
      if (!live() || entry.term.tab?.id !== id) return;
      entry.update = "out_of_date" in r ? r.out_of_date : undefined;
      rerender();
    },
    () => {},
  );
}

/** The installed Rein changed: every open, started tab is asked again, one at a time, in
 *  `order` (the active tab first). A tab no longer behind loses its bar. */
export async function recheckTabs(order: readonly CheckedTab[], live: (t: CheckedTab) => boolean, rerender: () => void, dontAsk: readonly string[]) {
  for (const t of order) {
    if (!live(t)) continue;
    t.asked = false;
    await askOutOfDate(t, () => live(t), rerender, dontAsk);
  }
}

/** The title bar's Update button: slides in and fades (200 ms), its ring pulses twice. */
export function UpdateButton({ tip, onClick }: { tip: string; onClick: () => void }) {
  return (
    <button
      type="button"
      id="update-button"
      aria-label={tip}
      title={tip}
      onClick={onClick}
      className="update-in flex h-[22px] shrink-0 items-center gap-[4px] rounded-full bg-primary px-[8px] text-[12px] font-medium text-primary-foreground transition-colors duration-[var(--motion-base)] hover:opacity-90"
    >
      <ArrowUp size={13} aria-hidden="true" />
      Update
    </button>
  );
}

/** The ask card (component 4): bottom right, not modal, never takes focus from the terminal.
 *  Rises 12 px and fades in; leaves the same way. */
export function UpdateCard({ version, onLater, onUpdate }: { version: string; onLater: () => void; onUpdate: () => void }) {
  const closing = useContext(ClosingContext);
  return (
    <div
      id="update-card"
      role="status"
      aria-live="polite"
      // A click answers without moving focus off the terminal.
      onMouseDown={(e) => e.preventDefault()}
      className={
        (closing ? "card-out " : "card-in ") +
        "fixed right-[12px] bottom-[38px] z-30 grid w-[min(340px,calc(100vw-24px))] gap-[10px] rounded-lg border border-border bg-card p-[12px] font-ui text-[13px] text-card-foreground shadow-lg"
      }
    >
      <div className="flex items-start gap-[8px]">
        <ArrowUp size={15} aria-hidden="true" className="mt-px shrink-0 text-primary" />
        <div className="min-w-0 flex-1">{reinAskText(version)}</div>
        <button type="button" tabIndex={-1} aria-label="Later" className="shrink-0 text-muted-foreground hover:text-foreground" onClick={onLater}>
          <X size={14} aria-hidden="true" />
        </button>
      </div>
      <div className="flex justify-end gap-[8px]">
        <Button size="sm" tabIndex={-1} onClick={onLater}>
          Later
        </Button>
        <Button size="sm" tabIndex={-1} variant="primary" onClick={onUpdate}>
          Update Rein
        </Button>
      </div>
    </div>
  );
}

/** The Update dialog's two rows: Rein and the window, each with what it is and what is newer. */
export function UpdateRows({ rows }: { rows: readonly UpdateRow[] }) {
  return (
    <ul id="update-rows" className="m-0 grid list-none gap-1 rounded-md border border-border bg-background p-3">
      {rows.map((r) => (
        <li key={r.name} className="flex flex-wrap items-baseline gap-x-2">
          <span className="min-w-[128px] font-medium">{r.name}</span>
          <span className="font-mono text-[12px]">{r.from}</span>
          {r.to && (
            <>
              <span className="text-muted-foreground">→</span>
              <span className="font-mono text-[12px] font-semibold text-primary">{r.to}</span>
            </>
          )}
          <span className="ml-auto text-[12px] text-muted-foreground">{r.note}</span>
        </li>
      ))}
    </ul>
  );
}
