// Freeze the shared prototypes once the bundled libraries have loaded (desktop.md §Security 1;
// security review of E-35). Tauri's `freezePrototype` would freeze them before any script
// runs, but xterm.js 6.0.0 assigns `toString` on a plain object while its module loads, which
// throws under a frozen `Object.prototype` (the "override mistake"). Freezing here, before any
// pty output, IPC reply or event is handled, keeps untrusted data from polluting a prototype.

export function hardenPrototypes(): void {
  Object.freeze(Object.prototype); // the same scope as Tauri's `freezePrototype`
}

// ── The web view's own reload, navigation and menu are off (desktop.md V12 10, E-89) ────────
// Operator report 2026-11-18: right-click → Refresh emptied the window and left its clients
// attached. Measured on Linux (WebKitGTK) before this: the right-click menu offered Back,
// Forward, Stop, Reload and Inspect Element; F5 outside the terminal did nothing visible.

/** A key the web view would act on itself: reload, history, print, find, view source, caret
 *  browsing — and F12 (the inspector) unless this is a debug build. Ctrl on Linux / Windows,
 *  Cmd on macOS. Inside the terminal xterm sends these to the program and cancels the
 *  browser's default itself; this guards everywhere else. */
export function isBrowserKey(e: { key: string; ctrlKey: boolean; metaKey: boolean; shiftKey: boolean; altKey: boolean }, debug: boolean): boolean {
  const mod = e.ctrlKey || e.metaKey;
  const k = e.key.length === 1 ? e.key.toLowerCase() : e.key;
  if (k === "F5" || k === "F7" || (k === "F12" && !debug)) return true;
  if (e.altKey && !mod && (k === "ArrowLeft" || k === "ArrowRight")) return true;
  if (mod && !e.altKey && ["r", "p", "f", "g", "u"].includes(k)) return true; // Ctrl+Shift+R too
  return false;
}

/** Install the guards: the browser keys lose their default outside the terminal (the app's own
 *  handlers still see the key — Ctrl+Shift+R stays the panel's refresh), and the web view's
 *  context menu never opens (the app's own menus open first, then this cancels the rest). */
export function blockBrowserUi(debug: boolean): void {
  window.addEventListener(
    "keydown",
    (e) => {
      const inTerm = e.target instanceof Element && e.target.closest(".xterm") !== null;
      if (!inTerm && isBrowserKey(e, debug)) e.preventDefault();
    },
    true,
  );
  window.addEventListener("contextmenu", (e) => e.preventDefault());
}
