// How the island adapts to its owner: what is not used leaves, what is used comes forward. Pure
// rules; src-tauri/src/usage.rs keeps the counts and island/usage.ts applies the decisions.
//
// - A tool pill (not one that warns on its own) that has not been opened for UNUSED_DAYS leaves the
//   mode it is in. The island says so and offers to undo it.
// - A full mode makes room for a pill the user asks for by dropping the one unused the longest.
// - The options outside the mode are offered most-used first, and one that has been offered for
//   CHIP_DAYS without ever being used stops being offered (it stays in Settings → Modes).

import { MAX_DECLARED } from "./pills";
import { WATCH_IDS } from "./modes";

export const UNUSED_DAYS = 14;
export const CHIP_DAYS = 30;
const DAY = 86_400_000;

/** PillUse in src-tauri/src/usage.rs. */
export interface PillUse {
  uses: number;
  /** Unix ms of the last time it was opened, 0 when never. */
  last: number;
  /** Unix ms when the island first showed it. */
  since: number;
}

/** Notice in src-tauri/src/usage.rs: a pill the island took out of a mode on its own. */
export interface UsageNotice {
  pill: string;
  mode: string;
  at: number;
}

export interface UsageData {
  pills: Record<string, PillUse>;
  notices: UsageNotice[];
}

export const IDLE_USAGE: UsageData = { pills: {}, notices: [] };

/** The page's copy of what Rust keeps. */
export const Usage = {
  data: { pills: {}, notices: [] } as UsageData,
};

/** When a pill last showed a sign of use: the last time it was opened, else when it first appeared. */
export function lastSign(use: PillUse | undefined, now: number): number {
  if (!use) return now;
  return use.last || use.since || now;
}

/** Tool pills of the set that have been idle for UNUSED_DAYS. Pills that warn by themselves never are. */
export function stalePills(set: readonly string[], usage: UsageData, now: number): string[] {
  return set.filter((id) => !WATCH_IDS.has(id) && now - lastSign(usage.pills[id], now) >= UNUSED_DAYS * DAY);
}

/** The pill a new one takes the place of in a full set: tools before pills that warn, then the longest unused. */
export function pillToReplace(set: readonly string[], usage: UsageData, now: number): string | null {
  const ranked = [...set].sort(
    (a, b) =>
      Number(WATCH_IDS.has(a)) - Number(WATCH_IDS.has(b)) ||
      lastSign(usage.pills[a], now) - lastSign(usage.pills[b], now),
  );
  return ranked[0] ?? null;
}

/** The set with `id` in it: added when there is room, else in the place of the one unused the longest. */
export function withFeature(
  set: readonly string[],
  id: string,
  usage: UsageData,
  now: number,
  max = MAX_DECLARED,
): { set: string[]; replaced: string | null } {
  if (set.includes(id)) return { set: [...set], replaced: null };
  if (set.length < max) return { set: [...set, id], replaced: null };
  const out = pillToReplace(set, usage, now);
  return { set: [...set.filter((x) => x !== out), id], replaced: out };
}

/** The options outside the mode, most used first; the ones never used keep their given order. */
export function chipOrder(candidates: readonly string[], usage: UsageData): string[] {
  return candidates
    .map((id, index) => ({ id, index }))
    .sort(
      (a, b) =>
        (usage.pills[b.id]?.uses ?? 0) - (usage.pills[a.id]?.uses ?? 0) ||
        (usage.pills[b.id]?.last ?? 0) - (usage.pills[a.id]?.last ?? 0) ||
        a.index - b.index,
    )
    .map((x) => x.id);
}

/** The chips worth showing: most used first, without those offered for CHIP_DAYS and never used. */
export function visibleChips(candidates: readonly string[], usage: UsageData, now: number, limit = 3): string[] {
  const alive = candidates.filter((id) => {
    const u = usage.pills[id];
    return !(u && u.uses === 0 && u.since > 0 && now - u.since >= CHIP_DAYS * DAY);
  });
  return chipOrder(alive, usage).slice(0, limit);
}
