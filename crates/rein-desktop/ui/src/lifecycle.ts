// Closing a project (desktop.md §Look and feel v2 V11 component 1, D-059, E-66; quit asks
// nothing since V12, D-061): the confirm's words, as data. Pure; `agent-bar.tsx` draws them.
import { roleName } from "./agents.ts";

/** "Closing this tab stops the Architect and the Engineer in <project>." — the roles the core
 *  reported running; none: the status is not known. */
export function closeTabText(roles: readonly string[], project: string): string {
  if (roles.length === 0) return `Closing this tab stops the agents in ${project}.`;
  const names = roles.map((r) => roleName(r).replace(/^The /, "the "));
  const who = names.length === 1 ? names[0] : `${names.slice(0, -1).join(", ")} and ${names[names.length - 1]}`;
  return `Closing this tab stops ${who} in ${project}.`;
}

/** After a close: what happened to the session (the user's server keeps its sessions). */
export function closedNote(title: string, server: "app" | "user"): string {
  return server === "app" ? `${title} closed — its agents were stopped` : `${title} closed — its tmux session keeps running`;
}
