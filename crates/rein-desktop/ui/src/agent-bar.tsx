// The agent-exit bar and the close confirm (desktop.md §Look and feel v2 V9 component 4, E-62),
// drawn from the pure `agents.ts`. Text goes in as React text nodes only.

import { useEffect, useRef, useState } from "react";
import { Button } from "../components/button.tsx";
import { Switch } from "../components/switch.tsx";
import { Dialog, DialogContent, DialogFooter } from "../components/dialog.tsx";
import { Spinner } from "../components/loaders.tsx";
import { POLL_MS, agentBar, confirmText, shouldPoll, type Agent } from "./agents.ts";
import { projectAgents } from "./ipc.ts";
import { agentsGone } from "./restore-ui.tsx";

/** What the poll reads and sets on a tab (the shell's own tab entry). */
export interface PolledTab {
  term: { tab: { id: number } | null };
  open: boolean;
  exited: boolean;
  closing?: boolean;
  agents?: Agent[];
  autoFailed?: boolean;
  kept?: string | null;
  restored?: boolean;
  gone?: boolean;
}

/** V9 (E-62): ask the core which agents of the active tab still run — every 5 s, only while
 *  the window is visible and the tab's process runs. All ended (setting on): close by itself.
 *  E-70: a restored tab's first answer tells whether its agents are gone (V12 component 5).
 *  Moved out of `shell.tsx` unchanged otherwise. */
export function useAgentPoll<T extends PolledTab>(
  current: () => T | undefined,
  stillOpen: (t: T) => boolean,
  closeProject: (t: T, confirmed: boolean, auto?: boolean) => void,
  closeWhenAgentsExit: boolean,
  rerender: () => void,
) {
  const closeRef = useRef(closeProject);
  closeRef.current = closeProject;
  const whenRef = useRef(closeWhenAgentsExit);
  whenRef.current = closeWhenAgentsExit;
  const at = useRef({ current, stillOpen, rerender });
  at.current = { current, stillOpen, rerender };
  useEffect(() => {
    const tick = () => {
      const t = at.current.current();
      const id = t?.term.tab?.id;
      if (!t || id === undefined) return;
      const visible = document.visibilityState === "visible";
      if (!shouldPoll({ active: true, visible, open: t.open, exited: t.exited }) || t.closing) return;
      projectAgents(id).then(
        (r) => {
          if (!at.current.stillOpen(t) || t.exited || !("agents" in r)) return;
          t.agents = r.agents;
          if (t.restored) Object.assign(t, { restored: false, gone: agentsGone(r.agents) });
          const view = agentBar(r.agents, { closeWhenAllExit: whenRef.current && !t.autoFailed, kept: t.kept ?? null });
          if (view.kind === "close") closeRef.current(t, false, true);
          at.current.rerender();
        },
        () => {},
      );
    };
    const timer = setInterval(tick, POLL_MS);
    return () => clearInterval(timer);
  }, []);
}

/** One line above the terminal: what exited, Close project, Keep open. */
export function AgentBar({
  text,
  closing,
  onClose,
  onKeep,
}: {
  text: string;
  /** Close project is running (`rein start --kill`). */
  closing: boolean;
  onClose: () => void;
  onKeep: () => void;
}) {
  return (
    <div
      id="agent-bar"
      role="status"
      className="fade-in flex h-[36px] shrink-0 items-center gap-[8px] border-b border-border bg-muted px-[12px] font-ui text-[13px] text-foreground"
    >
      <span className="min-w-0 flex-1 truncate">{text}</span>
      <Button size="sm" onClick={onKeep} disabled={closing}>
        Keep open
      </Button>
      <Button size="sm" variant="primary" onClick={onClose} disabled={closing} aria-busy={closing || undefined}>
        {closing && <Spinner size={12} />}
        Close project
      </Button>
    </div>
  );
}

/** "The Architect is still running. Close it too?" — No is the default (focused, Escape). */
export function CloseConfirm({ running, onYes, onNo }: { running: string[]; onYes: () => void; onNo: () => void }) {
  return (
    <Dialog open onOpenChange={(open) => !open && onNo()}>
      <DialogContent title="Close the project?" aria-describedby={undefined}>
        <p className="m-0 text-[13px] text-foreground">{confirmText(running)}</p>
        <DialogFooter>
          <Button autoFocus onClick={onNo}>
            No, keep it
          </Button>
          <Button variant="primary" onClick={onYes}>
            Close project
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

/** V11 (E-66): closing a project or quitting stops its agents — Close / Cancel (Cancel focused),
 *  with "Don't ask again". */
export function StopConfirm({
  title,
  text,
  yes,
  onYes,
  onNo,
}: {
  title: string;
  text: string;
  yes: string;
  onYes: (dontAskAgain: boolean) => void;
  onNo: () => void;
}) {
  const [dontAsk, setDontAsk] = useState(false);
  return (
    <Dialog open onOpenChange={(open) => !open && onNo()}>
      <DialogContent title={title} aria-describedby={undefined}>
        <p className="m-0 text-[13px] text-foreground">{text}</p>
        <label className="flex items-center gap-2 text-[12px] text-muted-foreground">
          <Switch aria-label="Don't ask again" checked={dontAsk} onCheckedChange={setDontAsk} />
          Don't ask again
        </label>
        <DialogFooter>
          <Button autoFocus onClick={onNo}>
            Cancel
          </Button>
          <Button variant="primary" onClick={() => onYes(dontAsk)}>
            {yes}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
