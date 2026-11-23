// The update watch drawn (updates.md §Components 1 and 4 for Rein, D-065): the title bar's
// Update button, the ask card, the Update dialog's two rows, and the open tabs asked again
// when the installed Rein changes. The core decides when to look; this file draws what the
// `updates` event says and starts no timer of its own. Motion: CSS only, each effect once,
// none under reduced motion (style.css).

import { useContext, useEffect, useRef, useState, type ReactNode } from "react";
import { Button } from "../components/button.tsx";
import { ClosingContext, Exit } from "../components/dialog.tsx";
import { ArrowUp, Check, TriangleAlert, Copy } from "./icons.ts";
import { appUpdateCancel, appUpdateStart, clipboardWrite, listen, projectCheck, updatesGet, updatesLater } from "./ipc.ts";
import { shouldCheck } from "./project.ts";
import { Panel } from "./release.tsx";
import { StructuredText } from "./structured-text.tsx";
import {
  appCard,
  appCardText,
  appNewer,
  BY_HAND_TEXT,
  installedChanged,
  reinAsk,
  reinAskText,
  runningNotes,
  updatedTo,
  type AppCard,
  type UpdateRow,
  type UpdatesState,
} from "./updates.ts";

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

/** A card of the stack (component 4): not modal, never takes focus from the terminal — a click
 *  answers without moving focus off it, and its buttons are not tab stops. Rises 12 px and
 *  fades in; leaves the same way. */
function Card({ id, icon, children, actions }: { id: string; icon: ReactNode; children: ReactNode; actions?: ReactNode }) {
  const closing = useContext(ClosingContext);
  return (
    <div
      id={id}
      role="status"
      aria-live="polite"
      onMouseDown={(e) => e.preventDefault()}
      className={
        (closing ? "card-out " : "card-in ") +
        "grid w-[min(340px,calc(100vw-24px))] gap-[10px] rounded-lg border border-border bg-card p-[12px] font-ui text-[13px] text-card-foreground shadow-lg"
      }
    >
      <div className="flex items-start gap-[8px]">
        <span className="mt-px shrink-0 text-primary">{icon}</span>
        <div className="min-w-0 flex-1">{children}</div>
      </div>
      {actions && <div className="flex justify-end gap-[8px]">{actions}</div>}
    </div>
  );
}

/** The check mark that draws itself once (stroke, 300 ms); drawn at once under reduced motion. */
export function CheckMark({ size = 15 }: { size?: number }) {
  return (
    <svg width={size} height={size} viewBox="0 0 16 16" aria-hidden="true" className="check-draw">
      <path d="M3 8.5 6.5 12 13 4.5" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" pathLength={24} />
    </svg>
  );
}

/** The ask card for Rein: Later · Update Rein (which opens the existing confirm). */
export function UpdateCard({ version, onLater, onUpdate }: { version: string; onLater: () => void; onUpdate: () => void }) {
  return (
    <Card
      id="update-card"
      icon={<ArrowUp size={15} aria-hidden="true" />}
      actions={
        <>
          <Button size="sm" tabIndex={-1} onClick={onLater}>
            Later
          </Button>
          <Button size="sm" tabIndex={-1} variant="primary" onClick={onUpdate}>
            Update Rein
          </Button>
        </>
      }
    >
      {reinAskText(version)}
    </Card>
  );
}

/** The package manager's line with Copy (Linux, macOS: the window does not install itself). */
function CommandLine({ command }: { command: string }) {
  const [copied, setCopied] = useState(false);
  return (
    <div className="mt-[8px] flex items-center gap-[6px]">
      <code className="min-w-0 flex-1 truncate rounded bg-muted px-[6px] py-[3px] font-mono text-[12px]" title={command}>
        {command}
      </code>
      <Button size="sm" tabIndex={-1} onClick={() => clipboardWrite(command).then(() => setCopied(true), () => {})}>
        {copied ? <Check size={13} aria-hidden="true" /> : <Copy size={13} aria-hidden="true" />}
        {copied ? "Copied" : "Copy"}
      </Button>
    </div>
  );
}

/** The window's card (component 4, E-87): the ask, then Downloading with the determinate line
 *  and Cancel, "Checking the signature…", the drawn check and "Restarting…"; a failure shows
 *  its sentence and Try again. */
export function AppUpdateCard({ card, onLater, onStart, onCancel }: { card: AppCard; onLater: () => void; onStart: () => void; onCancel: () => void }) {
  const later = (
    <Button size="sm" tabIndex={-1} onClick={onLater}>
      Later
    </Button>
  );
  const actions =
    card.at === "ask" ? (
      card.mode === "in_place" ? (
        <>
          {later}
          <Button size="sm" tabIndex={-1} variant="primary" onClick={onStart}>
            Update and restart
          </Button>
        </>
      ) : (
        later
      )
    ) : card.at === "downloading" ? (
      <Button size="sm" tabIndex={-1} onClick={onCancel}>
        Cancel
      </Button>
    ) : card.at === "failed" ? (
      <>
        {later}
        <Button size="sm" tabIndex={-1} variant="primary" onClick={onStart}>
          Try again
        </Button>
      </>
    ) : undefined;
  const icon =
    card.at === "restarting" ? <CheckMark /> : card.at === "failed" ? <TriangleAlert size={15} aria-hidden="true" className="text-destructive" /> : <ArrowUp size={15} aria-hidden="true" />;
  return (
    <Card id="app-update-card" icon={icon} actions={actions}>
      <div data-at={card.at} className="tabular-nums">
        {appCardText(card)}
      </div>
      {card.at === "ask" && card.mode === "command" && card.command && <CommandLine command={card.command} />}
      {card.at === "ask" && card.mode === "by_hand" && <div className="mt-[4px] text-muted-foreground">{BY_HAND_TEXT}</div>}
      {(card.at === "downloading" || card.at === "verifying") && (
        <div className="progress-fill mt-[8px]" role="progressbar" aria-valuemin={0} aria-valuemax={100} aria-valuenow={card.at === "downloading" ? card.percent : 100}>
          <span style={{ width: `${card.at === "downloading" ? card.percent : 100}%` }} />
        </div>
      )}
    </Card>
  );
}

