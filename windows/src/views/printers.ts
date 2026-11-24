// The Printers card: every printer Windows has (the PDF and fax ones left out), the ones with a
// problem first and in red, a way to ignore one that is always off, and "Check now".

import { h, clear, dot } from "./dom";
import { Bridge } from "../core/bridge";
import { PRINTERS_COLOR, PRINTERS_ID, Printers, problemLabel, type PrinterView, type PrintersReport } from "../core/printers";
import { pillDefinition } from "../core/pills";
import { State } from "../core/state";
import { language, t } from "../i18n/i18n";

export interface PrintersCard {
  el: HTMLElement;
  sync(): void;
}

const OK_GREEN = "#25D366";
const BAD_RED = "#F4505E";
const QUIET_GREY = "#6B7078";

async function apply(change: Promise<PrintersReport | null>) {
  const report = await change;
  if (!report) return;
  Printers.report = report;
  State.notify();
}

export function buildPrintersCard(): PrintersCard {
  const count = h("span", { class: "wa-unread" });
  const head = h("div", { class: "int-head" }, dot(PRINTERS_COLOR, 7), h("b", { text: pillDefinition(PRINTERS_ID)?.name ?? "Impressoras" }), count);
  const body = h("div", { class: "pr-list" });
  const check = h("button", { class: "link-btn pr-check" });
  check.addEventListener("click", () => void apply(Bridge.printersRefresh()));
  const el = h("div", { class: "int-card pr-card" }, head, body, check);

  function row(p: PrinterView): HTMLElement {
    const color = p.ignored ? QUIET_GREY : p.problem ? BAD_RED : OK_GREEN;
    const link = h("button", { class: "link-btn pr-ignore", text: p.ignored ? t("Stop ignoring") : t("Ignore") });
    link.addEventListener("click", () => void apply(p.ignored ? Bridge.printersUnignore(p.name) : Bridge.printersIgnore(p.name)));
    return h("div", { class: p.problem && !p.ignored ? "pr-row bad" : "pr-row" },
      dot(color, 6),
      h("span", { class: "pr-name", text: p.name }),
      h("span", { class: "pr-state", text: p.problem ? problemLabel(p.problem) : "" }),
      link,
    );
  }

  let key = "";

  function sync() {
    const r = Printers.report;
    const next = [
      language(), r.checked, r.failed, r.problems,
      ...r.printers.map((p) => `${p.name}|${p.problem ?? ""}|${p.ignored}`),
    ].join("~");
    check.textContent = t("Check now");
    if (next === key) return;
    key = next;
    count.textContent = r.problems > 0 ? String(r.problems) : "";
    count.style.display = r.problems > 0 ? "" : "none";
    clear(body);
    if (!r.checked) {
      body.append(h("div", { class: "wa-empty", text: t("Looking at the printers…") }));
    } else if (r.failed && r.printers.length === 0) {
      body.append(h("div", { class: "wa-note err", text: t("Could not read the printers.") }));
    } else if (r.printers.length === 0) {
      body.append(h("div", { class: "wa-empty", text: t("No printers found") }));
    } else {
      if (r.problems === 0) body.append(h("div", { class: "pr-fine", text: t("All printers are working") }));
      body.append(...r.printers.map(row));
    }
  }

  return { el, sync };
}
