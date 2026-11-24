// How the island adapts to its owner (src/core/adapt.ts) and the modes it works on (src/core/modes.ts).

import { test } from "node:test";
import assert from "node:assert/strict";
import {
  CHIP_DAYS, UNUSED_DAYS, chipOrder, pillToReplace, stalePills, visibleChips, withFeature,
} from "../src/core/adapt.ts";
import { FEATURE_IDS, WATCH_IDS, seedModes } from "../src/core/modes.ts";

const DAY = 86_400_000;
const NOW = 1_800_000_000_000;
const use = (uses, daysAgoLast, daysAgoSince = daysAgoLast) => ({
  uses,
  last: daysAgoLast == null ? 0 : NOW - daysAgoLast * DAY,
  since: NOW - daysAgoSince * DAY,
});
const usage = (pills) => ({ pills, notices: [] });

test("a tool unused for two weeks goes; one used lately, or one that warns by itself, stays", () => {
  const u = usage({
    integration_tasks: use(3, UNUSED_DAYS + 1, 40),
    integration_pad: use(5, 2, 40),
    integration_ytmusic: use(0, null, UNUSED_DAYS + 3),
    integration_printers: use(0, null, 60),
  });
  const set = ["integration_tasks", "integration_pad", "integration_ytmusic", "integration_printers", "integration_whatsapp"];
  assert.deepEqual(stalePills(set, u, NOW), ["integration_tasks", "integration_ytmusic"]);
  assert.ok(WATCH_IDS.has("integration_printers") && WATCH_IDS.has("integration_whatsapp") && WATCH_IDS.has("integration_mail"));
});

test("a pill the island has never seen gets its two weeks from now, not from the start of time", () => {
  assert.deepEqual(stalePills(["integration_tasks"], usage({}), NOW), []);
  assert.deepEqual(stalePills(["integration_tasks"], usage({ integration_tasks: use(0, null, UNUSED_DAYS - 1) }), NOW), []);
  assert.deepEqual(stalePills(["integration_tasks"], usage({ integration_tasks: use(0, null, UNUSED_DAYS) }), NOW), ["integration_tasks"]);
});

test("a full set makes room by dropping the tool unused the longest, never a pill that warns first", () => {
  const u = usage({
    integration_tasks: use(2, 3),
    integration_pad: use(1, 9),
    integration_ytmusic: use(1, 5),
    integration_whatsapp: use(0, null, 90),
  });
  const full = ["integration_whatsapp", "integration_tasks", "integration_pad", "integration_ytmusic"];
  assert.equal(pillToReplace(full, u, NOW), "integration_pad");
  assert.deepEqual(withFeature(full, "integration_printers", u, NOW), {
    set: ["integration_whatsapp", "integration_tasks", "integration_ytmusic", "integration_printers"],
    replaced: "integration_pad",
  });
  // Room left: added, nothing dropped. Already in: nothing changes.
  assert.deepEqual(withFeature(["integration_tasks"], "integration_pad", u, NOW), { set: ["integration_tasks", "integration_pad"], replaced: null });
  assert.deepEqual(withFeature(full, "integration_tasks", u, NOW), { set: full, replaced: null });
  // Only pills that warn left: the longest unused of them gives way.
  const onlyWatch = ["integration_whatsapp", "integration_mail", "integration_printers"];
  assert.equal(pillToReplace(onlyWatch, usage({ integration_mail: use(1, 1), integration_whatsapp: use(1, 8), integration_printers: use(1, 3) }), NOW), "integration_whatsapp");
  assert.equal(pillToReplace([], u, NOW), null);
});

test("the options outside the mode come most used first, then the given order", () => {
  const u = usage({ integration_pad: use(9, 1), integration_tasks: use(9, 4), integration_ytmusic: use(2, 1) });
  assert.deepEqual(
    chipOrder(["integration_tasks", "integration_printers", "integration_pad", "integration_ytmusic", "integration_mail"], u),
    ["integration_pad", "integration_tasks", "integration_ytmusic", "integration_printers", "integration_mail"],
  );
});

test("an option offered for a month and never used stops being offered; the rest stay, three at most", () => {
  const u = usage({
    integration_pad: use(0, null, CHIP_DAYS + 1),
    integration_tasks: use(0, null, CHIP_DAYS - 1),
    integration_ytmusic: use(4, 2, 80),
  });
  const shown = visibleChips(["integration_pad", "integration_tasks", "integration_ytmusic", "integration_printers", "integration_mail", "integration_whatsapp"], u, NOW);
  assert.deepEqual(shown, ["integration_ytmusic", "integration_tasks", "integration_printers"]);
  assert.ok(!shown.includes("integration_pad"));
  assert.deepEqual(visibleChips(["integration_pad"], usage({}), NOW), ["integration_pad"], "never seen: offered");
});

test("the modes it starts with: Work watches, Rest is empty, Music plays", () => {
  const all = new Set(FEATURE_IDS.concat(["integration_spotify"]));
  const [work, rest, music] = seedModes(all);
  assert.deepEqual(work.pills, ["integration_whatsapp", "integration_mail", "integration_printers"]);
  assert.deepEqual(rest.pills, ["integration_whatsapp2"], "Rest keeps only the second WhatsApp");
  assert.deepEqual(music.pills, ["integration_ytmusic"]);
  // Where this build has no YouTube Music, Music falls back to Spotify.
  const linux = seedModes(new Set(["integration_whatsapp", "integration_spotify"]));
  assert.deepEqual(linux[2].pills, ["integration_spotify"]);
  assert.deepEqual(linux[0].pills, ["integration_whatsapp"]);
  assert.deepEqual(linux[1].pills, [], "no second WhatsApp offered: Rest is empty");
});
