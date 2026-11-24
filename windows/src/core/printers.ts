// The Printers pill on the page: what src-tauri/src/printers.rs sees of the printers installed in
// Windows. A printer that stopped warns once; the card lists them all.

import { t } from "../i18n/i18n";

export const PRINTERS_ID = "integration_printers";
export const PRINTERS_COLOR = "#A78BFA";

export type PrinterProblem =
  | "offline" | "paperOut" | "paperJam" | "tonerOut" | "doorOpen" | "service" | "binFull" | "error" | "stuckJob";

export interface PrinterView {
  name: string;
  problem: PrinterProblem | null;
  ignored: boolean;
}

/** Report in src-tauri/src/printers.rs. */
export interface PrintersReport {
  /** The pill is on and looking. */
  on: boolean;
  /** At least one look has been made. */
  checked: boolean;
  /** The last look could not read Windows' printers. */
  failed: boolean;
  /** Problems first, then by name. */
  printers: PrinterView[];
  /** Printers with a problem that are not ignored. */
  problems: number;
}

export interface PrintersAlert {
  name: string;
  problem: PrinterProblem;
}

export const IDLE_PRINTERS: PrintersReport = { on: false, checked: false, failed: false, printers: [], problems: 0 };

/** The page's copy of what Rust sees. */
export const Printers = {
  report: { ...IDLE_PRINTERS } as PrintersReport,
};

export function problemLabel(problem: PrinterProblem): string {
  switch (problem) {
    case "offline": return t("Offline");
    case "paperOut": return t("Out of paper");
    case "paperJam": return t("Paper jam");
    case "tonerOut": return t("Out of toner or ink");
    case "doorOpen": return t("Door open");
    case "service": return t("Needs service");
    case "binFull": return t("Output tray full");
    case "stuckJob": return t("Stuck print job");
    default: return t("Printer error");
  }
}
