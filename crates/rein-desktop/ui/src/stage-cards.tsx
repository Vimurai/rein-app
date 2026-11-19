// What the stage shows in place of a terminal (moved out of shell.tsx by E-97, no change): the
// starting line (V7), a restored tab's card (V12), the set-up card (V9) and, with no tab
// open, the start screen.

import type { MutableRefObject } from "react";
import { ProgressLine } from "../components/loaders.tsx";
import type { StatusBar as StatusBarData } from "./chrome.ts";
import type { Runner } from "./frame.tsx";
import type { HealthView } from "./ipc.ts";
import type { ActionCall } from "./keymap.ts";
import type { OpenTab } from "./open-tab.ts";
import { SetupCard } from "./project-ui.tsx";
import { RestoreCard } from "./restore-ui.tsx";
import { StartScreen } from "./start-screen.tsx";
import { startScreen, type RecentView } from "./start.ts";

export interface StageCardsProps {
  status: StatusBarData;
  current: OpenTab | undefined;
  cur: MutableRefObject<number>;
  detach: (i?: number, note?: string) => void;
  open: (project: string, restore?: boolean) => (() => Promise<void>) | null;
  run: (call: ActionCall) => void;
  rerender: () => void;
  termOpen: boolean;
  recent: RecentView[];
  recentLoaded: boolean;
  health: HealthView | null;
  healthRuns: number;
  runner: Runner;
}

export function StageCards({ status, current, cur, detach, open, run, rerender, termOpen, recent, recentLoaded, health, healthRuns, runner }: StageCardsProps) {
  return (
    <>
      {/* V7: the tab is starting — a 2 px line in the stage's own top padding, never over
          the terminal. */}
      {status.starting && !current?.setup && !current?.card && <ProgressLine className="absolute inset-x-0 top-0 z-10" />}
      {current?.card && <RestoreCard t={current} rerender={rerender} onClose={() => detach(cur.current, `${current.title} closed`)} />}
      {current?.setup && (
        <SetupCard
          project={current.project}
          reply={current.setup.reply}
          inside={current.setup.inside}
          onOpenInside={(parent) => {
            detach(cur.current, `${current.title} closed — opening the project it is inside`);
            open(parent);
          }}
          onSetUp={() => {
            const s = current.setup;
            if (!s) return;
            current.setup = undefined;
            rerender();
            void current.term.start(s.id);
          }}
          onOpenOther={() => run({ name: "project.open" })}
          onClose={() => detach(cur.current)}
        />
      )}
      {!termOpen && (
        <StartScreen
          model={startScreen(
            recent,
            health?.setup ? { reason: health.reason ?? "WSL is not ready", command: health.setup } : null,
            Math.floor(Date.now() / 1000),
            { loading: !recentLoaded, checking: healthRuns > 0 },
          )}
          r={runner}
        />
      )}
    </>
  );
}
