// The window's overlays in the modern look (desktop.md §Look and feel v2 V2, E-50): which-key,
// the project picker, the menu, the paste confirm and the first-run choice — shadcn Dialog and
// Command driven only by the tokens. What each one shows and does is unchanged from E-36..E-41.

import { useState, type KeyboardEvent } from "react";
import { Command, CommandEmpty, CommandInput, CommandItem, CommandList, CommandShortcut } from "../components/command.tsx";
import { Button, Kbd } from "../components/button.tsx";
import { Dialog, DialogContent, DialogFooter } from "../components/dialog.tsx";
import { ACTIONS } from "./actions.ts";
import { TruncStart } from "./frame.tsx";
import { Folder, Keyboard, MousePointerClick } from "./icons.ts";
import { fuzzy, type RecentView } from "./start.ts";
import type { ActionCall } from "./keymap.ts";
import type { MenuEntry } from "./menu.ts";
import { pasteLines } from "./paste.ts";

const LABELS = new Map<string, string>(ACTIONS.map((a) => [a.name, a.label]));

export function label(call: ActionCall): string {
  return LABELS.get(call.name) ?? call.name;
}

/** The keys that can follow the leader, each with its action in plain words (E-36): a card
 *  just above the status bar, at the left. */
export function WhichKey({ leader, next }: { leader: string; next: readonly [string, ActionCall][] }) {
  return (
    <div
      role="dialog"
      aria-label={`keys after ${leader}`}
      className="fixed bottom-[38px] left-3 z-40 min-w-64 rounded-lg border border-border bg-popover p-2 font-ui text-[13px] text-popover-foreground shadow-md"
    >
      <div className="flex items-center gap-2 px-1 pb-2 text-[12px] text-muted-foreground">
        <Kbd>{leader}</Kbd>
        <span>then</span>
      </div>
      <ul className="grid gap-1">
        {next.map(([k, a]) => (
          <li key={k} className="flex items-center gap-3 px-1">
            <Kbd className="min-w-7">{k}</Kbd>
            <span>{label(a)}</span>
          </li>
        ))}
      </ul>
    </div>
  );
}

/** The keys a palette answers to, under its list. */
function PaletteHints({ hints, count }: { hints: readonly [string, string][]; count?: string }) {
  return (
    <div className="flex shrink-0 items-center gap-4 border-t border-border px-3 py-2 text-[12px] text-muted-foreground">
      {hints.map(([k, what]) => (
        <span key={k} className="flex items-center gap-1.5">
          <Kbd>{k}</Kbd>
          {what}
        </span>
      ))}
      {count && <span className="ml-auto tabular-nums">{count}</span>}
    </div>
  );
}

/** The project picker (E-38): recent projects, fuzzy filtered, `j` `k` (while the filter is
 *  empty) `C-n` `C-p` ↑ ↓ to move, `Enter` to open, `Del` to forget the selected one, `Esc`
 *  to close. */
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
      <DialogContent title="Projects" bare aria-describedby={undefined} className="w-[min(640px,calc(100vw-32px))]">
        <Command shouldFilter={false} value={current} onValueChange={setSelected} onKeyDown={onKeyDown} loop>
          <CommandInput value={query} onValueChange={setQuery} placeholder="Find a recent project" autoFocus />
          <CommandList>
            {items.length === 0 && <CommandEmpty>No project matches.</CommandEmpty>}
            {items.map((r) => (
              <CommandItem
                key={r.path}
                value={r.path}
                onSelect={() => onPick(r.path)}
                className={r.exists === false ? "opacity-60" : undefined}
              >
                <Folder size={15} aria-hidden="true" className="shrink-0 text-muted-foreground group-data-[selected=true]:text-popover-foreground" />
                <span className="shrink-0 font-medium">{r.name}</span>
                {r.exists === false && <span className="shrink-0 text-[12px] text-muted-foreground group-data-[selected=true]:text-popover-foreground">missing</span>}
                <TruncStart text={r.path} className="ml-auto pl-4 font-mono text-[11px] text-muted-foreground group-data-[selected=true]:text-popover-foreground" />
              </CommandItem>
            ))}
          </CommandList>
        </Command>
        <PaletteHints
          hints={[
            ["Enter", "open"],
            ["Del", "forget"],
            ["Esc", "close"],
          ]}
          count={`${items.length} of ${recent.length}`}
        />
      </DialogContent>
    </Dialog>
  );
}

/** The menu (D-034): every action in plain words with its shortcut; click or Enter runs it.
 *  The only place a newcomer needs to know. */
export function MenuOverlay({
  entries,
  initialQuery = "",
  title = "Menu",
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
      <DialogContent title={title} bare aria-describedby={undefined}>
        <Command loop>
          <CommandInput placeholder="Find an action" defaultValue={initialQuery} autoFocus />
          <CommandList>
            <CommandEmpty>No action matches.</CommandEmpty>
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
        <PaletteHints
          hints={[
            ["Enter", "run"],
            ["Esc", "close"],
          ]}
        />
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
        <pre className="m-0 overflow-hidden rounded-md border border-border bg-background p-3 font-mono text-[12px] whitespace-pre text-foreground">
          {preview.join("\n")}
          {n > preview.length ? `\n… ${n - preview.length} more` : ""}
        </pre>
        <DialogFooter>
          <Button autoFocus onClick={onClose}>
            Cancel
          </Button>
          <Button variant="primary" onClick={onPaste}>
            Paste {n} lines
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

/** The first-run choice (D-034): two cards, each with a three-line preview. */
export function FirstRun({ onChoose }: { onChoose: (style: "simple" | "modal") => void }) {
  const choice = (style: "simple" | "modal", title: string, lines: string[], autoFocus: boolean) => (
    <button
      type="button"
      autoFocus={autoFocus}
      onClick={() => onChoose(style)}
      className="flex flex-1 cursor-pointer flex-col items-start gap-3 rounded-lg border border-border bg-background p-4 text-left text-foreground outline-none transition-colors duration-150 hover:border-ring focus-visible:ring-2 focus-visible:ring-ring"
    >
      <span className="flex size-8 items-center justify-center rounded-md bg-muted text-foreground">
        {style === "simple" ? <MousePointerClick size={16} aria-hidden="true" /> : <Keyboard size={16} aria-hidden="true" />}
      </span>
      <span className="text-[14px] font-semibold">{title}</span>
      <span className="grid gap-1 text-[12px] text-muted-foreground">
        {lines.map((l) => (
          <span key={l}>{l}</span>
        ))}
      </span>
    </button>
  );
  return (
    <Dialog open onOpenChange={() => {}}>
      <DialogContent
        title="How do you like to work?"
        aria-describedby={undefined}
        onEscapeKeyDown={(e) => e.preventDefault()}
        className="w-[min(640px,calc(100vw-32px))]"
      >
        <div className="flex gap-3">
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
        <p className="m-0 text-[12px] text-muted-foreground">You can change this any time: menu → Input style.</p>
      </DialogContent>
    </Dialog>
  );
}
