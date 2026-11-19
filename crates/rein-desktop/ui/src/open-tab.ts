// One open tab as the shell keeps it (moved out of shell.tsx by E-97, no change), and the
// state of the sheet Update project / Set up this project runs in.

import type { PolledTab } from "./agent-bar.tsx";
import type { CardView } from "./ipc.ts";
import type { CheckReply } from "./project.ts";
import type { FailedTab } from "./start-failed-ui.tsx";
import type { TermTab } from "./term.ts";

/** One open tab: its terminal, its pane, and what the chrome shows for it. */
/** The agents, the bar and the countdown (V9, E-71) are `PolledTab`'s fields (agent-bar.tsx). */
export interface OpenTab extends PolledTab, FailedTab {
  /** For the tab's life (the title bar keys its pill by it). */
  id: number;
  term: TermTab;
  el: HTMLDivElement;
  /** As asked for (`:open`, a handoff), and canonical once open. */
  requested: string;
  project: string;
  title: string;
  open: boolean;
  /** `rein start` exited (Enter restarts it): a dot on the tab. */
  exited: boolean;
  /** E-61: `exited` because the tmux setup was refused; a bell rang while out of sight. */
  refused?: boolean;
  bell?: boolean;
  /** A failed tmux overlay was told in a toast already (V6: once per tab). */
  overlayTold?: boolean;
  /** V9 (E-63): a folder without `.ai/` — the set-up card, no terminal yet. */
  setup?: { id: number; reply: CheckReply | null; inside: string | null } | undefined;
  /** V12 (E-70): a restored tab — its card (no session / no folder), the first status read
   *  opened without taking the front (its status read and "gone" are PolledTab's). */
  card?: CardView | undefined;
  quiet?: boolean;
  /** The out-of-date check ran for this tab (once per open). */
  asked?: boolean;
  /** rein's rows when it reported the project out of date; the bar shows while set. */
  update?: string[] | undefined;
  /** Not now / Don't ask: the bar is hidden for this tab. */
  updateHidden?: boolean;
}

/** V9 3: Update project, running in its sheet for this tab. `run`: a failed start's sheet
 *  (V14) runs set-up in the tab's own terminal instead. */
export type UpdateSheetState = { entry: OpenTab; id: number; rows: string[]; run?: () => void } | null;
