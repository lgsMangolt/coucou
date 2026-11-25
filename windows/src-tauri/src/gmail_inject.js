// Runs inside mail.google.com, in the window gmail.rs opens (never in a window of Coucou's own).
//
// It does one thing: it reads the inbox as Gmail shows it to the signed-in user (the unread
// conversations of the first page: who, subject, first words, and how many are unread in all) and
// writes that into the page's title, which the app reads. It sends nothing anywhere and clicks
// nothing.
//
// Gmail has no light address that answers a signed-in window (its inbox feed asks for a password
// of its own), so the selectors below follow Gmail's page. When Google changes the page, the list
// can stop until this file follows.
(() => {
  if (window.__coucouGmail || location.hostname !== "mail.google.com" || window.top !== window) return;

  const MARK = "[coucou-gmail]";
  const clean = (s) => (s || "").replace(/\s+/g, " ").trim();
  const started = Date.now();

  /** Gmail's own title ("Inbox (3) - you@gmail.com - Gmail"), kept so it can be put back. */
  let own = "";
  let turn = 0;
  function send(report) {
    if (!document.documentElement) return;
    if (!document.title.startsWith(MARK)) own = document.title;
    // `n` changes every time: a title that does not change is not announced.
    // A page's title is cut at 4096 characters: a report that long would arrive broken, so it
    // loses its first words, then its oldest ids, until it fits.
    const out = Object.assign({}, report, { n: ++turn });
    let text = JSON.stringify(out);
    if (text.length > 3800 && out.messages) {
      out.messages = out.messages.map((m) => Object.assign({}, m, { snippet: "" }));
      text = JSON.stringify(out);
    }
    while (text.length > 3800 && out.known && out.known.length > 0) {
      out.known = out.known.slice(0, Math.floor(out.known.length / 2));
      text = JSON.stringify(out);
    }
    document.title = MARK + text;
    // Gmail's title goes back a moment later (both changes at once would be announced as one).
    setTimeout(() => {
      if (document.title.startsWith(MARK)) document.title = own || "Gmail";
    }, 1500);
  }

  /** The number Gmail puts in its title, or null when the title says nothing. */
  function titleCount() {
    const title = document.title.startsWith(MARK) ? own : document.title;
    if (!title) return null;
    const found = title.match(/\(([\d.,\s ]+)\)/);
    if (!found) return 0;
    const n = parseInt(found[1].replace(/\D/g, ""), 10);
    return isFinite(n) ? n : null;
  }

  function message(row) {
    const who = row.querySelector("span[email]");
    const thread = row.querySelector("[data-legacy-thread-id]");
    const id = (thread && thread.getAttribute("data-legacy-thread-id")) || "";
    return {
      id,
      // Kept short: the whole report travels in the page's title, which has a limit.
      from: clean(who && (who.getAttribute("name") || who.textContent || who.getAttribute("email"))).slice(0, 40),
      subject: clean((row.querySelector(".bog") || {}).textContent).slice(0, 90),
      snippet: clean((row.querySelector(".y2") || {}).textContent).replace(/^[-–—\s]+/, "").slice(0, 60),
    };
  }

  /** What the inbox shows, or null while there is nothing to say yet (loading, another folder). */
  function snapshot() {
    if (!location.pathname.startsWith("/mail/")) return null;
    if (!location.hash.startsWith("#inbox") && location.hash !== "") return null;
    const rows = Array.from(document.querySelectorAll("tr.zA"));
    if (rows.length === 0 && !document.querySelector("div[role='main']")) {
      // Signed in, and still nothing a minute later: the page did not load, or it changed.
      return Date.now() - started > 60000 ? { loggedIn: true, failed: true, unread: 0, messages: [] } : null;
    }
    const unread = rows.filter((row) => row.classList.contains("zE")).map(message).filter((m) => m.id);
    const messages = unread.slice(0, 10);
    const counted = titleCount();
    // `known`: every unread conversation of the page, so the app can tell new mail from old.
    return { loggedIn: true, failed: false, unread: counted === null ? messages.length : counted, messages, known: unread.slice(0, 50).map((m) => m.id) };
  }

  let last = "";
  let lastAt = 0;
  function report(force) {
    const s = snapshot();
    if (!s) return;
    const key = JSON.stringify(s);
    const now = Date.now();
    // Only what changed, and a sign of life once a minute.
    if (!force && key === last && now - lastAt < 60000) return;
    last = key;
    lastAt = now;
    send(s);
  }

  window.__coucouGmail = { refresh: () => report(true) };
  setInterval(() => report(false), 5000);
})();
