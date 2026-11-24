// The printers' reports → the page (src-tauri/src/printers.rs). A printer that just stopped lights
// the pill red with its name and the problem, the island shows itself and the error sound plays,
// once per problem; the pill wears how many printers are down, and goes back to idle when none is.

import { Bridge, onEvent } from "../core/bridge";
import { pillDefinition } from "../core/pills";
import { PRINTERS_ID, Printers, problemLabel, type PrintersAlert, type PrintersReport } from "../core/printers";
import { Sound } from "../core/sound";
import { State } from "../core/state";

export interface PrintersHost {
  /** The compact island, so the warning is seen. */
  reveal(): void;
}

function task() {
  return State.tasks.find((x) => x.id === PRINTERS_ID);
}

export function registerPrintersHandlers(island: PrintersHost) {
  void onEvent<PrintersReport>("printers", applyReport);
  void onEvent<PrintersAlert>("printers-alert", (a) => handleAlert(island, a));
  void Bridge.printersGet().then((r) => {
    if (r) applyReport(r);
  });
  State.subscribe(syncPillName);
}

/** "Impressoras · 2" while two are down, else the pill's name. */
export function syncPillName() {
  const t = task();
  if (!t) return;
  const base = pillDefinition(PRINTERS_ID)?.name ?? "Impressoras";
  const n = Printers.report.problems;
  const name = n > 0 ? `${base} · ${n}` : base;
  if (t.name !== name) t.name = name;
}

export function applyReport(report: PrintersReport) {
  Printers.report = report;
  const t = task();
  if (t && report.problems === 0) {
    if (t.state === "error") {
      t.state = "idle";
      t.steps = [];
      t.stepIndex = 0;
    }
    if (t.pillBadge === "error") t.pillBadge = null;
  }
  syncPillName();
  State.notify();
}

export function handleAlert(island: PrintersHost, alert: PrintersAlert) {
  const t = task();
  if (!t || State.paused) return;
  t.state = "error";
  t.steps = [`${alert.name}: ${problemLabel(alert.problem)}`];
  t.stepIndex = 0;
  if (State.focusId !== PRINTERS_ID) t.pillBadge = "error";
  Sound.play("error");
  island.reveal();
  State.notify();
}
