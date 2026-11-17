// An agent's exit closes the project after a countdown (desktop.md §Look and feel v2 V12
// component 7, D-061; E-71). Operator direction: `/exit` in one pane closes both agents and the
// project's session; the app cannot tell `/exit` from a crash, so it counts down 5 s with
// Keep open and Close now. Pure, with the clock passed in: tested under Node; `agent-bar.tsx`
// applies it on every poll answer and every tick, `shell.tsx` draws it.
//
// Only the active tab of a visible window counts down; any other tab with an exit is marked
// and starts again from 5 when the user comes to it. With `close_when_agents_exit` off, V9's
// bar as built (agents.ts). All roles ended: V9's close without asking, as built.

import { agentBar, roleName, type Agent, type BarView } from "./agents.ts";

export const COUNTDOWN_S = 5;

/** A tab's running countdown: the set of ended roles it is for, and when it reaches zero. */
export interface Countdown {
  key: string;
  deadline: number;
}

export type CountdownView = BarView | { kind: "countdown"; text: string; key: string; left: number };

/** The tab's state the countdown reads and writes. */
export interface CountdownTab {
  agents?: Agent[] | undefined;
  /** Roles not running at a restored tab's first status read: ended (V12 7). */
  endedAtRestore?: string[] | undefined;
  kept?: string | null | undefined;
  autoFailed?: boolean | undefined;
  exited: boolean;
  closing?: boolean | undefined;
  countdown?: Countdown | null | undefined;
  /** An agent exited while the tab was not in front: the tab bar shows a marker. */
  marked?: boolean | undefined;
}

/** The agents as the countdown counts them: a role not running at the restored tab's first
 *  read is ended (an exit while the app was closed was never seen running). */
export function counted(agents: readonly Agent[], endedAtRestore: readonly string[] = []): Agent[] {
  return agents.map((a) => (a.state !== "running" && endedAtRestore.includes(a.role) ? { ...a, state: "ended" } : a));
}

/** "The Engineer exited." / "The Architect and the Engineer exited." */
function exitedText(key: string): string {
  const roles = key.split(",");
  const n = roles.map((r, i) => (i === 0 ? roleName(r) : roleName(r).replace(/^The /, "the ")));
  return `${n.length <= 1 ? (n[0] ?? "") : `${n.slice(0, -1).join(", ")} and ${n[n.length - 1]}`} exited.`;
}

/** One tab at `now`. `shown`: it is the active tab of a visible window. Writes the tab's
 *  `countdown` and `marked`; `fire` is true when the countdown has reached zero (the caller
 *  closes the project without its confirm). */
export function countdownStep(
  t: CountdownTab,
  opts: { closeWhenAgentsExit: boolean; project: string; shown: boolean; now: number },
): { view: CountdownView; fire: boolean } {
  const none = { view: { kind: "none" } as CountdownView, fire: false };
  if (!t.agents || t.exited || t.closing) {
    t.countdown = null;
    return none;
  }
  const base = agentBar(counted(t.agents, t.endedAtRestore), {
    closeWhenAllExit: opts.closeWhenAgentsExit && !t.autoFailed,
    kept: t.kept ?? null,
  });
  // The setting off (V9's bar as built), nothing ended, kept, or all ended (V9's close).
  if (!opts.closeWhenAgentsExit || base.kind !== "bar") {
    t.countdown = null;
    if (base.kind === "none") t.marked = false;
    return { view: base, fire: false };
  }
  if (!opts.shown) {
    t.countdown = null;
    t.marked = true;
    return none;
  }
  t.marked = false;
  if (!t.countdown || t.countdown.key !== base.key) t.countdown = { key: base.key, deadline: opts.now + COUNTDOWN_S * 1000 };
  const left = Math.max(0, Math.ceil((t.countdown.deadline - opts.now) / 1000));
  return {
    view: { kind: "countdown", text: `${exitedText(base.key)} Closing ${opts.project} in ${left} s.`, key: base.key, left },
    fire: left === 0,
  };
}
