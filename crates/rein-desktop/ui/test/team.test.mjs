// desktop.md §Look and feel v2 V10 (D-058, E-64; the ruling after the read of rein 0.5.0): what
// the Team section offers for each shape of `rein team show`, the form's help text (the core's
// check is the one that counts — src/team.rs), and the run's last line. New suite: the
// Project sheet is new.
import { test } from "node:test";
import assert from "node:assert/strict";
import { MODE_BUTTON, MODE_CONFIRM, formHint, formReady, offer, runNote } from "../src/team.ts";

test("the sheet offers by shape and names no mode", () => {
  assert.deepEqual(offer("team"), { mode: "legacy", form: true, note: null });
  assert.deepEqual(offer("single"), { mode: "team", form: true, note: null });
  const other = offer("other");
  assert.equal(other.mode, null);
  assert.equal(other.form, false);
  assert.match(other.note, /shown as rein printed it/);
  assert.equal(MODE_BUTTON.team, "Turn on team mode");
  assert.equal(MODE_BUTTON.legacy, "Go back to single-developer mode");
  assert.match(MODE_CONFIRM.legacy.text, /removes \.ai\/team\.json with every developer's id range/);
  assert.match(MODE_CONFIRM.team.text, /Close and reopen the project/);
});

test("the form's help text, as a table", () => {
  const rows = [
    ["empty is not yet an error", "", "", "", null],
    ["a good row", "dev@example.com", "425", "524", null],
    ["a leading dash", "-dev@x.io", "1", "2", /e-mail/],
    ["two @", "a@b@c", "1", "2", /e-mail/],
    ["a space", "a b@c", "1", "2", /e-mail/],
    ["a %", "a%b@c", "1", "2", /e-mail/],
    ["non-ASCII", "dé@x.io", "1", "2", /e-mail/],
    ["0", "a@b", "0", "2", /From/],
    ["too big", "a@b", "1", "100000", /To/],
    ["not a number", "a@b", "1e3", "2", /From/],
    ["from above to", "a@b", "5", "1", /not above/],
  ];
  for (const [label, e, f, t, want] of rows) {
    const h = formHint(e, f, t);
    if (want === null) assert.equal(h, null, label);
    else assert.match(h ?? "", want, label);
  }
  assert.equal(formReady("dev@x.io", "1", "100"), true);
  assert.equal(formReady("dev@x.io", "", "100"), false);
  assert.equal(formReady("dev@x.io", "5", "1"), false);
});

test("the run's last line", () => {
  assert.deepEqual(runNote({ code: 0, lines: [], cut: false, timed_out: false }), { status: "ok", text: "rein finished." });
  assert.match(runNote({ code: 1, lines: [], cut: false, timed_out: false }).text, /exit 1/);
  assert.match(runNote({ code: null, lines: [], cut: false, timed_out: true }).text, /in time/);
});
