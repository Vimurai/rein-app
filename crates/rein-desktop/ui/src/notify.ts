// Notifications in the window (status.md §Components 6 channel 1, D-062; E-78). The core
// decides which event is raised and when (notify.rs: once on a change, the 30 s silence,
// nothing for the tab in front of a focused window); it sends `notify` with the fixed
// sentence. This side only shows it as the existing toast, and only when the event's tab is
// not the current tab. Pure: no DOM.

/** The closed list of events (notify.rs `Event`), in the table's order. */
export const EVENTS = ["needs_you", "stopped", "stuck", "ended", "handoff"] as const;
export type NotifyEvent = (typeof EVENTS)[number];

/** The `notify` event's payload. */
export interface Notification {
  event: NotifyEvent;
  tab: number;
  sentence: string;
}

/** Settings' label for each event. */
export const EVENT_LABELS: Record<NotifyEvent, string> = {
  needs_you: "A project needs you",
  stopped: "A project stops after working",
  stuck: "A project is stuck",
  ended: "An agent ends",
  handoff: "A handoff names you",
};

/** The toast's text for `n`, or null: an unknown event, an empty sentence, or the current
 *  tab (`current`: its core id). */
export function toastText(n: Notification, current: number | undefined): string | null {
  if (!(EVENTS as readonly string[]).includes(n.event)) return null;
  if (typeof n.sentence !== "string" || n.sentence.length === 0) return null;
  if (n.tab === current) return null;
  return n.sentence;
}

/** Settings' events list with `event` switched on or off, in the table's order. */
export function withEvent(events: readonly string[], event: NotifyEvent, on: boolean): NotifyEvent[] {
  return EVENTS.filter((e) => (e === event ? on : events.includes(e)));
}
