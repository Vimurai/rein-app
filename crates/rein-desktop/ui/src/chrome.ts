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
  /** Stays the same for the tab's life (V7: a closed tab's neighbours are not re-drawn). */
  id?: number;
  title: string;
  current: boolean;
  /** The pty has exited (`rein start` ended): a dot on the tab. */
  exited: boolean;
  /** `rein start` has not drawn yet (V7 `tabPhase`): a spinner in place of the close icon. */
  starting?: boolean;
}

export interface TabControl {
  id: number;
  label: string;
  current: boolean;
  exited: boolean;
  starting: boolean;
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
      id: t.id ?? i,
      label: t.title,
      current: t.current,
      exited: t.exited,
      starting: t.starting === true,
      select: { name: "tab.goto", arg: String(i + 1) },
      close: { name: "tab.detach", arg: String(i + 1) },
    })),
    add: { name: "project.open" },
    version: { label: version, action: { name: "release.update" } },
    menu: { name: "menu.show" },
  };
}

export type HealthIcon = "ok" | "warn" | "fail" | "unknown" | "checking";

/** The status bar's health badge: an icon and plain words. `busy`: a health run is in flight
 *  (V7) — it says so, whatever the last result was. */
export function healthBadge(h: HealthView | null, busy = false): { icon: HealthIcon; text: string } {
  if (busy) return { icon: "checking", text: "Checking…" };
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
  /** The last command (rule 4: the UI teaches the CLI) with a copy button; `copied`: the
   *  icon is a check for a moment after a copy (V7). */
  command: { text: string; copy: ActionCall; copied: boolean } | null;
  /** The current tab is starting (V7): `Starting rein…` where the command chip sits. */
  starting: boolean;
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
  busy: { starting?: boolean; health?: boolean; copied?: boolean } = {},
): StatusBar {
  return {
    project: project === null ? null : { text: tildePath(project), action: { name: "project.pick" } },
    health: { ...healthBadge(health, busy.health === true), action: { name: "health.show" } },
    command: lastCommand === null ? null : { text: lastCommand, copy: { name: "cmd.yank" }, copied: busy.copied === true },
    starting: busy.starting === true,
    mode,
  };
}

/** V6 (E-54): a tab's overlay that was not applied is told once, in a toast with the health
 *  row's words — never for a tab that has closed, never twice for the same tab. `why`: what
 *  `tab_tmux` answered; `open`: the tab is still open; `told`: this tab was told already. */
export function overlayNotice(why: string | null, open: boolean, told: boolean): string | null {
  return why !== null && open && !told ? why : null;
}

/** How long a toast stays (V2). */
export const TOAST_MS = 4000;
/** How long the copy icon shows a check after a copy (V7). */
export const COPIED_MS = 1200;
