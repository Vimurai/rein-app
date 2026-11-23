// The update watch's view on the web side (updates.md §Components 1, 2 and 4, §Data Model,
// D-065). The core holds the facts and sends `updates` on every change; this file only
// decides what to draw from them and never checks on a timer of its own (a hidden web view's
// timers are throttled). Pure: no DOM.

/** `UpdatesState` as the core sends it. */
export interface UpdatesState {
  rein: { installed: string | null; newest: string | null; checked_at: number | null; offline: boolean };
  app: {
    running: string;
    newest: string | null;
    notes: string | null;
    /** in_place: the core installs it (Windows); command: the package manager's line;
     *  by_hand: signed for another key, install it by hand (E-86). */
    mode: "in_place" | "command" | "by_hand" | "none";
    command: string | null;
    /** The version this window was updated from, once after the restart. */
    updated_from: string | null;
  };
  job: UpdateJob;
  asked: { rein: string | null; app: string | null };
}

/** The window's own install (E-86): a failed step is one of five fixed sentences in the core. */
export type UpdateStep = "fetch" | "too_large" | "signature" | "checksum" | "start";
export type UpdateJob =
  | "idle"
  | "verifying"
  | "restarting"
  | { downloading: { done: number; total: number } }
  | { failed: UpdateStep };

/** `a` compared with `b` as semver (major.minor.patch, a pre-release below its release);
 *  `null` when either is not a version. */
export function compareVersions(a: string, b: string): number | null {
  const parse = (v: string) => {
    const m = /^v?(\d{1,9})\.(\d{1,9})\.(\d{1,9})(?:-([0-9A-Za-z.-]+))?(?:\+[0-9A-Za-z.-]+)?$/.exec(v);
    return m ? { n: [Number(m[1]), Number(m[2]), Number(m[3])], pre: m[4] ?? null } : null;
  };
  const x = parse(a);
  const y = parse(b);
  if (!x || !y) return null;
  for (let i = 0; i < 3; i++) if (x.n[i] !== y.n[i]) return (x.n[i] ?? 0) < (y.n[i] ?? 0) ? -1 : 1;
  if (x.pre === y.pre) return 0;
  if (x.pre === null) return 1;
  if (y.pre === null) return -1;
  return x.pre < y.pre ? -1 : 1;
}

/** The newer published Rein, when one is known and newer than the installed one. */
export function reinNewer(s: UpdatesState | null): string | null {
  const { installed, newest } = s?.rein ?? { installed: null, newest: null };
  if (!installed || !newest) return null;
  return (compareVersions(newest, installed) ?? 0) > 0 ? newest : null;
}

/** The newer window, when one is known (the window's own update, a later task). */
export function appNewer(s: UpdatesState | null): string | null {
  if (!s?.app.newest || s.app.mode === "none") return null;
  return (compareVersions(s.app.newest, s.app.running) ?? 0) > 0 ? s.app.newest : null;
}

/** The title bar's Update button shows while either is newer (component 4). */
export function showUpdateButton(s: UpdatesState | null): boolean {
  return reinNewer(s) !== null || appNewer(s) !== null;
}

/** The ask card for Rein: once per version — not for a version answered "Later". */
export function reinAsk(s: UpdatesState | null): string | null {
  const v = reinNewer(s);
  return v !== null && s?.asked.rein !== v ? v : null;
}

/** The ask card's sentence for Rein. */
export const reinAskText = (v: string) => `Rein ${v} is available. Your projects keep running during the update.`;

/** The Update dialog's two rows: what is installed or running, and the newer one if any. */
export interface UpdateRow {
  name: string;
  from: string;
  to: string | null;
  note: string;
}

