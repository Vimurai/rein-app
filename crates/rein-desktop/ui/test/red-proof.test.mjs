// E-65 red-proof, deliberately failing: shows that a failing window-UI test now turns
// `test (windows-latest)` red. Lives only on branch e65-windows-ci-red-proof; removed before
// the fix reaches master.
import { test } from "node:test";
import assert from "node:assert/strict";

test("E-65 red-proof: this test fails on purpose", () => {
  assert.equal(1, 2, "deliberate failure (E-65)");
});
