// Prototype hardening (security review of E-35): after hardenPrototypes() a prototype
// pollution attempt throws and changes nothing. Run in a child process so the test runner's
// own Object.prototype stays writable.
import { test } from "node:test";
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const harden = fileURLToPath(new URL("../src/harden.ts", import.meta.url));

test("after hardening, Object.prototype cannot be polluted", () => {
  const script = `
    const { hardenPrototypes } = await import(${JSON.stringify(harden)});
    hardenPrototypes();
    // The classic gadget: a merge that walks attacker-chosen keys into obj["__proto__"].
    const obj = {};
    let threw = false;
    try { obj["__proto__"]["polluted"] = 1; } catch { threw = true; }
    process.stdout.write(JSON.stringify({ threw, polluted: ({}).polluted ?? null, frozen: Object.isFrozen(Object.prototype) }));
  `;
  const out = execFileSync(process.execPath, ["--input-type=module", "-e", script], { encoding: "utf8" });
  assert.deepEqual(JSON.parse(out), { threw: true, polluted: null, frozen: true });
});
