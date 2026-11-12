// Entry point: freeze Object.prototype, read config.toml through the core, mount the app.

import "@xterm/xterm/css/xterm.css";
import "./style.css";
import { createRoot } from "react-dom/client";
import { TooltipProvider } from "../components/tooltip.tsx";
import { App } from "./app.tsx";
import { DEFAULT_CONFIG, type ConfigView } from "./config.ts";
import { hardenPrototypes } from "./harden.ts";
import { configGet, uiGet } from "./ipc.ts";

// Every import above has run; nothing untrusted has been handled yet.
hardenPrototypes();

// The DIR `rein-desktop DIR` was started with, set by the core before this script ran.
const startDir = (globalThis as { __REIN_START_DIR__?: string | null }).__REIN_START_DIR__ ?? null;

const initial: ConfigView = await configGet().catch((e: { message?: string }) => ({
  config: DEFAULT_CONFIG,
  path: null,
  error: `config: ${e.message ?? String(e)}`,
}));

const initialUi = await uiGet().catch(() => null);

const mount = document.getElementById("root");
if (mount) {
  createRoot(mount).render(
    <TooltipProvider>
      <App initial={initial} initialUi={initialUi} startDir={startDir} />
    </TooltipProvider>,
  );
}
