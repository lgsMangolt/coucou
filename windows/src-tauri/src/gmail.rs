// The Gmail pill: the unread messages of the user's Gmail inbox, and a warning when one arrives.
//
// Like YouTube Music (ytmusic.rs), this is Google's own page in a window of Coucou's, signed in
// once by the user (the two share a profile folder, so one sign-in serves both):
// - the window stays on Gmail's inbox and `gmail_inject.js` reads what the page shows: the unread
//   conversations of the first page (who, subject, first words) and how many are unread in all.
//   Gmail's light inbox feed would cost less, but it asks a signed-in window for a password;
// - what it reads is written into the page's title, which arrives here; nothing leaves the page;
// - a click on a message opens Gmail in the user's browser. Coucou never sends, marks or deletes.
// The window has no access to Coucou's commands, is hidden once signed in, and exists only while
// the pill is active. Coucou never asks for or sees the Google password.

use std::collections::HashSet;
use std::sync::{LazyLock, Mutex, MutexGuard, PoisonError};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder, WindowEvent};

pub const PILL_ID: &str = "integration_gmail";
const LABEL: &str = "gmail";
const SIGN_IN_URL: &str =
    "https://accounts.google.com/ServiceLogin?service=mail&continue=https%3A%2F%2Fmail.google.com%2Fmail%2Fu%2F0%2F";
const INBOX_URL: &str = "https://mail.google.com/mail/u/0/#inbox";
const LINK_PREFIX: &str = "https://mail.google.com/";
const GMAIL_HOST: &str = "mail.google.com";
const EVENT: &str = "gmail";
const ALERT: &str = "gmail-alert";
/// What the injected script puts in front of its report in the page's title (plain characters:
/// the browser drops control characters from a title).
const TITLE_MARK: &str = "[coucou-gmail]";
const INJECT: &str = include_str!("gmail_inject.js");
const MAX_TEXT: usize = 200;
const MAX_MESSAGES: usize = 10;
const MAX_SEEN: usize = 500;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Message {
    pub id: String,
    pub from: String,
    pub address: String,
    pub subject: String,
    pub snippet: String,
    /// Unix ms, 0 when the feed gave no date.
    pub at: i64,
    /// Gmail's own link to the message; empty when it is not one.
    pub link: String,
}

/// What the island shows of the inbox.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Inbox {
    /// The window exists: the pill is active.
    pub window: bool,
    /// The page reported at least once.
    pub checked: bool,
    pub logged_in: bool,
    /// Signed in, but the feed could not be read.
    pub failed: bool,
    pub unread: u32,
    pub messages: Vec<Message>,
}

/// A message that arrived since the last look.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Alert {
    pub from: String,
    pub subject: String,
    /// How many arrived together.
    pub count: usize,
}

#[derive(Default)]
struct Inner {
    inbox: Inbox,
    /// The unread conversations already shown: only what is not here warns.
    seen: HashSet<String>,
    /// The first signed-in report only fills `seen`: what was unread before warns nobody.
    primed: bool,
}

static STATE: LazyLock<Mutex<Inner>> = LazyLock::new(|| Mutex::new(Inner::default()));

fn state() -> MutexGuard<'static, Inner> {
    STATE.lock().unwrap_or_else(PoisonError::into_inner)
}

fn cut(text: &str) -> String {
    text.chars().filter(|c| !c.is_control()).take(MAX_TEXT).collect()
}

/// What the page reported, held to what the island may show: it is third-party text.
fn from_page(json: &str) -> Option<Inbox> {
    let mut page = serde_json::from_str::<Inbox>(json).ok()?;
    page.window = true;
    page.checked = true;
    page.messages.truncate(MAX_MESSAGES);
    for m in &mut page.messages {
        m.id = cut(&m.id);
        m.from = cut(&m.from);
        m.address = cut(&m.address);
        m.subject = cut(&m.subject);
        m.snippet = cut(&m.snippet);
        if !m.link.starts_with(LINK_PREFIX) || m.link.chars().any(char::is_control) {
            m.link.clear();
        }
    }
    page.messages.retain(|m| !m.id.is_empty());
    if !page.logged_in {
        page.unread = 0;
        page.messages.clear();
    }
    page.unread = page.unread.max(page.messages.len() as u32).min(99_999);
    Some(page)
}

/// Takes the report in and says what is new in it, if anything should warn.
fn take(inner: &mut Inner, page: Inbox) -> Option<Alert> {
    // New mail comes in at the top. An older conversation that moves up into the first page
    // (because others were read or archived) shows below ones already seen, and is not news.
    let fresh: Vec<&Message> = page.messages.iter().take_while(|m| !inner.seen.contains(&m.id)).collect();
    let alert = match fresh.first() {
        Some(first) if inner.primed => Some(Alert {
            from: if first.from.is_empty() { first.address.clone() } else { first.from.clone() },
            subject: first.subject.clone(),
            count: fresh.len(),
        }),
        _ => None,
    };
    if page.logged_in && !page.failed {
        if inner.seen.len() > MAX_SEEN {
            inner.seen.clear();
        }
        inner.seen.extend(page.messages.iter().map(|m| m.id.clone()));
        inner.primed = true;
    }
    inner.inbox = page;
    alert
}

