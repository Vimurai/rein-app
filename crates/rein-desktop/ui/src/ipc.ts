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
export const projectsList = () => invoke<import("./start.ts").RecentView[]>("projects_list", {});
export const projectsForget = (path: string) =>
  invoke<import("./start.ts").RecentView[]>("projects_forget", { path });
export interface HealthCheck {
  id: string;
  status: "ok" | "warn" | "fail";
  line: string;
  fix: string | null;
}
export interface HealthView {
  status: "ok" | "warn" | "fail" | "unknown";
  checks: HealthCheck[];
  reason: string | null;
  /** Windows: WSL missing or without a distro — the command to run (E-45). */
  setup: string | null;
  taken_at: number;
}
export interface ReleaseCheck {
  installed: string | null;
  newest: string | null;
  lines: string[];
  code: number | null;
}
export interface ProfilesRun {
  profiles: string[];
  lines: string[];
  code: number | null;
}
export const healthGet = (refresh: boolean) => invoke<HealthView>("health_get", { refresh });
export const releaseCheck = () => invoke<ReleaseCheck>("release_check", {});
/** `update` | `rollback`: the CLI's lines arrive on `output`; resolves to its exit code. */
export const releaseApply = (kind: "update" | "rollback", output: Channel<string>) =>
  invoke<number | null>("release_apply", { kind, output });
export const profilesPlan = (names: string[]) => invoke<ProfilesRun>("profiles_plan", { names });
export const profilesApply = (names: string[]) => invoke<ProfilesRun>("profiles_apply", { names });

export interface Effective<T> {
  value: T;
  source: "config" | "ui" | "default";
}
export interface UiView {
  input_style: Effective<"simple" | "modal">;
  theme: Effective<string>;
  font_size: Effective<number>;
  decorations: Effective<boolean>;
  tmux_overlay: Effective<boolean>;
  tmux_status: Effective<"hidden" | "app" | "keep">;
  first_run_done: boolean;
}
/** The system folder dialog; `null` when cancelled; the path is validated like `tab_open`. */
export const projectChoose = () => invoke<string | null>("project_choose", {});
/** Only from a user's paste action (desktop.md §Security 6). */
export const clipboardRead = () => invoke<string>("clipboard_read", {});
/** The tmux overlay (V6) for `tab`'s session in these colours; the mode is the settings'.
 *  Resolves even when tmux was left alone — the health view says why, and so does the answer. */
export const tabTmux = (tab: number, colours: ReturnType<typeof import("./theme.ts").tmuxColours>) =>
  // The health row's words when the overlay was not applied; null when it was or the tab closed.
  invoke<string | null>("tab_tmux", { tab, colours });
export const uiGet = () => invoke<UiView>("ui_get", {});
export const uiSet = (field: string, value: unknown) => invoke<UiView>("ui_set", { field, value });

/** Close the window (`:qa`, after every tab is detached): Tauri's core window command. */
export const closeWindow = () => invoke<void>("plugin:window|close", { label: "main" });
/** The title bar is ours (V2): dragging the empty part moves the window, our buttons minimise
 *  and maximise it. Exactly the window permissions the capability file grants. */
export const startDragging = () => invoke<void>("plugin:window|start_dragging", { label: "main" });
export const minimizeWindow = () => invoke<void>("plugin:window|minimize", { label: "main" });
export const toggleMaximize = () => invoke<void>("plugin:window|toggle_maximize", { label: "main" });
export const configGet = (reload = false) =>
  invoke<import("./config.ts").ConfigView>("config_get", { reload });
