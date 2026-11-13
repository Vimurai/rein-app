// The agent-exit bar and the close confirm (desktop.md §Look and feel v2 V9 component 4, E-62),
// drawn from the pure `agents.ts`. Text goes in as React text nodes only.

import { useState } from "react";
import { Button } from "../components/button.tsx";
import { Switch } from "../components/switch.tsx";
import { Dialog, DialogContent, DialogFooter } from "../components/dialog.tsx";
import { Spinner } from "../components/loaders.tsx";
import { confirmText } from "./agents.ts";

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
