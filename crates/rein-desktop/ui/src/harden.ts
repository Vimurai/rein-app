// Freeze the shared prototypes once the bundled libraries have loaded (desktop.md §Security 1;
// security review of E-35). Tauri's `freezePrototype` would freeze them before any script
// runs, but xterm.js 6.0.0 assigns `toString` on a plain object while its module loads, which
// throws under a frozen `Object.prototype` (the "override mistake"). Freezing here, before any
// pty output, IPC reply or event is handled, keeps untrusted data from polluting a prototype.

export function hardenPrototypes(): void {
  Object.freeze(Object.prototype); // the same scope as Tauri's `freezePrototype`
}
