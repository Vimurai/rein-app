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