export function updateRows(s: UpdatesState | null): UpdateRow[] {
  const rein = reinNewer(s);
  const app = appNewer(s);
  const reinNote = rein ? "newer release" : s?.rein.offline ? "the release server cannot be reached" : s?.rein.checked_at ? "up to date" : "not checked yet";
  return [
    { name: "Rein", from: s?.rein.installed ?? "not installed", to: rein, note: reinNote },
    { name: "rein-app window", from: s?.app.running ?? "unknown", to: app, note: app ? "newer release" : "no newer window release" },
  ];
}

/** The installed Rein changed between two states (never on the first one). */
export function installedChanged(prev: UpdatesState | null, next: UpdatesState): boolean {
  return prev !== null && prev.rein.installed !== next.rein.installed;
}

/** The order the open tabs are asked again after the installed Rein changed: the active tab
 *  first, then the others left to right (component 1: one at a time). */
export function recheckOrder(count: number, active: number): number[] {
  const all = Array.from({ length: count }, (_, i) => i);
  return active >= 0 && active < count ? [active, ...all.filter((i) => i !== active)] : all;
}

// ── The window's own update drawn (updates.md §Components 4, D-066; E-87) ──

/** The core's fixed sentence for each failed step (app_update.rs `Step::sentence`). */
export const STEP_SENTENCES: Record<UpdateStep, string> = {
  fetch: "The update could not be downloaded.",
  too_large: "The update is larger than its release says. Nothing was installed.",
  signature: "The update's signature did not verify. Nothing was installed.",
  checksum: "The update's checksum did not match. Nothing was installed.",
  start: "The installer could not be started.",
};

/** The ask card's sentence for the window. */
export const appAskText = (v: string) => `rein-app ${v} is available. Your projects keep running during the update.`;
/** `by_hand`: the release is signed for another key; this window does not install it. */
export const BY_HAND_TEXT = "Install it by hand from the release page.";

/** Whole percent of a download, 0–100; nothing known yet is 0. */
export function percent(done: number, total: number): number {
  if (!(total > 0) || !(done > 0)) return 0;
  return Math.min(100, Math.floor((done * 100) / total));
}

/** What the window's card shows: the ask (once per version), or the running job. */
export type AppCard =
  | { at: "ask"; version: string; mode: "in_place" | "command" | "by_hand"; command: string | null }
  | { at: "downloading"; version: string; percent: number }
  | { at: "verifying"; version: string }
  | { at: "restarting"; version: string }
  | { at: "failed"; version: string; step: UpdateStep; sentence: string };

export function appCard(s: UpdatesState | null): AppCard | null {
  if (!s) return null;
  const version = s.app.newest ?? "";
  const j = s.job;
  if (j === "verifying" || j === "restarting") return { at: j, version };
  if (typeof j === "object" && "downloading" in j) return { at: "downloading", version, percent: percent(j.downloading.done, j.downloading.total) };
  if (typeof j === "object" && "failed" in j) return { at: "failed", version, step: j.failed, sentence: STEP_SENTENCES[j.failed] ?? STEP_SENTENCES.fetch };
  const v = appNewer(s);
  if (v === null || s.asked.app === v || s.app.mode === "none") return null;
  return { at: "ask", version: v, mode: s.app.mode, command: s.app.command };
}

/** The card's one line for each state. */
export function appCardText(c: AppCard): string {
  switch (c.at) {
    case "ask":
      return appAskText(c.version);
    case "downloading":
      return `Downloading ${c.version} — ${c.percent} %`;
    case "verifying":
      return "Checking the signature…";
    case "restarting":
      return "Restarting…";
    case "failed":
      return c.sentence;
  }
}

/** After a restart into a new version: "Updated to X", once per run of the window. */
export function updatedTo(s: UpdatesState | null): string | null {
  return s?.app.updated_from ? s.app.running : null;
}

/** The running version's notes, for "What's new": only while no newer one is offered (the
 *  notes are then the running version's own). */
export function runningNotes(s: UpdatesState | null): string | null {
  return s && s.app.newest === null && s.app.notes ? s.app.notes : null;
}
