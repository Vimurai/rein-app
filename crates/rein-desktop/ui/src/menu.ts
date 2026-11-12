// The menu (desktop.md "Two ways in", D-034; E-40): one searchable list of every action in
// plain words, each with its shortcut in the current input style. Pure, so the pointer-parity
// test can walk it.

import { ACTIONS } from "./actions.ts";
import { expanded, type ActionCall, type KeyTable } from "./keymap.ts";
import { THEME_NAMES } from "./theme.ts";

export interface MenuEntry {
  call: ActionCall;
  label: string;
  /** The key in the current style, else the other style's (with its name), else "". */
  shortcut: string;
}

const LABEL = new Map<string, string>(ACTIONS.map((a) => [a.name, a.label]));

const same = (a: ActionCall, b: ActionCall) => a.name === b.name && (a.arg ?? "") === (b.arg ?? "");

/** `C-S-p` → `Ctrl+Shift+P`, `M-1` → `Alt+1`, `<leader> h` stays readable. */
export function showKey(keys: string): string {
  return keys
    .split(" ")
    .map((t) => {
      let rest = t;
      const mods: string[] = [];
      for (;;) {
        const m = /^([CMDS])-(.+)$/.exec(rest);
        if (!m?.[1] || !m[2]) break;
        mods.push({ C: "Ctrl", M: "Alt", D: "Cmd", S: "Shift" }[m[1]] ?? m[1]);
        rest = m[2];
      }
      const key = mods.length && rest.length === 1 ? rest.toUpperCase() : rest;
      return mods.length ? `${mods.join("+")}+${key}` : key;
    })
    .join(" ");
}

function keyFor(table: KeyTable, call: ActionCall): string | null {
  for (const [k, c] of table.shortcuts) if (same(c, call)) return k;
  for (const [k, c] of expanded(table)) if (same(c, call)) return k;
  if (call.name === "mode.normal" && table.escape.length) return table.escape.join(" ");
  return null;
}

/** The tooltip and accessible name of a button: the action in plain words and its key. */
export function describe(call: ActionCall, current: KeyTable, other: KeyTable): string {
  const base = LABEL.get(call.name) ?? call.name;
  const label =
    call.name === "tab.goto" && call.arg
      ? `Go to tab ${call.arg}`
      : call.name === "tab.detach" && call.arg
        ? `Close tab ${call.arg} (its session keeps running)`
        : call.name === "project.open" && call.arg
          ? `Open ${call.arg}`
          : base;
  const plain: ActionCall = call.name === "tab.detach" ? { name: call.name } : call;
  const mine = keyFor(current, plain);
  const theirs = mine === null ? keyFor(other, plain) : null;
  const key = mine !== null ? showKey(mine) : theirs !== null ? `${other.style}: ${showKey(theirs)}` : "";
  return key ? `${label}  (${key})` : label;
}

export function menuEntries(
  current: KeyTable,
  other: KeyTable,
  ctx: { tabs: number },
): MenuEntry[] {
  const out: MenuEntry[] = [];
  const add = (call: ActionCall, label: string) => {
    const mine = keyFor(current, call);
    const theirs = mine === null ? keyFor(other, call) : null;
    const shortcut = mine !== null ? showKey(mine) : theirs !== null ? `${other.style}: ${showKey(theirs)}` : "";
    out.push({ call, label, shortcut });
  };
  for (const a of ACTIONS) {
    const label = LABEL.get(a.name) ?? a.name;
    switch (a.arg) {
      case "n":
        for (let n = 1; n <= Math.max(1, Math.min(ctx.tabs, 9)); n++) add({ name: a.name, arg: String(n) }, `Go to tab ${n}`);
        break;
      case "name":
        for (const t of THEME_NAMES) add({ name: a.name, arg: t }, `${label}: ${t}`);
        break;
      case "style":
        add({ name: a.name, arg: "simple" }, `${label}: simple (shortcuts, no modes)`);
        add({ name: a.name, arg: "modal" }, `${label}: modal (vim-style modes)`);
        break;
      default:
        add({ name: a.name }, label);
    }
  }
  return out;
}
