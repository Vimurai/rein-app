// Home — every project at a glance (status.md §Home H1–H5, D-064; E-80): the pure part. The
// card order and the rule that a card never moves under the pointer or keyboard focus, the
// summary line, what one card shows, and where tab.next / tab.prev go with Home in the cycle.
// The core sends every field (`project_status`, E-76 / E-95 / E-80); nothing here reads a file.

import { age } from "./panels.ts";
import { taskTitle } from "./panel-text.ts";
import { TASK_ID, sentence, type ProjectStatus, type StatusState } from "./status.ts";

/** H2: the cards' order by state; tab order within a state. */
export const STATE_ORDER: readonly StatusState[] = ["needs_you", "ended", "setup", "quiet", "working", "starting", "unknown"];

/** The Home button's accessible name (H1). */
export const HOME_LABEL = "Home";

/** One open tab as Home orders it: its tab-bar index and its state (unknown when the core has
 *  not sent one yet). */
export interface CardKey {
  index: number;
  state: StatusState;
}

/** The tab-bar indexes in card order (H2). */
export function cardOrder(cards: readonly CardKey[]): number[] {
  const rank = (s: StatusState) => STATE_ORDER.indexOf(s);
  return [...cards].sort((a, b) => rank(a.state) - rank(b.state) || a.index - b.index).map((c) => c.index);
}

/** The order to draw (H2): `next` unless the grid is held (the pointer over it, or a card has
 *  keyboard focus) — then the cards keep the places they had, a closed one leaves, and a new
 *  one goes last. Keys are stable per tab (its id), not its index. */
export function heldOrder<K>(prev: readonly K[], next: readonly K[], held: boolean): K[] {
  if (!held) return [...next];
  const kept = prev.filter((k) => next.includes(k));
  return [...kept, ...next.filter((k) => !kept.includes(k))];
}

/** H2's summary line: "<n> projects open · <n> working · <n> need you · <n> quiet · <n>
 *  ended", a part with zero left out; null with no project open. */
export function summaryLine(states: readonly StatusState[]): string | null {
  if (states.length === 0) return null;
  const n = (s: StatusState) => states.filter((x) => x === s).length;
  const open = `${states.length} ${states.length === 1 ? "project" : "projects"} open`;
  const needs = n("needs_you");
  const parts = [
    open,
    n("working") ? `${n("working")} working` : null,
    needs ? `${needs} ${needs === 1 ? "needs" : "need"} you` : null,
    n("quiet") ? `${n("quiet")} quiet` : null,
    n("ended") ? `${n("ended")} ended` : null,
  ];
  return parts.filter(Boolean).join(" · ");
}

/** The trigger as a card shows it: one line, at most 120 characters (the core cut it too). */
export const TRIGGER_MAX = 120;

/** What one project card shows (H3). Every string is a text node. */
export interface CardModel {
  name: string;
  state: StatusState;
  sentence: string;
  /** "In progress: <id> — <title>", "No task in progress", or null when the tasks are not
   *  available. */
  task: string | null;
  /** "<whose turn>'s turn · last handoff <age>", or null when not known. */
  turn: string | null;
  trigger: string | null;
  /** The agents by role, "<role>: running" / "ended" (unknown left out). */
  agents: string[];
  /** Open / in progress / blocked; null: "Tasks not available". */
  chips: { open: number; in_progress: number; blocked: number } | null;
  /** The project's Now items. */
  now: number;
}

/** The card of one open project. `s` undefined: the core has sent nothing yet — its state is
 *  unknown and nothing is guessed. */
export function cardModel(name: string, s: ProjectStatus | undefined): CardModel {
  if (!s) return { name, state: "unknown", sentence: "—", task: null, turn: null, trigger: null, agents: [], chips: null, now: 0 };
  const counts = s.counts ?? null;
  const ip = s.in_progress;
  const task =
    counts === null
      ? null
      : ip && TASK_ID.test(ip.id)
        ? `In progress: ${ip.id}${ip.title ? ` — ${taskTitle(ip.title)}` : ""}`
        : "No task in progress";
  const turn = s.turn ? `${s.turn}'s turn${s.last_handoff_s != null ? ` · last handoff ${age(s.last_handoff_s)}` : ""}` : null;
  const trigger = s.trigger ? oneLine(s.trigger, TRIGGER_MAX) : null;
  const agents = (s.agents ?? []).filter((a) => a.state !== "unknown").map((a) => `${a.role}: ${a.state}`);
  return { name, state: s.state, sentence: sentence(s), task, turn, trigger, agents, chips: counts, now: s.now.length };
}

function oneLine(text: string, max: number): string {
  const flat = text.replace(/\s+/g, " ").trim();
  return flat.length > max ? `${flat.slice(0, max - 1)}…` : flat;
}

/** tab.next / tab.prev with Home in the cycle (H2): Home is place -1, before the first tab.
 *  `at`: -1 on Home, else the current tab's index. Returns the place to show. */
export function cycle(at: number, tabs: number, forward: boolean): number {
  const places = tabs + 1; // Home and the tabs
  const i = at + 1;
  return ((i + (forward ? 1 : places - 1)) % places) - 1;
}

/** H4: the side menu on Home — only "Needs you" has something to show. */
export const HOME_PANELS: readonly string[] = ["attention"];
export const HOME_PANEL_TIP = "Open a project to see this";

/** H5: what the window shows once the start-up restore is done. `startDir`: a project given on
 *  the command line (or by a second start); `last`: `[window] start = "last"`; `restored`: tabs
 *  the restore opened. True: Home. */
export function homeAtStart(startDir: boolean, last: boolean, restored: number): boolean {
  if (startDir) return false;
  if (restored === 0) return true;
  return !last;
}
