// The default keymap (desktop.md "Default keymap", D-033 ruling 2): the table the `modes`
// function reads. A table is data, so E-37 (config.toml) and E-40 (the `simple` input style)
// supply other tables to the same function.

import type { ActionName } from "./actions.ts";

export interface ActionCall {
  name: ActionName;
  arg?: string;
}

export interface KeyTable {
  /** TERMINAL: the chord that leaves to NORMAL; every other key goes to the pty. */
  escape: readonly string[];
  /** NORMAL: the leader key; `<leader>` in a binding stands for it. */
  leader: string;
  /** NORMAL: key sequence (tokens joined by one space) → action. */
  normal: ReadonlyMap<string, ActionCall>;
}

const a = (name: ActionName, arg?: string): ActionCall => (arg === undefined ? { name } : { name, arg });

export const DEFAULT_MODAL: KeyTable = {
  escape: ["C-\\", "C-n"],
  leader: "Space",
  normal: new Map<string, ActionCall>([
    ["i", a("mode.terminal")],
    ["a", a("mode.terminal")],
    ["Enter", a("mode.terminal")],
    [":", a("mode.command")],
    ["<leader> p", a("project.pick")],
    ["<leader> o", a("project.open")],
    ["g t", a("tab.next")],
    ["g T", a("tab.prev")],
    ...["1", "2", "3", "4", "5", "6", "7", "8", "9"].map((n): [string, ActionCall] => [n, a("tab.goto", n)]),
    ["<leader> h", a("health.show")],
    ["<leader> u", a("release.update")],
    ["<leader> r", a("release.rollback")],
    ["<leader> s", a("profiles.show")],
    ["y", a("cmd.yank")],
    ["Z Z", a("tab.detach")],
  ]),
};

/** A table's bindings with `<leader>` replaced by its leader key. */
export function expanded(table: KeyTable): Map<string, ActionCall> {
  const out = new Map<string, ActionCall>();
  for (const [keys, call] of table.normal) out.set(keys.replaceAll("<leader>", table.leader), call);
  return out;
}
