// Overlays on the grid (desktop.md "Screen", §Components 6 `overlays`). E-36: the leader's
// which-key overlay — a rounded box border with the title in it, one row per key that can
// follow, the action in plain words. More overlays (picker, health, profiles, confirm) join
// in E-38..E-40.

import { ACTIONS } from "./actions.ts";
import type { ActionCall } from "./keymap.ts";

const LABELS = new Map<string, string>(ACTIONS.map((a) => [a.name, a.label]));

export function label(call: ActionCall): string {
  return LABELS.get(call.name) ?? call.name;
}

/** `╭─ title ───╮` … `╰───╯`, `width` cells wide, with `rows` inside. */
export function boxRows(title: string, rows: readonly string[], width: number): string[] {
  const inner = Math.max(title.length + 4, width - 2);
  const pad = (s: string) => (s.length > inner - 2 ? `${s.slice(0, inner - 3)}…` : s.padEnd(inner - 2));
  const head = `─ ${title} `;
  return [
    `╭${head}${"─".repeat(Math.max(0, inner - head.length))}╮`,
    ...rows.map((r) => `│ ${pad(r)} │`),
    `╰${"─".repeat(inner)}╯`,
  ];
}

export function WhichKey({ leader, next }: { leader: string; next: readonly [string, ActionCall][] }) {
  const width = Math.max(30, ...next.map(([k, a]) => k.length + label(a).length + 8));
  const lines = boxRows(leader, next.map(([k, a]) => `${k.padEnd(3)}  ${label(a)}`), width);
  return (
    <div className="which-key" role="dialog" aria-label={`keys after ${leader}`}>
      {lines.map((l, i) => (
        <div key={i} className="row">
          {l}
        </div>
      ))}
    </div>
  );
}
