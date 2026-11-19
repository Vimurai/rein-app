// Notifications in the window (status.md §Components 6 channel 1, E-78): the toast shows the
// core's sentence unless the event is about the current tab; Settings' events list. When an
// event is raised is the core's (notify.rs tests).
import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { EVENTS, EVENT_LABELS, toastText, withEvent } from "../src/notify.ts";
import { DEFAULT_CONFIG } from "../src/config.ts";

const n = (over = {}) => ({ event: "stopped", tab: 4, sentence: "qrtable has stopped — it is finished or waiting.", ...over });

test("the toast shows the core's sentence for a tab that is not current", () => {
  assert.equal(toastText(n(), 2), "qrtable has stopped — it is finished or waiting.");
  assert.equal(toastText(n(), undefined), "qrtable has stopped — it is finished or waiting.", "no current tab");
  assert.equal(toastText(n(), 4), null, "the current tab");
  assert.equal(toastText(n({ event: "waiting" }), 2), null, "not an event of the closed list");
  assert.equal(toastText(n({ sentence: "" }), 2), null);
  assert.equal(toastText(n({ sentence: 5 }), 2), null);
});

test("the closed list is the core's, and Settings keeps its order", () => {
  const rs = readFileSync(new URL("../../src/notify.rs", import.meta.url), "utf8");
  const names = rs.match(/EVENT_NAMES: \[&str; 5\] = \[([^\]]*)\]/)[1].match(/"[a-z_]+"/g).map((s) => JSON.parse(s));
  assert.deepEqual([...EVENTS], names);
  assert.deepEqual(DEFAULT_CONFIG.notifications.events, names, "the default is every event");
  assert.deepEqual(Object.keys(EVENT_LABELS), names);
  assert.deepEqual(withEvent(["ended", "needs_you"], "stuck", true), ["needs_you", "stuck", "ended"]);
  assert.deepEqual(withEvent(EVENTS, "handoff", false), ["needs_you", "stopped", "stuck", "ended"]);
  assert.deepEqual(withEvent(["ended", "bogus"], "ended", false), [], "nothing outside the list is kept");
});
