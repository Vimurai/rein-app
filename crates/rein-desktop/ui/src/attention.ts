// "Needs you" (panels.md §Components 6, D-056; E-61): what only the operator can move, across
// every open tab. A pure function of the tabs' app state and each tab's `attention` facts
// from the core (panel_attention.rs) — no DOM, tested under Node. Every string from a
// project's files stays a string and is put on the page as text by the view.

import type { ActionCall } from "./keymap.ts";
import { handoffSeconds } from "./panels.ts";

/** The core's facts for one tab (`panel_data(tab, "attention")`). */
export interface AttentionFacts {
  tasks_read: boolean;
  blocked: { id: string; description: string }[];
  in_progress: boolean;
  last_handoff: string | null;
  mentions: string[];
}

/** What the app itself holds about one open tab. */
export interface AttentionTab {
  /** The tab's place in the tab bar (0-based): "go to tab" switches to it. */
  index: number;
  /** The tab's project name, as the tab bar shows it. */
  title: string;
  /** `rein start` ended in this tab (not counting a refused setup). */
  exited: boolean;
  /** The tab's tmux setup was refused (V8). */
  refused: boolean;
  /** A bell rang while the tab did not have focus. */
  bell: boolean;
  /** rein said the project is out of date (V9). */
  update: boolean;
  /** The core's facts, once read (null before the first answer, or not available). */
  facts: AttentionFacts | null;
}

export type AttentionKind = "ended" | "refused" | "health" | "update" | "blocked" | "quiet" | "bell" | "mention";

export interface AttentionItem {
  key: string;
  kind: AttentionKind;
  /** The project it is about; "Every project" for the installation's health. */
  project: string;
  /** The tab to go to, or null (health is not one tab's). */
  tab: number | null;
  /** The app's own sentence. */
  text: string;
  /** Text from a project's file (a task's description, a handoff line), shown as a quote. */
  quote: string | null;
}

/** The most items the panel lists (panels.md §Data Model). */
export const MAX_ITEMS = 200;
/** "No handoff for more than 30 minutes while a task is in progress." */
export const QUIET_S = 30 * 60;

/** The known gap, said in the panel's empty state (and in docs/operator-guide.md). */
export const KNOWN_GAP =
  "An agent waiting at a permission prompt is not shown here: the app cannot see the prompt — not while the window is open, and not while the app is closed, when the session keeps running. Until rein gives a signal, a bell from the tab is the only hint, and only if Claude Code rings one.";

/** A span of time as the panel says it: minutes up to two hours, then hours, then days. */
export function span(s: number): string {
  const m = Math.floor(s / 60);
  if (m < 120) return `${m} min`;
  if (m < 48 * 60) return `${Math.floor(m / 60)} h`;
  return `${Math.floor(m / 1440)} d`;
}

/** The items, in tab order, each tab's in a fixed order; health first. At most `MAX_ITEMS`. */
export function attentionItems(tabs: readonly AttentionTab[], health: "ok" | "warn" | "fail" | "unknown", nowMs: number): AttentionItem[] {
  const out: AttentionItem[] = [];
  if (health === "warn" || health === "fail") {
    out.push({
      key: "health",
      kind: "health",
      project: "Every project",
      tab: null,
      text: health === "fail" ? "The installation health check is failing." : "The installation health check has warnings.",
      quote: null,
    });
  }
  const now = Math.floor(nowMs / 1000);
  for (const t of tabs) {
    const item = (kind: AttentionKind, text: string, quote: string | null = null, n = "") =>
      out.push({ key: `${t.index}:${kind}:${n}`, kind, project: t.title, tab: t.index, text, quote });
    if (t.refused) item("refused", "The tmux setup was refused; the tab says why.");
    else if (t.exited) item("ended", "rein start has ended in this tab.");
    if (t.update) item("update", "rein says the project is out of date.");
    if (t.bell) item("bell", "The terminal rang its bell.");
    const f = t.facts;
    if (!f) continue;
    for (const b of f.blocked) item("blocked", `${b.id} is blocked.`, b.description || null, b.id);
    if (f.in_progress) {
      const at = f.last_handoff === null ? null : handoffSeconds(f.last_handoff);
      // A timestamp not in the known shape is not guessed at: no item.
      if (at !== null && now - at > QUIET_S) {
        item("quiet", `A task is in progress and there has been no handoff for ${span(now - at)}.`);
      }
    }
    f.mentions.forEach((m, i) => item("mention", "The last handoff mentions the operator:", m, String(i)));
  }
  return out.slice(0, MAX_ITEMS);
}

/** A bell marks its tab only when that tab is not the one in front of the user. */
export const bellMarks = (isCurrent: boolean, windowFocused: boolean) => !isCurrent || !windowFocused;
/** A marked tab is cleared once it is the current tab of a focused window. */
export const bellClears = (isCurrent: boolean, windowFocused: boolean) => isCurrent && windowFocused;

/** The count on the rail's "Needs you" button: a number, no animation; over 99 it says 99+. */
export const badgeText = (n: number) => (n > 99 ? "99+" : String(n));

/** "Go to tab": the panel's one action. It runs `tab.goto` and nothing else — it never
 *  writes, runs a command or types into a terminal. */
export const goToTab = (run: (c: ActionCall) => void) => (index: number) => run({ name: "tab.goto", arg: String(index + 1) });
