// The only bridge to the core. No `@tauri-apps/api` package (D-035): the UI talks to the
// injected `__TAURI_INTERNALS__` through this module alone, so every call site is typed and
// greppable. Commands are added here as the IPC table grows (desktop.md §API).

interface Internals {
  invoke(cmd: string, args?: Record<string, unknown>): Promise<unknown>;
  transformCallback(cb: (v: unknown) => void, once?: boolean): number;
}

function internals(): Internals | null {
  const t = (globalThis as { __TAURI_INTERNALS__?: Internals }).__TAURI_INTERNALS__;
  return t ?? null;
}

/** Listen for a core event; the handler receives the payload. No-op outside the window. */
export async function listen<T>(event: string, handler: (payload: T) => void): Promise<void> {
  const t = internals();
  if (!t) return;
  const id = t.transformCallback((e) => handler((e as { payload: T }).payload));
  await t.invoke("plugin:event|listen", { event, target: { kind: "Any" }, handler: id });
}
