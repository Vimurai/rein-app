// A tab's phase (desktop.md §Look and feel v2 V7 component 2, D-054): `starting` from
// `tab_open` until the first of — the terminal switches to the alternate screen (tmux has
// drawn), the tab has received 4 KiB of pty output in total (older ConPTY builds draw the
// alternate screen themselves and never forward `ESC[?1049h`; E-53 measured `rein start`'s own
// lines under 1 KiB and tmux's first frame at about 5 KiB), the user types into the tab, or
// 20 s pass. `exited` once the pty has exited. Pure; the UI only renders it.

export type TabPhase = "starting" | "running" | "exited";

/** What the tab has seen since it was (re)started. */
export interface TabSignals {
  /** The terminal switched to the alternate screen. */
  altScreen: boolean;
  /** Bytes of pty output received in total. */
  bytes: number;
  /** The user typed into the tab. */
  typed: boolean;
  /** The pty exited. */
  exited: boolean;
  /** Milliseconds since `tab_open` was called. */
  elapsedMs: number;
}

/** Output after which a tab counts as drawn even without the alternate screen. */
export const START_BYTES = 4096;
/** The longest a tab shows as starting. */
export const START_LIMIT_MS = 20_000;

export function tabPhase(s: TabSignals): TabPhase {
  if (s.exited) return "exited";
  if (s.altScreen || s.bytes >= START_BYTES || s.typed || s.elapsedMs >= START_LIMIT_MS) return "running";
  return "starting";
}
