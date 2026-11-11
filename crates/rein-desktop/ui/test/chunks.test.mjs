// Pty stream helpers: input pieces stay under the core's 64 KiB write limit without
// splitting a surrogate pair; queued output joins in order.
import { test } from "node:test";
import assert from "node:assert/strict";
import { MAX_WRITE_BYTES, concat, splitInput } from "../src/chunks.ts";

const bytes = (s) => new TextEncoder().encode(s).length;

test("a large paste splits into pieces of at most 64 KiB that rejoin exactly", () => {
  for (const s of ["", "ls\r", "x".repeat(200_000), "€".repeat(70_000), "😀".repeat(40_000), "a😀".repeat(30_000)]) {
    const parts = splitInput(s);
    assert.equal(parts.join(""), s);
    for (const p of parts) {
      assert.ok(bytes(p) <= MAX_WRITE_BYTES, `${bytes(p)} bytes`);
      assert.ok(p.length > 0);
      const last = p.charCodeAt(p.length - 1);
      assert.ok(!(last >= 0xd800 && last <= 0xdbff), "no piece ends inside a surrogate pair");
    }
  }
});

test("concat joins chunks in order", () => {
  const out = concat([Uint8Array.of(1, 2), Uint8Array.of(), Uint8Array.of(3)]);
  assert.deepEqual([...out], [1, 2, 3]);
});
