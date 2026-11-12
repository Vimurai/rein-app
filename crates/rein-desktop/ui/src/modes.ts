// The modes state machine (desktop.md §Core Concept rule 2, §Components 6 `modes`, D-033
// ruling 2): `(mode, pending keys, key) → (mode, pending keys, action?)`, over TERMINAL /
// NORMAL / COMMAND, with the key table as an input. Pure TypeScript — no React, no DOM.
//
// TERMINAL forwards every key except the escape chord: a key that could begin the chord is
// held, and released (forwarded) as soon as the chord cannot complete, so the forwarded
// stream is exactly the input with each complete chord removed.

import { expanded, type ActionCall, type KeyTable } from "./keymap.ts";

export type Mode = "TERMINAL" | "NORMAL" | "COMMAND";

export interface ModeState {
  mode: Mode;
  pending: readonly string[];
}

export interface Step extends ModeState {
  /** The action the key completed, if any. */
  action?: ActionCall;
  /** TERMINAL: keys to send to the pty, in order. */
  forward: readonly string[];
  /** COMMAND: the key goes to the command line editor. */
  edit?: string;
}

const startsWith = (seq: readonly string[], prefix: readonly string[]) =>
  prefix.length <= seq.length && prefix.every((k, i) => seq[i] === k);

/** The mode an action moves to, if it is a mode action. */
function modeAfter(action: ActionCall, mode: Mode): Mode {
  switch (action.name) {
    case "mode.terminal":
      return "TERMINAL";
    case "mode.normal":
      return "NORMAL";
    case "mode.command":
      return "COMMAND";
    default:
      return mode;
  }
}

export function step(table: KeyTable, state: ModeState, key: string): Step {
  switch (state.mode) {
    case "TERMINAL":
      return terminal(table, state, key);
    case "NORMAL":
      return normal(table, state, key);
    case "COMMAND":
      return key === "Esc"
        ? { mode: "NORMAL", pending: [], forward: [], action: { name: "mode.normal" } }
        : { mode: "COMMAND", pending: [], forward: [], edit: key };
  }
}

function terminal(table: KeyTable, state: ModeState, key: string): Step {
  // A shortcut (Ctrl+Shift+P …) works in TERMINAL too, unless a chord is in progress.
  const shortcut = state.pending.length === 0 ? table.shortcuts.get(key) : undefined;
  if (shortcut) return { mode: "TERMINAL", pending: [], forward: [], action: shortcut };
  if (table.escape.length === 0) return { mode: "TERMINAL", pending: [], forward: [key] };
  const seq = [...state.pending, key];
  if (seq.length === table.escape.length && startsWith(seq, table.escape)) {
    return { mode: "NORMAL", pending: [], forward: [], action: { name: "mode.normal" } };
  }
  // Hold the longest tail of `seq` that can still become the chord; forward the rest.
  for (let i = 0; i < seq.length; i++) {
    const tail = seq.slice(i);
    if (startsWith(table.escape, tail)) {
      return { mode: "TERMINAL", pending: tail, forward: seq.slice(0, i) };
    }
  }
  return { mode: "TERMINAL", pending: [], forward: seq };
}

function normal(table: KeyTable, state: ModeState, key: string): Step {
  if (key === "Esc") return { mode: "NORMAL", pending: [], forward: [] };
  const bindings = expanded(table);
  const lookup = (seq: readonly string[]): Step | null => {
    const joined = seq.join(" ");
    const action = bindings.get(joined);
    if (action) return { mode: modeAfter(action, "NORMAL"), pending: [], forward: [], action };
    for (const k of bindings.keys()) {
      if (k.startsWith(`${joined} `)) return { mode: "NORMAL", pending: seq, forward: [] };
    }
    return null;
  };
  // A key that breaks a pending sequence starts over on its own; then the shortcuts; an
  // unbound key is ignored.
  const shortcut = table.shortcuts.get(key);
  return (
    lookup([...state.pending, key]) ??
    (state.pending.length ? lookup([key]) : null) ??
    (shortcut ? { mode: "NORMAL", pending: [], forward: [], action: shortcut } : null) ?? {
      mode: "NORMAL",
      pending: [],
      forward: [],
    }
  );
}

/** What can follow `pending` (the which-key overlay after the leader): the next key → action. */
export function continuations(table: KeyTable, pending: readonly string[]): [string, ActionCall][] {
  const prefix = pending.join(" ");
  const out: [string, ActionCall][] = [];
  for (const [keys, action] of expanded(table)) {
    if (keys.startsWith(`${prefix} `)) out.push([keys.slice(prefix.length + 1), action]);
  }
  return out;
}

/** The leader's which-key overlay appears this long after the leader was pressed. */
export const WHICH_KEY_DELAY_MS = 400;
