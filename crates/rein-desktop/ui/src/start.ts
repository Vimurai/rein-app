// The start screen (desktop.md §Core Concept "Screen", E-38): the wordmark in block
// characters, the recent projects numbered 1–9 (a missing one dimmed), and five key hints.
// Nothing else. Pure, so it tests under Node.

import type { Span } from "./grid.ts";
import type { ActionCall, KeyTable } from "./keymap.ts";

export interface RecentView {
  path: string;
  name: string;
  last_opened: number;
  /** `false`: the directory is gone. `null`: not checked (Windows). */
  exists: boolean | null;
}

export const WORDMARK = ["█▀█ █▀▀ █ █▄ █", "█▀▄ ██▄ █ █ ▀█"];

export const HINTS = "p projects   o open directory   h checkhealth   u update   q quit";

/** The lines drawn centred in the stage when no project is open. `setup`: Windows without
 *  WSL or without a distro — the reason and the command to run come first (E-45). */
export function startLines(
  recent: readonly RecentView[],
  setup: { reason: string; command: string } | null = null,
): (string | Span)[] {
  const shown = recent.slice(0, 9);
  const width = Math.max(0, ...shown.map((r) => r.name.length));
  const rows: (string | Span)[] = shown.map((r, i) => ({
    text: `${i + 1}  ${r.name.padEnd(width)}  ${r.path}${r.exists === false ? "  (missing)" : ""}`,
    cls: r.exists === false ? "dim" : "",
  }));
  const blocker: (string | Span)[] = setup
    ? [
        { text: setup.reason, cls: "error" },
        { text: `run in an administrator terminal:  ${setup.command}`, cls: "" },
        "c  check again",
        "",
      ]
    : [];
  return [
    ...WORDMARK.map((text) => ({ text, cls: "accent" })),
    "",
    ...blocker,
    ...(rows.length ? rows : ["no recent projects"]),
    "",
    HINTS,
  ];
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
