// A project's status as the core sends it (status.md §Components 2, §Data Model, D-062; E-76):
// the `project_status` event and `project_status_all()`. The core decides the state; the web
// side draws it (E-77) and decides nothing. Types only.

import type { Item } from "./attention.ts";

export type StatusState = "setup" | "ended" | "starting" | "needs_you" | "working" | "quiet" | "unknown";

export interface ProjectStatus {
  /** The tab's core id. */
  tab: number;
  state: StatusState;
  /** Seconds since the tab's window last printed; null when not read. */
  quiet_s: number | null;
  /** How long output had been arriving before it stopped (gaps under 15 s ignored). */
  worked_s: number;
  /** Reserved for rein's waiting signal; always null until then. */
  waiting: boolean | null;
  now: Item[];
  info: Item[];
}
