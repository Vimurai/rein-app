// Loaded before app.js: if the UI fails to start, the window says why instead of staying
// blank. The text goes in through `textContent` only.

function show(what: string): void {
  const pre = document.createElement("pre");
  pre.className = "fatal";
  pre.textContent = `the rein-app window failed to start:\n${what}`;
  document.body.prepend(pre);
}

window.addEventListener("error", (e) => show(`${e.message} (${e.filename}:${e.lineno}:${e.colno})`));
window.addEventListener("unhandledrejection", (e) => {
  const reason: unknown = (e as PromiseRejectionEvent).reason;
  const stack = reason instanceof Error && reason.stack ? `\n${reason.stack}` : "";
  show(`${String(reason)}${stack}`);
});
