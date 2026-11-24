// The Notepad card: one page of notes, saved as you type (a moment after the last key, and when
// you leave the field). Built once; the text is only replaced from outside while the field is not
// being used, so what you are typing is never overwritten.

import { h, dot } from "./dom";
import { Bridge } from "../core/bridge";
import { PAD_COLOR, PAD_ID, Personal } from "../core/personal";
import { pillDefinition } from "../core/pills";
import { t } from "../i18n/i18n";

export interface PadCardHost {
  /** The island takes the keyboard for the notes, and gives it back. */
  keyboard(on: boolean): void;
  /** Notes are being written: the island stays open. */
  holdOpen(on: boolean): void;
}

export interface PadCard {
  el: HTMLElement;
  sync(): void;
}

const SAVE_AFTER_MS = 500;

export function buildPadCard(host: PadCardHost): PadCard {
  const head = h("div", { class: "int-head" }, dot(PAD_COLOR, 7), h("b", { text: pillDefinition(PAD_ID)?.name ?? "Bloco de notas" }));
  const field = h("textarea", { class: "pad-field", spellcheck: "true", maxlength: "20000" }) as HTMLTextAreaElement;
  const el = h("div", { class: "int-card pad-card" }, head, field);

  let timer: number | null = null;
  let loaded = false;

  function save() {
    if (timer != null) window.clearTimeout(timer);
    timer = null;
    Personal.data.pad = field.value;
    void Bridge.personalSetPad(field.value);
  }

  field.addEventListener("input", () => {
    if (timer != null) window.clearTimeout(timer);
    timer = window.setTimeout(save, SAVE_AFTER_MS);
  });
  field.addEventListener("focus", () => {
    host.keyboard(true);
    host.holdOpen(true);
  });
  field.addEventListener("blur", () => {
    if (timer != null) save();
    host.keyboard(false);
    host.holdOpen(false);
  });
  field.addEventListener("keydown", (e) => {
    if (e.key === "Escape") {
      // Out of the field, not the island folding away.
      e.preventDefault();
      e.stopPropagation();
      field.blur();
    }
  });

  function sync() {
    field.placeholder = t("Write your notes here…");
    if (document.activeElement === field) return;
    // Taken from Rust once it has answered; after that only what this field saved.
    if (!loaded || field.value !== Personal.data.pad) {
      field.value = Personal.data.pad;
      loaded = true;
    }
  }

  return { el, sync };
}
