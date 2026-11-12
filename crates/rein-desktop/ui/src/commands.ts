// The command line (desktop.md §API "Command line"): `:open DIR`, `:projects`, `:checkhealth`,
// `:update`, `:rollback`, `:profiles`, `:config`, `:theme NAME`, `:settings`, `:q`, `:qa`, `:help`. Tab
// completes command names, directories and theme names; an unknown command is
// `E492: Not a command: <name>`. Pure — directory listing comes in through `dirs`.

import type { ActionCall } from "./keymap.ts";
import type { ActionName } from "./actions.ts";

interface CommandSpec {
  action: ActionName;
  /** The argument the command needs, if any. */
  arg?: "dir" | "theme";
}

export const COMMANDS: ReadonlyMap<string, CommandSpec> = new Map<string, CommandSpec>([
  ["open", { action: "project.open", arg: "dir" }],
  ["projects", { action: "project.pick" }],
  ["checkhealth", { action: "health.show" }],
  ["update", { action: "release.update" }],
  ["rollback", { action: "release.rollback" }],
  ["profiles", { action: "profiles.show" }],
  ["config", { action: "config.open" }],
  ["theme", { action: "theme.set", arg: "theme" }],
  ["settings", { action: "settings.show" }],
  ["q", { action: "tab.detach" }],
  ["qa", { action: "app.quit" }],
  ["help", { action: "help.show" }],
]);

export type Parsed = { action: ActionCall } | { error: string } | { empty: true };

/** Parse the text after `:`. */
export function parse(line: string): Parsed {
  const text = line.trim();
  if (!text) return { empty: true };
  const space = text.search(/\s/);
  const name = space < 0 ? text : text.slice(0, space);
  const rest = space < 0 ? "" : text.slice(space).trim();
  const spec = COMMANDS.get(name);
  if (!spec) return { error: `E492: Not a command: ${name}` };
  if (spec.arg && !rest) return { error: "E471: Argument required" };
  if (!spec.arg && rest) return { error: "E488: Trailing characters" };
  return { action: spec.arg ? { name: spec.action, arg: rest } : { name: spec.action } };
}

export interface CompletionSources {
  themes: readonly string[];
  /** Directories whose path starts with the prefix (ending in `/`). */
  dirs: (prefix: string) => Promise<readonly string[]>;
}

/** The candidates for the last word of `line` (the text after `:`), each a full new line. */
export async function complete(line: string, src: CompletionSources): Promise<string[]> {
  const space = line.search(/\s/);
  if (space < 0) {
    return [...COMMANDS.keys()].filter((c) => c.startsWith(line)).sort();
  }
  const name = line.slice(0, space);
  const word = line.slice(space).trimStart();
  const head = `${name} `;
  switch (COMMANDS.get(name)?.arg) {
    case "theme":
      return src.themes.filter((t) => t.startsWith(word)).map((t) => head + t);
    case "dir":
      return (await src.dirs(word)).map((d) => head + d);
    default:
      return [];
  }
}

/** The longest text every candidate starts with (what Tab fills in before cycling). */
export function commonPrefix(items: readonly string[]): string {
  if (!items.length) return "";
  let p = items[0] ?? "";
  for (const s of items) while (!s.startsWith(p)) p = p.slice(0, -1);
  return p;
}
