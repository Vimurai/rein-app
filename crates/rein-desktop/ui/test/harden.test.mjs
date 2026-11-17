// Prototype hardening (security review of E-35): after hardenPrototypes() a prototype
// pollution attempt throws and changes nothing. Run in a child process so the test runner's
// own Object.prototype stays writable.
import { test } from "node:test";
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { fileURLToPath, pathToFileURL } from "node:url";
import { readFileSync } from "node:fs";
import { join } from "node:path";

const harden = fileURLToPath(new URL("../src/harden.ts", import.meta.url));
// Imported by file:// URL, not by path: on Windows `import("D:\\…")` is refused (E-65 — the
// child failed before hardening ran, on every Windows run).
const hardenUrl = pathToFileURL(harden).href;

test("after hardening, Object.prototype cannot be polluted", () => {
  const script = `
    const { hardenPrototypes } = await import(${JSON.stringify(hardenUrl)});
    hardenPrototypes();
    // The classic gadget: a merge that walks attacker-chosen keys into obj["__proto__"].
    const obj = {};
    let threw = false;
    try { obj["__proto__"]["polluted"] = 1; } catch { threw = true; }
    process.stdout.write(JSON.stringify({ threw, polluted: ({}).polluted ?? null, frozen: Object.isFrozen(Object.prototype) }));
  `;
  const out = execFileSync(process.execPath, ["--input-type=module", "-e", script], { encoding: "utf8" });
  assert.deepEqual(JSON.parse(out), { threw: true, polluted: null, frozen: true });
});

// desktop.md V12 10 (D-061 addendum 6, E-89): the web view's own reload, history and other
// browser keys do nothing outside the terminal; F12 only in a debug build.
test("the browser's own keys are recognised, and nothing the app or the terminal needs", async () => {
  const { isBrowserKey } = await import("../src/harden.ts");
  const k = (key, m = {}) => ({ key, ctrlKey: false, metaKey: false, shiftKey: false, altKey: false, ...m });
  const blocked = [
    k("F5"),
    k("F7"),
    k("F12"),
    k("r", { ctrlKey: true }),
    k("R", { ctrlKey: true, shiftKey: true }),
    k("r", { metaKey: true }),
    k("ArrowLeft", { altKey: true }),
    k("ArrowRight", { altKey: true }),
    k("p", { ctrlKey: true }),
    k("f", { ctrlKey: true }),
    k("g", { ctrlKey: true }),
    k("u", { ctrlKey: true }),
  ];
  for (const e of blocked) assert.equal(isBrowserKey(e, false), true, JSON.stringify(e));
  assert.equal(isBrowserKey(k("F12"), true), false, "the inspector stays in a debug build");
  const kept = [k("c", { ctrlKey: true, shiftKey: true }), k("v", { ctrlKey: true, shiftKey: true }), k("a"), k("Enter"), k("ArrowLeft"), k("1", { altKey: true }), k("Tab", { ctrlKey: true })];
  for (const e of kept) assert.equal(isBrowserKey(e, false), false, JSON.stringify(e));
  // Installed before the app mounts; the context menu is cancelled after the app's own menus.
  const src = readFileSync(join(fileURLToPath(new URL("..", import.meta.url)), "src/main.tsx"), "utf8");
  assert.ok(src.indexOf("blockBrowserUi(") > 0 && src.indexOf("blockBrowserUi(") < src.indexOf("createRoot("));
});
