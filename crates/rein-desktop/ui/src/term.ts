// The terminal stage (desktop.md §Components 2 and 6 `term`, §Security 3): one xterm.js
// instance per tab, fed by the core's pty channel.
//
// Output is queued and written once per animation frame; after xterm has consumed a batch
// the core is told (`pty_ack`), which is what lets it pause the pty when this side falls
// more than 4 MiB behind. Pty bytes are untrusted: they go only into xterm. OSC 52 is not
// implemented by xterm without an addon (none is loaded) and links open nothing.

import { Terminal, type ITheme } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import { WebglAddon } from "@xterm/addon-webgl";
import { concat, splitInput } from "./chunks.ts";
import { Channel, ptyAck, ptyResize, ptyWrite, tabDetach, tabOpen, type Tab } from "./ipc.ts";
import type { Theme } from "./theme.ts";

export interface TermOptions {
  fontFamily: string;
  fontSize: number;
  lineHeight: number;
  theme: Theme;
}

const ANSI_KEYS = [
  "black", "red", "green", "yellow", "blue", "magenta", "cyan", "white",
  "brightBlack", "brightRed", "brightGreen", "brightYellow",
  "brightBlue", "brightMagenta", "brightCyan", "brightWhite",
] as const;

/** The theme's sixteen colours, given to xterm so chrome and terminal match. */
export function xtermTheme(t: Theme): ITheme {
  const out: ITheme = { foreground: t.fg, background: t.bg, cursor: t.fg, selectionBackground: t.barBg };
  ANSI_KEYS.forEach((k, i) => {
    const c = t.ansi[i];
    if (c) out[k] = c;
  });
  return out;
}

/** A tab's terminal: `rein start` in the project, restartable with Enter after it exits. */
export class TermTab {
  readonly term: Terminal;
  readonly #fit = new FitAddon();
  readonly #el: HTMLElement;
  readonly #project: string;
  #tab: Tab | null = null;
  #queue: Uint8Array[] = [];
  #frame = 0;
  #exited = false;
  /** Bumped on every (re)start; output from an older start's channel is dropped, so its
   *  bytes are never acknowledged against the new pty. */
  #gen = 0;
  /** Bytes consumed before `tab_open` returned (no pty id yet): acknowledged once it does. */
  #owed = 0;
  /** Called with the tab once open, and with an error message if opening fails. */
  onOpen: (tab: Tab) => void = () => {};
  onError: (message: string) => void = () => {};
  /** Sees every key event xterm gets; `false` keeps the key from the terminal (the modes
   *  layer takes the escape chord, E-36). */
  keyFilter: (e: KeyboardEvent) => boolean = () => true;

  constructor(el: HTMLElement, project: string, opts: TermOptions) {
    this.#el = el;
    this.#project = project;
    this.term = new Terminal({
      fontFamily: opts.fontFamily,
      fontSize: opts.fontSize,
      lineHeight: opts.lineHeight,
      theme: xtermTheme(opts.theme),
      scrollback: 1000, // tmux keeps the real scrollback (desktop.md §Execution Constraints)
      allowProposedApi: false,
      // Hyperlinks (OSC 8) are shown but never opened by the app (§Security 3).
      linkHandler: { activate: () => {} },
    });
    this.term.loadAddon(this.#fit);
    this.term.open(el);
    try {
      const gl = new WebglAddon();
      gl.onContextLoss(() => gl.dispose()); // falls back to the DOM renderer
      this.term.loadAddon(gl);
    } catch {
      // no WebGL here: the DOM renderer is used
    }
    this.term.attachCustomKeyEventHandler((e) => this.keyFilter(e));
    this.term.onData((s) => this.#input(s));
    this.term.onResize(({ cols, rows }) => {
      if (this.#tab) ptyResize(this.#tab.pty, cols, rows).catch(() => {});
    });
  }

  get tab(): Tab | null {
    return this.#tab;
  }

  /** Start (or restart) `rein start` in the project, sized to the stage. */
  async start(): Promise<void> {
    this.#exited = false;
    this.#owed = 0;
    const gen = ++this.#gen;
    cancelAnimationFrame(this.#frame);
    this.#frame = 0;
    this.#queue = [];
    this.#fit.fit();
    const output = new Channel<ArrayBuffer>((buf) => {
      if (gen === this.#gen) this.#output(new Uint8Array(buf));
    });
    try {
      this.#tab = await tabOpen(this.#project, this.term.cols, this.term.rows, output);
      this.#ack(0);
      this.term.focus();
      this.onOpen(this.#tab);
    } catch (e) {
      const msg = (e as { message?: string }).message ?? String(e);
      this.onError(msg);
    }
  }

  /** Send `data` to the pty as if typed (a chord prefix the modes layer held, then released). */
  send(data: string): void {
    if (this.#exited || !this.#tab) return;
    for (const piece of splitInput(data)) ptyWrite(this.#tab.pty, piece).catch(() => {});
  }

  focus(): void {
    this.term.focus();
  }

  blur(): void {
    this.term.blur();
  }

  /** The stage changed size: refit in whole cells (the core gets the new size). */
  fit(): void {
    this.#fit.fit();
  }

  /** The child exited: say so in the stage; Enter restarts it. */
  exited(code: number): void {
    this.#exited = true;
    // The core reports the exit after the last output; draw that output first.
    cancelAnimationFrame(this.#frame);
    this.#flush();
    this.term.write(`\r\n\x1b[7m rein start exited with code ${code} — press Enter to restart \x1b[0m\r\n`);
  }

  /** Detach: close the pty (the tmux session keeps running) and drop the terminal. */
  async detach(): Promise<void> {
    cancelAnimationFrame(this.#frame);
    if (this.#tab) await tabDetach(this.#tab.id).catch(() => {});
    this.#tab = null;
    this.term.dispose();
    this.#el.replaceChildren();
  }

  #input(s: string): void {
    if (this.#exited) {
      if (s === "\r" && this.#tab) {
        const old = this.#tab;
        this.#tab = null;
        tabDetach(old.id)
          .catch(() => {})
          .then(() => this.start());
      }
      return;
    }
    const tab = this.#tab;
    if (!tab) return;
    for (const piece of splitInput(s)) ptyWrite(tab.pty, piece).catch(() => {});
  }

  #output(bytes: Uint8Array): void {
    this.#queue.push(bytes);
    if (!this.#frame) this.#frame = requestAnimationFrame(() => this.#flush());
  }

  /** One terminal write per animation frame; acknowledge once xterm has consumed it. */
  #flush(): void {
    this.#frame = 0;
    const batch = concat(this.#queue);
    this.#queue = [];
    const gen = this.#gen;
    this.term.write(batch, () => {
      if (gen === this.#gen) this.#ack(batch.length);
    });
  }

  #ack(bytes: number): void {
    this.#owed += bytes;
    const tab = this.#tab;
    if (!tab || this.#owed === 0) return;
    const n = this.#owed;
    this.#owed = 0;
    ptyAck(tab.pty, n).catch(() => {});
  }
}
