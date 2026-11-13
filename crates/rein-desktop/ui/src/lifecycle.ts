// The app's projects live only while the app does (desktop.md §Look and feel v2 V11, D-059,
// E-66): the confirms' words, as data. Pure; `agent-bar.tsx` draws them.
import { roleName } from "./agents.ts";

/** "Closing this tab stops the Architect and the Engineer in <project>." — the roles the core
 *  reported running; none: the status is not known. */
export function closeTabText(roles: readonly string[], project: string): string {
  if (roles.length === 0) return `Closing this tab stops the agents in ${project}.`;
  const names = roles.map((r) => roleName(r).replace(/^The /, "the "));
  const who = names.length === 1 ? names[0] : `${names.slice(0, -1).join(", ")} and ${names[names.length - 1]}`;
  return `Closing this tab stops ${who} in ${project}.`;
}

/** "Quitting stops the agents in N projects." */
export function quitText(n: number): string {
  return n === 1 ? "Quitting stops the agents in 1 project." : `Quitting stops the agents in ${n} projects.`;
}

/** After a close: what happened to the session (the user's server keeps its sessions). */
export function closedNote(title: string, server: "app" | "user"): string {
  return server === "app" ? `${title} closed — its agents were stopped` : `${title} closed — its tmux session keeps running`;
}
