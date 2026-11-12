// Overlays on the grid (desktop.md "Screen", §Components 6 `overlays`). E-36: the leader's
// which-key overlay — a rounded box border with the title in it, one row per key that can
// follow, the action in plain words. More overlays (picker, health, profiles, confirm) join
// in E-38..E-40.

import { useState, type KeyboardEvent } from "react";
import { Command, CommandEmpty, CommandInput, CommandItem, CommandList, CommandShortcut } from "../components/command.tsx";
import { Button } from "../components/button.tsx";
import { Dialog, DialogContent } from "../components/dialog.tsx";
import { ACTIONS } from "./actions.ts";
import { fuzzy, type RecentView } from "./start.ts";
import type { ActionCall } from "./keymap.ts";
import type { MenuEntry } from "./menu.ts";
import { pasteLines } from "./paste.ts";

const LABELS = new Map<string, string>(ACTIONS.map((a) => [a.name, a.label]));

export function label(call: ActionCall): string {
  return LABELS.get(call.name) ?? call.name;
}

/** `╭─ title ───╮` … `╰───╯`, `width` cells wide, with `rows` inside. */
export function boxRows(title: string, rows: readonly string[], width: number): string[] {
  const inner = Math.max(title.length + 4, width - 2);
  const pad = (s: string) => (s.length > inner - 2 ? `${s.slice(0, inner - 3)}…` : s.padEnd(inner - 2));
  const head = `─ ${title} `;
  return [
    `╭${head}${"─".repeat(Math.max(0, inner - head.length))}╮`,
    ...rows.map((r) => `│ ${pad(r)} │`),
    `╰${"─".repeat(inner)}╯`,
  ];
}

export function WhichKey({ leader, next }: { leader: string; next: readonly [string, ActionCall][] }) {
  const width = Math.max(30, ...next.map(([k, a]) => k.length + label(a).length + 8));
  const lines = boxRows(leader, next.map(([k, a]) => `${k.padEnd(3)}  ${label(a)}`), width);
  return (
    <div className="which-key" role="dialog" aria-label={`keys after ${leader}`}>
      {lines.map((l, i) => (
        <div key={i} className="row">
          {l}
        </div>
      ))}
    </div>
  );
}

/** The project picker (desktop.md "Screen" overlay; E-38): recent projects, fuzzy filtered,
 *  `j` `k` (while the filter is empty) `C-n` `C-p` ↑ ↓ to move, `Enter` to open, `Del` to
 *  forget the selected one, `Esc` to close. Built on the restyled Dialog and Command. */
export function ProjectPicker({
  recent,
  onPick,
  onForget,
  onClose,
}: {
  recent: readonly RecentView[];
  onPick: (path: string) => void;
  onForget: (path: string) => void;
  onClose: () => void;
}) {
  const [query, setQuery] = useState("");
  const items = fuzzy(query, recent);
  const [selected, setSelected] = useState(items[0]?.path ?? "");
  const current = items.some((r) => r.path === selected) ? selected : (items[0]?.path ?? "");
  const move = (by: number) => {
    const i = items.findIndex((r) => r.path === current);
    const next = items[Math.min(items.length - 1, Math.max(0, i + by))];
    if (next) setSelected(next.path);
  };
  const onKeyDown = (e: KeyboardEvent<HTMLDivElement>) => {
    const vim = !query && !e.ctrlKey && !e.altKey && !e.metaKey;
    if ((vim && e.key === "j") || (e.ctrlKey && e.key === "n")) move(1);
    else if ((vim && e.key === "k") || (e.ctrlKey && e.key === "p")) move(-1);
    else if (e.key === "Delete" && current) onForget(current);
    else return;
    e.preventDefault();
  };
  return (
    <Dialog open onOpenChange={(open) => !open && onClose()}>
      <DialogContent title="projects" aria-describedby={undefined}>
        <Command shouldFilter={false} value={current} onValueChange={setSelected} onKeyDown={onKeyDown} loop>
          <CommandInput value={query} onValueChange={setQuery} placeholder="type to filter" autoFocus />
          <CommandList>
            {items.length === 0 && <CommandEmpty>no project matches</CommandEmpty>}
            {items.map((r) => (
              <CommandItem
                key={r.path}
                value={r.path}
                onSelect={() => onPick(r.path)}
                className={r.exists === false ? "text-dim" : undefined}
              >
                <span>{r.name}</span>
                <CommandShortcut>
                  {r.path}
                  {r.exists === false ? "  (missing)" : ""}
                </CommandShortcut>
              </CommandItem>
            ))}
          </CommandList>
        </Command>
        <div className="flex h-cell justify-between px-1 text-dim">
          <span>Enter open · Del forget · Esc close</span>
          <span>
            {items.length}/{recent.length}
          </span>
        </div>
      </DialogContent>
    </Dialog>
  );
}

