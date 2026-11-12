// The start screen (desktop.md §Look and feel v2 V2, E-38 → E-50): a centred column — the
// wordmark, a primary `Open a project…`, the recent projects as rows (number, name, dim path,
// last opened; a missing one dimmed), a secondary `Check installation`, the WSL setup card on
// Windows when needed, and the key hints. Pure, so it tests under Node; shell.tsx draws it.

import type { ActionCall, KeyTable } from "./keymap.ts";

export interface RecentView {
  path: string;
  name: string;
  /** Seconds since the Unix epoch. */
  last_opened: number;
  /** `false`: the directory is gone. `null`: not checked (Windows). */
  exists: boolean | null;
}

export const OPEN_LABEL = "Open a project…";
export const CHECK_LABEL = "Check installation";

/** The keys the start screen answers to, in the words its hint line shows. */
export const HINTS: readonly [string, string][] = [
  ["p", "projects"],
  ["o", "open directory"],
  ["h", "check health"],
  ["u", "update"],
  ["q", "quit"],
];

export interface StartRow {
  /** The digit that opens it (1–9). */
  n: number;
  name: string;
  path: string;
  /** "3 h ago", "yesterday", … */
  ago: string;
  missing: boolean;
  action: ActionCall;
}

export interface StartScreen {
  /** Windows without WSL or without a distro (E-45): the reason and the command to run;
   *  `checking` while a check runs (V7: `Checking WSL…`). */
  setup: { reason: string; command: string; again: ActionCall; checking: boolean } | null;
  /** The recent projects have not arrived yet (V7: skeleton rows). */
  loading: boolean;
  open: { label: string; action: ActionCall };
  check: { label: string; action: ActionCall };
  recent: StartRow[];
  hints: readonly [string, string][];
}

/** How long ago `then` was, at `now` (both seconds since the epoch), in a few plain words. */
export function ago(then: number, now: number): string {
  const s = Math.max(0, now - then);
  if (s < 60) return "just now";
  if (s < 3600) return `${Math.floor(s / 60)} min ago`;
  if (s < 86400) return `${Math.floor(s / 3600)} h ago`;
  const days = Math.floor(s / 86400);
  if (days === 1) return "yesterday";
  if (days < 30) return `${days} days ago`;
  return new Date(then * 1000).toISOString().slice(0, 10);
}

/** What the start screen shows; at most nine recent projects, newest first as given. */
export function startScreen(
  recent: readonly RecentView[],
  setup: { reason: string; command: string } | null,
  now: number,
  state: { loading?: boolean; checking?: boolean } = {},
): StartScreen {
  return {
    setup: setup && { ...setup, again: { name: "health.show" }, checking: state.checking === true },
    loading: state.loading === true,
    open: { label: OPEN_LABEL, action: { name: "project.open" } },
    check: { label: CHECK_LABEL, action: { name: "health.show" } },
    recent: recent.slice(0, 9).map((r, i) => ({
      n: i + 1,
      name: r.name,
      path: r.path,
      ago: ago(r.last_opened, now),
      missing: r.exists === false,
      action: { name: "project.open", arg: r.path },
    })),
    hints: HINTS,
  };
}

/** The start screen's keys over a key table: the five hints (digits are `tab.goto`, which
 *  opens recent project N while no tab is open). */
export function withStartKeys(table: KeyTable): KeyTable {
  const normal = new Map<string, ActionCall>(table.normal);
  normal.set("p", { name: "project.pick" });
  normal.set("o", { name: "project.open" });
  normal.set("h", { name: "health.show" });
  normal.set("u", { name: "release.update" });
  normal.set("q", { name: "app.quit" });
  normal.set("c", { name: "health.show" }); // "check again" when WSL is not ready (E-45)
  // Digit N opens recent project N, in either input style (simple has no digit keys of its own).
  for (const n of ["1", "2", "3", "4", "5", "6", "7", "8", "9"]) normal.set(n, { name: "tab.goto", arg: n });
  return { ...table, normal };
}

/** Fuzzy match for the project picker: every character of `query`, in order, anywhere in
 *  the name or path (case-insensitive). Recency order is kept. */
export function fuzzy(query: string, items: readonly RecentView[]): RecentView[] {
  const q = query.toLowerCase().replace(/\s+/g, "");
  if (!q) return [...items];
  const matches = (text: string) => {
    let i = 0;
    for (const c of text.toLowerCase()) if (c === q[i]) i++;
    return i === q.length;
  };
  return items.filter((r) => matches(r.name) || matches(r.path));
}
