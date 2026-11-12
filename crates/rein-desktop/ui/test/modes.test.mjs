// desktop.md §Test Plan "Modes" (E-36): every row of the default keymap, the leader and its
// which-key list, an unbound NORMAL key ignored, and — a property test — TERMINAL forwards
// every key sequence except the escape chord.
import { test } from "node:test";
import assert from "node:assert/strict";
import { DEFAULT_MODAL } from "../src/keymap.ts";
import { WHICH_KEY_DELAY_MS, continuations, step } from "../src/modes.ts";
import { keyBytes, keyName } from "../src/keys.ts";
import { COMMANDS, commonPrefix, complete, parse } from "../src/commands.ts";

const T = DEFAULT_MODAL;

/** Feed `keys` from `state`; return the final step and every forwarded key. */
function run(keys, state = { mode: "NORMAL", pending: [] }) {
  let s = { ...state, forward: [] };
  const forwarded = [];
  const actions = [];
  for (const k of keys) {
    s = step(T, s, k);
    forwarded.push(...s.forward);
    if (s.action) actions.push(s.action);
  }
  return { last: s, forwarded, actions };
}

test("every row of the default keymap", () => {
  const rows = [
    // [mode, keys, action, arg, mode after]
    ["TERMINAL", ["C-\\", "C-n"], "mode.normal", undefined, "NORMAL"],
    ["NORMAL", ["i"], "mode.terminal", undefined, "TERMINAL"],
    ["NORMAL", ["a"], "mode.terminal", undefined, "TERMINAL"],
    ["NORMAL", ["Enter"], "mode.terminal", undefined, "TERMINAL"],
    ["NORMAL", [":"], "mode.command", undefined, "COMMAND"],
    ["NORMAL", ["Space", "p"], "project.pick", undefined, "NORMAL"],
    ["NORMAL", ["Space", "o"], "project.open", undefined, "NORMAL"],
    ["NORMAL", ["g", "t"], "tab.next", undefined, "NORMAL"],
    ["NORMAL", ["g", "T"], "tab.prev", undefined, "NORMAL"],
    ...[1, 2, 3, 4, 5, 6, 7, 8, 9].map((n) => ["NORMAL", [String(n)], "tab.goto", String(n), "NORMAL"]),
    ["NORMAL", ["Space", "h"], "health.show", undefined, "NORMAL"],
    ["NORMAL", ["Space", "u"], "release.update", undefined, "NORMAL"],
    ["NORMAL", ["Space", "r"], "release.rollback", undefined, "NORMAL"],
    ["NORMAL", ["Space", "s"], "profiles.show", undefined, "NORMAL"],
    ["NORMAL", ["y"], "cmd.yank", undefined, "NORMAL"],
    ["NORMAL", ["Z", "Z"], "tab.detach", undefined, "NORMAL"],
  ];
  for (const [mode, keys, name, arg, after] of rows) {
    const { last, actions, forwarded } = run(keys, { mode, pending: [] });
    assert.deepEqual(actions, [arg === undefined ? { name } : { name, arg }], keys.join(" "));
    assert.equal(last.mode, after, keys.join(" "));
    assert.deepEqual(last.pending, [], keys.join(" "));
    assert.deepEqual(forwarded, [], `${keys.join(" ")} sends nothing to the pty`);
  }
  // `:q` and `:qa` are command-line rows.
  assert.deepEqual(parse("q"), { action: { name: "tab.detach" } });
  assert.deepEqual(parse("qa"), { action: { name: "app.quit" } });
});

test("the leader waits, and which-key lists what can follow it", () => {
  const { last } = run(["Space"]);
  assert.equal(last.mode, "NORMAL");
  assert.deepEqual(last.pending, ["Space"]);
  assert.equal(WHICH_KEY_DELAY_MS, 400);
  const next = new Map(continuations(T, last.pending).map(([k, a]) => [k, a.name]));
  assert.deepEqual(
    next,
    new Map([
      ["p", "project.pick"],
      ["o", "project.open"],
      ["h", "health.show"],
      ["u", "release.update"],
      ["r", "release.rollback"],
      ["s", "profiles.show"],
    ]),
  );
  // Esc drops the pending leader; a key that breaks it is tried on its own.
  assert.deepEqual(run(["Space", "Esc"]).last.pending, []);
  assert.deepEqual(run(["Space", "y"]).actions, [{ name: "cmd.yank" }]);
});

test("an unbound key in NORMAL is ignored", () => {
  for (const k of ["x", "q", "C-a", "F5", "Tab", "g"]) {
    const { last, actions, forwarded } = run(k === "g" ? ["g", "x"] : [k]);
    assert.deepEqual(actions, [], k);
    assert.equal(last.mode, "NORMAL", k);
    assert.deepEqual(last.pending, [], k);
    assert.deepEqual(forwarded, [], k);
  }
});