/** The menu (D-034): every action in plain words with its shortcut; click or Enter runs it.
 *  The only place a newcomer needs to know. */
export function MenuOverlay({
  entries,
  initialQuery = "",
  title = "menu",
  onRun,
  onClose,
}: {
  entries: readonly MenuEntry[];
  initialQuery?: string;
  title?: string;
  onRun: (call: ActionCall) => void;
  onClose: () => void;
}) {
  return (
    <Dialog open onOpenChange={(open) => !open && onClose()}>
      <DialogContent title={title} aria-describedby={undefined}>
        <Command loop>
          <CommandInput placeholder="type to find an action" defaultValue={initialQuery} autoFocus />
          <CommandList>
            <CommandEmpty>no action matches</CommandEmpty>
            {entries.map((e) => (
              <CommandItem
                key={`${e.call.name} ${e.call.arg ?? ""}`}
                value={`${e.label} ${e.shortcut}`}
                onSelect={() => onRun(e.call)}
              >
                <span>{e.label}</span>
                <CommandShortcut>{e.shortcut}</CommandShortcut>
              </CommandItem>
            ))}
          </CommandList>
        </Command>
      </DialogContent>
    </Dialog>
  );
}

export function PasteConfirm({ text, onPaste, onClose }: { text: string; onPaste: () => void; onClose: () => void }) {
  const n = pasteLines(text);
  const preview = text.split(/\r\n|\r|\n/).slice(0, 6);
  return (
    <Dialog open onOpenChange={(open) => !open && onClose()}>
      <DialogContent
        title={`Paste ${n} lines?`}
        aria-describedby={undefined}
        onKeyDown={(e) => {
          if (e.key === "y") onPaste();
          else if (e.key === "n") onClose();
        }}
      >
        <div className="px-1 whitespace-pre text-dim">
          {preview.join("\n")}
          {n > preview.length ? `\n… ${n - preview.length} more` : ""}
        </div>
        <div className="flex justify-end gap-2 px-1">
          <Button autoFocus onClick={onClose}>
            [ Cancel ]
          </Button>
          <Button variant="primary" onClick={onPaste}>
            [ Paste {n} lines ]
          </Button>
        </div>
      </DialogContent>
    </Dialog>
  );
}

/** The first-run choice (D-034): two large choices, each with a three-line preview. */
export function FirstRun({ onChoose }: { onChoose: (style: "simple" | "modal") => void }) {
  const choice = (style: "simple" | "modal", title: string, lines: string[], autoFocus: boolean) => (
    <button
      type="button"
      autoFocus={autoFocus}
      onClick={() => onChoose(style)}
      className="flex flex-1 cursor-pointer flex-col items-start border border-border bg-bar-bg px-1 text-left font-mono text-bar-fg outline-none hover:border-accent focus-visible:border-accent focus-visible:text-accent"
    >
      <span className="font-bold">{title}</span>
      {lines.map((l) => (
        <span key={l} className="text-dim">
          {l}
        </span>
      ))}
    </button>
  );
  return (
    <Dialog open onOpenChange={() => {}}>
      <DialogContent title="How do you like to work?" aria-describedby={undefined} onEscapeKeyDown={(e) => e.preventDefault()}>
        <div className="flex gap-2 px-1">
          {choice(
            "simple",
            "Simple — click and shortcuts",
            ["Keys always go to the terminal.", "Ctrl+Shift+P opens the menu.", "Everything is a button."],
            true,
          )}
          {choice(
            "modal",
            "Modal — like vim",
            ["Ctrl+\\ Ctrl+n: leave the terminal.", "Space: the leader keys.", ": the command line."],
            false,
          )}
        </div>
        <div className="mt-[var(--cell-h)] px-1 pb-[calc(var(--cell-h)/2)] text-dim">
          You can change this any time: menu → Input style.
        </div>
      </DialogContent>
    </Dialog>
  );
}
