// The window frame as data (desktop.md §Look and feel v2 V2, E-49): what the title bar and
// the status bar show and which action each control runs. Pure, so pointer parity tests
// under Node. `frame.tsx` draws it. Every control runs an action from the closed list
// (actions.ts) — the same one a key runs — except the window buttons, which move the window.

import type { ActionCall } from "./keymap.ts";
import type { HealthView } from "./ipc.ts";

/** Which frame the core asked for (`globalThis.__REIN_CHROME__`, set before any script). */
export type Chrome = "windows" | "macos" | "linux";

export function chromeOf(value: unknown): Chrome {
  return value === "windows" || value === "macos" ? value : "linux";
}

/** The window buttons we draw: on Windows, where the window has no native title bar. */
export function windowButtons(chrome: Chrome): readonly ("minimize" | "maximize" | "close")[] {
  return chrome === "windows" ? ["minimize", "maximize", "close"] : [];
}

/** Room left for the native traffic lights at the start of the row (macOS, V2). */
export function titleInset(chrome: Chrome): number {
  return chrome === "macos" ? 80 : 0;
}

export interface TabView {
  title: string;
  current: boolean;
  /** The pty has exited (`rein start` ended): a dot on the tab. */
  exited: boolean;
}

export interface TabControl {
  label: string;
  current: boolean;
  exited: boolean;
  /** Click: show the tab. */
  select: ActionCall;
  /** The close icon (on hover), and a middle click on the tab. */
  close: ActionCall;
}

export interface TitleBar {
  tabs: TabControl[];
  add: ActionCall;
  /** The version chip: opens Update. */
  version: { label: string; action: ActionCall };
  menu: ActionCall;
}

export function titleBar(tabs: readonly TabView[], version: string): TitleBar {
  return {
    tabs: tabs.map((t, i) => ({
      label: t.title,
      current: t.current,
      exited: t.exited,
      select: { name: "tab.goto", arg: String(i + 1) },
      close: { name: "tab.detach", arg: String(i + 1) },
    })),
    add: { name: "project.open" },
    version: { label: version, action: { name: "release.update" } },
    menu: { name: "menu.show" },
  };
}

export type HealthIcon = "ok" | "warn" | "fail" | "unknown" | "checking";

/** The status bar's health badge: an icon and plain words. */
export function healthBadge(h: HealthView | null): { icon: HealthIcon; text: string } {
  switch (h?.status) {
    case "ok":
      return { icon: "ok", text: "Install OK" };
    case "warn":
      return { icon: "warn", text: "Install warning" };
    case "fail":
      return { icon: "fail", text: "Install failed" };
    case "unknown":
      return { icon: "unknown", text: "Health unknown" };
    default:
      return { icon: "checking", text: "Checking health" };
  }
}

export interface StatusBar {
  /** The current tab's project path (dim); a click opens the project picker. */
  project: { text: string; action: ActionCall } | null;
  health: { icon: HealthIcon; text: string; action: ActionCall };
  /** The last command (rule 4: the UI teaches the CLI) with a copy button. */
  command: { text: string; copy: ActionCall } | null;
  /** Only in the `modal` input style. No clock: tmux shows one. */
  mode: string | null;
}

/** A project path as the status bar shows it (V2: `~/Dev/schraubr_laravel`): a path in a home
 *  directory — `/home/<user>/…` (Linux, WSL), `/Users/<user>/…` (macOS), `/root/…` — starts
 *  with `~`. The UI is not told the home directory, so this reads the path's own shape. */
export function tildePath(path: string): string {
  const m = /^(?:\/home\/[^/]+|\/Users\/[^/]+|\/root)(?=\/|$)/.exec(path);
  return m ? `~${path.slice(m[0].length)}` : path;
}

export function statusBar(
  project: string | null,
  health: HealthView | null,
  lastCommand: string | null,
  mode: string | null,
): StatusBar {
  return {
    project: project === null ? null : { text: tildePath(project), action: { name: "project.pick" } },
    health: { ...healthBadge(health), action: { name: "health.show" } },
    command: lastCommand === null ? null : { text: lastCommand, copy: { name: "cmd.yank" } },
    mode,
  };
}

/** How long a toast stays (V2). */
export const TOAST_MS = 4000;
