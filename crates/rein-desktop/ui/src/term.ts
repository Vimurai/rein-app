// The terminal stage (desktop.md §Components 2 and 6 `term`, §Security 3): one xterm.js
// instance per tab, fed by the core's pty channel.
//
// Output is queued and written once per animation frame; after xterm has consumed a batch
// the core is told (`pty_ack`), which is what lets it pause the pty when this side falls
// more than 4 MiB behind. Pty bytes are untrusted: they go only into xterm. OSC 52 is not
// implemented by xterm without an addon (none is loaded) and links open nothing.

import { Terminal } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import { WebglAddon } from "@xterm/addon-webgl";
import { concat, splitInput } from "./chunks.ts";
import { Channel, ptyAck, ptyResize, ptyWrite, tabDetach, tabOpen, type Tab } from "./ipc.ts";
import { START_BYTES, START_LIMIT_MS, tabPhase, type TabPhase } from "./phase.ts";
import { PtySize } from "./pty-size.ts";
import { xtermTheme, type Theme } from "./theme.ts";

export interface TermOptions {
  fontFamily: string;
  fontSize: number;
  lineHeight: number;
  theme: Theme;
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
  /** The pty's size: a resize while `tab_open` is pending is sent once it returns (E-52). */
  readonly #size = new PtySize((pty, cols, rows) => {
    ptyResize(pty, cols, rows).catch(() => {});
  });
  /** What `tabPhase` reads (V7), since the last (re)start. */
  #alt = false;
  #bytes = 0;
  #typed = false;
  #startedAt = 0;
  #limit: ReturnType<typeof setTimeout> | undefined;
  /** The phase may have changed (alternate screen, 4 KiB, a key, the exit, 20 s). */
  onPhase: () => void = () => {};
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
    // A face that finishes loading later (bold, italic, or a font.family the system loads
    // late) would leave the glyph atlas drawn with a fallback: rebuild it (V3).
    document.fonts?.addEventListener("loadingdone", this.#fontsLoaded);
    this.term.attachCustomKeyEventHandler((e) => this.keyFilter(e));
    this.term.onData((s) => this.#input(s));
    // A key or a paste from the user ends `starting` (V7). Not `onData`: xterm's own answers
    // to terminal queries (tmux asks at attach) arrive there too, with no key pressed.
    this.term.onKey(() => this.#userTyped());
    // tmux has drawn: it switched the terminal to the alternate screen (V7).
    this.term.buffer.onBufferChange((b) => {
      if (b.type === "alternate" && !this.#alt) {
        this.#alt = true;
        this.onPhase();
      }
    });
    this.term.onResize(({ cols, rows }) => this.#size.resized(cols, rows));
  }

  readonly #fontsLoaded = (): void => {
    this.term.clearTextureAtlas();
    this.term.refresh(0, this.term.rows - 1);
    this.#fitStage();
  };

  /** Fit the terminal to its pane in whole cells and centre it (desktop.md V2: the stage is
   *  borderless and the padding equal on both sides; E-49). FitAddon would keep 14 px for a
   *  scrollbar on the right that tmux never needs — the gap the E-51 screens showed — so the
   *  cell size is read where FitAddon reads it, and FitAddon is only the fallback before the
   *  first render has measured a cell. What is left over, under one cell, is split on both
   *  sides and shows the terminal's background. */
  #fitStage(): void {
    const core = (this.term as unknown as { _core?: { _renderService?: { dimensions?: { css?: { cell?: { width: number; height: number } } } } } })._core;
    const cell = core?._renderService?.dimensions?.css?.cell;
    const w = this.#el.clientWidth;
    const h = this.#el.clientHeight;
    if (!cell || !cell.width || !cell.height || !w || !h) {
      this.#fit.fit();
      return;
    }
    const cols = Math.max(2, Math.floor(w / cell.width));
    const rows = Math.max(2, Math.floor(h / cell.height));
    if (cols !== this.term.cols || rows !== this.term.rows) this.term.resize(cols, rows);
    const el = this.term.element;
    if (el) {
      const dx = Math.floor((w - cols * cell.width) / 2);
      const dy = Math.floor((h - rows * cell.height) / 2);
      el.style.transform = `translate(${dx}px, ${dy}px)`;
    }
  }

  get tab(): Tab | null {
    return this.#tab;
  }

  /** Starting, running or exited (V7 `tabPhase`), at `now` (`performance.now()`). */
  phase(now: number): TabPhase {
    return tabPhase({
      altScreen: this.#alt,
      bytes: this.#bytes,
      typed: this.#typed,
      exited: this.#exited,
      elapsedMs: now - this.#startedAt,
    });
  }

  /** Start (or restart) `rein start` in the project, sized to the stage. */
  async start(): Promise<void> {
    this.#exited = false;
    this.#alt = this.term.buffer.active.type === "alternate";
    this.#bytes = 0;
    this.#typed = false;
    this.#startedAt = performance.now();
    clearTimeout(this.#limit);
    this.#limit = setTimeout(() => this.onPhase(), START_LIMIT_MS + 50);
    this.onPhase();
    this.#owed = 0;
    const gen = ++this.#gen;
    cancelAnimationFrame(this.#frame);
    this.#frame = 0;
    this.#queue = [];
    this.#size.reset();
    // A pane created this frame has no measured cell yet, and the fit would leave xterm's
    // default 80 × 24: fit after the next frame, so the pty opens at the stage's size.
    await new Promise<void>((r) => requestAnimationFrame(() => r()));
    if (gen !== this.#gen) return;
    this.#fitStage();
    const output = new Channel<ArrayBuffer>((buf) => {
      if (gen === this.#gen) this.#output(new Uint8Array(buf));
    });
    try {
      const { cols, rows } = this.term;
      const opened = await tabOpen(this.#project, cols, rows, output);
      if (gen !== this.#gen) {
        // Closed (or restarted) while opening: this pty belongs to no terminal.
        await tabDetach(opened.id).catch(() => {});
        return;
      }
      this.#tab = opened;
      // The stage may have changed size while the pty was opening (seconds on Windows).
      this.#fitStage();
      this.#size.opened(this.#tab.pty, cols, rows, this.term);
      this.#ack(0);
      this.term.focus();
      this.onOpen(this.#tab);
    } catch (e) {
      if (gen !== this.#gen) return; // closed or restarted meanwhile: not this tab's error
      const msg = (e as { message?: string }).message ?? String(e);
      this.onError(msg);
    }
  }

  /** Send `data` to the pty as if typed (a chord prefix the modes layer held, then released). */
  send(data: string): void {
    if (this.#exited || !this.#tab) return;
    for (const piece of splitInput(data)) ptyWrite(this.#tab.pty, piece).catch(() => {});
  }

  /** Copy / Paste / Select all (E-40). `paste` goes through xterm, so bracketed paste and
   *  the same input path as typing apply. */
  selection(): string {
    return this.term.getSelection();
  }

  selectAll(): void {
    this.term.selectAll();
  }

  paste(text: string): void {
    this.#userTyped();
    this.term.paste(text);
  }

  #userTyped(): void {
    if (this.#typed) return;
    this.#typed = true;
    this.onPhase();
  }

  focus(): void {
    this.term.focus();
  }

  /** The pane became visible again (a tab switch): refit, and repaint every row — the
   *  renderer draws nothing while its element is `display: none`. */
  shown(): void {
    this.#fitStage();
    this.term.refresh(0, this.term.rows - 1);
  }

  /** Font or theme changed (config.toml was saved, or `:theme`): restyle and refit. */
  restyle(opts: TermOptions): void {
    this.term.options.fontFamily = opts.fontFamily;
    this.term.options.fontSize = opts.fontSize;
    this.term.options.lineHeight = opts.lineHeight;
    this.term.options.theme = xtermTheme(opts.theme);
    this.#fitStage();
  }

  blur(): void {
    this.term.blur();
  }

  /** The stage changed size: refit in whole cells (the core gets the new size). */
  fit(): void {
    this.#fitStage();
  }

  /** The child exited: say so in the stage; Enter restarts it. */
  exited(code: number): void {
    this.#exited = true;
    clearTimeout(this.#limit);
    this.onPhase();
    // The core reports the exit after the last output; draw that output first.
    cancelAnimationFrame(this.#frame);
    this.#flush();
    this.term.write(`\r\n\x1b[7m rein start exited with code ${code} — press Enter to restart \x1b[0m\r\n`);
  }

  /** Detach: close the pty (the tmux session keeps running) and drop the terminal. */
  async detach(): Promise<void> {
    this.#gen++; // a start still waiting for its first frame opens nothing
    clearTimeout(this.#limit);
    this.#size.reset();
    cancelAnimationFrame(this.#frame);
    if (this.#tab) await tabDetach(this.#tab.id).catch(() => {});
    this.#tab = null;
    document.fonts?.removeEventListener("loadingdone", this.#fontsLoaded);
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
    const before = this.#bytes;
    this.#bytes += bytes.length;
    if (before < START_BYTES && this.#bytes >= START_BYTES) this.onPhase();
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
