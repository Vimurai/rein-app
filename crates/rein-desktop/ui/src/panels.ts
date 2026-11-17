// The side menu's panels as data (panels.md, D-056; E-60): the descriptor schema and the
// compiled-in registry, the key each panel gets, and the pure view models the three views
// draw. No DOM: tested under Node. Everything that comes from a project's `.ai/` files stays
// a string here and is put on the page as text by the views — never HTML.

/** The sources the core serves (`panel_data`, panel_ipc.rs) and the views the app draws. */
export const SOURCES = ["summary", "handoffs", "tasks", "attention"] as const;
export const VIEWS = ["summary", "list", "timeline"] as const;
/** The icon names a descriptor may use; rail.tsx maps each to a bundled lucide icon. */
export const PANEL_ICONS = ["layout-dashboard", "messages-square", "list-checks", "bell"] as const;
export type Source = (typeof SOURCES)[number];
export type View = (typeof VIEWS)[number];
export type PanelIcon = (typeof PANEL_ICONS)[number];

/** Task groups of the `list` view, in the order a descriptor may name them. */
export const TASK_GROUPS = ["IN_PROGRESS", "OPEN", "BLOCKED", "DONE"] as const;
export type TaskGroup = (typeof TASK_GROUPS)[number];
export const GROUP_LABEL: Record<TaskGroup, string> = {
  IN_PROGRESS: "In progress",
  OPEN: "Open",
  BLOCKED: "Blocked",
  DONE: "Done",
};

export interface ListOptions {
  /** The groups shown, in this order. */
  groups: TaskGroup[];
  /** The most rows a group shows (the newest); `DONE` shows `done` at most. */
  limit: number;
  done: number;
}
export interface TimelineOptions {
  /** `whose_turn`: the last handoff's "to" is pinned above the entries. */
  pin: "whose_turn" | null;
}
/** The `list` view over the `attention` source (E-61): only a row limit. */
export interface AttentionOptions {
  limit: number;
}
export type Options = Record<string, never> | ListOptions | TimelineOptions | AttentionOptions;

export interface Descriptor {
  schema: 1;
  id: string;
  title: string;
  icon: PanelIcon;
  source: Source;
  view: View;
  options: Options;
}

/** The ids the core knows (config.rs `PANEL_IDS`), in rail order. */
export const PANEL_IDS = ["dashboard", "communication", "tasks", "attention"] as const;
export type PanelId = (typeof PANEL_IDS)[number];

/** The compiled-in descriptors (panels.md §Components 7). */
export const DESCRIPTORS: readonly Descriptor[] = [
  { schema: 1, id: "dashboard", title: "Dashboard", icon: "layout-dashboard", source: "summary", view: "summary", options: {} },
  {
    schema: 1,
    id: "communication",
    title: "Communication",
    icon: "messages-square",
    source: "handoffs",
    view: "timeline",
    options: { pin: "whose_turn" },
  },
  {
    schema: 1,
    id: "tasks",
    title: "Tasks",
    icon: "list-checks",
    source: "tasks",
    view: "list",
    options: { groups: ["IN_PROGRESS", "OPEN", "BLOCKED", "DONE"], limit: 200, done: 10 },
  },
  { schema: 1, id: "attention", title: "Needs you", icon: "bell", source: "attention", view: "list", options: { limit: 200 } },
];

const ID = /^[a-z][a-z0-9-]{1,31}$/;
const isObject = (v: unknown): v is Record<string, unknown> => typeof v === "object" && v !== null && !Array.isArray(v);
const keysExactly = (o: Record<string, unknown>, keys: readonly string[]) =>
  Object.keys(o).length === keys.length && keys.every((k) => Object.prototype.hasOwnProperty.call(o, k));
const intIn = (v: unknown, lo: number, hi: number) => typeof v === "number" && Number.isInteger(v) && v >= lo && v <= hi;

/** A view's options, checked against its closed set (the `list` view's set depends on the
 *  source: task groups for `tasks`, only a limit for `attention`). */
function optionsOk(view: View, source: Source, o: unknown): string | null {
  if (!isObject(o)) return "options is not an object";
  if (view === "list" && source === "attention") {
    if (!keysExactly(o, ["limit"])) return "attention list options are {limit}";
    return intIn(o.limit, 1, 200) ? null : "limit is 1–200";
  }
  switch (view) {
    case "summary":
      return keysExactly(o, []) ? null : "summary takes no options";
    case "timeline":
      if (!keysExactly(o, ["pin"])) return "timeline options are {pin}";
      return o.pin === null || o.pin === "whose_turn" ? null : "pin is whose_turn or null";
    case "list": {
      if (!keysExactly(o, ["groups", "limit", "done"])) return "list options are {groups, limit, done}";
      const g = o.groups;
      if (!Array.isArray(g) || g.length === 0 || g.length > TASK_GROUPS.length) return "groups is a list of task groups";
      if (!g.every((x) => (TASK_GROUPS as readonly unknown[]).includes(x))) return "an unknown task group";
      if (new Set(g).size !== g.length) return "a task group is named twice";
      if (!intIn(o.limit, 1, 200) || !intIn(o.done, 1, 200)) return "limit and done are 1–200";
      return null;
    }
  }
}

