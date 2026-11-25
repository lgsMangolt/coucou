// The Gmail pill on the page: what src-tauri/src/gmail.rs reports of the inbox (the unread
// messages, read from Gmail's own page in a window signed in by the user).

export const GMAIL_ID = "integration_gmail";
/** Gmail's red. */
export const GMAIL_RED = "#EA4335";

/** Message in src-tauri/src/gmail.rs. */
export interface GmailMessage {
  id: string;
  from: string;
  address: string;
  subject: string;
  snippet: string;
  /** Unix ms, 0 when unknown. */
  at: number;
  link: string;
}

/** Inbox in src-tauri/src/gmail.rs. */
export interface GmailInbox {
  /** The window exists: the pill is on. */
  window: boolean;
  /** The page reported at least once. */
  checked: boolean;
  loggedIn: boolean;
  /** Signed in, but the inbox could not be read. */
  failed: boolean;
  unread: number;
  messages: GmailMessage[];
}

/** Alert in src-tauri/src/gmail.rs: mail that arrived since the last look. */
export interface GmailAlert {
  from: string;
  subject: string;
  count: number;
}

export const IDLE_GMAIL: GmailInbox = { window: false, checked: false, loggedIn: false, failed: false, unread: 0, messages: [] };

/** The page's copy of the inbox. */
export const Gmail = {
  inbox: { ...IDLE_GMAIL } as GmailInbox,
};

/** What the card shows: starting, waiting for the sign-in, the inbox that could not be read, or the list. */
export function gmailView(s: GmailInbox): "starting" | "signIn" | "unreadable" | "inbox" {
  if (!s.window || !s.checked) return "starting";
  if (!s.loggedIn) return "signIn";
  return s.failed ? "unreadable" : "inbox";
}

/** Who a message is from, as short as the row needs it. */
export function gmailSender(m: GmailMessage): string {
  return m.from || m.address || "?";
}

/** The pill's line when mail arrives: who and what, or how many. */
export function gmailAlertLine(a: GmailAlert, many: (n: number) => string): string {
  if (a.count > 1) return many(a.count);
  return [a.from, a.subject].filter(Boolean).join(": ");
}

/** The unread count as the pill and the card wear it. */
export function gmailCount(unread: number): string {
  return unread > 99 ? "99+" : String(unread);
}
