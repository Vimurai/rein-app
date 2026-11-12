// The window's view of config.toml (desktop.md §Components 4, §Data Model `ConfigFile`, E-37):
// the core reads and validates the file; this turns the result into what the UI uses — the
// key table for `modes`, the font list. Pure: no DOM.

import { isActionName } from "./actions.ts";
import { DEFAULT_MODAL, type ActionCall, type KeyTable } from "./keymap.ts";

export interface ConfigFile {
  font: { family: string; size: number };
  theme: { name: string; custom: Record<string, string> };
  window: { decorations: boolean; opacity: number; start: "dashboard" | "last" };
  keys: { escape: string[]; leader: string; normal: Record<string, string> };
  wsl: { distro: string | null };
}

export interface ConfigView {
  config: ConfigFile;
  path: string | null;
  error: string | null;
}

export const DEFAULT_CONFIG: ConfigFile = {
  font: { family: "", size: 14 },
  theme: { name: "gruvbox-dark", custom: {} },
  window: { decorations: true, opacity: 1, start: "dashboard" },
  keys: { escape: ["C-\\", "C-n"], leader: "Space", normal: {} },
  wsl: { distro: null },
};

/** The platform's monospace fonts when config.toml names none (W6 on Windows). */
export function fontFamily(cfg: ConfigFile, windows: boolean): string {
  if (cfg.font.family.trim()) return cfg.font.family;
  return windows ? `"Cascadia Mono", Consolas, monospace` : `"JetBrains Mono", "DejaVu Sans Mono", Menlo, monospace`;
}

/** `"tab.goto 3"` → `{ name: "tab.goto", arg: "3" }`; the core already refused anything else. */
function call(binding: string): ActionCall | null {
  const [name = "", arg] = binding.split(/ (.*)/s);
  if (!isActionName(name)) return null;
  return arg === undefined ? { name } : { name, arg };
}

/** The default keymap with config.toml's escape chord, leader and NORMAL bindings over it. */
export function keyTable(cfg: ConfigFile): KeyTable {
  const normal = new Map(DEFAULT_MODAL.normal);
  for (const [keys, binding] of Object.entries(cfg.keys.normal)) {
    const c = call(binding);
    if (c) normal.set(keys, c);
  }
  return { escape: cfg.keys.escape, leader: cfg.keys.leader, normal };
}
