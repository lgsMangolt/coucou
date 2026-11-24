// YouTube Music's reports → the page (src-tauri/src/ytmusic.rs): the pill wears the song's title,
// and the card redraws from the player's state.

import { Bridge, onEvent } from "../core/bridge";
import { pillDefinition } from "../core/pills";
import { State } from "../core/state";
import { YTMUSIC_ID, YtMusic, type YtMusicState } from "../core/ytmusic";

export function registerYtMusicHandlers() {
  void onEvent<YtMusicState>("ytmusic", applyYtMusic);
  // Rust may have read the player before this page listened: ask once.
  void Bridge.ytmusicState().then((s) => {
    if (s) applyYtMusic(s);
  });
  State.subscribe(syncPillName);
}

export function applyYtMusic(next: YtMusicState) {
  YtMusic.state = next;
  syncPillName();
  State.notify();
}

/** The song's title, else the pill's name. */
export function syncPillName() {
  const task = State.tasks.find((x) => x.id === YTMUSIC_ID);
  if (!task) return;
  const name = YtMusic.state.title || (pillDefinition(YTMUSIC_ID)?.name ?? "YouTube Music");
  if (task.name !== name) task.name = name;
}
