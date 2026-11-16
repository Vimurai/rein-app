// The Project sheet's Team section as data (desktop.md §Look and feel v2 V10, D-058, E-64; the
// ruling after the read of rein 0.5.0). Pure, so it is tested under Node; `team-sheet.tsx`
// draws it. The core's check is the one that counts — these mirror it for help text only. The
// app names no mode: it offers by the shape of `rein team show`'s output.

export type Shape = "team" | "single" | "other";
export type Prefix = "E" | "P";
export type Mode = "team" | "legacy";

/** One finished run of rein: exit code and escaped lines (≤ 200). */
export interface Run {
  code: number | null;
  lines: string[];
  cut: boolean;
  timed_out: boolean;
}
export interface TeamView {
  show: Run;
  trunk: Run;
  shape: Shape;
}
export type ModeEnd = Run | { declined: boolean };

/** What the sheet offers for each shape (the ruling). */
export function offer(shape: Shape): { mode: Mode | null; form: boolean; note: string | null } {
  switch (shape) {
    case "team":
      return { mode: "legacy", form: true, note: null };
    case "single":
      return { mode: "team", form: true, note: null };
    default:
      return { mode: null, form: false, note: "rein's answer is not one this version reads — it is shown as rein printed it." };
  }
}

export const MODE_BUTTON: Record<Mode, string> = {
  team: "Turn on team mode",
  legacy: "Go back to single-developer mode",
};

/** The confirms' fixed sentences (V10; the core's native dialog repeats them). */
export const MODE_CONFIRM: Record<Mode, { title: string; text: string }> = {
  team: {
    title: "Turn on team mode?",
    text: "rein turns this project's task list into one file per task, writes .ai/team.json with you as the planner, and stops tracking TASKS.md. The project must have no uncommitted changes. Commit the result afterwards. Close and reopen the project so its agents pick up the change. The Tasks panel does not read team projects yet.",
  },
  legacy: {
    title: "Go back to single-developer mode?",
    text: "rein folds the task files back into one list and removes .ai/team.json with every developer's id range. The project must have no uncommitted changes. Commit the result afterwards. Close and reopen the project so its agents pick up the change.",
  },
};

/** V10 component 4: what the sheet cannot do, because rein has no command for it. */
export const NOT_OFFERED = "Changing or removing a developer, or a range, is not offered: rein has no command for it. Edit .ai/team.json and commit it.";

const EMAIL = /^(?!-)[A-Za-z0-9._+-]{1,64}@[A-Za-z0-9.-]{1,190}$/;

/** Help text for the form, or null when the values look acceptable (the core decides). */
export function formHint(email: string, from: string, to: string): string | null {
  if (email !== "" && !EMAIL.test(email)) return "An e-mail of letters, digits and . _ + - with one @.";
  const n = (s: string) => (/^[0-9]{1,5}$/.test(s) ? Number(s) : NaN);
  const a = n(from);
  const b = n(to);
  if (from !== "" && !(a >= 1 && a <= 99999)) return "From is a whole number from 1 to 99999.";
  if (to !== "" && !(b >= 1 && b <= 99999)) return "To is a whole number from 1 to 99999.";
  if (a >= 1 && b >= 1 && a > b) return "From is not above To.";
  return null;
}

/** The form can be sent: every field filled and no hint. */
export function formReady(email: string, from: string, to: string): boolean {
  return email !== "" && from !== "" && to !== "" && formHint(email, from, to) === null;
}

/** A run's last line for the operator: rein's lines are shown as printed above it. */
export function runNote(r: Run): { status: "ok" | "fail"; text: string } {
  if (r.timed_out) return { status: "fail", text: "rein did not finish in time and was stopped." };
  return r.code === 0 ? { status: "ok", text: "rein finished." } : { status: "fail", text: `rein answered with exit ${r.code ?? "?"} — its words are above.` };
}
