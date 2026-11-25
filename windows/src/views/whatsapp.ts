// The WhatsApp card — a DOM port of WhatsAppCardView, WhatsAppRow, WhatsAppConversationView,
// WhatsAppBubble and WhatsAppAvatar (WhatsAppViews.swift). Sizes, colours and wording are the Mac's.
//
// The list: one row per conversation (never one per message) with initials, name, age, the last
// message cut short and the unread count; when the gateway isn't linked, why, and a way to Settings.
// A click on a row opens the conversation: its last ten messages and a reply field. Enter sends,
// Shift+Enter starts a new line, Escape goes back to the list. Nothing is sent without the user's
// own Enter or click, and while a reply is being typed the island stays open.
//
// Built once and synced in place: the field keeps its draft and the keyboard while messages arrive.

import { h, svg, clear, dot } from "./dom";
import { ICONS } from "./icons";
import { Bridge } from "../core/bridge";
import { State } from "../core/state";
import {
  WHATSAPP_GREEN, waPillId, waStore, canSend, initials, keyAction, lastMessages, linkColor, linkLabel,
  preview, previewParts, sendErrorLabel, timeAgo, type WaConversation, type WaEntry, type WaMessage, type WaOrder,
} from "../core/whatsapp";
import { language, t, tl } from "../i18n/i18n";

export interface WhatsAppCardHost {
  openSettings(): void;
  /** The island takes the keyboard for the reply field, and gives it back. */
  keyboard(on: boolean): void;
  /** A reply is being typed: the island stays open until it is sent or cleared. */
  holdOpen(on: boolean): void;
}

export interface WhatsAppCard {
  el: HTMLElement;
  sync(): void;
}

/** Initials on WhatsApp green: no photo is ever fetched (they live on WhatsApp's servers). */
function avatar(name: string, isGroup: boolean, size = 16): HTMLElement {
  const letters = initials(name);
  const el = h("span", { class: "wa-avatar", style: `width:${size}px;height:${size}px;font-size:${size * 0.42}px` });
  if (isGroup && letters === "#") el.append(svg(ICONS.stack, size * 0.6, { fill: WHATSAPP_GREEN }));
  else el.textContent = letters;
  return el;
}

function bubble(m: WaMessage, isGroup: boolean): HTMLElement {
  const { label, value } = previewParts(m);
  const box = h("div", { class: m.fromMe ? "wa-bubble mine" : "wa-bubble" });
  if (isGroup && !m.fromMe && m.senderName) box.append(h("div", { class: "wa-sender", text: m.senderName }));
  if (label) box.append(h("div", { class: "wa-media", text: t(label) }));
  if (value) box.append(h("div", { class: "wa-text", text: value }));
  return h("div", { class: m.fromMe ? "wa-line mine" : "wa-line" }, box);
}