/** Check a descriptor against the closed schema 1 (panels.md §Data Model). Anything unknown
 *  — a source, view, icon or key — is a refusal of the whole descriptor, never a guess. */
export function checkDescriptor(raw: unknown): { ok: Descriptor } | { refused: string } {
  if (!isObject(raw)) return { refused: "not an object" };
  if (!keysExactly(raw, ["schema", "id", "title", "icon", "source", "view", "options"])) {
    return { refused: "keys are schema, id, title, icon, source, view, options" };
  }
  if (raw.schema !== 1) return { refused: "schema is not 1" };
  if (typeof raw.id !== "string" || !ID.test(raw.id)) return { refused: "id is not [a-z][a-z0-9-]{1,31}" };
  if (typeof raw.title !== "string" || raw.title.length === 0 || [...raw.title].length > 40) return { refused: "title is 1–40 characters" };
  if (!(PANEL_ICONS as readonly unknown[]).includes(raw.icon)) return { refused: "an unknown icon" };
  if (!(SOURCES as readonly unknown[]).includes(raw.source)) return { refused: "an unknown source" };
  if (!(VIEWS as readonly unknown[]).includes(raw.view)) return { refused: "an unknown view" };
  if (raw.source === "attention" && raw.view !== "list") return { refused: "the attention source is drawn by the list view" };
  const why = optionsOk(raw.view as View, raw.source as Source, raw.options);
  if (why) return { refused: why };
  return { ok: raw as unknown as Descriptor };
}

export function descriptor(id: string): Descriptor | null {
  return DESCRIPTORS.find((d) => d.id === id) ?? null;
}

/** The panels on offer: the enabled ids, in rail order (config's order does not reorder). */
export function enabledPanels(enabled: readonly string[]): Descriptor[] {
  return DESCRIPTORS.filter((d) => enabled.includes(d.id));
}

/** A panel's digit (Ctrl+Shift+<n>, modal `Space p <n>`): its place in `PANEL_IDS`, 1-based. */
export function panelDigit(id: string): number | null {
  const i = (PANEL_IDS as readonly string[]).indexOf(id);
  return i < 0 ? null : i + 1;
}

// ── host ────────────────────────────────────────────────────────────────────────────────

// status.md §Components 7 (E-74): 320 px to the smaller of 720 px and half the window.
export const PANEL_MIN = 320;
export const PANEL_MAX = 720;
export const PANEL_DEFAULT = 400;
/** Below this window width the panel lies over the terminal instead of beside it. */
export const OVERLAY_BELOW = 900;
/** How often an open panel reads its source while the window is visible (the core stats
 *  the file and parses only on a change); a change shows within 6 s. */
export const POLL_MS = 5000;

/** The widest the panel may be in a window this wide; never below the minimum. */
export const maxWidth = (windowWidth: number) => Math.max(PANEL_MIN, Math.min(PANEL_MAX, Math.floor(windowWidth / 2)));
export const clampWidth = (w: number, max: number = PANEL_MAX) => Math.round(Math.min(Math.min(max, PANEL_MAX), Math.max(PANEL_MIN, w)));
export const overlays = (windowWidth: number) => windowWidth < OVERLAY_BELOW;
/** A source is read only while its panel is open and the window can be seen. */
export const shouldRead = (open: boolean, visible: boolean) => open && visible;

/** "updated 12 s ago" from `read_at` (seconds since the epoch). */
export function updatedAgo(readAt: number, nowMs: number): string {
  return `updated ${age(Math.max(0, Math.floor(nowMs / 1000) - readAt))}`;
}

/** A duration in seconds as the panels say it. */
export function age(s: number): string {
  if (s < 60) return `${s} s ago`;
  if (s < 3600) return `${Math.floor(s / 60)} min ago`;
  if (s < 86400) return `${Math.floor(s / 3600)} h ago`;
  return `${Math.floor(s / 86400)} d ago`;
}

/** A handoff's timestamp ("2026-11-13 12:20 UTC") in seconds since the epoch, or null when
 *  it is not in that shape (the file is untrusted: no guess). */
