// Gmail's reports → the page (src-tauri/src/gmail.rs). Mail that just arrived lights the pill
// with who wrote and the subject, the island shows itself and a sound plays; the pill wears how
// many messages are unread, and goes quiet again when none is.

import { Bridge, onEvent } from "../core/bridge";
import { GMAIL_ID, Gmail, gmailAlertLine, gmailCount, type GmailAlert, type GmailInbox } from "../core/gmail";
import { pillDefinition } from "../core/pills";
import { Sound } from "../core/sound";
import { State } from "../core/state";
import { t } from "../i18n/i18n";

export interface GmailHost {
  /** The compact island, so the warning is seen. */
  reveal(): void;
}

/** How long the pill keeps saying who wrote. The unread count stays on its name. */
const RESET_AFTER_MS = 60_000;
let resetTimer: number | null = null;

function task() {
  return State.tasks.find((x) => x.id === GMAIL_ID);
}

export function registerGmailHandlers(island: GmailHost) {
  void onEvent<GmailInbox>("gmail", applyInbox);
  void onEvent<GmailAlert>("gmail-alert", (a) => handleAlert(island, a));
  // Rust may have read the inbox before this page listened: ask once.
  void Bridge.gmailState().then((s) => {
    if (s) applyInbox(s);
  });
  State.subscribe(syncPillName);
}

/** "Gmail · 3" while three are unread, else the pill's name. */
export function syncPillName() {
  const x = task();
  if (!x) return;
  const base = pillDefinition(GMAIL_ID)?.name ?? "Gmail";
  const n = Gmail.inbox.unread;
  const name = n > 0 ? `${base} · ${gmailCount(n)}` : base;
  if (x.name !== name) x.name = name;
}

export function applyInbox(inbox: GmailInbox) {
  Gmail.inbox = inbox;
  const x = task();
  if (x && inbox.unread === 0) {
    if (x.state === "finished") {
      x.state = "idle";
      x.steps = [];
      x.stepIndex = 0;
    }
    if (x.pillBadge === "finished") x.pillBadge = null;
  }
  syncPillName();
  State.notify();
}

export function handleAlert(island: GmailHost, alert: GmailAlert) {
  const x = task();
  if (!x || State.paused) return;
  x.state = "finished";
  x.steps = [gmailAlertLine(alert, (n) => t("{n} new emails", { n }))];
  x.stepIndex = 0;
  if (State.focusId !== GMAIL_ID) x.pillBadge = "finished";
  Sound.play("pop");
  island.reveal();
  if (resetTimer != null) window.clearTimeout(resetTimer);
  resetTimer = window.setTimeout(() => {
    resetTimer = null;
    const y = task();
    if (!y || y.state !== "finished") return;
    y.state = "idle";
    y.steps = [];
    y.stepIndex = 0;
    State.notify();
  }, RESET_AFTER_MS);
  State.notify();
}
