// The Gmail card: sign in once, then the latest unread conversations (who, subject, first words). A click on
// one opens it in Gmail, in the browser. Nothing is sent, marked or deleted from here.

import { h, clear, dot } from "./dom";
import { Bridge } from "../core/bridge";
import { GMAIL_ID, GMAIL_RED, Gmail, gmailCount, gmailSender, gmailView, type GmailMessage } from "../core/gmail";
import { pillDefinition } from "../core/pills";
import { language, t } from "../i18n/i18n";

export interface GmailCard {
  el: HTMLElement;
  sync(): void;
}

export function buildGmailCard(): GmailCard {
  const count = h("span", { class: "wa-unread gm-count" });
  const head = h("div", { class: "int-head" }, dot(GMAIL_RED, 7), h("b", { text: pillDefinition(GMAIL_ID)?.name ?? "Gmail" }), count);

  // ── Starting, waiting for the sign-in, or the inbox that could not be read ──
  const message = h("div", { class: "yt-message" });
  const signIn = h("button", { class: "link-btn" });
  signIn.addEventListener("click", () => void Bridge.gmailSignIn());
  const gate = h("div", { class: "yt-gate" }, message, signIn);

  // ── The unread messages ──
  const list = h("div", { class: "gm-list" });
  const open = h("button", { class: "link-btn" });
  open.addEventListener("click", () => void Bridge.gmailOpen(""));
  const refresh = h("button", { class: "link-btn" });
  refresh.addEventListener("click", () => void Bridge.gmailRefresh());
  const foot = h("div", { class: "gm-foot" }, open, refresh);
  const inbox = h("div", { class: "gm-inbox" }, list, foot);

  const el = h("div", { class: "int-card gm-card" }, head, gate, inbox);

  function row(m: GmailMessage): HTMLElement {
    const r = h("button", { class: "gm-row", title: m.snippet },
      h("span", { class: "gm-from", text: gmailSender(m) }),
      h("span", { class: "gm-subject", text: m.subject || m.snippet }),
    );
    r.addEventListener("click", () => void Bridge.gmailOpen(m.id));
    return r;
  }

  let key = "";

  function sync() {
    const s = Gmail.inbox;
    const view = gmailView(s);
    gate.style.display = view === "inbox" ? "none" : "";
    inbox.style.display = view === "inbox" ? "" : "none";
    count.textContent = s.unread > 0 ? gmailCount(s.unread) : "";
    count.style.display = view === "inbox" && s.unread > 0 ? "" : "none";
    if (view !== "inbox") {
      signIn.style.display = view === "starting" ? "none" : "";
      signIn.textContent = view === "unreadable" ? t("Open Gmail's window") : t("Sign in");
      message.textContent = view === "signIn"
        ? t("Sign in to Gmail to see your unread mail here. A window opens: sign in with your Google account, then close it.")
        : view === "unreadable"
          ? t("Could not read Gmail. Open its window and check that you are signed in.")
          : t("Starting Gmail…");
      return;
    }
    open.textContent = t("Open Gmail");
    refresh.textContent = t("Check now");
    const next = [language(), ...s.messages.map((m) => `${m.id}|${m.from}|${m.subject}`)].join("~");
    if (next === key) return;
    key = next;
    clear(list);
    if (s.messages.length === 0) list.append(h("div", { class: "wa-empty", text: t("No unread mail") }));
    else list.append(...s.messages.map(row));
  }

  return { el, sync };
}
