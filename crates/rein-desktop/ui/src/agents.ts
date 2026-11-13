// The agent-exit bar as data (desktop.md §Look and feel v2 V9 components 4–5, D-057, E-62).
// Pure, so it is tested under Node; `agent-bar.tsx` draws it and `shell.tsx` polls.
//
// The core answers per role `running | ended | unknown`; an agent counts as ended only after
// the core saw it running in this tab, so a layout still being built never reads as exited.
// One of several ended: a bar with Close project / Keep open, nothing else by itself. All
// ended: the project closes without asking when `close_when_agents_exit` is on (the default);
// otherwise the same bar. Close project with an agent still running asks first (default No).

export type AgentState = "running" | "ended" | "unknown";
export interface Agent {
  role: string;
  state: AgentState;
}
/** `project_agents`' answer. */
export type AgentsReply = { agents: Agent[] } | { unavailable: string };
/** `project_close`' answer: closed, or the roles still running to confirm (none: not known). */
export type CloseReply = { closed: boolean } | { confirm: string[] };

/** The poll: at most one short process per 5 s (the core holds the same gate). */
export const POLL_MS = 5000;

/** Poll only the active tab, with the window visible, while its process runs (V9). */
export function shouldPoll(t: { active: boolean; visible: boolean; open: boolean; exited: boolean }): boolean {
  return t.active && t.visible && t.open && !t.exited;
}

/** `engineer` → `The Engineer`. Roles are the core's checked words (lower case, `-`, `_`). */
export function roleName(role: string): string {
  const words = role.split(/[-_]/).filter(Boolean);
  return `The ${words.map((w) => w.charAt(0).toUpperCase() + w.slice(1)).join(" ")}`;
}

/** "The Architect", "The Architect and the Engineer", "The A, the B and the C". */
function names(roles: readonly string[]): string {
  const n = roles.map((r, i) => (i === 0 ? roleName(r) : roleName(r).replace(/^The /, "the ")));
  return n.length <= 1 ? (n[0] ?? "") : `${n.slice(0, -1).join(", ")} and ${n[n.length - 1]}`;
}

export type BarView =
  | { kind: "none" }
  /** One or more ended, not all — or all with the setting off: the bar. `key` is the set of
   *  ended roles: Keep open hides the bar until that set changes. */
  | { kind: "bar"; text: string; key: string }
  /** All ended and the setting on: close without asking. */
  | { kind: "close" };

/** What the window does with the latest answer for a tab. `kept`: the key the operator chose
 *  Keep open for. */
export function agentBar(agents: readonly Agent[], opts: { closeWhenAllExit: boolean; kept: string | null }): BarView {
  const ended = agents.filter((a) => a.state === "ended").map((a) => a.role);
  if (ended.length === 0) return { kind: "none" };
  const all = agents.every((a) => a.state === "ended");
  if (all && opts.closeWhenAllExit) return { kind: "close" };
  const key = ended.join(",");
  if (key === opts.kept) return { kind: "none" };
  const text = `${names(ended)} ${ended.length === 1 ? "has" : "have"} exited.`;
  return { kind: "bar", text, key };
}

/** The confirm dialog's question for the roles still running (none: not known). */
export function confirmText(running: readonly string[]): string {
  if (running.length === 0) return "An agent may still be running. Close it too?";
  return running.length === 1
    ? `${names(running)} is still running. Close it too?`
    : `${names(running)} are still running. Close them too?`;
}
