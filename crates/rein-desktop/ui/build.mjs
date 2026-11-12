// Bundle the UI into dist/: app.js (one ES module: React, the shell, xterm.js), app.css (our
// styles and xterm's), tw.css (Tailwind, compiled here at build time — D-037), index.html.
// No inline script or style, so the CSP `default-src 'self'` holds (desktop.md §Security 1).
import { build } from "esbuild";
import { execFileSync } from "node:child_process";
import { copyFileSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { generate } from "./gen-actions.mjs";

export const ESBUILD = {
  entryPoints: { app: "src/main.tsx", boot: "src/boot.ts" },
  bundle: true,
  format: "esm",
  target: "es2022",
  jsx: "automatic",
  legalComments: "none",
  minify: true,
  define: { "process.env.NODE_ENV": '"production"' },
  logLevel: "warning",
};

/** The Tailwind CLI's script, run by this Node (no shell, no `.cmd` shim on Windows). The
 *  package exports no `package.json`, so its `bin` entry is read from disk. */
export function tailwindCli() {
  const dir = fileURLToPath(new URL("node_modules/@tailwindcss/cli/", import.meta.url));
  const pkg = JSON.parse(readFileSync(join(dir, "package.json"), "utf8"));
  const bin = typeof pkg.bin === "string" ? pkg.bin : pkg.bin.tailwindcss;
  return join(dir, bin);
}

export function buildTailwind(out) {
  execFileSync(process.execPath, [tailwindCli(), "-i", "src/tailwind.css", "-o", out, "--minify"], {
    stdio: ["ignore", "ignore", "inherit"],
  });
}

if (import.meta.url === `file://${process.argv[1]}` || process.argv[1]?.endsWith("build.mjs")) {
  // The Action list from ../actions.json (the source the Rust side reads too).
  writeFileSync("src/actions.ts", generate());
  rmSync("dist", { recursive: true, force: true });
  mkdirSync("dist");
  await build({ ...ESBUILD, outdir: "dist" });
  buildTailwind("dist/tw.css");
  copyFileSync("index.html", "dist/index.html");
}