fn publish(app: &AppHandle, inbox: Inbox) {
    let _ = app.emit_to(crate::island::WINDOW_LABEL, EVENT, inbox);
}

fn handle_title(app: &AppHandle, title: &str) {
    let Some(json) = title.strip_prefix(TITLE_MARK) else { return };
    let Some(page) = from_page(json) else { return };
    let alert = take(&mut state(), page.clone());
    publish(app, page);
    if let Some(alert) = alert {
        let _ = app.emit_to(crate::island::WINDOW_LABEL, ALERT, alert);
    }
}

/// Top-level pages stay on the web: nothing with a scheme of its own opens in this window.
fn allowed(url: &tauri::Url) -> bool {
    matches!(url.scheme(), "https" | "about" | "blob" | "data")
}

/// Gmail sends a window nobody is signed in to over to Google's sign-in page.
fn leaves_gmail(url: &tauri::Url) -> bool {
    url.scheme() == "https" && url.host_str() != Some(GMAIL_HOST)
}

/// The window went to a page that is not Gmail: nobody is signed in (or the user is signing in).
fn signed_out(app: &AppHandle) {
    let snapshot = {
        let mut s = state();
        s.inbox = Inbox { window: true, checked: true, ..Inbox::default() };
        s.inbox.clone()
    };
    publish(app, snapshot);
}

fn create(app: &AppHandle, visible: bool, start: &str) -> Result<WebviewWindow, String> {
    // The folder YouTube Music's window uses: one Google sign-in for both.
    let profile = crate::platform::local_dir().join("ytmusic");
    crate::platform::ensure_private_dir(&profile).map_err(|e| e.to_string())?;
    let url = start.parse::<tauri::Url>().map_err(|e| e.to_string())?;
    let handle = app.clone();
    let watcher = app.clone();
    let window = WebviewWindowBuilder::new(app, LABEL, WebviewUrl::External(url))
        .title("Gmail · Coucou")
        .inner_size(1000.0, 720.0)
        .visible(visible)
        .data_directory(profile)
        .initialization_script(INJECT)
        .on_navigation(move |url| {
            if !allowed(url) {
                return false;
            }
            if leaves_gmail(url) {
                signed_out(&watcher);
            }
            true
        })
        .on_document_title_changed(move |_, title| handle_title(&handle, &title))
        .build()
        .map_err(|e| e.to_string())?;
    // The close button hides it and takes it back to the inbox, which is what is read.
    let hider = window.clone();
    window.on_window_event(move |event| {
        if let WindowEvent::CloseRequested { api, .. } = event {
            api.prevent_close();
            let _ = hider.hide();
            let _ = hider.eval(&format!("location.hash.startsWith(\"#inbox\") || location.assign(\"{INBOX_URL}\")"));
        }
    });
    let snapshot = {
        let mut s = state();
        s.inbox.window = true;
        s.inbox.clone()
    };
    publish(app, snapshot);
    Ok(window)
}

/// The pill is turned on or off (Settings, a mode): the window follows.
pub fn sync(app: &AppHandle, active_integrations: &[String]) {
    let on = active_integrations.iter().any(|id| id == PILL_ID);
    let exists = app.get_webview_window(LABEL).is_some();
    if on && !exists {
        let app = app.clone();
        // Windows are built off the thread that runs a command, or the build waits on itself.
        tauri::async_runtime::spawn(async move {
            if let Err(reason) = create(&app, false, INBOX_URL) {
                crate::log::line(format!("gmail: could not open the window: {reason}"));
            }
        });
    } else if !on && exists {
        if let Some(window) = app.get_webview_window(LABEL) {
            let _ = window.destroy();
        }
        *state() = Inner::default();
        publish(app, Inbox::default());
    }
}

fn only(window: &WebviewWindow) -> Result<(), String> {
    if window.label() == crate::island::WINDOW_LABEL { Ok(()) } else { Err("not from this window".into()) }
}

#[tauri::command]
pub fn gmail_state(window: WebviewWindow) -> Result<Inbox, String> {
    only(&window)?;
    Ok(state().inbox.clone())
}

/// Asks the page to read the inbox now.
#[tauri::command]
pub fn gmail_refresh(window: WebviewWindow) -> Result<bool, String> {
    only(&window)?;
    Ok(window
        .app_handle()
        .get_webview_window(LABEL)
        .is_some_and(|w| w.eval("window.__coucouGmail && window.__coucouGmail.refresh()").is_ok()))
}