/** After a restart into a new version: "Updated to X" with the check mark, and "What's new"
 *  once the running version's notes are known. Stays until closed. */
export function UpdatedToast({ version, notes, onNotes, onClose }: { version: string; notes: boolean; onNotes: () => void; onClose: () => void }) {
  return (
    <Card
      id="updated-toast"
      icon={<CheckMark />}
      actions={
        <>
          {notes && (
            <Button size="sm" tabIndex={-1} onClick={onNotes}>
              What's new
            </Button>
          )}
          <Button size="sm" tabIndex={-1} onClick={onClose}>
            Close
          </Button>
        </>
      }
    >
      Updated to {version}
    </Card>
  );
}

/** The release notes in a reader: plain text through the closed structured-text component. */
export function NotesReader({ version, notes, onClose, portal = true }: { version: string; notes: string; onClose: () => void; portal?: boolean }) {
  // Close is focused on open; on close the dialog gives focus back to where it was (the terminal).
  return (
    <Panel
      portal={portal}
      title={`What's new in rein-app ${version}`}
      onClose={onClose}
      footer={
        <Button variant="primary" autoFocus onClick={onClose}>
          Close
        </Button>
      }
    >
      <StructuredText text={notes} />
    </Panel>
  );
}

/** Everything at the bottom right, stacked: the Rein ask, the window's card, "Updated to". The
 *  cards hide while the Update dialog is open (it shows the same facts). */
export function UpdateStack({ updates, dialog, onReinUpdate }: { updates: UpdatesState | null; dialog: boolean; onReinUpdate: () => void }) {
  const [closedToast, setClosedToast] = useState(false);
  const [notes, setNotes] = useState(false);
  // Later on a failure hides that failure here (the job stays failed in the core until a new start).
  const [hidden, setHidden] = useState<string | null>(null);
  const rein = reinAsk(updates);
  const app = appCard(updates);
  const appKey = app ? `${app.at}:${app.version}:${app.at === "failed" ? app.step : ""}` : null;
  const showApp = app !== null && !dialog && appKey !== hidden;
  const updated = updatedTo(updates);
  const running = runningNotes(updates);
  const later = () => {
    setHidden(appKey);
    void updatesLater("app").catch(() => {});
  };
  return (
    <>
      <div id="update-stack" className="pointer-events-none fixed right-[12px] bottom-[38px] z-30 flex flex-col items-end gap-[8px] [&>*]:pointer-events-auto">
        <Exit show={updated !== null && !closedToast} render={() => <UpdatedToast version={updated ?? ""} notes={running !== null} onNotes={() => setNotes(true)} onClose={() => setClosedToast(true)} />} />
        <Exit
          show={showApp}
          render={() =>
            app && <AppUpdateCard card={app} onLater={later} onStart={() => void appUpdateStart().catch(() => {})} onCancel={() => void appUpdateCancel().catch(() => {})} />
          }
        />
        <Exit show={rein !== null && !dialog} render={() => <UpdateCard version={rein ?? ""} onLater={() => void updatesLater("rein").catch(() => {})} onUpdate={onReinUpdate} />} />
      </div>
      <Exit show={notes && running !== null} render={() => <NotesReader version={updates?.app.running ?? ""} notes={running ?? ""} onClose={() => setNotes(false)} />} />
    </>
  );
}

/** The window's row in the Update dialog: its own button (or the command) and its notes. */
export function AppRow({ updates, onStart }: { updates: UpdatesState | null; onStart: () => void }) {
  const v = appNewer(updates);
  if (!updates || v === null) return null;
  const { mode, command, notes } = updates.app;
  return (
    <div id="app-row" className="grid gap-2 rounded-md border border-border bg-background p-3">
      <div className="flex flex-wrap items-center gap-2">
        <span className="font-medium">rein-app window {v}</span>
        <span className="ml-auto">
          {mode === "in_place" && (
            <Button size="sm" variant="primary" onClick={onStart}>
              Update and restart
            </Button>
          )}
        </span>
      </div>
      {mode === "command" && command && <CommandLine command={command} />}
      {mode === "by_hand" && <div className="text-muted-foreground">{BY_HAND_TEXT}</div>}
      {notes && (
        <div className="max-h-[160px] overflow-y-auto text-[12px]">
          <StructuredText text={notes} />
        </div>
      )}
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
