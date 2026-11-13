// The set-up card, the out-of-date bar and the Update project sheet (desktop.md §Look and feel
// v2 V9 components 2–3, E-63), drawn from the pure `project.ts`. rein's rows and output go in
// as React text nodes only.

import { useEffect, useRef, useState } from "react";
import { Button } from "../components/button.tsx";
import { Card } from "../components/card.tsx";
import { Spinner } from "../components/loaders.tsx";
import { Channel, projectCheck, projectFix, projectFixCancel } from "./ipc.ts";
import { FolderOpen, X } from "./icons.ts";
import { SETUP_CARD, UPDATE_BAR_TEXT, cardRows, fixOutcome, type CheckReply, type FixOutcome } from "./project.ts";
import { Output, Panel, Runs, StatusLine } from "./release.tsx";

/** In place of the terminal: what is missing, as rein says it, and the three ways on. */
export function SetupCard({
  project,
  reply,
  onSetUp,
  onOpenOther,
  onClose,
}: {
  project: string;
  /** `project_check`'s answer; `null` while rein is asked. */
  reply: CheckReply | null;
  onSetUp: () => void;
  onOpenOther: () => void;
  onClose: () => void;
}) {
  const { rows, note } = cardRows(reply);
  return (
    <div id="setup-card" className="absolute inset-0 z-10 flex items-center justify-center bg-background p-6">
      <Card className="grid w-[min(620px,100%)] gap-4 p-5">
        <div className="grid gap-1">
          <h2 className="m-0 text-[15px] font-semibold text-foreground">{SETUP_CARD.title}</h2>
          <code className="font-mono text-[12px] [overflow-wrap:anywhere] text-muted-foreground">{project}</code>
        </div>
        {rows.length > 0 && (
          <ul className="m-0 grid list-none gap-1 rounded-md border border-border bg-muted/40 p-3 font-mono text-[12px] text-foreground">
            {rows.map((r, i) => (
              <li key={i} className="[overflow-wrap:anywhere]">
                {r}
              </li>
            ))}
          </ul>
        )}
        {note && (
          <p className="m-0 flex items-center gap-2 text-[12px] text-muted-foreground" aria-busy={reply === null || undefined}>
            {reply === null && <Spinner size={12} />}
            {note}
          </p>
        )}
        <p className="m-0 text-[13px] text-foreground">{SETUP_CARD.does}</p>
        <div className="flex flex-wrap justify-end gap-2">
          <Button onClick={onClose}>
            <X size={14} aria-hidden="true" />
            Close tab
          </Button>
          <Button onClick={onOpenOther}>
            <FolderOpen size={14} aria-hidden="true" />
            Open another folder
          </Button>
          <Button variant="primary" autoFocus onClick={onSetUp}>
            Set up this project
          </Button>
        </div>
      </Card>
    </div>
  );
}

/** One line above the terminal: rein says the project is out of date. */
export function UpdateBar({ onUpdate, onNotNow, onDontAsk }: { onUpdate: () => void; onNotNow: () => void; onDontAsk: () => void }) {
  return (
    <div
      id="update-bar"
      role="status"
      className="fade-in flex h-[36px] shrink-0 items-center gap-[8px] border-b border-border bg-muted px-[12px] font-ui text-[13px] text-foreground"
    >
      <span className="min-w-0 flex-1 truncate">{UPDATE_BAR_TEXT}</span>
      <Button size="sm" variant="ghost" onClick={onDontAsk}>
        Don't ask for this project
      </Button>
      <Button size="sm" onClick={onNotNow}>
        Not now
      </Button>
      <Button size="sm" variant="primary" onClick={onUpdate}>
        Update project
      </Button>
    </div>
  );
}

type SheetPhase = { at: "running"; lines: string[] } | { at: "done"; lines: string[]; outcome: FixOutcome };

/** Update project: `rein init` with its output as it comes, Cancel while it runs, then the
 *  check once more (the ruling, 4). `rows` are what `--check` reported. */
export function UpdateSheet({
  tab,
  rows,
  onCommand,
  onDone,
  onClose,
}: {
  tab: number;
  rows: readonly string[];
  onCommand: (cmd: string) => void;
  onDone: (outcome: FixOutcome) => void;
  onClose: () => void;
}) {
  const [phase, setPhase] = useState<SheetPhase>({ at: "running", lines: [] });
  const [cancelling, setCancelling] = useState(false);
  const lines = useRef<string[]>([]);
  const started = useRef(false);
  useEffect(() => {
    if (started.current) return;
    started.current = true;
    onCommand("rein init");
    const out = new Channel<string>((l) => {
      lines.current = [...lines.current, l];
      setPhase({ at: "running", lines: lines.current });
    });
    const finish = (outcome: FixOutcome) => {
      setPhase({ at: "done", lines: lines.current, outcome });
      onDone(outcome);
    };
    projectFix(tab, out).then(
      (end) =>
        end.code === 0 && end.stopped === null
          ? projectCheck(tab).then(
              (r) => finish(fixOutcome(end, r)),
              () => finish(fixOutcome(end, null)),
            )
          : finish(fixOutcome(end, null)),
      (e: { message?: string }) =>
        finish({ status: "fail", text: `rein init did not run: ${e.message ?? String(e)}`, keepBar: true }),
    );
  }, [tab, onCommand, onDone]);
  const running = phase.at === "running";
  return (
    <Panel
      title="Update project"
      onClose={() => !running && onClose()}
      footer={
        running ? (
          <>
            <Button
              autoFocus
              disabled={cancelling}
              onClick={() => {
                setCancelling(true);
                void projectFixCancel();
              }}
            >
              Cancel
            </Button>
            <Button variant="primary" disabled aria-busy="true">
              <Spinner size={14} />
              Updating…
            </Button>
          </>
        ) : (
          <Button variant="primary" autoFocus onClick={onClose}>
            Close
          </Button>
        )
      }
    >
      <Runs cmd="rein init" />
      {rows.length > 0 && (
        <div className="grid gap-1">
          <span className="text-[12px] text-muted-foreground">rein init --check reported</span>
          <Output lines={rows} />
        </div>
      )}
      {running && phase.lines.length === 0 && <StatusLine status="busy">Running rein init — this can take minutes the first time…</StatusLine>}
      <Output lines={phase.lines} />
      {phase.at === "done" && (
        <StatusLine status={phase.outcome.status}>
          <span className="font-medium">{phase.outcome.text}</span>
        </StatusLine>
      )}
    </Panel>
  );
}
