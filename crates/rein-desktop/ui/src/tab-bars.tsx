// The bars above the stage for the current tab (moved out of shell.tsx by E-97, no change):
// a failed start (V14), the out-of-date bar (V9 3), the agents-gone bar (V12 7) and the
// agent bar or countdown (V9 5).

import type { Dispatch, MutableRefObject, SetStateAction } from "react";
import { AgentBar } from "./agent-bar.tsx";
import type { CountdownView } from "./countdown.ts";
import type { OpenTab, UpdateSheetState } from "./open-tab.ts";
import { UpdateBar } from "./project-ui.tsx";
import { AgentsGoneBar } from "./restore-ui.tsx";
import { StartFailedBar } from "./start-failed-ui.tsx";
import { whichBar } from "./start-failed.ts";

export interface TabBarsProps {
  current: OpenTab | undefined;
  agentView: CountdownView | { readonly kind: "none" };
  cur: MutableRefObject<number>;
  detach: (i?: number, note?: string) => void;
  closeProject: (entry: OpenTab, confirmed: boolean, auto?: boolean) => void;
  onDontAskUpdate: (project: string) => void;
  rerender: () => void;
  say: (text: string, error?: boolean) => void;
  setUpdateSheet: Dispatch<SetStateAction<UpdateSheetState>>;
}

export function TabBars({ current, agentView, cur, detach, closeProject, onDontAskUpdate, rerender, say, setUpdateSheet }: TabBarsProps) {
  return (
    <>
      {current?.failed && (
        <StartFailedBar
          state={current.failed}
          onAction={(a) => {
            const id = current.term.tab?.id;
            if (a === "close") return detach(cur.current);
            if (a === "retry") return void ((current.failed = undefined), rerender(), current.term.restart());
            if (id === undefined || current.failed?.at !== "needs_init") return;
            const run = () => {
              setUpdateSheet(null);
              current.failed = undefined;
              current.setUpOnce = true; // V14 5: no second offer in this tab open
              rerender();
              void current.term.start(id);
            };
            setUpdateSheet({ entry: current, id, rows: current.failed.rows, run });
          }}
        />
      )}
      {current && whichBar(current) === "update" && (
        <UpdateBar
          onUpdate={() => {
            const id = current.term.tab?.id;
            if (id !== undefined && current.update) setUpdateSheet({ entry: current, id, rows: current.update });
          }}
          onNotNow={() => {
            current.updateHidden = true;
            rerender();
          }}
          onDontAsk={() => {
            current.updateHidden = true;
            onDontAskUpdate(current.project);
            rerender();
          }}
        />
      )}
      {current?.gone && !current.exited && <AgentsGoneBar t={current} rerender={rerender} say={say} onClose={() => closeProject(current, true)} />}
      {current && (agentView.kind === "bar" || agentView.kind === "countdown") && (
        <AgentBar
          text={agentView.text}
          closing={current.closing === true}
          countdown={agentView.kind === "countdown"}
          onClose={() => closeProject(current, agentView.kind === "countdown")}
          onKeep={() => {
            current.kept = agentView.key;
            rerender();
          }}
        />
      )}
    </>
  );
}
