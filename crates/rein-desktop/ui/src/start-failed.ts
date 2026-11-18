// A start that fails (desktop.md V14, D-068; E-94): the pure half. The core decides that a
// tab's start failed (an ordinary start, a non-zero end, no session afterwards) and rein's
// `init --check` decides what can be offered; this file only turns that into the bar's words
// and buttons, and decides which one bar a tab shows. No text here comes from rein.

import { INIT_FAILED, type CheckReply } from "./project.ts";

/** A failed start: asking rein, rein's rows for `rein init`, or nothing to offer. */
export type FailedState = { at: "checking" } | { at: "needs_init"; rows: string[] } | { at: "no_fix" };

export const FAILED_LINE = "rein start did not start this project.";
export const NEEDS_INIT_LINE = "rein init --check reports changes for this folder.";
export const NO_FIX_LINE = "rein's message is in the terminal above.";
export const CHECKING_LINE = "Asking rein…";

export type FailedAction = "setup" | "retry" | "close";

export interface FailedBarView {
  lines: [string, string];
  buttons: { label: string; action: FailedAction; primary: boolean }[];
}

const RETRY = { label: "Try again", action: "retry" as const };
const CLOSE = { label: "Close tab", action: "close" as const, primary: false };

/** The bar's three forms (V14 component 3). */
export function failedBar(f: FailedState): FailedBarView {
  switch (f.at) {
    case "needs_init":
      return {
        lines: [FAILED_LINE, NEEDS_INIT_LINE],
        buttons: [{ label: "Set up this project", action: "setup", primary: true }, { ...RETRY, primary: false }, CLOSE],
      };
    case "no_fix":
      return { lines: [FAILED_LINE, NO_FIX_LINE], buttons: [{ ...RETRY, primary: true }, CLOSE] };
    default:
      return { lines: [FAILED_LINE, CHECKING_LINE], buttons: [{ ...RETRY, primary: false }, CLOSE] };
  }
}

/** `project_check`'s answer for a failed tab: `needs_init` only on rein's word; anything
 *  else (no answer, an error) offers nothing. */
export function failedFrom(reply: CheckReply | null): FailedState {
  if (reply && "start_failed" in reply && "needs_init" in reply.start_failed) {
    return { at: "needs_init", rows: reply.start_failed.needs_init };
  }
  return { at: "no_fix" };
}

/** The one bar above a tab's terminal: a failed start wins, and "Don't ask for this project"
 *  (which hides only the out-of-date bar) never hides it. */
export function whichBar(t: {
  failed?: FailedState | undefined;
  update?: readonly string[] | undefined;
  updateHidden?: boolean | undefined;
  exited: boolean;
  setup?: unknown;
}): "failed" | "update" | null {
  if (t.failed) return "failed";
  if (t.update && !t.updateHidden && !t.exited && !t.setup) return "update";
  return null;
}

/** After the set-up child of a failed tab ends: `rein init` failed → the bar returns as
 *  `no_fix` and is not offered again in this tab open (V14 component 5: no loop). */
export function afterSetUp(code: number, setUpOnce: boolean): FailedState | undefined {
  return setUpOnce && code === INIT_FAILED ? { at: "no_fix" } : undefined;
}
