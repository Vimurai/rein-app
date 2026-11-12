// The window's view of config.toml (desktop.md §Components 4, §Data Model `ConfigFile`, E-37):
// the core reads and validates the file; this turns the result into what the UI uses — the
// key table for `modes`, the font list. Pure: no DOM.

import { isActionName } from "./actions.ts";
import { terminalStack } from "./fonts.ts";
import { DEFAULT_MODAL, DEFAULT_SIMPLE, type ActionCall, type KeyTable } from "./keymap.ts";

export interface ConfigFile {
  font: { family: string; size: number };
  theme: { name: string; custom: Record<string, string> };
  window: { decorations: boolean; opacity: number; start: "dashboard" | "last" };
  keys: { escape: string[]; leader: string; normal: Record<string, string> };
  tmux: { overlay: boolean; status: "hidden" | "app" | "keep" };
  wsl: { distro: string | null };
}

export interface ConfigView {
  config: ConfigFile;
  path: string | null;
  error: string | null;
}

export const DEFAULT_CONFIG: ConfigFile = {
  font: { family: "", size: 14 },
  theme: { name: "catppuccin-mocha", custom: {} },
  window: { decorations: true, opacity: 1, start: "dashboard" },
  keys: { escape: ["C-\\", "C-n"], leader: "Space", normal: {} },
  tmux: { overlay: true, status: "hidden" },
  wsl: { distro: null },
};

/** The terminal font stack: config.toml's `font.family`, else the bundled JetBrains Mono,
 *  with the bundled symbols font always in it (V3, D-051). */
export function fontFamily(cfg: ConfigFile): string {
  return terminalStack(cfg.font.family);
}

/** `"tab.goto 3"` → `{ name: "tab.goto", arg: "3" }`; the core already refused anything else. */
function call(binding: string): ActionCall | null {
  const [name = "", arg] = binding.split(/ (.*)/s);
  if (!isActionName(name)) return null;
  return arg === undefined ? { name } : { name, arg };
}

/** The key table for the input style: `modal` is the default keymap with config.toml's
 *  escape chord, leader and NORMAL bindings over it; `simple` has no modes (D-034). Both
 *  keep the simple shortcuts. */
export function keyTable(cfg: ConfigFile, style: "simple" | "modal" = "modal"): KeyTable {
  if (style === "simple") return DEFAULT_SIMPLE;
  const normal = new Map(DEFAULT_MODAL.normal);
  for (const [keys, binding] of Object.entries(cfg.keys.normal)) {
    const c = call(binding);
    if (c) normal.set(keys, c);
  }
  return { ...DEFAULT_MODAL, escape: cfg.keys.escape, leader: cfg.keys.leader, normal };
}
