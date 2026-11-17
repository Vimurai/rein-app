// The agent-exit bar and the close confirm (desktop.md §Look and feel v2 V9 component 4, E-62),
// drawn from the pure `agents.ts`. Text goes in as React text nodes only.

import { useEffect, useRef, useState } from "react";
import { Button } from "../components/button.tsx";
import { Switch } from "../components/switch.tsx";
import { Dialog, DialogContent, DialogFooter } from "../components/dialog.tsx";
import { Spinner } from "../components/loaders.tsx";
import { POLL_MS, confirmText, shouldPoll } from "./agents.ts";
import { countdownStep, type CountdownTab, type CountdownView } from "./countdown.ts";
import { projectAgents } from "./ipc.ts";
import { agentsGone } from "./restore-ui.tsx";

/** What the poll reads and sets on a tab (the shell's own tab entry). */
export interface PolledTab extends CountdownTab {
  term: { tab: { id: number } | null };
  title: string;
  open: boolean;
  restored?: boolean;
  gone?: boolean;
  /** What the bar above the terminal shows for this tab now (V9 bar or V12 countdown). */
  barView?: CountdownView | undefined;
}

/** V9 (E-62): ask the core which agents of the active tab still run — every 5 s, only while
 *  the window is visible and the tab's process runs. E-70: a restored tab's first answer tells
 *  whether its agents are gone, and which roles were not running (ended, V12 7). E-71: every
 *  answer and a 250 ms tick run the countdown for every tab (`countdown.ts`) — only the
 *  active tab of a visible window counts down; at zero the project closes without its
 *  confirm; all ended closes as V9 built it. */
export function useAgentPoll<T extends PolledTab>(
  current: () => T | undefined,
  all: () => readonly T[],
  closeProject: (t: T, confirmed: boolean, auto?: boolean) => void,
  closeWhenAgentsExit: boolean,
  rerender: () => void,
) {
  const at = useRef({ current, all, closeProject, closeWhenAgentsExit, rerender });
  at.current = { current, all, closeProject, closeWhenAgentsExit, rerender };
  useEffect(() => {
    const apply = (): boolean => {
      const { current: cur, all: tabs, closeProject: close, closeWhenAgentsExit: on } = at.current;
      const visible = document.visibilityState === "visible";
      let changed = false;
      for (const t of tabs()) {
        const before = JSON.stringify([t.barView, t.marked]);
        const { view, fire } = countdownStep(t, { closeWhenAgentsExit: on, project: t.title, shown: visible && t === cur(), now: Date.now() });
        t.barView = view;
        if (view.kind === "close" || fire) close(t, true, true);
        if (JSON.stringify([t.barView, t.marked]) !== before) changed = true;
      }
      return changed;
    };
    const tick = () => {
      const t = at.current.current();
      const id = t?.term.tab?.id;
      if (!t || id === undefined) return;
      const visible = document.visibilityState === "visible";
      if (!shouldPoll({ active: true, visible, open: t.open, exited: t.exited }) || t.closing) return;
      projectAgents(id).then(
        (r) => {
          if (!at.current.all().includes(t) || t.exited || !("agents" in r)) return;
          t.agents = r.agents;
          if (t.restored) {
            t.restored = false;
            t.gone = agentsGone(r.agents);
            if (!t.gone) t.endedAtRestore = r.agents.filter((a) => a.state !== "running").map((a) => a.role);
          }
          apply();
          at.current.rerender();
        },
        () => {},
      );
    };
    const poll = setInterval(tick, POLL_MS);
    // The number in the bar, a tab left mid-countdown (marked), a marked tab shown again (from
    // 5), the close at zero: four times a second, only while some tab counts down or is marked
    // (no process, no IPC). A new exit is seen by the poll answer above, which applies too.
    const clock = setInterval(() => {
      if (at.current.all().some((t) => t.countdown || t.barView?.kind === "countdown" || t.marked) && apply()) at.current.rerender();
    }, 250);
    return () => {
      clearInterval(poll);
      clearInterval(clock);
    };
  }, []);
}

/** One line above the terminal: what exited, Close project, Keep open. */
export function AgentBar({
  text,
  closing,
  onClose,
  onKeep,
  countdown = false,
}: {
  text: string;
  /** Close project is running (`rein start --kill`). */
  closing: boolean;
  onClose: () => void;
  onKeep: () => void;
  /** V12 7 (E-71): the countdown — "Close now", and Keep open has the keyboard. */
  countdown?: boolean;
}) {
  return (
    <div
      id="agent-bar"
      data-own-keys=""
      role="status"
      className="fade-in flex h-[36px] shrink-0 items-center gap-[8px] border-b border-border bg-muted px-[12px] font-ui text-[13px] text-foreground"
    >
      <span className="min-w-0 flex-1 truncate">{text}</span>
      <Button size="sm" onClick={onKeep} disabled={closing} autoFocus={countdown}>
        Keep open
      </Button>
      <Button size="sm" variant="primary" onClick={onClose} disabled={closing} aria-busy={closing || undefined}>
        {closing && <Spinner size={12} />}
        {countdown ? "Close now" : "Close project"}
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