test("COMMAND: keys go to the line editor; Esc returns to NORMAL", () => {
  const s = step(T, { mode: "COMMAND", pending: [] }, "x");
  assert.equal(s.edit, "x");
  assert.equal(s.mode, "COMMAND");
  const esc = step(T, { mode: "COMMAND", pending: [] }, "Esc");
  assert.equal(esc.mode, "NORMAL");
  assert.deepEqual(esc.action, { name: "mode.normal" }, "leaving COMMAND is an action, so the line is reset");
});

/** A small deterministic PRNG, so a failing sequence can be reproduced from its seed. */
function rng(seed) {
  let x = seed >>> 0 || 1;
  return () => ((x ^= x << 13), (x ^= x >>> 17), (x ^= x << 5), (x >>> 0) / 2 ** 32);
}

test("property: TERMINAL forwards exactly the input with every escape chord removed", () => {
  const alphabet = ["a", "C-\\", "C-n", "Enter", "Space", "C-c", ":", "Esc", "\\", "n"];
  const chord = T.escape;
  for (let seed = 1; seed <= 2000; seed++) {
    const r = rng(seed);
    const len = 1 + Math.floor(r() * 12);
    const keys = Array.from({ length: len }, () => alphabet[Math.floor(r() * alphabet.length)]);
    // Expected: scan left to right; a complete chord is consumed (and returns to NORMAL).
    let state = { mode: "TERMINAL", pending: [] };
    const sent = [];
    let left = false;
    for (const k of keys) {
      if (state.mode !== "TERMINAL") break;
      const s = step(T, state, k);
      sent.push(...s.forward);
      state = s;
      if (s.action) left = true;
    }
    const contains = keys.join("\u0000").includes(chord.join("\u0000"));
    if (!contains) {
      assert.equal(left, false, `seed ${seed}: ${keys.join(" ")}`);
      // Everything typed is sent, except a still-held chord prefix at the very end.
      assert.deepEqual([...sent, ...state.pending], keys, `seed ${seed}: ${keys.join(" ")}`);
    } else {
      assert.equal(left, true, `seed ${seed}: the chord leaves TERMINAL`);
      const at = keys.join("\u0000").indexOf(chord.join("\u0000"));
      const before = keys.join("\u0000").slice(0, at).split("\u0000").filter((k, i) => at > 0 || i > 0);
      assert.deepEqual(sent, at === 0 ? [] : before, `seed ${seed}: ${keys.join(" ")}`);
    }
  }
});

test("key names and the bytes of a held chord key", () => {
  const k = (key, m = {}) => keyName({ key, ctrlKey: false, altKey: false, shiftKey: false, metaKey: false, ...m });
  assert.equal(k("\\", { ctrlKey: true }), "C-\\");
  assert.equal(k("n", { ctrlKey: true }), "C-n");
  assert.equal(k("P", { ctrlKey: true, shiftKey: true }), "C-S-p");
  assert.equal(k(":", { shiftKey: true }), ":");
  assert.equal(k("T", { shiftKey: true }), "T");
  assert.equal(k("Tab", { shiftKey: true }), "S-Tab");
  assert.equal(k(" "), "Space");
  assert.equal(k("Escape"), "Esc");
  assert.equal(k("Shift", { shiftKey: true }), null);
  assert.equal(keyBytes("C-\\"), "\x1c");
  assert.equal(keyBytes("C-n"), "\x0e");
  assert.equal(keyBytes("a"), "a");
  assert.equal(keyBytes("F1"), null);
});

test("command line: the listed commands, E492, completion", async () => {
  assert.deepEqual(parse("open /home/me/p"), { action: { name: "project.open", arg: "/home/me/p" } });
  assert.deepEqual(parse("theme nord"), { action: { name: "theme.set", arg: "nord" } });
  assert.deepEqual(parse("checkhealth"), { action: { name: "health.show" } });
  assert.deepEqual(parse("bogus"), { error: "E492: Not a command: bogus" });
  assert.deepEqual(parse("open"), { error: "E471: Argument required" });
  assert.deepEqual(parse("q now"), { error: "E488: Trailing characters" });
  assert.deepEqual(parse("  "), { empty: true });
  assert.deepEqual(
    [...COMMANDS.keys()].sort(),
    ["checkhealth", "config", "help", "open", "profiles", "projects", "q", "qa", "rollback", "theme", "update"],
  );
  const src = {
    themes: ["gruvbox-dark", "github-light", "nord"],
    dirs: async (p) => ["/home/me/proj/", "/home/me/prod/"].filter((d) => d.startsWith(p)),
  };
  assert.deepEqual(await complete("pro", src), ["profiles", "projects"]);
  assert.deepEqual(await complete("theme g", src), ["theme gruvbox-dark", "theme github-light"]);
  assert.deepEqual(await complete("open /home/me/pro", src), ["open /home/me/proj/", "open /home/me/prod/"]);
  assert.deepEqual(await complete("q x", src), []);
  assert.equal(commonPrefix(["open /home/me/proj/", "open /home/me/prod/"]), "open /home/me/pro");
});
