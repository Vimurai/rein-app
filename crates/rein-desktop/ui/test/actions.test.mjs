// The Action list's one source is ../actions.json (desktop.md §Data Model; E-36): the committed
// src/actions.ts must be exactly what gen-actions.mjs makes of it — names, args and labels.
import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { generate } from "../gen-actions.mjs";

test("src/actions.ts is generated from actions.json and is not stale", () => {
  const committed = readFileSync(new URL("../src/actions.ts", import.meta.url), "utf8");
  assert.equal(committed, generate(), "run `node gen-actions.mjs` (npm run build does it)");
});
