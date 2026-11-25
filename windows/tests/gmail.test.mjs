// The Gmail pill's page rules (src/core/gmail.ts).

import { test } from "node:test";
import assert from "node:assert/strict";

const { IDLE_GMAIL, gmailAlertLine, gmailCount, gmailSender, gmailView } = await import("../src/core/gmail.ts");

test("the card shows where the inbox stands", () => {
  assert.equal(gmailView(IDLE_GMAIL), "starting");
  assert.equal(gmailView({ ...IDLE_GMAIL, window: true }), "starting", "no report yet");
  assert.equal(gmailView({ ...IDLE_GMAIL, window: true, checked: true }), "signIn");
  assert.equal(gmailView({ ...IDLE_GMAIL, window: true, checked: true, loggedIn: true, failed: true }), "unreadable");
  assert.equal(gmailView({ ...IDLE_GMAIL, window: true, checked: true, loggedIn: true }), "inbox");
});

test("who wrote, and the line the pill wears when mail arrives", () => {
  const m = { id: "1", from: "", address: "ana@example.com", subject: "", snippet: "", at: 0, link: "" };
  assert.equal(gmailSender(m), "ana@example.com");
  assert.equal(gmailSender({ ...m, from: "Ana" }), "Ana");
  const many = (n) => `${n} new`;
  assert.equal(gmailAlertLine({ from: "Ana", subject: "Oi", count: 1 }, many), "Ana: Oi");
  assert.equal(gmailAlertLine({ from: "", subject: "Oi", count: 1 }, many), "Oi");
  assert.equal(gmailAlertLine({ from: "Ana", subject: "Oi", count: 3 }, many), "3 new");
  assert.equal(gmailCount(7), "7");
  assert.equal(gmailCount(12838), "99+");
});
