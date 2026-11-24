// WhatsApp's reports → the page (src-tauri/src/whatsapp). The parts of WhatsAppController.swift that
// touch the island: a new message lights the pill with the conversation's name (never the text),
// Mochi smiles and the "pop" sound plays — once for a burst, not ten times — and the pill goes back
// to idle a minute later; the pill gets its small red dot while the gateway is down.

import { Bridge, onEvent } from "../core/bridge";
import { Sound } from "../core/sound";
import { State } from "../core/state";
import { waPillId, waStore, type WaIslandView } from "../core/whatsapp";
import type { BotEmoteName } from "../core/layout";

export interface WhatsAppHost {
  /** The compact island, so the badge is seen. */
  reveal(): void;
  emote(name: BotEmoteName): void;
}

export type WaPillEvent = { kind: "message"; name: string } | { kind: "read" } | { kind: "reset" };

/** A burst of messages is one reaction. */
const ANNOUNCE_GAP_MS = 4_000;
const RESET_AFTER_MS = 60_000;

/** Per account: when the last burst was announced, and the timer that sends the pill back to idle. */
const lastAnnounce: number[] = [-Infinity, -Infinity];
const resetTimer: (number | null)[] = [null, null];

/** `account`: 0 is the first WhatsApp, 1 the second (its own events, its own pill). */
export function registerWhatsAppHandlers(island: WhatsAppHost, account = 0) {
  const events = account ? "whatsapp2" : "whatsapp";
  void onEvent<WaIslandView>(events, (view) => applyWhatsApp(view, account));
  void onEvent<WaPillEvent>(`${events}-pill`, (e) => handlePillEvent(island, e, performance.now(), account));
  void Bridge.whatsappIsland(account).then((view) => {
    if (view) applyWhatsApp(view, account);
  });
}

function task(account: number) {
  return State.tasks.find((x) => x.id === waPillId(account));
}

/** A report from Rust: the card redraws, the pill's red dot follows the gateway. */
export function applyWhatsApp(view: WaIslandView, account = 0) {
  const store = waStore(account);
  const wasDown = store.view.down;
  store.view = view;
  const t = task(account);
  if (t && view.down !== wasDown) {
    if (view.down) t.pillBadge = "error";
    else if (t.pillBadge === "error") t.pillBadge = null;
  }
  State.notify();
}

export function handlePillEvent(island: WhatsAppHost, e: WaPillEvent, now = performance.now(), account = 0) {
  const t = task(account);
  if (!t) return;
  switch (e.kind) {
    case "message": {
      if (State.paused) return;
      t.state = "finished";
      t.steps = [e.name];
      t.stepIndex = 0;
      if (State.focusId !== waPillId(account)) t.pillBadge = "finished";
      if (now - lastAnnounce[account] > ANNOUNCE_GAP_MS) {
        lastAnnounce[account] = now;
        island.emote("happy");
        Sound.play("pop");
      }
      island.reveal();
      const pending = resetTimer[account];
      if (pending != null) window.clearTimeout(pending);
      resetTimer[account] = window.setTimeout(() => {
        resetTimer[account] = null;
        const x = task(account);
        if (!x || x.state !== "finished") return;
        x.state = "idle";
        x.steps = [];
        x.stepIndex = 0;
        if (x.pillBadge === "finished") x.pillBadge = null;
        State.notify();
      }, RESET_AFTER_MS);
      break;
    }
    case "read":
      if (t.pillBadge === "finished") t.pillBadge = null;
      break;
    case "reset": {
      const pending = resetTimer[account];
      if (pending != null) window.clearTimeout(pending);
      resetTimer[account] = null;
      t.state = "idle";
      t.steps = [];
      t.stepIndex = 0;
      t.pillBadge = null;
      break;
    }
  }
  State.notify();
}
