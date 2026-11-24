// Settings → Modes: a mode is the set of pills the island shows (up to MAX_DECLARED, the main tool
// is always on). Work, Rest and Music come first; each can be edited, and new ones added. One
// click on "Use this mode" shows its pills, through the same path as the Active pills page.
// Nothing here changes what a pill does: a pill that is not in the mode is simply not shown, so
// it asks for nothing and reads nothing while it is off.

import { h } from "../views/dom";
import { t } from "../i18n/i18n";
import { MAX_DECLARED, availablePills, sanitizeDeclared } from "../core/pills";
import type { Mode, Settings } from "../core/state";

export interface ModesHost {
  settings(): Settings;
  /** Shows these pills and keeps the mode as the one in use (declaredChanged in main.ts). */
  apply(pills: string[], modeId: string): void;
  save(): void;
  toggle(on: boolean, onChange: (v: boolean) => void): HTMLElement;
}

/** What a first visit starts with: the pills each mode is about, where this build has them. */
export function seedModes(offered: ReadonlySet<string>): Mode[] {
  const only = (ids: string[]) => ids.filter((id) => offered.has(id));
  return [
    { id: "work", name: t("Work"), pills: only(["integration_whatsapp", "integration_mail", "integration_printers"]) },
    { id: "rest", name: t("Rest"), pills: [] },
    { id: "music", name: t("Music"), pills: only(["integration_ytmusic"]).length ? only(["integration_ytmusic"]) : only(["integration_spotify"]) },
  ];
}

export function modesSection(host: ModesHost): HTMLElement {
  const list = h("div", { style: "display:flex;flex-direction:column;gap:14px" });
  const addBtn = h("button", { text: t("New mode") });

  function shownPills(mode: Mode): string[] {
    const s = host.settings();
    return sanitizeDeclared({ mainPill: s.mainPill, activeIntegrations: mode.pills }).activeIntegrations;
  }

  /** The mode in use follows its own edits at once; the others wait for their click. */
  function edited(mode: Mode) {
    const s = host.settings();
    if (s.activeMode === mode.id) host.apply(shownPills(mode), mode.id);
    else host.save();
    draw();
  }

  function card(mode: Mode): HTMLElement {
    const s = host.settings();
    const inUse = s.activeMode === mode.id;
    const defs = availablePills().filter((p) => p.id !== s.mainPill);
    const name = h("input", {
      type: "text", maxlength: "40", placeholder: t("Mode name"), autocomplete: "off", spellcheck: "false",
      style: "flex:1 1 auto;min-width:0",
    }) as HTMLInputElement;
    name.value = mode.name;
    name.addEventListener("change", () => {
      mode.name = name.value.trim() || mode.name;
      host.save();
      draw();
    });

    const actions: HTMLElement[] = [];
    if (inUse) {
      actions.push(h("span", { class: "state", text: t("In use") }));
    } else {
      actions.push(h("button", {
        class: "primary", text: t("Use this mode"), onclick: () => {
          host.apply(shownPills(mode), mode.id);
          draw();
        },
      }));
    }
    if (s.modes.length > 1) {
      actions.push(h("button", {
        class: "danger", text: t("Delete"), onclick: () => {
          s.modes = s.modes.filter((m) => m.id !== mode.id);
          if (inUse) s.activeMode = "";
          host.save();
          draw();
        },
      }));
    }

    const chosen = mode.pills.map((id) => defs.find((d) => d.id === id)?.name).filter(Boolean);
    const picker = h("details", {},
      h("summary", { text: `${chosen.length}/${MAX_DECLARED} · ${chosen.join(" · ") || "—"}` }),
      ...defs.map((def) => {
        const on = mode.pills.includes(def.id);
        const full = !on && mode.pills.length >= MAX_DECLARED;
        const sw = host.toggle(on, (v) => {
          if (v && mode.pills.length >= MAX_DECLARED) {
            draw();
            return;
          }
          mode.pills = v ? [...mode.pills, def.id] : mode.pills.filter((x) => x !== def.id);
          edited(mode);
        });
        if (full) sw.setAttribute("disabled", "");
        return h("div", { class: "row", style: full ? "opacity:.5" : "" },
          h("i", { class: "dot", style: `background:${def.color}` }),
          h("label", { text: def.name }),
          sw,
        );
      }),
    );
    // Opened again after each redraw, so ticking a few pills is not interrupted.
    if (openId === mode.id) picker.setAttribute("open", "");
    picker.addEventListener("toggle", () => {
      if (picker.open) openId = mode.id;
      else if (openId === mode.id) openId = "";
    });

    return h("div", { style: "display:flex;flex-direction:column;gap:8px" },
      h("div", { class: "row" }, name, ...actions),
      picker,
    );
  }

  let openId = "";

  function draw() {
    const s = host.settings();
    if (s.modes.length === 0) {
      s.modes = seedModes(new Set(availablePills().map((p) => p.id)));
      host.save();
    }
    list.replaceChildren(...s.modes.map(card));
  }

  addBtn.addEventListener("click", () => {
    const s = host.settings();
    const mode: Mode = { id: `mode-${Date.now().toString(36)}`, name: t("New mode"), pills: [] };
    s.modes = [...s.modes, mode];
    openId = mode.id;
    host.save();
    draw();
  });

  draw();
  return h("section", {},
    h("h2", {}, h("span", { text: t("Modes") })),
    h("div", { class: "hint", text: t("Up to {max} pills per mode. The main tool is always on.", { max: MAX_DECLARED }) }),
    list,
    h("div", { class: "row" }, addBtn),
  );
}
