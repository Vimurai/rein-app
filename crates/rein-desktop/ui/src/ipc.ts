// The only bridge to the core. No `@tauri-apps/api` package (D-035): the UI talks to the
// injected `__TAURI_INTERNALS__` through this module alone, so every call site is typed and
// greppable. Commands mirror the core's closed table (desktop.md §API, src/ipc_commands.rs).

interface Internals {
  invoke(cmd: string, args?: Record<string, unknown>): Promise<unknown>;
  transformCallback(cb: (v: unknown) => void, once?: boolean): number;
}

function internals(): Internals | null {
  const t = (globalThis as { __TAURI_INTERNALS__?: Internals }).__TAURI_INTERNALS__;
  return t ?? null;
}

/** An IPC refusal from the core: a stable `kind` and a plain-words `message`. */
export interface IpcError {
  kind: string;
  message: string;
}

export function isIpcError(e: unknown): e is IpcError {
  return typeof e === "object" && e !== null && "kind" in e && "message" in e;
}

async function invoke<T>(cmd: string, args: Record<string, unknown>): Promise<T> {
  const t = internals();
  if (!t) throw { kind: "no_core", message: "not running inside the rein-app window" };
  return (await t.invoke(cmd, args)) as T;
}

/** Listen for a core event; the handler receives the payload. No-op outside the window. */
export async function listen<T>(event: string, handler: (payload: T) => void): Promise<void> {
  const t = internals();
  if (!t) return;
  const id = t.transformCallback((e) => handler((e as { payload: T }).payload));
  await t.invoke("plugin:event|listen", { event, target: { kind: "Any" }, handler: id });
}

/**
 * A core → web stream (Tauri's channel protocol): messages carry an index and are delivered
 * to `onMessage` in order even if they arrive out of order.
 */
export class Channel<T> {
  readonly id: number;
  #next = 0;
  #pending = new Map<number, T>();

  constructor(onMessage: (m: T) => void) {
    const t = internals();
    this.id = t
      ? t.transformCallback((raw) => {
          const r = raw as { index: number; message?: T; end?: boolean };
          if (r.end || r.message === undefined) return;
          this.#pending.set(r.index, r.message);
          for (let m = this.#pending.get(this.#next); m !== undefined; m = this.#pending.get(this.#next)) {
            this.#pending.delete(this.#next);
            this.#next += 1;
            onMessage(m);
          }
        })
      : -1;
  }

  toJSON(): string {
    return `__CHANNEL__:${this.id}`;
  }
}

/** A tab as the core reports it. */
export interface Tab {
  id: number;
  project: string;
  pty: number;
  title: string;
}

export const tabOpen = (project: string, cols: number, rows: number, output: Channel<ArrayBuffer>) =>
  invoke<Tab>("tab_open", { project, cols, rows, output });
export const ptyWrite = (pty: number, data: string) => invoke<void>("pty_write", { pty, data });
export const ptyResize = (pty: number, cols: number, rows: number) =>
  invoke<void>("pty_resize", { pty, cols, rows });
export const ptyAck = (pty: number, bytes: number) => invoke<void>("pty_ack", { pty, bytes });
export const tabDetach = (tab: number) => invoke<void>("tab_detach", { tab });
export const clipboardWrite = (text: string) => invoke<void>("clipboard_write", { text });
export const dirComplete = (prefix: string) => invoke<string[]>("dir_complete", { prefix });