export function handoffSeconds(ts: string): number | null {
  const m = /^(\d{4})-(\d{2})-(\d{2}) (\d{2}):(\d{2}) UTC$/.exec(ts.trim());
  if (!m) return null;
  const [y, mo, d, h, mi] = m.slice(1).map(Number) as [number, number, number, number, number];
  const t = Date.UTC(y, mo - 1, d, h, mi);
  return Number.isNaN(t) ? null : Math.floor(t / 1000);
}

/** "2026-11-13 12:20 UTC · 3 h ago", or the timestamp as written. */
export function when(ts: string, nowMs: number): string {
  const s = handoffSeconds(ts);
  return s === null ? ts : `${ts} · ${age(Math.max(0, Math.floor(nowMs / 1000) - s))}`;
}

// ── the sources' answers (panel_sources.rs) ─────────────────────────────────────────────

export interface Task {
  id: string;
  status: string;
  owner: string;
  tier: string;
  description: string;
  blocked_by: string[];
  updated_at: string;
}
export interface Tasks {
  tasks: Task[];
  cut: boolean;
  focus: string;
}
export interface Section {
  name: string;
  text: string;
}
export interface Handoff {
  timestamp: string;
  from: string;
  to: string;
  trigger: string;
  sections: Section[];
  cut: boolean;
}
export interface Handoffs {
  handoffs: Handoff[];
  cut: boolean;
}
export interface Summary {
  counts: { status: string; n: number }[];
  in_progress: Task | null;
  last_handoff: { timestamp: string; from: string; to: string; trigger: string } | null;
  whose_turn: string;
  focus: string;
}
export type Reply<T> = { ok: T; read_at: number } | { unavailable: string };

// ── view models ─────────────────────────────────────────────────────────────────────────

export interface Row {
  label: string;
  value: string;
}

/** The Dashboard: key facts as rows, counts as chips. Health and the tmux server come from
 *  what the app already holds. */
export function summaryModel(
  s: Summary,
  app: { health: string; server: string },
  nowMs: number,
): { rows: Row[]; chips: { label: string; n: number }[] } {
  const rows: Row[] = [
    { label: "Whose turn", value: s.whose_turn || "—" },
    { label: "Focus", value: s.focus || "—" },
    { label: "In progress", value: s.in_progress ? `${s.in_progress.id} · ${s.in_progress.description}` : "—" },
    {
      label: "Last handoff",
      value: s.last_handoff ? `${s.last_handoff.from} → ${s.last_handoff.to} · ${when(s.last_handoff.timestamp, nowMs)}` : "—",
    },
    { label: "Trigger", value: s.last_handoff?.trigger || "—" },
    { label: "Health", value: app.health },
    { label: "tmux server", value: app.server },
  ];
  const label = (st: string) => GROUP_LABEL[st as TaskGroup] ?? st;
  return { rows, chips: s.counts.map((c) => ({ label: label(c.status), n: c.n })) };
}

export interface Group {
  key: string;
  label: string;
  rows: Task[];
  /** Rows left out of this group by its limit. */
  more: number;
}

/** The Tasks list: the descriptor's groups in its order, newest last in the file → newest
 *  first here; any other status in an "Other" group at the end so nothing is hidden. */
export function taskGroups(t: Tasks, o: ListOptions): Group[] {
  const newestFirst = [...t.tasks].reverse();
  const groups: Group[] = o.groups.map((g) => {
    const all = newestFirst.filter((x) => x.status === g);
    const cap = g === "DONE" ? o.done : o.limit;
    return { key: g, label: GROUP_LABEL[g], rows: all.slice(0, cap), more: Math.max(0, all.length - cap) };
  });
  const known = new Set<string>(o.groups);
  const other = newestFirst.filter((x) => !known.has(x.status) && !(TASK_GROUPS as readonly string[]).includes(x.status));
  if (other.length) groups.push({ key: "OTHER", label: "Other", rows: other.slice(0, o.limit), more: Math.max(0, other.length - o.limit) });
  return groups;
}

export interface Entry {
  key: string;
  title: string;
  when: string;
  trigger: string;
  sections: Section[];
  cut: boolean;
}

/** The Communication timeline: newest first, and whose turn it is (the last handoff's "to"). */
export function timelineModel(h: Handoffs, nowMs: number): { whoseTurn: string | null; entries: Entry[] } {
  const last = h.handoffs.at(-1);
  const entries = [...h.handoffs].reverse().map((x, i) => ({
    key: `${h.handoffs.length - 1 - i}`,
    title: `${x.from || "?"} → ${x.to || "?"}`,
    when: when(x.timestamp, nowMs),
    trigger: x.trigger,
    sections: x.sections,
    cut: x.cut,
  }));
  return { whoseTurn: last ? last.to || null : null, entries };
}
