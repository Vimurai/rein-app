// desktop.md §Look and feel v2 V7 "Tests (V7)" (D-054): a tab is `starting` until the first of
// its exits — the alternate screen, 4 KiB of output, a key, 20 s — and `exited` once the pty
// exits. Output that arrives before the alternate screen (rein start's own lines) keeps it
// starting. New suite: the rule is new (E-54); nothing tested a tab's phase before.
import { test } from "node:test";
import assert from "node:assert/strict";
import { START_BYTES, START_LIMIT_MS, tabPhase } from "../src/phase.ts";

const fresh = { altScreen: false, bytes: 0, typed: false, exited: false, elapsedMs: 0 };

test("each exit of the starting phase, as a table", () => {
  const rows = [
    ["just opened", {}, "starting"],
    ["rein start's own lines (E-53: under 1 KiB) keep it starting", { bytes: 900, elapsedMs: 400 }, "starting"],
    ["one byte short of 4 KiB", { bytes: START_BYTES - 1 }, "starting"],
    ["19.999 s", { elapsedMs: START_LIMIT_MS - 1 }, "starting"],
    ["alternate screen: tmux has drawn", { altScreen: true, bytes: 1200 }, "running"],
    ["4 KiB without the alternate screen (older ConPTY)", { bytes: START_BYTES }, "running"],
    ["a key typed into the tab", { typed: true }, "running"],
    ["20 s", { elapsedMs: START_LIMIT_MS }, "running"],
    ["the pty exited while starting", { exited: true }, "exited"],
    ["the pty exited after drawing", { exited: true, altScreen: true }, "exited"],
  ];
  for (const [label, s, want] of rows) assert.equal(tabPhase({ ...fresh, ...s }), want, label);
});