/** `account`: 0 is the first WhatsApp, 1 the second: its own conversations, its own commands. */
export function buildWhatsAppCard(host: WhatsAppCardHost, account = 0): WhatsAppCard {
  const store = waStore(account);
  const pillId = waPillId(account);
  // ── The list ──
  const unreadTag = h("span", { class: "wa-unread" });
  const head = h("div", { class: "int-head" }, dot(WHATSAPP_GREEN, 7), h("b", { text: account ? "WhatsApp 2" : "WhatsApp" }), unreadTag);
  const listBody = h("div", {});
  // Notes on money read from your own chat, each waiting for your click (ledger.rs).
  const pendingBox = h("div", { class: "wa-pending" });
  const listView = h("div", { class: "wa-list" }, head, pendingBox, listBody);

  // ── The conversation ──
  const back = h("button", { class: "int-back wa-back", title: tl("Back") }, svg(ICONS.chevronLeft, 10, { stroke: 2.4 }));
  const convoAvatar = h("span", {});
  const convoName = h("b", { class: "wa-name" });
  const convoHead = h("div", { class: "wa-head" }, back, convoAvatar, convoName);
  const messages = h("div", { class: "wa-messages" });
  const field = h("textarea", { class: "wa-field", rows: "1", spellcheck: "true" }) as HTMLTextAreaElement;
  const sendBtn = h("button", { class: "wa-send", title: tl("Send") }, svg(ICONS.arrowUp, 12, { fill: "#0B0C0E" }));
  const reply = h("div", { class: "wa-reply" }, field, sendBtn);
  const note = h("div", { class: "wa-note" });
  // A reply Coucou suggests for the last message received. A click puts it in the field; it is
  // never sent by itself.
  const suggestion = h("button", { class: "wa-suggest", title: tl("Use this reply") });
  suggestion.style.display = "none";
  const convoView = h("div", { class: "wa-convo" }, convoHead, messages, suggestion, reply, note);

  const el = h("div", { class: "int-card wa-card" }, listView, convoView);

  let openChat: string | null = null;
  /** The row whose "Ignore this chat" is showing (a right-click on it). */
  let ignoring: string | null = null;
  let listKey = "";
  let convoKey = "";
  /** The conversation and last received message the suggestion was asked for. */
  let suggestKey = "";
  let suggested = "";

  function showSuggestion(state: "none" | "writing" | "ready") {
    suggestion.style.display = state === "none" ? "none" : "";
    suggestion.classList.toggle("writing", state === "writing");
    suggestion.disabled = state !== "ready";
    suggestion.textContent = state === "writing" ? t("Writing a suggestion…") : state === "ready" ? suggested : "";
  }

  /** Asked once per received message, and only for the conversation that is open. */
  function syncSuggestion(c: WaConversation) {
    const last = c.messages[c.messages.length - 1];
    const key = last && !last.fromMe && store.view.link.state === "connected" ? `${c.id}~${last.id}` : "";
    if (key === suggestKey) return;
    suggestKey = key;
    suggested = "";
    if (!key) {
      showSuggestion("none");
      return;
    }
    showSuggestion("writing");
    void Bridge.whatsappSuggest(c.id, account).then((text) => {
      if (suggestKey !== key) return;
      suggested = (text ?? "").trim();
      showSuggestion(suggested ? "ready" : "none");
    });
  }

  suggestion.addEventListener("click", () => {
    if (!suggested) return;
    field.value = suggested;
    host.holdOpen(true);
    syncSendButton();
    field.focus();
  });

  const current = (): WaConversation | undefined =>
    openChat ? store.view.conversations.find((c) => c.id === openChat) : undefined;

  function open(id: string) {
    openChat = id;
    ignoring = null;
    field.value = "";
    convoKey = "";
    suggestKey = "";
    showSuggestion("none");
    void Bridge.whatsappOpen(id, account);
    host.keyboard(true);
    window.setTimeout(() => field.focus(), 120);
    sync();
  }

  function close() {
    if (openChat == null) return;
    void Bridge.whatsappClose(openChat, account);
    openChat = null;
    field.value = "";
    suggestKey = "";
    showSuggestion("none");
    host.holdOpen(false);
    host.keyboard(false);
    listKey = "";
    sync();
  }

  async function send() {
    const chat = openChat;
    const text = field.value;
    if (!chat || !canSend(text, store.view.sending)) return;
    const sent = await Bridge.whatsappSend(chat, text, account);
    if (sent && openChat === chat && field.value === text) {
      field.value = "";
      host.holdOpen(false);
      syncSendButton();
    }
    field.focus();
  }

  back.addEventListener("click", close);
  sendBtn.addEventListener("click", () => void send());
  field.addEventListener("input", () => {
    host.holdOpen(field.value.trim().length > 0);
    syncSendButton();
  });
  field.addEventListener("keydown", (e) => {
    const action = keyAction(e);
    if (action === "send") {
      e.preventDefault();
      void send();
    } else if (action === "back") {
      // Back to the list, not the island folding away.
      e.preventDefault();
      e.stopPropagation();
      close();
    }
  });

  // Leaving the card (another pill, another view, the island folded) closes the conversation.
  State.subscribe(() => {
    if (openChat == null) return;
    if (State.mode !== "expanded" || State.view !== "overview" || State.focusTask?.id !== pillId || !store.view.on) {
      close();
    }
  });

  function syncSendButton() {
    const ok = canSend(field.value, store.view.sending);
    sendBtn.classList.toggle("ready", ok);
    sendBtn.disabled = !ok;
    field.disabled = store.view.sending;
  }

  function row(c: WaConversation): HTMLElement {
    const unread = c.unread > 0;
    const r = h("button", { class: unread ? "int-row wa-row unread" : "int-row wa-row" },
      avatar(c.name, c.isGroup),
      h("span", { class: "int-name wa-row-name", text: c.name }),
      h("span", { class: "int-ago", text: timeAgo(c.lastActivity) }),
    );
    const last = c.messages[c.messages.length - 1];
    if (ignoring === c.id) {
      r.append(h("span", { class: "wa-spacer" }), h("span", {
        class: "link-btn wa-ignore",
        text: t("Ignore this chat"),
        onclick: (e: Event) => {
          e.stopPropagation();
          ignoring = null;
          void Bridge.whatsappIgnore(c.id, account);
        },
      }));
    } else {
      if (last) r.append(h("span", { class: "int-sub wa-preview", text: preview(last, c.isGroup) }));
      r.append(h("span", { class: "wa-spacer" }));
      if (unread) r.append(h("span", { class: "wa-count", text: c.unread > 99 ? "99+" : String(c.unread) }));
    }
    r.addEventListener("click", () => open(c.id));
    r.addEventListener("contextmenu", (e) => {
      e.preventDefault();
      ignoring = ignoring === c.id ? null : c.id;
      listKey = "";
      sync();
    });
    return r;
  }

  function entryCard(e: WaEntry): HTMLElement {
    const out = e.kind === "expense";
    const label = h("div", { class: "wa-entry-line" },
      h("b", { class: out ? "wa-entry-kind out" : "wa-entry-kind in", text: out ? t("Expense") : t("Income") }),
      h("span", { class: "wa-entry-amount", text: e.amount }),
    );
    const detail = [e.category, e.person, e.note].filter(Boolean).join(" · ");
    const save = h("button", { class: "wa-entry-save", text: t("Save") }) as HTMLButtonElement;
    const drop = h("button", { class: "wa-entry-drop", text: t("Discard") }) as HTMLButtonElement;
    const problem = h("div", { class: "wa-note err" });
    save.addEventListener("click", () => {
      save.disabled = drop.disabled = true;
      problem.textContent = "";
      Bridge.whatsappLedgerSave(e.id).catch((error) => {
        save.disabled = drop.disabled = false;
        problem.textContent = String(error);
      });
    });
    drop.addEventListener("click", () => {
      save.disabled = drop.disabled = true;
      void Bridge.whatsappLedgerDiscard(e.id);
    });
    return h("div", { class: "wa-entry" },
      label,
      h("div", { class: "wa-entry-detail", text: detail ? `${detail} · ${e.date}` : e.date }),
      h("div", { class: "wa-entry-actions" }, save, drop),
      problem,
    );
  }

  /** What the user typed in an order's reply, kept while the card is redrawn. */
  const replies = new Map<number, string>();

  /** Puts the text on the clipboard (the old way when the page has no clipboard API). */
  async function copyText(text: string) {
    try {
      await navigator.clipboard.writeText(text);
    } catch {
      const area = h("textarea", { style: "position:fixed;opacity:0" }) as HTMLTextAreaElement;
      area.value = text;
      document.body.append(area);
      area.select();
      document.execCommand("copy");
      area.remove();
    }
  }

  /** A print order: what is asked, the reply Coucou would give (to copy, correct and send), the
   * printer, and the clicks. Nothing is sent from here. */
  function orderCard(o: WaOrder): HTMLElement {
    const stage = o.state === "approved" ? t("Order approved") : o.state === "quoted" ? t("Waiting for the customer") : t("Print order");
    const label = h("div", { class: "wa-entry-line" },
      h("b", { class: "wa-entry-kind in", text: stage }),
      h("span", { class: "wa-entry-amount", text: o.total || t("No price") }),
    );
    const reply = h("textarea", { class: "wa-order-reply", rows: "5", spellcheck: "true" }) as HTMLTextAreaElement;
    reply.value = replies.get(o.id) ?? o.reply;
    reply.addEventListener("input", () => replies.set(o.id, reply.value));
    // Typing in it is not a shortcut of the island.
    reply.addEventListener("keydown", (e) => e.stopPropagation());
    reply.style.display = o.reply || replies.has(o.id) ? "" : "none";
    const copy = h("button", { class: "wa-entry-save", text: t("Copy") }) as HTMLButtonElement;
    const file = h("button", { class: "wa-entry-drop", text: t("Get file") }) as HTMLButtonElement;
    const drop = h("button", { class: "wa-entry-drop", text: o.state === "approved" ? t("Done") : t("Discard") }) as HTMLButtonElement;
    const problem = h("div", { class: "wa-note err" });
    const all = [copy, file, drop];
    const busy = (on: boolean) => { for (const b of all) b.disabled = on || (b === copy && !reply.value.trim()); };
    busy(false);
    reply.addEventListener("input", () => busy(false));
    const failed = (error: unknown) => { busy(false); problem.textContent = String(error); };
    copy.style.display = reply.style.display;
    copy.addEventListener("click", () => {
      problem.textContent = "";
      void copyText(reply.value).then(() => {
        copy.textContent = t("Copied");
        window.setTimeout(() => { copy.textContent = t("Copy"); }, 1500);
        if (o.state === "draft") void Bridge.whatsappOrderCopied(o.id, account);
      });
    });
    file.addEventListener("click", () => {
      busy(true);
      problem.textContent = "";
      Bridge.whatsappOrderFile(o.id, account).then(() => busy(false), failed);
    });
    drop.addEventListener("click", () => {
      busy(true);
      replies.delete(o.id);
      void Bridge.whatsappOrderDiscard(o.id, account);
    });
    return h("div", { class: "wa-entry" },
      label,
      h("div", { class: "wa-entry-detail", text: [o.name, o.line, o.note].filter(Boolean).join(" · ") }),
      h("div", { class: "wa-entry-detail", text: `${o.fileName} · ${t("Printer: {0}", { 0: o.printer })}` }),
      reply,
      h("div", { class: "wa-entry-actions" }, ...all),
      problem,
    );
  }

  function syncList() {
    const v = store.view;
    const key = [
      language(), v.link.state, "detail" in v.link ? v.link.detail : "", v.unread, ignoring,
      ...v.pending.map((e) => `${e.id}|${e.amount}|${e.kind}|${e.category}|${e.person}|${e.note}`),
      ...v.orders.map((o) => `o${o.id}|${o.state}|${o.total}|${o.line}|${o.note}|${o.printer}|${o.saved ?? ""}|${o.reply}`),
      // The ages move on a minute at a time.
      Math.floor(Date.now() / 60_000),
      ...v.conversations.map((c) => `${c.id}|${c.name}|${c.unread}|${c.lastActivity}|${c.messages[c.messages.length - 1]?.id ?? ""}`),
    ].join("~");
    if (key === listKey) return;
    listKey = key;
    unreadTag.textContent = v.unread > 0 ? t("{0} unread", { 0: v.unread }) : "";
    unreadTag.style.display = v.unread > 0 ? "" : "none";
    clear(pendingBox);
    pendingBox.append(...v.orders.map(orderCard), ...v.pending.map(entryCard));
    clear(listBody);
    if (v.link.state !== "connected") {
      listBody.append(
        h("div", { class: "int-status" }, dot(linkColor(v.link), 5), h("span", { class: "wa-status-text", text: linkLabel(v.link) })),
        h("div", { class: "int-actions" }, h("button", {
          class: "link-btn wa-settings", text: t("Settings…"), onclick: () => host.openSettings(),
        })),
      );
    } else if (v.conversations.length === 0) {
      listBody.append(h("div", { class: "wa-empty", text: t("No new messages") }));
    } else {
      listBody.append(h("div", { class: "int-rows wa-rows" }, ...v.conversations.map(row)));
    }
  }

  function syncConversation(c: WaConversation) {
    const v = store.view;
    const shown = lastMessages(c);
    const key = [language(), c.id, c.name, ...shown.map((m) => m.id)].join("~");
    if (key !== convoKey) {
      const wasAtBottom = messages.scrollHeight - messages.scrollTop - messages.clientHeight < 24;
      const first = convoKey === "";
      convoKey = key;
      clear(convoAvatar);
      convoAvatar.append(avatar(c.name, c.isGroup));
      convoName.textContent = c.name;
      convoName.title = c.id.split("@")[0];
      clear(messages);
      for (const m of shown) messages.append(bubble(m, c.isGroup));
      // The newest at the bottom, in view.
      if (first || wasAtBottom) messages.scrollTop = messages.scrollHeight;
    }
    field.placeholder = t("Reply…  (Shift+Enter: new line)");
    suggestion.title = t("Use this reply");
    if (suggestion.classList.contains("writing")) suggestion.textContent = t("Writing a suggestion…");
    syncSuggestion(c);
    note.className = "wa-note";
    if (v.link.state !== "connected") {
      note.textContent = linkLabel(v.link);
      note.classList.add("warn");
    } else if (v.sendError) {
      note.textContent = sendErrorLabel(v.sendError);
      note.classList.add("err");
    } else {
      note.textContent = "";
    }
    syncSendButton();
  }

  function sync() {
    const c = current();
    // Kept open through a missed check (the draft too): only gone when WhatsApp is off or the
    // conversation left the list (ignored).
    if (openChat != null && (!c || !store.view.on)) {
      close();
      return;
    }
    listView.style.display = c ? "none" : "";
    convoView.style.display = c ? "" : "none";
    if (c) syncConversation(c);
    else syncList();
  }

  return { el, sync };
}
