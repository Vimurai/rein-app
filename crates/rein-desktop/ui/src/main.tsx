// Entry point (E-42): apply the theme, freeze Object.prototype, mount the React shell.

import "@xterm/xterm/css/xterm.css";
import "./style.css";
import { createRoot } from "react-dom/client";
import { TooltipProvider } from "../components/tooltip.tsx";
import { hardenPrototypes } from "./harden.ts";
import { Shell } from "./shell.tsx";
import { GRUVBOX_DARK, cssVariables } from "./theme.ts";

// Windows has neither JetBrains Mono nor DejaVu by default (desktop.md §Windows window W6).
const FONT_FAMILY = navigator.userAgent.includes("Windows")
  ? `"Cascadia Mono", Consolas, monospace`
  : `"JetBrains Mono", "DejaVu Sans Mono", Menlo, monospace`;
const FONT_SIZE_PX = 14;
const LINE_HEIGHT = 1.25;

// Every import above has run; nothing untrusted has been handled yet.
hardenPrototypes();

const root = document.documentElement.style;
for (const [k, v] of Object.entries(cssVariables(GRUVBOX_DARK))) root.setProperty(k, v);
root.setProperty("--font-family", FONT_FAMILY);
root.setProperty("--font-size", `${FONT_SIZE_PX}px`);
root.setProperty("--line-height", String(LINE_HEIGHT));

// The DIR `rein-desktop DIR` was started with, set by the core before this script ran.
const startDir = (globalThis as { __REIN_START_DIR__?: string | null }).__REIN_START_DIR__ ?? null;

const mount = document.getElementById("root");
if (mount) {
  createRoot(mount).render(
    <TooltipProvider>
      <Shell
        theme={GRUVBOX_DARK}
        fontFamily={FONT_FAMILY}
        fontSize={FONT_SIZE_PX}
        lineHeight={LINE_HEIGHT}
        startDir={startDir}
      />
    </TooltipProvider>,
  );
}
