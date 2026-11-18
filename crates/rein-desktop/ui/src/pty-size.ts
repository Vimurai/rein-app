// The pty's size in cells follows the terminal (desktop.md §Components 2, E-52, D-052).
//
// `tab_open` is given the terminal's size when it is called, and the terminal can change
// size before it returns — on Windows the open runs `wsl.exe` and takes seconds, and the
// first real fit lands in that window. A size seen while no pty exists is kept, and the
// pty is resized to it as soon as it opens; afterwards every change is sent, and an
// unchanged size is not re-sent.

export type SendSize = (pty: number, cols: number, rows: number) => void;

export class PtySize {
  readonly #send: SendSize;
  #pty: number | null = null;
  /** The size the pty has (what it was opened at, or last resized to). */
  #cols = 0;
  #rows = 0;

  constructor(send: SendSize) {
    this.#send = send;
  }

  /** A (re)start began: there is no pty until `opened`. */
  reset(): void {
    this.#pty = null;
  }

  /** The pty opened at `cols` × `rows`; the terminal is `now` — resize if it moved meanwhile. */
  opened(pty: number, cols: number, rows: number, now: { cols: number; rows: number }): void {
    this.#pty = pty;
    this.#cols = cols;
    this.#rows = rows;
    this.resized(now.cols, now.rows);
  }

  /** The terminal is now `cols` × `rows`. */
  resized(cols: number, rows: number): void {
    if (this.#pty === null || (cols === this.#cols && rows === this.#rows)) return;
    this.#cols = cols;
    this.#rows = rows;
    this.#send(this.#pty, cols, rows);
  }
}

/** A size in cells. */
export interface Cells {
  cols: number;
  rows: number;
}

/** How long an open waits for the terminal's first measured size (E-91) before it opens at
 *  the size the terminal has, as before. Far under the 20 s a tab may show as starting
 *  (phase.ts START_LIMIT_MS): the first measure takes a few frames. */
export const MEASURE_LIMIT_MS = 2000;

/** The size a pty opens at (D-061 addendum 7 ruling 2, E-91), or `null` to wait a frame.
 *  An attach to a live session that opens at a guess (xterm's 80 × 24 before the first
 *  measure) makes tmux resize the session twice, and the agents re-print their text wrapped
 *  at the guess. `measured`: this tab's own fit, when its pane is shown and the renderer has
 *  measured a cell. `stage`: the last size any tab measured — every tab's terminal fills the
 *  same stage, so a hidden tab (restored behind the current one) or one not measured yet
 *  opens at it. Neither within `MEASURE_LIMIT_MS`: the terminal's own size, as before. */
export function openSize(s: { measured: Cells | null; stage: Cells | null; current: Cells; elapsedMs: number }): Cells | null {
  if (s.measured) return s.measured;
  if (s.stage) return s.stage;
  return s.elapsedMs >= MEASURE_LIMIT_MS ? s.current : null;
}
