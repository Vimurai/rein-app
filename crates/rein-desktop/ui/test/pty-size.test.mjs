// The pty's size follows the terminal (desktop.md §Components 2, D-052). New: no earlier test
// covered the size the pty is given; term.ts drops nothing while `tab_open` is pending.
import { test } from "node:test";
import assert from "node:assert/strict";
import { PtySize } from "../src/pty-size.ts";

function recorder() {
  const sent = [];
  return { sent, size: new PtySize((pty, cols, rows) => sent.push([pty, cols, rows])) };
}

test("a size seen while the pty is opening is sent as soon as it opens", () => {
  // The operator's report: opened at xterm's default, fitted to the stage meanwhile.
  const { sent, size } = recorder();
  size.reset();
  size.resized(318, 62); // no pty yet: nothing to send to
  assert.deepEqual(sent, []);
  size.opened(7, 80, 24, { cols: 318, rows: 62 });
  assert.deepEqual(sent, [[7, 318, 62]]);
});

test("a pty opened at the terminal's size is not resized, and later changes are sent once each", () => {
  const { sent, size } = recorder();
  size.opened(3, 135, 29, { cols: 135, rows: 29 });
  assert.deepEqual(sent, []);
  size.resized(135, 29);
  size.resized(318, 62);
  size.resized(318, 62);
  size.resized(200, 50);
  assert.deepEqual(sent, [
    [3, 318, 62],
    [3, 200, 50],
  ]);
});

test("after a restart nothing goes to the old pty, and the new one gets the current size", () => {
  const { sent, size } = recorder();
  size.opened(1, 100, 30, { cols: 100, rows: 30 });
  size.reset();
  size.resized(120, 40);
  assert.deepEqual(sent, []);
  size.opened(2, 100, 30, { cols: 120, rows: 40 });
  assert.deepEqual(sent, [[2, 120, 40]]);
});
