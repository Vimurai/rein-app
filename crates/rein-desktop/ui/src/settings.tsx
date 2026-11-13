// Settings (desktop.md "Two ways in", D-034; E-41): input style, theme (live preview), font
// size, title bar, and the config file's location. Choices are saved in the app's own
// ui.json; a row config.toml sets is shown as "set in config.toml" and is read-only — the
// app never writes config.toml.

import type { ReactNode } from "react";
import { Button } from "../components/button.tsx";
import { Dialog, DialogContent, DialogFooter } from "../components/dialog.tsx";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "../components/select.tsx";
import { Switch } from "../components/switch.tsx";
import { FolderOpen, Minus, Plus, RotateCcw } from "./icons.ts";
import type { UiView } from "./ipc.ts";
import { THEME_NAMES } from "./theme.ts";

export const FONT_MIN = 6;
export const FONT_MAX = 72;

/** One setting: its name (and, when config.toml sets it, a line saying so) at the left, the
 *  control at the right. */
function Row({ label, fromConfig, children }: { label: string; fromConfig: boolean; children: ReactNode }) {
  return (
    <div className="flex min-h-12 items-center justify-between gap-4 border-b border-border/50 py-2 last:border-b-0">
      <span className="grid gap-0.5">
        <span className="font-medium">{label}</span>
        {fromConfig && <span className="text-[12px] text-muted-foreground">Set in config.toml</span>}
      </span>
      <span className="flex items-center gap-2">{children}</span>
    </div>
  );
}

export function SettingsOverlay({
  ui,
  onSet,
  onOpenConfig,
  onClose,
  themeInUse,
  note,
}: {
  ui: UiView;
  /** One ui.json field; `null` clears it (the default applies again). */
  onSet: (
    field: "input_style" | "theme" | "font_size" | "decorations" | "tmux_server" | "close_when_agents_exit",
    value: unknown,
  ) => void;
  onOpenConfig: () => void;
  onClose: () => void;
  /** The theme on screen: a session's `:theme NAME` wins over the saved one until changed here. */
  themeInUse: string;
  /** The config path once shown, or a refused change; drawn inside the overlay. */
  note: { text: string; error: boolean } | null;
}) {
  const ro = (k: keyof Omit<UiView, "first_run_done" | "update_dont_ask" | "close_dont_ask">) => ui[k].source === "config";
  const size = ui.font_size.value;
  return (
    <Dialog open onOpenChange={(open) => !open && onClose()}>
      <DialogContent title="Settings" aria-describedby={undefined}>
        <div className="-my-2">
          <Row label="Input style" fromConfig={false}>
            <Select value={ui.input_style.value} onValueChange={(v) => onSet("input_style", v)}>
              <SelectTrigger aria-label="Input style">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="simple">Simple — shortcuts, no modes</SelectItem>
                <SelectItem value="modal">Modal — vim-style modes</SelectItem>
              </SelectContent>
            </Select>
          </Row>
          <Row label="Theme" fromConfig={ro("theme")}>
            <Select value={themeInUse} onValueChange={(v) => onSet("theme", v)} disabled={ro("theme")}>
              <SelectTrigger aria-label="Theme">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                {THEME_NAMES.map((t) => (
                  <SelectItem key={t} value={t}>
                    {t}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </Row>
          <Row label="Font size" fromConfig={ro("font_size")}>
            <Button
              size="icon"
              aria-label="Smaller font"
              disabled={ro("font_size") || size <= FONT_MIN}
              onClick={() => onSet("font_size", size - 1)}
            >
              <Minus size={14} aria-hidden="true" />
            </Button>
            <span className="w-8 text-center tabular-nums">{size}</span>
            <Button
              size="icon"
              aria-label="Bigger font"
              disabled={ro("font_size") || size >= FONT_MAX}
              onClick={() => onSet("font_size", size + 1)}
            >
              <Plus size={14} aria-hidden="true" />
            </Button>
            <Button variant="ghost" size="icon" aria-label="Reset font size" disabled={ro("font_size")} onClick={() => onSet("font_size", null)}>
              <RotateCcw size={14} aria-hidden="true" />
            </Button>
          </Row>
          <Row label="tmux server" fromConfig={ro("tmux_server")}>
            <Select value={ui.tmux_server.value} onValueChange={(v) => onSet("tmux_server", v)} disabled={ro("tmux_server")}>
              <SelectTrigger aria-label="tmux server">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="app">rein-app's own — new tabs</SelectItem>
                <SelectItem value="user">My own — new tabs</SelectItem>
              </SelectContent>
            </Select>
          </Row>
          <Row label="Close the project when its agents exit" fromConfig={ro("close_when_agents_exit")}>
            <Switch
              aria-label="Close the project when its agents exit"
              checked={ui.close_when_agents_exit.value}
              disabled={ro("close_when_agents_exit")}
              onCheckedChange={(on) => onSet("close_when_agents_exit", on)}
            />
          </Row>
          <Row label="Title bar" fromConfig={ro("decorations")}>
            <Switch
              aria-label="Title bar"
              checked={ui.decorations.value}
              disabled={ro("decorations")}
              onCheckedChange={(on) => onSet("decorations", on)}
            />
          </Row>
        </div>
        {note && (
          <div
            role="status"
            className={`rounded-md border bg-background p-3 font-mono text-[12px] [overflow-wrap:anywhere] whitespace-pre-wrap text-foreground ${note.error ? "border-destructive" : "border-border"}`}
          >
            {note.text}
          </div>
        )}
        <DialogFooter className="justify-between">
          <Button onClick={onOpenConfig}>
            <FolderOpen size={14} aria-hidden="true" />
            Open config file location
          </Button>
          <Button variant="primary" autoFocus onClick={onClose}>
            Close
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
