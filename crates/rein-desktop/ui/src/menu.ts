// The menu (desktop.md "Two ways in", D-034; E-40): one searchable list of every action in
// plain words, each with its shortcut in the current input style. In `simple` style (E-55) it
// starts with what a click-first user reaches for, leaves out the modes and shows only simple
// keys. Pure, so the pointer-parity test can walk it.

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

/** The `simple` style's menu order (E-55, desktop.md "Two ways in"): what a click-first user
 *  reaches for first. Actions not named here follow, in the list's order, before the last two. */
export const SIMPLE_FIRST = [
  "project.open",
  "project.pick",
  "tab.next",
  "tab.prev",
  "tab.goto",
  "tab.detach",
  "health.show",
  "release.update",
  "release.rollback",
  "profiles.show",
  "profiles.apply",
] as const;
export const SIMPLE_LAST = ["settings.show", "app.quit"] as const;
/** Modes exist only in the `modal` style; a click-first user cannot use them (E-55). */
export const MODE_ACTIONS = ["mode.normal", "mode.terminal", "mode.command"] as const;

export function menuEntries(
  current: KeyTable,
  other: KeyTable,
  ctx: { tabs: number },
): MenuEntry[] {
  const simple = current.style === "simple";
  const byName = new Map<string, MenuEntry[]>();
  const add = (call: ActionCall, label: string) => {
    const mine = keyFor(current, call);
    // `simple` shows only its own keys: a `modal:` hint is a key a click-first user has not
    // got (E-55). `modal` names the other style's key, as before.
    const theirs = mine === null && !simple ? keyFor(other, call) : null;
    const shortcut = mine !== null ? showKey(mine) : theirs !== null ? `${other.style}: ${showKey(theirs)}` : "";
    const list = byName.get(call.name) ?? [];
    list.push({ call, label, shortcut });
    byName.set(call.name, list);
  };
  for (const a of ACTIONS) {
    if (simple && (MODE_ACTIONS as readonly string[]).includes(a.name)) continue;
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
  const named = [...byName.keys()];
  const order = simple
    ? [
        ...SIMPLE_FIRST,
        ...named.filter((n) => !(SIMPLE_FIRST as readonly string[]).includes(n) && !(SIMPLE_LAST as readonly string[]).includes(n)),
        ...SIMPLE_LAST,
      ]
    : named;
  return order.flatMap((n) => byName.get(n) ?? []);
}
