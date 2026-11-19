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
import { EVENTS, EVENT_LABELS, withEvent } from "./notify.ts";
import { DESCRIPTORS } from "./panels.ts";
import { THEME_NAMES } from "./theme.ts";

export const FONT_MIN = 6;
export const FONT_MAX = 72;
/** `[notifications] stuck_after_min` (status.md §Data Model). */
export const STUCK_MIN = 3;
export const STUCK_MAX = 120;

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
    field:
      | "input_style"
      | "theme"
      | "font_size"
      | "decorations"
      | "tmux_server"
      | "close_when_agents_exit"
      | "panels_rail"
      | "panels_enabled"
      | "tabs_status"
      | "notifications_enabled"
      | "notifications_system"
      | "notifications_events"
      | "notifications_stuck_after_min",
    value: unknown,
  ) => void;
  onOpenConfig: () => void;
  onClose: () => void;
  /** The theme on screen: a session's `:theme NAME` wins over the saved one until changed here. */
  themeInUse: string;
  /** The config path once shown, or a refused change; drawn inside the overlay. */
  note: { text: string; error: boolean } | null;
}) {
  const ro = (k: keyof Omit<UiView, "first_run_done" | "update_dont_ask" | "close_dont_ask" | "panel_width" | "panel_open">) =>
    ui[k].source === "config";
  const enabled = ui.panels_enabled.value;
  const size = ui.font_size.value;
  const stuck = ui.notifications_stuck_after_min.value;
  return (
    <Dialog open onOpenChange={(open) => !open && onClose()}>
      <DialogContent title="Settings" aria-describedby={undefined}>
        <div className="-my-2 min-h-0 overflow-y-auto pr-3">
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
          {/* status.md §3 (E-77): the status mark on each tab and in the status bar. */}
          <Row label="Project status on tabs" fromConfig={ro("tabs_status")}>
            <Switch
              aria-label="Project status on tabs"
              checked={ui.tabs_status.value === "shown"}
              disabled={ro("tabs_status")}
              onCheckedChange={(on) => onSet("tabs_status", on ? "shown" : "hidden")}
            />
          </Row>
          {/* status.md §6 and §Data Model (E-78): notifications, one switch per event, and when
              a project counts as stuck. `system` does nothing until the system-notification task. */}
          <Row label="Notifications" fromConfig={ro("notifications_enabled")}>
            <Switch
              aria-label="Notifications"
              checked={ui.notifications_enabled.value}
              disabled={ro("notifications_enabled")}
              onCheckedChange={(on) => onSet("notifications_enabled", on)}
            />
          </Row>
          {EVENTS.map((e) => (
            <Row key={e} label={`Notify: ${EVENT_LABELS[e]}`} fromConfig={ro("notifications_events")}>
              <Switch
                aria-label={`Notify: ${EVENT_LABELS[e]}`}
                checked={ui.notifications_events.value.includes(e)}
                disabled={ro("notifications_events") || !ui.notifications_enabled.value}
                onCheckedChange={(on) => onSet("notifications_events", withEvent(ui.notifications_events.value, e, on))}
              />
            </Row>
          ))}
          <Row label="Stuck after (minutes)" fromConfig={ro("notifications_stuck_after_min")}>
            <Button
              size="icon"
              aria-label="Stuck sooner"
              disabled={ro("notifications_stuck_after_min") || stuck <= STUCK_MIN}
              onClick={() => onSet("notifications_stuck_after_min", stuck - 1)}
            >
              <Minus size={14} aria-hidden="true" />
            </Button>
            <span className="w-8 text-center tabular-nums">{stuck}</span>
            <Button
              size="icon"
              aria-label="Stuck later"
              disabled={ro("notifications_stuck_after_min") || stuck >= STUCK_MAX}
              onClick={() => onSet("notifications_stuck_after_min", stuck + 1)}
            >
              <Plus size={14} aria-hidden="true" />
            </Button>
          </Row>
          <Row label="System notifications" fromConfig={ro("notifications_system")}>
            <Switch
              aria-label="System notifications"
              checked={ui.notifications_system.value}
              disabled={ro("notifications_system")}
              onCheckedChange={(on) => onSet("notifications_system", on)}
            />
          </Row>
          {/* panels.md §Data Model (E-60): the rail, and one switch per panel. */}
          <Row label="Side menu" fromConfig={ro("panels_rail")}>
            <Switch
              aria-label="Side menu"
              checked={ui.panels_rail.value === "shown"}
              disabled={ro("panels_rail")}
              onCheckedChange={(on) => onSet("panels_rail", on ? "shown" : "hidden")}
            />
          </Row>
          {DESCRIPTORS.map((d) => (
            <Row key={d.id} label={`Panel: ${d.title}`} fromConfig={ro("panels_enabled")}>
              <Switch
                aria-label={`Panel: ${d.title}`}
                checked={enabled.includes(d.id)}
                disabled={ro("panels_enabled")}
                onCheckedChange={(on) =>
                  onSet(
                    "panels_enabled",
                    DESCRIPTORS.map((x) => x.id).filter((id) => (id === d.id ? on : enabled.includes(id))),
                  )
                }
              />
            </Row>
          ))}
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
