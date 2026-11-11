// Bundle the UI into dist/: app.js (one ES module), app.css, index.html. No inline script
// or style, so the CSP `default-src 'self'` holds (desktop.md §Security 1).
import { build } from "esbuild";
import { copyFileSync, mkdirSync, rmSync } from "node:fs";

rmSync("dist", { recursive: true, force: true });
mkdirSync("dist");
await build({
  entryPoints: ["src/main.ts"],
  bundle: true,
  format: "esm",
  target: "es2022",
  outfile: "dist/app.js",
  legalComments: "none",
  logLevel: "warning",
});
copyFileSync("index.html", "dist/index.html");
copyFileSync("src/style.css", "dist/app.css");
