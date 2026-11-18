// updates.md §Components 1 and 4 for Rein, §Tests (D-065): what the web side draws from the
// update watch's state. New suite: the watch is new; the schedule itself is the core's test
// (crates/rein-desktop/src/updates.rs, the_schedule_follows_the_clock).
import { test } from "node:test";
import assert from "node:assert/strict";
import {
  appNewer,
  compareVersions,
  installedChanged,
  recheckOrder,
  reinAsk,
  reinAskText,
  reinNewer,
  showUpdateButton,
  updateRows,
} from "../src/updates.ts";

const state = (rein = {}, asked = {}, app = {}) => ({
  rein: { installed: "0.5.0", newest: null, checked_at: null, offline: false, ...rein },
  app: { running: "0.1.0", newest: null, notes: null, mode: "none", command: null, ...app },
  job: "idle",
  asked: { rein: null, app: null, ...asked },
});

test("versions compare as semver, a pre-release below its release, junk is no answer", () => {
  assert.equal(compareVersions("0.5.1", "0.5.0"), 1);
  assert.equal(compareVersions("0.10.0", "0.9.9"), 1, "numbers, not text");
  assert.equal(compareVersions("v0.5.0", "0.5.0"), 0);
  assert.equal(compareVersions("0.6.0-rc.1", "0.6.0"), -1);
  assert.equal(compareVersions("0.6.0", "0.6.0-rc.1"), 1);
  assert.equal(compareVersions("0.6.0+g1", "0.6.0"), 0, "build metadata is ignored");
  assert.equal(compareVersions("latest", "0.5.0"), null);
});

test("the button and the card show only for a newer Rein; Later quiets the card, not the button", () => {
  assert.equal(reinNewer(null), null);
  assert.equal(showUpdateButton(state()), false, "nothing known yet");
  assert.equal(showUpdateButton(state({ newest: "0.5.0" })), false, "current");
  assert.equal(showUpdateButton(state({ newest: "0.4.9" })), false, "never an older one");
  assert.equal(showUpdateButton(state({ installed: null, newest: "0.5.1" })), false, "nothing installed to update");
  const newer = state({ newest: "0.5.1" });
  assert.equal(reinNewer(newer), "0.5.1");
  assert.equal(showUpdateButton(newer), true);
  assert.equal(reinAsk(newer), "0.5.1");
  const later = state({ newest: "0.5.1" }, { rein: "0.5.1" });
  assert.equal(reinAsk(later), null, "Later: this version does not ask again");
  assert.equal(showUpdateButton(later), true, "the button stays");
  assert.equal(reinAsk(state({ newest: "0.5.2" }, { rein: "0.5.1" })), "0.5.2", "a newer version asks once more");
  assert.equal(reinAskText("0.5.1"), "Rein 0.5.1 is available. Your projects keep running during the update.");
});

test("the window's row offers nothing until its own update exists", () => {
  assert.equal(appNewer(state({}, {}, { newest: "0.2.0" })), null, "mode none: no offer");
  assert.equal(appNewer(state({}, {}, { newest: "0.2.0", mode: "in_place" })), "0.2.0");
  assert.equal(appNewer(state({}, {}, { newest: "0.1.0", mode: "in_place" })), null, "no same version");
});

test("the Update dialog has two rows: Rein and the window", () => {
  assert.deepEqual(updateRows(state({ newest: "0.5.1", checked_at: 1 })), [
    { name: "Rein", from: "0.5.0", to: "0.5.1", note: "newer release" },
    { name: "rein-app window", from: "0.1.0", to: null, note: "no newer window release" },
  ]);
  assert.equal(updateRows(state({ newest: "0.5.0", checked_at: 1 }))[0].note, "up to date");
  assert.equal(updateRows(state({ offline: true }))[0].note, "the release server cannot be reached");
  assert.equal(updateRows(state())[0].note, "not checked yet");
  assert.deepEqual(
    updateRows(null).map((r) => r.from),
    ["not installed", "unknown"],
    "before the first state",
  );
});

test("a changed installed Rein asks every open tab again, the active tab first", () => {
  assert.equal(installedChanged(null, state()), false, "the first state is no change");
  assert.equal(installedChanged(state(), state()), false);
  assert.equal(installedChanged(state(), state({ newest: "0.5.1" })), false, "only the installed version counts");
  assert.equal(installedChanged(state(), state({ installed: "0.5.1" })), true);
  assert.deepEqual(recheckOrder(4, 2), [2, 0, 1, 3]);
  assert.deepEqual(recheckOrder(3, 0), [0, 1, 2]);
  assert.deepEqual(recheckOrder(2, -1), [0, 1], "no current tab");
  assert.deepEqual(recheckOrder(0, 0), []);
});
