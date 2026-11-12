// The default keymap (desktop.md "Default keymap", D-033 ruling 2): the table the `modes`
// function reads. A table is data, so E-37 (config.toml) and E-40 (the `simple` input style)
// supply other tables to the same function.

import type { ActionName } from "./actions.ts";

export interface ActionCall {
  name: ActionName;
  arg?: string;
}

export interface KeyTable {
  /** `simple` (no modes; the default, D-034) | `modal` (the nvim-style keymap). */
  style: "simple" | "modal";
  /** TERMINAL: the chord that leaves to NORMAL (none in `simple`); every other key goes to
   *  the pty, except the shortcuts. */
  escape: readonly string[];
  /** NORMAL: the leader key; `<leader>` in a binding stands for it. */
  leader: string;
  /** NORMAL: key sequence (tokens joined by one space) → action. */
  normal: ReadonlyMap<string, ActionCall>;
  /** Single-key shortcuts honoured in every mode, unless a NORMAL binding takes the key
   *  (D-034: the `simple` shortcuts keep working in `modal`). */
  shortcuts: ReadonlyMap<string, ActionCall>;
}

/** The `simple` style's shortcuts — the ones common terminals use (desktop.md "Two ways in"). */
export const SIMPLE_SHORTCUTS: ReadonlyMap<string, ActionCall> = new Map<string, ActionCall>([
  ["C-S-p", { name: "menu.show" }],
  ["C-S-o", { name: "project.open" }],
  ["C-S-t", { name: "project.pick" }],
  ["C-Tab", { name: "tab.next" }],
  ["C-S-Tab", { name: "tab.prev" }],
  ...["1", "2", "3", "4", "5", "6", "7", "8", "9"].map((n): [string, ActionCall] => [`M-${n}`, { name: "tab.goto", arg: n }]),
  ["C-S-w", { name: "tab.detach" }],
  ["C-S-q", { name: "app.quit" }],
  ["C-S-c", { name: "term.copy" }],
  ["C-S-v", { name: "term.paste" }],
  ["C-=", { name: "font.bigger" }],
  ["C--", { name: "font.smaller" }],
  ["C-0", { name: "font.reset" }],
  ["C-,", { name: "settings.show" }],
  ["F1", { name: "help.show" }],
]);

const a = (name: ActionName, arg?: string): ActionCall => (arg === undefined ? { name } : { name, arg });

export const DEFAULT_MODAL: KeyTable = {
  style: "modal",
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
    // Beyond the blueprint's table, so every action has a key in at least one style
    // (pointer parity, D-034): the rest of the leader.
    ["<leader> S", a("profiles.apply")],
    ["<leader> c", a("config.open")],
    ["<leader> R", a("config.reload")],
    ["<leader> t", a("theme.set")],
    ["<leader> ,", a("settings.show")],
    ["<leader> i", a("input.style")],
    ["<leader> <leader>", a("menu.show")],
  ]),
  shortcuts: SIMPLE_SHORTCUTS,
};

/** The `simple` style (the default): no modes, no escape chord; keys go to the terminal
 *  except the shortcuts. Without a tab, the start screen's keys apply (start.ts). */
export const DEFAULT_SIMPLE: KeyTable = {
  style: "simple",
  escape: [],
  leader: "Space",
  normal: new Map(),
  shortcuts: SIMPLE_SHORTCUTS,
};

/** A table's bindings with `<leader>` replaced by its leader key. */
export function expanded(table: KeyTable): Map<string, ActionCall> {
  const out = new Map<string, ActionCall>();
  for (const [keys, call] of table.normal) out.set(keys.replaceAll("<leader>", table.leader), call);
  return out;
}
