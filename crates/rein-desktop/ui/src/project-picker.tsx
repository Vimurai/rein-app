// The Open project dialog (desktop.md V13, D-067; the E-38 picker grown): the recent projects,
// each with when it was last opened and an open / missing mark, then always Choose a folder….
// `+`, `<leader> p`, `:projects`, the start screen's `p` and the menu row open it. The rows come
// from `pickRows` (start.ts); a folder still becomes a project only through `tab_open`.

import { useMemo, useRef, useState, type KeyboardEvent } from "react";
import { Command, CommandInput, CommandItem, CommandList } from "../components/command.tsx";
import { Dialog, DialogContent } from "../components/dialog.tsx";
import { Skeleton } from "../components/loaders.tsx";
import { TruncStart } from "./frame.tsx";
import { Folder, FolderOpen } from "./icons.ts";
import { PaletteHints } from "./overlays.tsx";
import { missingText, pickEmptyText, pickRows, type RecentView } from "./start.ts";

/** The value of the Choose a folder… row: no path starts with a control character. */
const CHOOSE = "\u0001choose";

const DIM = "text-muted-foreground group-data-[selected=true]:text-popover-foreground";
const CHIP = "shrink-0 rounded-sm border border-border px-1.5 text-[11px] leading-[16px] " + DIM;

export function ProjectPicker({
  recent,
  loading,
  openPaths,
  onPick,
  onChoose,
  onForget,
  onClose,
}: {
  recent: readonly RecentView[];
  /** The first list has not arrived yet: skeleton rows above Choose a folder…. */
  loading: boolean;
  /** The projects a tab has: their rows carry the open mark. */
  openPaths: readonly string[];
  onPick: (path: string) => void;
  /** The dialog closes, then the system folder dialog opens. */
  onChoose: () => void;
  onForget: (path: string) => void;
  onClose: () => void;
}) {
  const [query, setQuery] = useState("");
  const [status, setStatus] = useState("");
  /** A click on a row takes the focus; typing after it still filters. */
  const input = useRef<HTMLInputElement>(null);
  // "ago" is computed once, when the dialog opens (V13: no timer while open).
  const now = useMemo(() => Date.now() / 1000, []);
  const rows = pickRows(recent, openPaths, query, now);
  const values = rows.map((r) => (r.kind === "project" ? r.path : CHOOSE));
  const [selected, setSelected] = useState(values[0] ?? CHOOSE);
  const current = values.includes(selected) ? selected : (values[0] ?? CHOOSE);
  const run = (value: string) => {
    const row = rows.find((r) => r.kind === "project" && r.path === value);
    if (!row || row.kind !== "project") return onChoose();
    if (row.missing) {
      setStatus(missingText(row.name, row.path));
      return input.current?.focus();
    }
    onPick(row.path);
  };
  const move = (by: number) => {
    const i = values.indexOf(current);
    const next = values[Math.min(values.length - 1, Math.max(0, i + by))];
    if (next) setSelected(next);
  };
  const onKeyDown = (e: KeyboardEvent<HTMLDivElement>) => {
    const plain = !query && !e.ctrlKey && !e.altKey && !e.metaKey;
    if ((plain && e.key === "j") || (e.ctrlKey && e.key === "n")) move(1);
    else if ((plain && e.key === "k") || (e.ctrlKey && e.key === "p")) move(-1);
    else if (plain && e.key === "o") onChoose();
    else if (e.key === "Delete" && current !== CHOOSE) {
      setStatus("");
      onForget(current);
    } else return;
    e.preventDefault();
  };
  const projects = rows.length - 1;
  return (
    <Dialog open onOpenChange={(open) => !open && onClose()}>
      <DialogContent title="Open project" bare aria-describedby={undefined} className="w-[min(640px,calc(100vw-32px))]">
        <Command shouldFilter={false} value={current} onValueChange={setSelected} onKeyDown={onKeyDown} loop>
          <CommandInput ref={input} value={query} onValueChange={setQuery} placeholder="Find a recent project" autoFocus />
          <CommandList className="max-h-[min(380px,60vh)]">
            {loading && !recent.length
              ? [0, 1, 2].map((i) => (
                  <div key={i} aria-hidden="true" className="flex items-center gap-3 px-3 py-2">
                    <Skeleton className="size-4" />
                    <Skeleton className="h-3.5 w-28" />
                    <Skeleton className="h-3 flex-1" />
                  </div>
                ))
              : projects === 0 && <div className="px-3 py-2 text-[13px] text-muted-foreground">{pickEmptyText(recent)}</div>}
            {rows.map((r) =>
              r.kind === "project" ? (
                <CommandItem key={r.path} value={r.path} onSelect={run} className={r.missing ? "opacity-60" : undefined}>
                  <Folder size={15} aria-hidden="true" className={`shrink-0 ${DIM}`} />
                  <span className="shrink-0 font-medium">{r.name}</span>
                  {r.open && <span className={CHIP}>open</span>}
                  {r.missing && <span className={CHIP}>missing</span>}
                  <span className={`shrink-0 text-[12px] ${DIM}`}>{r.ago}</span>
                  <TruncStart text={r.path} className={`ml-auto pl-4 font-mono text-[11px] ${DIM}`} />
                </CommandItem>
              ) : (
                <CommandItem key={CHOOSE} value={CHOOSE} onSelect={run} className="border-t border-border">
                  <FolderOpen size={15} aria-hidden="true" className={`shrink-0 ${DIM}`} />
                  <span className="font-medium">Choose a folder…</span>
                </CommandItem>
              ),
            )}
          </CommandList>
        </Command>
        {status && (
          <div role="status" className="border-t border-border px-3 py-2 text-[12px] [overflow-wrap:anywhere] text-foreground">
            {status}
          </div>
        )}
        <PaletteHints
          hints={[
            ["Enter", "open"],
            ["o", "choose a folder"],
            ["Del", "forget"],
            ["Esc", "close"],
          ]}
          count={`${projects} of ${recent.length}`}
        />
      </DialogContent>
    </Dialog>
  );
}
