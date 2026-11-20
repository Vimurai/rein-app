// Keys on a set-up card or a restored tab's card (desktop.md §V9 "Keys on a set-up card",
// D-070 ruling 1; E-98). A card is part of its tab's stage, not an overlay: the plain keys
// (Tab, Shift+Tab, Enter, Space, Esc, letters) stay with it and work its buttons; every chord of
// the every-mode table (SIMPLE_SHORTCUTS: the chords with Ctrl or Alt) runs its action, in both
// input styles. A real overlay (the picker, a confirm sheet, Settings, the paste and stop
// questions) still takes every key. Pure: the shell asks, then acts.

import type { ActionCall } from "./keymap.ts";

/** What the window does with key `token` while a card is the current stage:
 *  - `{ action }` — run it (a shortcut chord);
 *  - `"card"` — leave the key to the card (its own buttons and focus);
 *  - `"overlay"` — a real overlay is open and takes it: the window does nothing. */
export type CardKey = { action: ActionCall } | "card" | "overlay";

export function cardKey(token: string, shortcuts: ReadonlyMap<string, ActionCall>, overlayOpen: boolean): CardKey {
  if (overlayOpen) return "overlay";
  const action = shortcuts.get(token);
  return action ? { action } : "card";
}