/// Shows the window on Google's sign-in page (which goes on to Gmail when already signed in).
#[tauri::command]
pub async fn gmail_sign_in(window: WebviewWindow) -> Result<bool, String> {
    only(&window)?;
    let app = window.app_handle().clone();
    let page = match app.get_webview_window(LABEL) {
        Some(w) => {
            let _ = w.eval(&format!("location.assign(\"{SIGN_IN_URL}\")"));
            w
        }
        None => create(&app, true, SIGN_IN_URL)?,
    };
    let _ = page.unminimize();
    page.show().map_err(|e| e.to_string())?;
    let _ = page.set_focus();
    Ok(true)
}

/// Opens a message of the list (or the inbox, with no id) in the user's browser.
#[tauri::command]
pub fn gmail_open(window: WebviewWindow, id: String) -> Result<(), String> {
    only(&window)?;
    let link = state().inbox.messages.iter().find(|m| m.id == id).map(|m| m.link.clone()).unwrap_or_default();
    crate::platform::open_url(if link.starts_with(LINK_PREFIX) { &link } else { INBOX_URL });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn report(ids: &[&str]) -> Inbox {
        let messages: Vec<String> = ids
            .iter()
            .map(|id| format!(r#"{{"id":"{id}","from":"Ana","address":"ana@example.com","subject":"Oi {id}","link":"https://mail.google.com/mail?message_id={id}"}}"#))
            .collect();
        from_page(&format!(r#"{{"loggedIn":true,"unread":{},"messages":[{}]}}"#, ids.len(), messages.join(","))).unwrap()
    }

    #[test]
    fn a_page_report_is_held_to_what_the_island_may_show() {
        let long = "x".repeat(500);
        let json = format!(
            r#"{{"loggedIn":true,"unread":1,"n":5,"messages":[{{"id":"a","subject":"{long}","link":"https://evil.example/x"}},{{"id":"","subject":"no id"}},{{"id":"b","link":"https://mail.google.com/mail?m=b"}}]}}"#
        );
        let p = from_page(&json).unwrap();
        assert!(p.window && p.checked && p.logged_in);
        assert_eq!(p.messages.len(), 2);
        assert_eq!(p.messages[0].subject.chars().count(), MAX_TEXT);
        assert_eq!(p.messages[0].link, "", "a link that is not Gmail's is dropped");
        assert_eq!(p.messages[1].link, "https://mail.google.com/mail?m=b");
        assert_eq!(p.unread, 2, "never fewer than the messages listed");
        assert!(from_page("not json").is_none());

        let out = from_page(r#"{"loggedIn":false,"unread":7,"messages":[{"id":"a"}]}"#).unwrap();
        assert_eq!((out.unread, out.messages.len()), (0, 0));
    }

    #[test]
    fn only_what_arrives_after_the_first_look_warns() {
        let mut inner = Inner::default();
        assert_eq!(take(&mut inner, report(&["a", "b"])), None, "what was already unread warns nobody");
        assert_eq!(take(&mut inner, report(&["a", "b"])), None);
        let alert = take(&mut inner, report(&["d", "c", "a", "b"])).unwrap();
        assert_eq!(alert, Alert { from: "Ana".into(), subject: "Oi d".into(), count: 2 });
        assert_eq!(take(&mut inner, report(&["d", "c"])), None, "reading some is not news");
        assert_eq!(take(&mut inner, report(&["d", "c", "old"])), None, "an older one moving up the page is not news");
        assert_eq!(take(&mut inner, report(&["e", "d", "older"])).map(|a| a.count), Some(1));
        assert_eq!(inner.inbox.unread, 3);
    }

    #[test]
    fn a_failed_look_does_not_reset_what_was_seen() {
        let mut inner = Inner::default();
        take(&mut inner, report(&["a"]));
        let failed = from_page(r#"{"loggedIn":true,"failed":true}"#).unwrap();
        assert_eq!(take(&mut inner, failed), None);
        assert_eq!(take(&mut inner, report(&["a"])), None);
    }

    #[test]
    fn only_the_marked_title_is_a_report_and_the_window_stays_on_the_web() {
        assert!("Inbox - Gmail".strip_prefix(TITLE_MARK).is_none());
        assert!(TITLE_MARK.chars().all(|c| !c.is_control()));
        let url = |s: &str| s.parse::<tauri::Url>().unwrap();
        assert!(allowed(&url(INBOX_URL)) && allowed(&url(SIGN_IN_URL)));
        assert!(leaves_gmail(&url(SIGN_IN_URL)) && !leaves_gmail(&url(INBOX_URL)));
        assert!(!leaves_gmail(&url("about:blank")));
        assert!(!allowed(&url("file:///C:/Windows/win.ini")));
        assert!(INBOX_URL.starts_with(LINK_PREFIX));
    }
}
