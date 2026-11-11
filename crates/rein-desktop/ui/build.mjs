// Bundle the UI into dist/: app.js (one ES module), app.css (our styles and xterm's, imported
// by main.ts), index.html. No inline script or style, so the CSP `default-src 'self'` holds
// (desktop.md §Security 1).
import { build } from "esbuild";
import { copyFileSync, mkdirSync, rmSync } from "node:fs";

rmSync("dist", { recursive: true, force: true });
mkdirSync("dist");
await build({
  entryPoints: { app: "src/main.ts", boot: "src/boot.ts" },
  bundle: true,
  format: "esm",
  target: "es2022",
  outdir: "dist",
  legalComments: "none",
  minify: true,
  logLevel: "warning",
});
copyFileSync("index.html", "dist/index.html");
