// The YouTube Music card: sign in once, then the song playing, its controls (previous, play or
// pause, next, like, dislike) and a search whose results play on a click. The search has the card
// to itself (the magnifier opens it, the arrow goes back): both do not fit the island's height at
// once. Built once and synced in place, so the search field keeps its text and the keyboard while
// the song moves on.

import { h, svg, clear } from "./dom";
import { ICONS } from "./icons";
import { Bridge } from "../core/bridge";
import { formatTime } from "../core/spotify";
import { YTMUSIC_RED, YtMusic, ytPosition, ytView, type YtAction, type YtHit } from "../core/ytmusic";
import { language, t, tl, type Msg } from "../i18n/i18n";

export interface YtMusicCardHost {
  /** The island takes the keyboard for the search field, and gives it back. */
  keyboard(on: boolean): void;
  /** A search is being typed: the island stays open. */
  holdOpen(on: boolean): void;
}

export interface YtMusicCard {
  el: HTMLElement;
  sync(): void;
}

export function buildYtMusicCard(host: YtMusicCardHost): YtMusicCard {
  const head = h("div", { class: "int-head" }, svg(ICONS.musicNote, 12, { fill: YTMUSIC_RED }), h("b", { text: "YouTube Music" }));

  // ── Not started, or waiting for the sign-in ──
  const message = h("div", { class: "yt-message" });
  const signIn = h("button", { class: "link-btn yt-signin" });
  signIn.addEventListener("click", () => void Bridge.ytmusicSignIn());
  const gate = h("div", { class: "yt-gate" }, message, signIn);

  // ── The player ──
  const title = h("div", { class: "yt-title" });
  const artist = h("div", { class: "yt-artist" });
  const time = h("div", { class: "yt-time" });
  const info = h("div", { class: "yt-info" }, title, artist, time);

  function control(path: string, action: YtAction, label: Msg, opts: { stroke?: number } = {}) {
    const btn = h("button", { class: "yt-btn", title: label }, svg(path, 14, opts.stroke ? { stroke: opts.stroke } : { fill: "currentColor" }));
    btn.addEventListener("click", () => void Bridge.ytmusicControl(action));
    return btn;
  }
  const prev = control(ICONS.backward, "previous", tl("Previous"));
  const playPause = h("button", { class: "yt-btn yt-play" });
  playPause.addEventListener("click", () => void Bridge.ytmusicControl("playPause"));
  const next = control(ICONS.forward, "next", tl("Next"));
  const dislike = control(ICONS.thumbDown, "dislike", tl("Dislike"), { stroke: 1.8 });
  const like = control(ICONS.thumbUp, "like", tl("Like"), { stroke: 1.8 });
  const find = h("button", { class: "yt-btn yt-find", title: tl("Search songs") }, svg(ICONS.magnifier, 12));
  const controls = h("div", { class: "yt-controls" }, dislike, prev, playPause, next, like, find);

  const input = h("input", { class: "yt-search", type: "text", spellcheck: "false", autocomplete: "off", maxlength: "100" }) as HTMLInputElement;
  const go = h("button", { class: "yt-go", title: tl("Search songs") }, svg(ICONS.arrowUp, 12, { fill: "#0B0C0E" }));
  const back = h("button", { class: "yt-back", title: tl("Back") }, svg(ICONS.chevronLeft, 10, { stroke: 2.4 }));
  const searchRow = h("div", { class: "yt-searchrow" }, back, input, go);
  const resultsBox = h("div", { class: "yt-results" });
  const hint = h("div", { class: "yt-hint" });

  /** The search has the card to itself while it is open. */
  let searching = false;
  function setSearching(on: boolean) {
    searching = on;
    sync();
    if (on) input.focus();
    else input.blur();
  }
  find.addEventListener("click", () => setSearching(true));
  back.addEventListener("click", () => setSearching(false));

  function search() {
    const query = input.value.trim();
    if (!query) return;
    void Bridge.ytmusicSearch(query);
  }
  go.addEventListener("click", search);
  input.addEventListener("focus", () => {
    host.keyboard(true);
    host.holdOpen(true);
  });
  input.addEventListener("blur", () => {
    host.keyboard(false);
    host.holdOpen(false);
  });
  input.addEventListener("keydown", (e) => {
    if (e.key === "Enter") {
      e.preventDefault();
      search();
    } else if (e.key === "Escape") {
      // Back to the song, not the island folding away.
      e.preventDefault();
      e.stopPropagation();
      setSearching(false);
    }
  });

  const now = h("div", { class: "yt-now" }, info, controls);
  const finder = h("div", { class: "yt-finder" }, searchRow, hint, resultsBox);
  const player = h("div", { class: "yt-player" }, now, finder);
  const el = h("div", { class: "int-card yt-card" }, head, gate, player);

  let resultsKey = "";

  function row(hit: YtHit): HTMLElement {
    const r = h("button", { class: "yt-hit" },
      h("span", { class: "yt-hit-title", text: hit.title }),
      h("span", { class: "yt-hit-sub", text: hit.subtitle }),
    );
    r.addEventListener("click", () => void Bridge.ytmusicPlay(hit.index));
    return r;
  }

  function sync() {
    const s = YtMusic.state;
    const view = ytView(s);
    gate.style.display = view === "player" ? "none" : "";
    player.style.display = view === "player" ? "" : "none";
    if (view !== "player") {
      signIn.style.display = view === "signIn" ? "" : "none";
      signIn.textContent = t("Sign in");
      message.textContent = view === "signIn"
        ? t("Sign in to YouTube Music to use it from here. A window opens: sign in with your Google account, then close it; the music keeps playing.")
        : t("Starting YouTube Music…");
      return;
    }
    now.style.display = searching ? "none" : "";
    finder.style.display = searching ? "" : "none";
    hint.textContent = t("Type a song or an artist and press Enter.");
    hint.style.display = s.results.length ? "none" : "";
    title.textContent = s.title || t("Nothing is playing");
    artist.textContent = [s.artist, s.album].filter(Boolean).join(" · ");
    time.textContent = s.title && s.duration > 0
      ? `${formatTime(ytPosition(s, Date.now()))} / ${formatTime(s.duration)}`
      : "";
    clear(playPause);
    playPause.append(svg(s.playing ? ICONS.pause : ICONS.play, 16, { fill: "currentColor" }));
    playPause.title = s.playing ? t("Pause") : t("Play");
    like.classList.toggle("on", s.like === "like");
    dislike.classList.toggle("on", s.like === "dislike");
    input.placeholder = t("Search songs");

    const key = [language(), ...s.results.map((r) => `${r.index}|${r.title}|${r.subtitle}`)].join("~");
    if (key !== resultsKey) {
      resultsKey = key;
      clear(resultsBox);
      resultsBox.append(...s.results.slice(0, 8).map(row));
    }
  }

  return { el, sync };
}
