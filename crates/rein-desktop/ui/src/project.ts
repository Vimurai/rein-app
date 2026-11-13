// Set-up and out-of-date as data (desktop.md §Look and feel v2 V9 components 1–3, D-057,
// E-63; the ruling of 2026-11-13 14:10). Pure, so it is tested under Node; `project-ui.tsx`
// draws it and `shell.tsx` asks the core. rein's rows arrive capped and made visible by the
// core; every other word here is the app's own.

/** `tab_open` for a folder without `.ai/`: a tab with no terminal yet. */
export interface SetupView {
  id: number;
  project: string;
  title: string;
}

/** `project_check`'s answer. */
export type CheckReply = { ready: boolean } | { not_set_up: string[] } | { out_of_date: string[] } | { unavailable: string };

/** `project_fix`'s answer: the exit code, or why it was stopped. */
export interface FixEnd {
  code: number | null;
  stopped: "cancelled" | "timed_out" | null;
}

/** The set-up child's exit when `rein init` failed (`project_setup::INIT_FAILED`). */
export const INIT_FAILED = 119;
export const INIT_FAILED_SENTENCE = "rein init did not succeed — its output is above. Nothing else was run.";

/** The set-up card (V9 component 2): the app's own words around rein's rows. */
export const SETUP_CARD = {
  title: "This folder is not set up for Rein.",
  /** What Set up this project does — the app's own text (the ruling, 6). */
  does: "Set up runs rein init here, in this tab's terminal, then rein start.",
  checking: "Asking rein what set-up will do…",
  noAnswer: "rein did not say what set-up will do.",
} as const;

/** The card's rows from `project_check` on a set-up tab: rein's lines, or none. */
export function cardRows(reply: CheckReply | null): { rows: string[]; note: string | null } {
  if (reply === null) return { rows: [], note: SETUP_CARD.checking };
  if ("not_set_up" in reply) return { rows: reply.not_set_up, note: null };
  return { rows: [], note: SETUP_CARD.noAnswer };
}

/** Ask once after the tab has started, unless the operator chose "Don't ask" (V9 3). */
export function shouldCheck(t: { project: string; started: boolean; asked: boolean }, dontAsk: readonly string[]): boolean {
  return t.started && !t.asked && !dontAsk.includes(t.project);
}

/** The out-of-date bar's line; rein's rows are shown in the sheet, above the run. */
export const UPDATE_BAR_TEXT = "rein reports this project is out of date — Update project runs rein init.";

/** What the window does after Update project: the sheet's last line and the bar. */
export interface FixOutcome {
  status: "ok" | "warn" | "fail";
  text: string;
  /** The bar stays offered in this tab. */
  keepBar: boolean;
}

export const AGENTS_KEEP_OLD = "Running agents keep their old settings until the project is closed and opened again.";

/** The ruling (4): after exit 0 check once more — 0: done; 1: one line, no second offer. */
export function fixOutcome(end: FixEnd, recheck: CheckReply | null): FixOutcome {
  if (end.stopped === "cancelled") return { status: "warn", text: "Cancelled — rein init was stopped before it finished.", keepBar: true };
  if (end.stopped === "timed_out")
    return { status: "fail", text: "rein init did not finish within 10 minutes and was stopped.", keepBar: true };
  if (end.code !== 0) return { status: "fail", text: `rein init ended with exit ${end.code ?? "?"}.`, keepBar: true };
  if (recheck !== null && "ready" in recheck) return { status: "ok", text: `Project updated. ${AGENTS_KEEP_OLD}`, keepBar: false };
  if (recheck !== null && "out_of_date" in recheck)
    return { status: "warn", text: "rein init finished, but rein still reports changes pending.", keepBar: false };
  return { status: "warn", text: "rein init finished; rein did not answer the check that followed.", keepBar: false };
}
