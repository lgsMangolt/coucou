// Modes (Settings → Modes): a mode is a set of pills (up to MAX_DECLARED, the main tool is always on).
// Shared by the Settings page and the island's mode switcher.

import { availablePills, sanitizeDeclared } from "./pills";
import type { Mode, Settings } from "./state";
import { t } from "../i18n/i18n";

/** The options a mode is about, in the order they are offered. */
export const FEATURE_IDS: readonly string[] = [
  "integration_whatsapp", "integration_whatsapp2", "integration_mail", "integration_ytmusic",
  "integration_printers", "integration_tasks", "integration_pad",
];

/** Pills that warn on their own: they are never taken out of a mode for lack of use. */
export const WATCH_IDS: ReadonlySet<string> = new Set([
  "integration_whatsapp", "integration_whatsapp2", "integration_mail", "integration_printers",
]);

/** What a first visit starts with: the pills each mode is about, where this build has them. */
export function seedModes(offered: ReadonlySet<string>): Mode[] {
  const only = (ids: string[]) => ids.filter((id) => offered.has(id));
  const music = only(["integration_ytmusic"]);
  return [
    { id: "work", name: t("Work"), pills: only(["integration_whatsapp", "integration_mail", "integration_printers"]) },
    { id: "rest", name: t("Rest"), pills: only(["integration_whatsapp2"]) },
    { id: "music", name: t("Music"), pills: music.length ? music : only(["integration_spotify"]) },
  ];
}

/** The modes, seeded on first use. True when they were just created (and want saving). */
export function ensureModes(settings: Settings): boolean {
  if (settings.modes.length > 0) return false;
  settings.modes = seedModes(new Set(availablePills().map((p) => p.id)));
  return true;
}

/** The pills a mode shows here: the ones this build offers, without the main tool, at most four. */
export function modePills(settings: Settings, mode: Mode): string[] {
  return sanitizeDeclared({ mainPill: settings.mainPill, activeIntegrations: mode.pills }).activeIntegrations;
}

export function activeMode(settings: Settings): Mode | undefined {
  return settings.modes.find((m) => m.id === settings.activeMode);
}

/** The options this build has, for the chips and the like. */
export function featuresHere(): string[] {
  const offered = new Set(availablePills().map((p) => p.id));
  return FEATURE_IDS.filter((id) => offered.has(id));
}
