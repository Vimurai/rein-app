// Open tabs remembered and restored (desktop.md V12 components 4–5, D-061; E-70): the pure
// half — the card and bar sentences, "are the agents gone", and the record from the tabs.
// No DOM; tested under Node (test/lifecycle.test.mjs). `restore-ui.tsx` draws it.

import type { Agent } from "./agents.ts";

export const notRunningText = (project: string) => `${project} is not running. Its session ended while the app was closed.`;
export const missingText = (project: string) => `${project} is not there any more: its folder is gone.`;
export const agentsGoneText = (project: string) => `The agents in ${project} are no longer running.`;

/** The first status read of a restored tab: no role pane running means its agents are gone
 *  (an exit while the app was closed — the "seen running in this tab" rule cannot apply). */
export function agentsGone(agents: readonly Agent[]): boolean {
  return !agents.some((a) => a.state === "running");
}

/** A tab as the record sees it. */
export interface RecordTab {
  term: { coreId: number | null };
}

/** The record from the tabs: core ids in tab order, and where the current one sits among
 *  them (a tab with no core id — a refused one — is left out). */
export function recordOf(tabs: readonly RecordTab[], current: number): { ids: number[]; active: number | null } {
  const ids: number[] = [];
  let active: number | null = null;
  tabs.forEach((t, i) => {
    const id = t.term.coreId;
    if (id === null) return;
    if (i === current) active = ids.length;
    ids.push(id);
  });
  return { ids, active };
}

