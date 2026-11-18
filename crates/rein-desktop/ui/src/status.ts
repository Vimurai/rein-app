// A project's status as the core sends it (status.md §Components 2, §Data Model, D-062; E-76):
// the `project_status` event and `project_status_all()`. The core decides the state; the web
// side draws it (E-77, §Components 3) and decides nothing. This half is pure: the sentence,
// the mark, the accessible name, the tooltip lines and the status-bar segment of one status.

import type { Item } from "./attention.ts";
import { age } from "./panels.ts";
import { taskTitle } from "./panel-text.ts";

export type StatusState = "setup" | "ended" | "starting" | "needs_you" | "working" | "quiet" | "unknown";

export interface ProjectStatus {
  /** The tab's core id. */
  tab: number;
  state: StatusState;
  /** Seconds since the tab's window last printed; null when not read. */
  quiet_s: number | null;
  /** How long output had been arriving before it stopped (gaps under 15 s ignored). */
  worked_s: number;
  /** Reserved for rein's waiting signal; always null until then. */
  waiting: boolean | null;
  now: Item[];
  info: Item[];
  /** The task in progress, whose turn and the last handoff's age (§Data Model). The core
   *  does not send them yet (status.rs); drawn here when present. */
  in_progress?: { id: string; title: string } | null;
  turn?: string | null;
  last_handoff_s?: number | null;
}

/** The mark of a state (lucide names, 12 px). `spinner` is the tab's existing V7 spinner;
 *  `unknown` has none. Shape tells the state, not colour alone. */
export type MarkName = "wrench" | "square" | "spinner" | "bell-dot" | "circle-dot" | "circle";

export const MARKS: Record<StatusState, MarkName | null> = {
  setup: "wrench",
  ended: "square",
  starting: "spinner",
  needs_you: "bell-dot",
  working: "circle-dot",
  quiet: "circle",
  unknown: null,
};

/** A task id is shown only in this shape (status.md §Data Model). */
export const TASK_ID = /^[A-Z]-[0-9]{1,6}$/;

/** How long a tab has been quiet, in the core's steps: 15 s under a minute, then minutes,
 *  then hours. */
export function quietSpan(s: number): string {
  if (s < 60) return `${s - (s % 15)} s`;
  if (s < 3600) return `${Math.floor(s / 60)} min`;
  return `${Math.floor(s / 3600)} h`;
}

/** The state's sentence (component 2's table); `—` for unknown. */
export function sentence(s: ProjectStatus): string {
  switch (s.state) {
    case "setup":
      return "Needs set-up.";
    case "ended":
      return "Ended.";
    case "starting":
      return "Starting…";
    case "needs_you":
      return "Needs you.";
    case "working":
      return "Working.";
    case "quiet":
      return s.quiet_s === null ? "Quiet." : `Quiet for ${quietSpan(s.quiet_s)}.`;
    default:
      return "—";
  }
}

/** The count beside the bell: only over one. */
export function markCount(s: ProjectStatus): number | null {
  return s.state === "needs_you" && s.now.length > 1 ? s.now.length : null;
}

/** "<project>: <sentence>" — the tab's accessible name while it has a mark. */
export function accessibleName(project: string, s: ProjectStatus): string {
  return `${project}: ${sentence(s)}`;
}

/** The task in progress as "<id> — <title>", or null when there is none or its id is not in
 *  the task-id shape. */
function inProgress(s: ProjectStatus): { id: string; title: string } | null {
  const t = s.in_progress;
  if (!t || !TASK_ID.test(t.id)) return null;
  return { id: t.id, title: taskTitle(t.title) };
}

/** The tab's tooltip: at most three lines — the sentence; the task in progress if any; whose
 *  turn and the last handoff's age if known. */
export function tooltipLines(s: ProjectStatus): string[] {
  const lines = [sentence(s)];
  const t = inProgress(s);
  if (t) lines.push(`In progress: ${t.id}${t.title ? ` — ${t.title}` : ""}`);
  if (s.turn) lines.push(`${s.turn}'s turn${s.last_handoff_s != null ? ` · last handoff ${age(s.last_handoff_s)}` : ""}`);
  return lines;
}

/** The status bar's segment for the current tab: "<sentence> · <task id> · <whose turn>'s
 *  turn", the parts not known left out. Null for unknown (nothing to say). */
export function statusSegment(s: ProjectStatus): string | null {
  if (s.state === "unknown") return null;
  const t = inProgress(s);
  return [sentence(s), t?.id, s.turn ? `${s.turn}'s turn` : null].filter(Boolean).join(" · ");
}

/** What the tab bar draws for one tab's status; null draws nothing (hidden, not known yet,
 *  or `unknown`). */
export interface StatusMark {
  state: StatusState;
  mark: MarkName;
  count: number | null;
  name: string;
  lines: string[];
  /** The status bar's segment (null for unknown, which has no mark anyway). */
  segment: string;
}

export function statusMark(project: string, s: ProjectStatus | undefined, shown: boolean): StatusMark | null {
  if (!shown || !s) return null;
  const mark = MARKS[s.state];
  if (mark === null) return null;
  return { state: s.state, mark, count: markCount(s), name: accessibleName(project, s), lines: tooltipLines(s), segment: statusSegment(s) ?? "" };
}
