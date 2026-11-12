// Settings (desktop.md "Two ways in", D-034; E-41): input style, theme (live preview), font
// size, title bar, and the config file's location. Choices are saved in the app's own
// ui.json; a row config.toml sets is shown as "set in config.toml" and is read-only — the
// app never writes config.toml.

import type { ReactNode } from "react";
import { Button } from "../components/button.tsx";
import { Dialog, DialogContent } from "../components/dialog.tsx";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "../components/select.tsx";
import { Switch } from "../components/switch.tsx";
import type { UiView } from "./ipc.ts";
import { THEME_NAMES } from "./theme.ts";

export const FONT_MIN = 6;
export const FONT_MAX = 72;

function Row({ label, fromConfig, children }: { label: string; fromConfig: boolean; children: ReactNode }) {
  return (
    <div className="flex items-center justify-between gap-2 px-1">
      <span>{label}</span>
      <span className="flex items-center gap-1">
        {children}
        {fromConfig && <span className="text-dim">set in config.toml</span>}
      </span>
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
  onSet: (field: "input_style" | "theme" | "font_size" | "decorations", value: unknown) => void;
  onOpenConfig: () => void;
  onClose: () => void;
  /** The theme on screen: a session's `:theme NAME` wins over the saved one until changed here. */
  themeInUse: string;
  /** The config path once shown, or a refused change; drawn inside the overlay. */
  note: { text: string; error: boolean } | null;
}) {
  const ro = (k: keyof Omit<UiView, "first_run_done">) => ui[k].source === "config";
  const size = ui.font_size.value;
  return (
    <Dialog open onOpenChange={(open) => !open && onClose()}>
      <DialogContent title="Settings" aria-describedby={undefined}>
        <Row label="Input style" fromConfig={false}>
          <Select value={ui.input_style.value} onValueChange={(v) => onSet("input_style", v)}>
            <SelectTrigger aria-label="Input style">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="simple">simple (shortcuts, no modes)</SelectItem>
              <SelectItem value="modal">modal (vim-style modes)</SelectItem>
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
            aria-label="Smaller font"
            disabled={ro("font_size") || size <= FONT_MIN}
            onClick={() => onSet("font_size", size - 1)}
          >
            [ - ]
          </Button>
          <span className="w-4 text-center">{size}</span>
          <Button
            aria-label="Bigger font"
            disabled={ro("font_size") || size >= FONT_MAX}
            onClick={() => onSet("font_size", size + 1)}
          >
            [ + ]
          </Button>
          <Button aria-label="Reset font size" disabled={ro("font_size")} onClick={() => onSet("font_size", null)}>
            [ reset ]
          </Button>
        </Row>
        <Row label="Title bar" fromConfig={ro("decorations")}>
          <Switch
            aria-label="Title bar"
            checked={ui.decorations.value}
            disabled={ro("decorations")}
            onCheckedChange={(on) => onSet("decorations", on)}
          />
        </Row>
        <div className="flex justify-between gap-2 px-1">
          <Button onClick={onOpenConfig}>[ Open config file location ]</Button>
          <Button variant="primary" autoFocus onClick={onClose}>
            [ Close ]
          </Button>
        </div>
        {note && (
          <div role="status" className={`px-1 [overflow-wrap:anywhere] whitespace-pre-wrap ${note.error ? "text-error" : "text-dim"}`}>
            {note.text}
          </div>
        )}
      </DialogContent>
    </Dialog>
  );
}
