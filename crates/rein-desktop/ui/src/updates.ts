// The update watch's view on the web side (updates.md §Components 1, 2 and 4, §Data Model,
// D-065). The core holds the facts and sends `updates` on every change; this file only
// decides what to draw from them and never checks on a timer of its own (a hidden web view's
// timers are throttled). Pure: no DOM.

/** `UpdatesState` as the core sends it. */
export interface UpdatesState {
  rein: { installed: string | null; newest: string | null; checked_at: number | null; offline: boolean };
  app: { running: string; newest: string | null; notes: string | null; mode: "in_place" | "command" | "none"; command: string | null };
  job: string;
  asked: { rein: string | null; app: string | null };
}

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
