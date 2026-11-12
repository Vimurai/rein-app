// The app around the shell (E-37): holds the active config.toml view, follows saves of it
// (`config_changed`), and owns the config actions — `:config` (show and copy the file's path;
// the app starts no editor), `config.reload`, `:theme NAME` (this session; config.toml is
// never written).

import { useEffect, useMemo, useState } from "react";
import { fontFamily, keyTable, type ConfigView } from "./config.ts";
import { clipboardWrite, configGet, listen, uiGet, uiSet, type UiView } from "./ipc.ts";
import { FirstRun } from "./overlays.tsx";
import { Shell, type Handlers } from "./shell.tsx";
import { THEMES, resolveTheme } from "./theme.ts";

const WINDOWS = navigator.userAgent.includes("Windows");
const LINE_HEIGHT = 1.25;

export function App({
  initial,
  initialUi,
  startDir,
}: {
  initial: ConfigView;
  /** ui.json as the core sees it (`null` outside the window, e.g. a test page). */
  initialUi: UiView | null;
  startDir: string | null;
}) {
  const [view, setView] = useState(initial);
  const [ui, setUi] = useState(initialUi);
  /** `:theme NAME` for this session; a save of config.toml takes over again. */
  const [themeName, setThemeName] = useState<string | null>(null);
  const [notice, setNotice] = useState({ text: initial.error ?? "", error: initial.error !== null, seq: 0 });

  useEffect(() => {
    void listen<ConfigView>("config_changed", (v) => {
      setView(v);
      setThemeName(null);
      setNotice((n) => ({ text: v.error ?? "config.toml reloaded", error: v.error !== null, seq: n.seq + 1 }));
    });
  }, []);

  const cfg = view.config;
  // Effective settings (D-034): config.toml if it sets them, else ui.json, else the default.
  const baseTheme = ui?.theme.value ?? cfg.theme.name;
  const theme = useMemo(
    () => resolveTheme(themeName ?? baseTheme, cfg.theme.custom),
    [themeName, baseTheme, cfg.theme.custom],
  );
  const style = ui?.input_style.value ?? "simple";
  const keys = useMemo(() => keyTable(cfg, style), [cfg, style]);
  const fontSize = ui?.font_size.value ?? cfg.font.size;

  const chooseStyle = (s: "simple" | "modal", first: boolean) =>
    uiSet("input_style", s)
      .then(() => (first ? uiSet("first_run_done", true) : uiGet()))
      .then(setUi);

  const handlers: Handlers = {
    "config.open": (_call, ui) => {
      const path = view.path;
      if (!path) {
        ui.say("no config directory — set HOME (XDG_CONFIG_HOME) or %APPDATA%", true);
        return;
      }
      const note = "the config file — edit it in your editor; saving it reloads the window";
      clipboardWrite(path).then(
        () => ui.showCommand(path, `${note} (path copied)`),
        () => ui.showCommand(path, `${note} (could not copy the path; y tries again)`),
      );
    },
    "config.reload": (_call, ui) => {
      configGet(true).then(
        (v) => {
          setView(v);
          setThemeName(null);
          ui.say(v.error ?? "config.toml reloaded", v.error !== null);
        },
        (e: { message?: string }) => ui.say(`E: ${e.message ?? String(e)}`, true),
      );
    },
    "input.style": (call, cmd) => {
      const next = call.arg === "simple" || call.arg === "modal" ? call.arg : style === "simple" ? "modal" : "simple";
      chooseStyle(next, false).then(
        () => cmd.say(`input style: ${next}${next === "modal" ? " — Ctrl+\\ Ctrl+n leaves the terminal" : ""}`),
        (e: { message?: string }) => cmd.say(`E: ${e.message ?? String(e)}`, true),
      );
    },
    "theme.set": (call, ui) => {
      const name = call.arg ?? "";
      if (!THEMES.has(name)) {
        ui.say(`E: unknown theme ${name} — ${[...THEMES.keys()].join(", ")}`, true);
        return;
      }
      setThemeName(name);
      ui.say(`theme ${name} for this session — set [theme] name in config.toml to keep it`);
    },
  };

  const shell = (
    <Shell
      theme={theme}
      keys={keys}
      fontFamily={fontFamily(cfg, WINDOWS)}
      fontSize={fontSize}
      lineHeight={LINE_HEIGHT}
      opacity={cfg.window.opacity}
      handlers={handlers}
      notice={notice}
      startDir={startDir}
      startLast={initial.config.window.start === "last"}
    />
  );
  return ui && !ui.first_run_done ? (
    <>
      {shell}
      <FirstRun onChoose={(s) => void chooseStyle(s, true)} />
    </>
  ) : (
    shell
  );
}
