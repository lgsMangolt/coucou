// The YouTube Music pill on the page: what src-tauri/src/ytmusic.rs reports of the player in its
// own window (the song, who sings it, playing or not, liked or not, the results of a search).

export const YTMUSIC_ID = "integration_ytmusic";
/** YouTube's red. */
export const YTMUSIC_RED = "#FF0033";

export interface YtHit {
  /** Which result of the page this is (what `ytmusicPlay` takes). */
  index: number;
  title: string;
  subtitle: string;
}

/** Player in src-tauri/src/ytmusic.rs. */
export interface YtMusicState {
  /** The player window exists: the pill is on. */
  window: boolean;
  loggedIn: boolean;
  title: string;
  artist: string;
  album: string;
  playing: boolean;
  like: "like" | "dislike" | "none";
  /** Seconds at `positionAt` (Unix ms); while playing it runs on from there. */
  position: number;
  positionAt: number;
  duration: number;
  results: YtHit[];
}

export type YtAction = "playPause" | "next" | "previous" | "like" | "dislike";

export const IDLE_YTMUSIC: YtMusicState = {
  window: false, loggedIn: false, title: "", artist: "", album: "", playing: false, like: "none",
  position: 0, positionAt: 0, duration: 0, results: [],
};

/** The page's copy of the player. */
export const YtMusic = {
  state: { ...IDLE_YTMUSIC } as YtMusicState,
};

/** The position now: extrapolated while playing, capped at the end. */
export function ytPosition(s: YtMusicState, nowMs: number): number {
  const elapsed = s.playing && s.positionAt > 0 ? Math.max(0, (nowMs - s.positionAt) / 1000) : 0;
  const p = s.position + elapsed;
  return s.duration > 0 ? Math.min(p, s.duration) : p;
}

/** What the card shows: starting, waiting for the sign-in, or the player. */
export function ytView(s: YtMusicState): "starting" | "signIn" | "player" {
  if (!s.window) return "starting";
  return s.loggedIn ? "player" : "signIn";
}
