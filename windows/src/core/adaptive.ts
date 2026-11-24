// The island adapting to its owner: counts what is opened, takes out what is not used, offers what
// is, and changes mode. The rules are in ./adapt.ts and ./modes.ts; src-tauri/src/usage.rs keeps
// the counts. Everything here is the decision applied to the settings, always with a way back.

import { Usage, stalePills, withFeature, type UsageData, type UsageNotice } from "./adapt";
import { Bridge } from "./bridge";
import { activeMode, featuresHere, modePills } from "./modes";
import { sanitizeDeclared } from "./pills";
import { State } from "./state";
import type { Mode } from "./state";

const OPENED_AGAIN_MS = 5 * 60_000;
const ADAPT_EVERY_MS = 6 * 3600_000;
const SWAP_NOTE_MS = 8_000;

/** The pills on the island now, besides the main one. */
export function declaredNow(): string[] {
  return sanitizeDeclared(State.settings).activeIntegrations;
}

/** A pill that took another one's place just now, and how to put things back. */
export const Swap = {
  current: null as null | { added: string; replaced: string; prevSet: string[]; prevModePills: string[] | null; until: number },
};

function take(data: UsageData | null) {
  if (!data) return;
  Usage.data = data;
  State.notify();
}

/** Starts the clock of the pills on the island, and of the options offered next to them. */
export function markSeen() {
  void Bridge.usageSeen([...declaredNow(), ...featuresHere()]).then(take);
}

/** A mode was picked: its pills are the island's now. */
export function applyMode(mode: Mode) {
  const s = State.settings;
  s.activeIntegrations = modePills(s, mode);
  s.activeMode = mode.id;
  State.loadIntegrationTasks();
  void Bridge.saveSettings(s);
  markSeen();
}

/** An option outside the mode was asked for: in when there is room, else in the place of the one unused the longest. */
export function enableFeature(id: string, focus: (id: string) => void) {
  const s = State.settings;
  const now = Date.now();
  const before = declaredNow();
  const { set, replaced } = withFeature(before, id, Usage.data, now);
  const mode = activeMode(s);
  Swap.current = replaced
    ? { added: id, replaced, prevSet: before, prevModePills: mode ? [...mode.pills] : null, until: now + SWAP_NOTE_MS }
    : null;
  s.activeIntegrations = set;
  // A mode is what the island shows: what was asked for is kept in it.
  if (mode) mode.pills = [...set];
  State.loadIntegrationTasks();
  void Bridge.saveSettings(s);
  void Bridge.usageTouch(id).then(take);
  markSeen();
  focus(id);
  if (replaced) {
    window.setTimeout(() => {
      if (Swap.current && Swap.current.until <= Date.now()) {
        Swap.current = null;
        State.notify();
      }
    }, SWAP_NOTE_MS + 200);
  }
}

export function undoSwap() {
  const swap = Swap.current;
  if (!swap) return;
  const s = State.settings;
  const mode = activeMode(s);
  s.activeIntegrations = swap.prevSet;
  if (mode && swap.prevModePills) mode.pills = swap.prevModePills;
  Swap.current = null;
  State.loadIntegrationTasks();
  void Bridge.saveSettings(s);
  State.notify();
}

/** A pill the island took out on its own comes back (the clock restarts: it is used again now). */
export function undoNotice(notice: UsageNotice) {
  const s = State.settings;
  const mode = s.modes.find((m) => m.id === notice.mode);
  const now = Date.now();
  if (mode) {
    mode.pills = withFeature(mode.pills, notice.pill, Usage.data, now).set;
    if (mode.id === s.activeMode) s.activeIntegrations = modePills(s, mode);
  }
  State.loadIntegrationTasks();
  void Bridge.saveSettings(s);
  void Bridge.usageTouch(notice.pill).then(take);
  void Bridge.usageDismiss(notice.pill, notice.mode).then(take);
}

export function dismissNotice(notice: UsageNotice) {
  void Bridge.usageDismiss(notice.pill, notice.mode).then(take);
}

/** Tools unused for two weeks leave the mode in use; the island says so (and `undoNotice` brings them back). */
export function adapt(now = Date.now()) {
  const s = State.settings;
  const mode = activeMode(s);
  if (!mode) return;
  const declared = declaredNow();
  const stale = stalePills(declared, Usage.data, now);
  if (stale.length === 0) return;
  s.activeIntegrations = declared.filter((id) => !stale.includes(id));
  mode.pills = mode.pills.filter((id) => !stale.includes(id));
  State.loadIntegrationTasks();
  void Bridge.saveSettings(s);
  for (const id of stale) void Bridge.usageNotice(id, mode.id).then(take);
}

const opened = new Map<string, number>();

/** The card of a pill is on screen: that is a use (once every few minutes at most). */
function countOpening() {
  if (State.mode !== "expanded" || State.view !== "overview") return;
  const id = State.focusId;
  if (!id || id === State.mainPillId || !declaredNow().includes(id)) return;
  const now = Date.now();
  if (now - (opened.get(id) ?? 0) < OPENED_AGAIN_MS) return;
  opened.set(id, now);
  void Bridge.usageTouch(id).then(take);
}

export function registerAdaptive() {
  void Bridge.usageGet().then((data) => {
    if (!data) return;
    Usage.data = data;
    markSeen();
    adapt();
    State.notify();
  });
  State.subscribe(countOpening);
  window.setInterval(() => adapt(), ADAPT_EVERY_MS);
}
