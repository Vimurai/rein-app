// The app around the shell (E-37): holds the active config.toml view, follows saves of it
// (`config_changed`), and owns the config actions — `:config` (show and copy the file's path;
// the app starts no editor), `config.reload`, `:theme NAME` (this session; config.toml is
// never written).

import { useEffect, useMemo, useState } from "react";
import { fontFamily, keyTable, type ConfigView } from "./config.ts";
import { clipboardWrite, configGet, listen } from "./ipc.ts";
import { Shell, type Handlers } from "./shell.tsx";
import { THEMES, resolveTheme } from "./theme.ts";

const WINDOWS = navigator.userAgent.includes("Windows");
const LINE_HEIGHT = 1.25;

export function App({ initial, startDir }: { initial: ConfigView; startDir: string | null }) {
  const [view, setView] = useState(initial);
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
  const theme = useMemo(
    () => resolveTheme(themeName ?? cfg.theme.name, cfg.theme.custom),
    [themeName, cfg.theme.name, cfg.theme.custom],
  );
  const keys = useMemo(() => keyTable(cfg), [cfg]);

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

  return (
    <Shell
      theme={theme}
      keys={keys}
      fontFamily={fontFamily(cfg, WINDOWS)}
      fontSize={cfg.font.size}
      lineHeight={LINE_HEIGHT}
      opacity={cfg.window.opacity}
      handlers={handlers}
      notice={notice}
      startDir={startDir}
      startLast={initial.config.window.start === "last"}
    />
  );
}
