// "Needs you" (panels.md §Components 6, D-056; E-61), triaged by status.md §Components 4–5
// (D-063; E-75). The rules live in the core now (attention.rs): this side sends what only it
// knows about each open tab — closed booleans and a bell count, by tab-bar index — and draws
// the two lists it gets back, putting its own tab titles on them. No DOM here; tested under
// Node. Every string from a project's files stays a string and is put on the page as text.

import type { ActionCall } from "./keymap.ts";

/** What only the web side knows about one open tab (`attention_items`). */
export interface TabState {
  index: number;
  tab: number | null;
  exited: boolean;
  refused: boolean;
  /** Bells rung while the tab was out of sight and not yet looked at; 0 when none. */
  bell: number;
  update: boolean;
}

export type Kind = "ended" | "refused" | "bell" | "quiet" | "mention" | "health_fail" | "blocked" | "update" | "health_warn";

export interface Item {
  key: string;
  kind: Kind;
  /** The tab-bar index it is about, or null (the installation's health). */
  index: number | null;
  /** The tab's core id, for a dismiss. */
  tab: number | null;
  text: string;
  quote: string | null;
  /** The blocked tasks a `blocked` row opens to. */
  list: { id: string; description: string }[];
  dismissable: boolean;
}

export interface Lists {
  now: Item[];
  info: Item[];
}

export const NO_ITEMS: Lists = { now: [], info: [] };

/** The known gap, behind "About this list" (and in docs/operator-guide.md): two sentences. */
export const KNOWN_GAP =
  "The app cannot see an agent waiting at a permission prompt, whether the window is open or closed. Until rein gives a signal, a bell from the tab is the only hint.";

/** The rows of one project: its name and tab, then its items in the core's order. */
export interface ProjectGroup {
  key: string;
  project: string;
  index: number | null;
  items: Item[];
}

/** Items grouped by project, in the order the projects first appear. `title` names a tab by
 *  its index; an item with no index belongs to "Every project". */
export function byProject(items: readonly Item[], title: (index: number) => string): ProjectGroup[] {
  const groups: ProjectGroup[] = [];
  const at = new Map<string, ProjectGroup>();
  for (const it of items) {
    const key = it.index === null ? "all" : `tab:${it.index}`;
    let g = at.get(key);
    if (!g) {
      g = { key, project: it.index === null ? "Every project" : title(it.index), index: it.index, items: [] };
      at.set(key, g);
      groups.push(g);
    }
    g.items.push(it);
  }
  return groups;
}

/** A bell marks its tab only when that tab is not the one in front of the user. */
export const bellMarks = (isCurrent: boolean, windowFocused: boolean) => !isCurrent || !windowFocused;
/** A marked tab is cleared once it is the current tab of a focused window. */
export const bellClears = (isCurrent: boolean, windowFocused: boolean) => isCurrent && windowFocused;

/** The count on the rail's "Needs you" button — Now items only: a number, no animation; over
 *  99 it says 99+. */
export const badgeText = (n: number) => (n > 99 ? "99+" : String(n));

/** "Go to tab": runs `tab.goto` and nothing else — it never writes, runs a command or types
 *  into a terminal. */
export const goToTab = (run: (c: ActionCall) => void) => (index: number) => run({ name: "tab.goto", arg: String(index + 1) });
