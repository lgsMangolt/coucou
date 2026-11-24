// The island's quick access to what the owner added: the mode switcher in the header, and under the
// pills a row of the options that are not in the mode — most used first — one click turns one on.
// A mode that is full gives the place of the pill unused the longest, and says so, with an undo.
// What is never used goes away by itself (src/core/adapt.ts), and says so too.

import { h, clear, dot } from "./dom";
import { Usage, UNUSED_DAYS, visibleChips } from "../core/adapt";
import { Swap, applyMode, declaredNow, dismissNotice, enableFeature, undoNotice, undoSwap } from "../core/adaptive";
import { ensureModes, featuresHere } from "../core/modes";
import { Bridge } from "../core/bridge";
import { pillDefinition } from "../core/pills";
import { State } from "../core/state";
import { language, t, tl } from "../i18n/i18n";

/** The most modes the header has room for. */
const MAX_SHOWN = 4;

export interface StripPart {
  el: HTMLElement;
  sync(): void;
}

const nameOf = (id: string) => pillDefinition(id)?.name ?? id;

// ── In the header: Work · Rest · Music ───────────────────────────────────────

export function buildModeSwitch(blip: () => void): StripPart {
  const el = h("div", { class: "modes", role: "group", title: tl("Modes") });
  let key = "";

  function sync() {
    const s = State.settings;
    if (ensureModes(s)) void Bridge.saveSettings(s);
    // The one in use is always shown, even beyond the first few.
    const shown = s.modes.slice(0, MAX_SHOWN);
    const current = s.modes.find((m) => m.id === s.activeMode);
    if (current && !shown.includes(current)) shown[MAX_SHOWN - 1] = current;
    const next = [language(), s.activeMode, ...shown.map((m) => `${m.id}|${m.name}`)].join("~");
    if (next === key) return;
    key = next;
    clear(el);
    for (const mode of shown) {
      const on = mode.id === s.activeMode;
      const btn = h("button", { class: on ? "mode on" : "mode", text: mode.name, title: mode.name });
      btn.setAttribute("aria-pressed", String(on));
      btn.addEventListener("click", () => {
        if (mode.id === State.settings.activeMode) return;
        blip();
        applyMode(mode);
      });
      el.append(btn);
    }
  }

  return { el, sync };
}

// ── Under the pills: what else can be turned on ──────────────────────────────

export function buildFeatureChips(host: { setFocus(id: string): void; blip(): void }): StripPart {
  const el = h("div", { class: "chips" });
  let key = "";

  function sync() {
    const s = State.settings;
    const now = Date.now();
    const notice = Usage.data.notices[Usage.data.notices.length - 1];
    const swap = Swap.current;
    const inUse = new Set([...declaredNow(), s.mainPill]);
    const options = visibleChips(featuresHere().filter((id) => !inUse.has(id)), Usage.data, now);
    const next = [
      language(), swap ? `${swap.added}>${swap.replaced}` : "", notice ? `${notice.pill}|${notice.mode}|${notice.at}` : "", ...options,
    ].join("~");
    el.style.display = swap || notice || options.length ? "" : "none";
    if (next === key) return;
    key = next;
    clear(el);

    if (swap) {
      el.append(
        h("span", { class: "chips-note", text: t("{name} took the place of {other}", { name: nameOf(swap.added), other: nameOf(swap.replaced) }) }),
        h("button", { class: "link-btn", text: t("Undo"), onclick: () => undoSwap() }),
      );
      return;
    }
    if (notice) {
      const mode = s.modes.find((m) => m.id === notice.mode)?.name ?? "";
      el.append(
        h("span", { class: "chips-note", text: t("{name} left {mode}: unused for {days} days", { name: nameOf(notice.pill), mode, days: UNUSED_DAYS }) }),
        h("button", { class: "link-btn", text: t("Undo"), onclick: () => undoNotice(notice) }),
        h("button", { class: "link-btn", text: t("OK"), onclick: () => dismissNotice(notice) }),
      );
      return;
    }
    el.append(h("span", { class: "chips-label", text: t("Turn on") }));
    for (const id of options) {
      const def = pillDefinition(id);
      const chip = h("button", { class: "chip", title: t("Turn on {name}", { name: nameOf(id) }) },
        dot(def?.color ?? "#9398a1", 6),
        h("span", { text: nameOf(id) }),
      );
      chip.addEventListener("click", () => {
        host.blip();
        enableFeature(id, host.setFocus);
      });
      el.append(chip);
    }
  }

  return { el, sync };
}
