// Runs inside music.youtube.com, in the window ytmusic.rs opens (never in a window of Coucou's own).
//
// It does two things and nothing else:
// - reports what the player shows (the song, who sings it, playing or paused, liked or not, the
//   results of a search) by writing it into the page's title, which the app reads;
// - does what the island asks: play/pause, next, previous, like, dislike, search, play a result.
// It reads the page as it is shown to the signed-in user and sends nothing anywhere.
//
// YouTube Music has no public interface, so the selectors below follow its own page. When YouTube
// changes the page, a control can stop answering until this file follows; the rest keeps working.
(() => {
  if (window.__coucou || location.hostname !== "music.youtube.com") return;

  const MARK = "[coucou-ytm]";
  const $ = (selector) => document.querySelector(selector);
  const clean = (el) => ((el && el.textContent) || "").replace(/\s+/g, " ").trim();
  const click = (el) => {
    if (!el) return false;
    el.click();
    return true;
  };

  /** true / false once the page knows whether somebody is signed in; null before. */
  function signedIn() {
    try {
      if (window.ytcfg && typeof window.ytcfg.get === "function") return !!window.ytcfg.get("LOGGED_IN");
    } catch (_) { /* not ready */ }
    return null;
  }

  function items() {
    return Array.from(document.querySelectorAll("ytmusic-responsive-list-item-renderer")).slice(0, 8);
  }

  function results() {
    if (!location.pathname.startsWith("/search")) return [];
    const out = [];
    items().forEach((el, index) => {
      const title = clean(el.querySelector(".title"));
      if (!title) return;
      out.push({ index, title, subtitle: clean(el.querySelector(".secondary-flex-columns")) });
    });
    return out;
  }

  function snapshot() {
    const loggedIn = signedIn();
    if (loggedIn === null) return null;
    const bar = $("ytmusic-player-bar");
    const inBar = (selector) => (bar ? bar.querySelector(selector) : null);
    const video = $("video");
    const byline = clean(inBar(".byline")).split("•").map((s) => s.trim()).filter(Boolean);
    const likeEl = inBar("ytmusic-like-button-renderer") || $("ytmusic-like-button-renderer");
    const status = ((likeEl && likeEl.getAttribute("like-status")) || "INDIFFERENT").toUpperCase();
    return {
      loggedIn,
      title: clean(inBar(".title")),
      artist: byline[0] || "",
      album: byline[1] || "",
      playing: !!video && !video.paused && !video.ended,
      like: status === "LIKE" ? "like" : status === "DISLIKE" ? "dislike" : "none",
      position: video && isFinite(video.currentTime) ? Math.floor(video.currentTime) : 0,
      duration: video && isFinite(video.duration) ? Math.floor(video.duration) : 0,
      results: results(),
    };
  }

  let last = "";
  let lastAt = 0;
  function report(force) {
    if (!document.documentElement) return;
    const s = snapshot();
    if (!s) return;
    const key = JSON.stringify(Object.assign({}, s, { position: 0 }));
    const now = Date.now();
    // Only what changed, and the position now and then: the island runs the clock on.
    if (!force && key === last && now - lastAt < 5000) return;
    last = key;
    lastAt = now;
    document.title = MARK + JSON.stringify(s);
  }

  const actions = {
    playPause() {
      const video = $("video");
      if (video) {
        const done = video.paused ? video.play() : video.pause();
        if (done && typeof done.catch === "function") done.catch(() => {});
        return true;
      }
      return click($("#play-pause-button"));
    },
    next: () => click($(".next-button")),
    previous: () => click($(".previous-button")),
    like: () => click($("ytmusic-like-button-renderer #button-shape-like button, ytmusic-like-button-renderer .like")),
    dislike: () => click($("ytmusic-like-button-renderer #button-shape-dislike button, ytmusic-like-button-renderer .dislike")),
  };

  window.__coucou = {
    control(action) {
      let done = false;
      try {
        done = !!(actions[action] && actions[action]());
      } catch (_) { /* the page changed under us */ }
      setTimeout(() => report(true), 350);
      return done;
    },

    search(query) {
      const go = () => location.assign("/search?q=" + encodeURIComponent(query));
      const input = $("ytmusic-search-box input");
      if (!input) return go();
      // Typed into the page's own search box, so the song that plays keeps playing.
      const setter = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value").set;
      input.focus();
      setter.call(input, query);
      input.dispatchEvent(new Event("input", { bubbles: true }));
      input.dispatchEvent(new KeyboardEvent("keydown", { key: "Enter", code: "Enter", keyCode: 13, which: 13, bubbles: true }));
      setTimeout(() => {
        if (!location.pathname.startsWith("/search")) go();
      }, 1800);
      setTimeout(() => report(true), 2500);
    },

    playResult(index) {
      const el = items()[index];
      if (!el) return false;
      const play = el.querySelector("ytmusic-play-button-renderer");
      const done = click(play && (play.querySelector("#play-button") || play));
      if (!done) el.dispatchEvent(new MouseEvent("dblclick", { bubbles: true }));
      setTimeout(() => report(true), 1500);
      return true;
    },
  };

  setInterval(() => report(false), 1000);
})();
