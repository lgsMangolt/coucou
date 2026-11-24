// The YouTube Music pill: music.youtube.com in a window of its own, signed in once with the user's
// Google account, and driven from the island (play/pause, next, previous, like, dislike, search).
//
// YouTube Music has no public interface, so this is the page itself, as a browser would show it:
// - the window's profile is kept in Coucou's own folder, so the sign-in lasts;
// - `ytmusic_inject.js` reads the player and writes what it sees into the page's title, which
//   arrives here (`on_document_title_changed`); nothing else leaves the page;
// - a control is a short script run in that page (`eval`), from a fixed list of actions.
// The window has no access to Coucou's commands (capabilities/default.json lists the windows that
// do), it is hidden once signed in, and it exists only while the pill is active.
//
// Coucou never asks for or sees the Google password: the user types it into Google's own page.

use std::sync::{LazyLock, Mutex, MutexGuard, PoisonError};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder, WindowEvent};

pub const PILL_ID: &str = "integration_ytmusic";
const LABEL: &str = "ytmusic";
const START_URL: &str = "https://music.youtube.com/";
const EVENT: &str = "ytmusic";
/// What the injected script puts in front of its report in the page's title.
// Plain characters only: the browser drops control characters from a page's title.
const TITLE_MARK: &str = "[coucou-ytm]";
const INJECT: &str = include_str!("ytmusic_inject.js");
const MAX_TEXT: usize = 200;
const MAX_QUERY: usize = 100;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Hit {
    pub index: u32,
    pub title: String,
    pub subtitle: String,
}

/// What the island shows of the player.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Player {
    /// The window exists: the pill is active.
    pub window: bool,
    pub logged_in: bool,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub playing: bool,
    /// "like", "dislike" or "none".
    pub like: String,
    /// Seconds, as of `position_at` (Unix ms); while playing the island runs it on.
    pub position: f64,
    pub position_at: i64,
    pub duration: f64,
    pub results: Vec<Hit>,
}

static STATE: LazyLock<Mutex<Player>> = LazyLock::new(|| Mutex::new(Player::default()));

fn state() -> MutexGuard<'static, Player> {
    STATE.lock().unwrap_or_else(PoisonError::into_inner)
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

fn cut(text: &str) -> String {
    text.chars().take(MAX_TEXT).collect()
}

/// What the page reported, held to what the island may show: it is third-party text.
fn from_page(json: &str) -> Option<Player> {
    let mut page = serde_json::from_str::<Player>(json).ok()?;
    page.window = true;
    page.position_at = now_ms();
    page.title = cut(&page.title);
    page.artist = cut(&page.artist);
    page.album = cut(&page.album);
    if !matches!(page.like.as_str(), "like" | "dislike") {
        page.like = "none".into();
    }
    page.position = page.position.clamp(0.0, 86_400.0);
    page.duration = page.duration.clamp(0.0, 86_400.0);
    page.results.truncate(8);
    for hit in &mut page.results {
        hit.title = cut(&hit.title);
        hit.subtitle = cut(&hit.subtitle);
    }
    Some(page)
}

fn publish(app: &AppHandle, player: Player) {
    let _ = app.emit_to(crate::island::WINDOW_LABEL, EVENT, player);
}

fn handle_title(app: &AppHandle, title: &str) {
    let Some(json) = title.strip_prefix(TITLE_MARK) else { return };
    let Some(page) = from_page(json) else { return };
    *state() = page.clone();
    publish(app, page);
}

/// Top-level pages stay on the web: nothing with a scheme of its own opens in this window.
fn allowed(url: &tauri::Url) -> bool {
    matches!(url.scheme(), "https" | "about" | "blob" | "data")
}

fn create(app: &AppHandle, visible: bool) -> Result<WebviewWindow, String> {
    let profile = crate::platform::local_dir().join("ytmusic");
    crate::platform::ensure_private_dir(&profile).map_err(|e| e.to_string())?;
    let url = START_URL.parse::<tauri::Url>().map_err(|e| e.to_string())?;
    let handle = app.clone();
    let window = WebviewWindowBuilder::new(app, LABEL, WebviewUrl::External(url))
        .title("YouTube Music · Coucou")
        .inner_size(1000.0, 720.0)
        .visible(visible)
        .data_directory(profile)
        .initialization_script(INJECT)
        .on_navigation(allowed)
        .on_document_title_changed(move |_, title| handle_title(&handle, &title))
        .build()
        .map_err(|e| e.to_string())?;
    // The close button hides it: the music goes on, and the sign-in stays.
    let hider = window.clone();
    window.on_window_event(move |event| {
        if let WindowEvent::CloseRequested { api, .. } = event {
            api.prevent_close();
            let _ = hider.hide();
        }
    });
    {
        let mut s = state();
        s.window = true;
        let snapshot = s.clone();
        drop(s);
        publish(app, snapshot);
    }
    Ok(window)
}

/// The pill is turned on or off (Settings, a mode): the window follows, the music with it.
pub fn sync(app: &AppHandle, active_integrations: &[String]) {
    let on = active_integrations.iter().any(|id| id == PILL_ID);
    let exists = app.get_webview_window(LABEL).is_some();
    if on && !exists {
        let app = app.clone();
        // Windows are built off the thread that runs a command, or the build waits on itself.
        tauri::async_runtime::spawn(async move {
            if let Err(reason) = create(&app, false) {
                crate::log::line(format!("ytmusic: could not open the player: {reason}"));
            }
        });
    } else if !on && exists {
        if let Some(window) = app.get_webview_window(LABEL) {
            let _ = window.destroy();
        }
        *state() = Player::default();
        publish(app, Player::default());
    }
}

fn only(window: &WebviewWindow) -> Result<(), String> {
    if window.label() == crate::island::WINDOW_LABEL { Ok(()) } else { Err("not from this window".into()) }
}

/// Runs a script in the player's page. False when there is no player window.
fn run(app: &AppHandle, script: &str) -> bool {
    app.get_webview_window(LABEL).is_some_and(|w| w.eval(script).is_ok())
}

#[tauri::command]
pub fn ytmusic_state(window: WebviewWindow) -> Result<Player, String> {
    only(&window)?;
    Ok(state().clone())
}

/// One of the fixed actions of the player bar.
#[tauri::command]
pub fn ytmusic_control(window: WebviewWindow, action: String) -> Result<bool, String> {
    only(&window)?;
    let action = match action.as_str() {
        "playPause" | "next" | "previous" | "like" | "dislike" => action,
        _ => return Err("unknown action".into()),
    };
    Ok(run(window.app_handle(), &format!("window.__coucou && window.__coucou.control(\"{action}\")")))
}

#[tauri::command]
pub fn ytmusic_search(window: WebviewWindow, query: String) -> Result<bool, String> {
    only(&window)?;
    let query: String = query.chars().filter(|c| !c.is_control()).take(MAX_QUERY).collect();
    let query = query.trim();
    if query.is_empty() {
        return Ok(false);
    }
    let literal = serde_json::to_string(query).map_err(|e| e.to_string())?;
    Ok(run(window.app_handle(), &format!("window.__coucou && window.__coucou.search({literal})")))
}

/// Plays one of the results of the last search.
#[tauri::command]
pub fn ytmusic_play(window: WebviewWindow, index: u32) -> Result<bool, String> {
    only(&window)?;
    Ok(run(window.app_handle(), &format!("window.__coucou && window.__coucou.playResult({index})")))
}

/// Shows the window so the user can sign in to Google in it.
#[tauri::command]
pub async fn ytmusic_sign_in(window: WebviewWindow) -> Result<bool, String> {
    only(&window)?;
    let app = window.app_handle().clone();
    let player = match app.get_webview_window(LABEL) {
        Some(w) => w,
        None => create(&app, true)?,
    };
    let _ = player.unminimize();
    player.show().map_err(|e| e.to_string())?;
    let _ = player.set_focus();
    Ok(true)
}

/// Hides it again; the music keeps playing.
#[tauri::command]
pub fn ytmusic_hide(window: WebviewWindow) -> Result<(), String> {
    only(&window)?;
    if let Some(w) = window.app_handle().get_webview_window(LABEL) {
        let _ = w.hide();
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_page_report_becomes_a_player_held_to_what_the_island_may_show() {
        let long = "x".repeat(500);
        let json = format!(
            r#"{{"loggedIn":true,"title":"{long}","artist":"Ana","like":"weird","playing":true,"position":12.7,"duration":99999999,"results":[{{"index":2,"title":"A","subtitle":"B"}}]}}"#
        );
        let p = from_page(&json).unwrap();
        assert!(p.window && p.logged_in && p.playing);
        assert_eq!(p.title.chars().count(), MAX_TEXT);
        assert_eq!(p.like, "none");
        assert_eq!(p.duration, 86_400.0);
        assert_eq!(p.results, vec![Hit { index: 2, title: "A".into(), subtitle: "B".into() }]);
        assert!(p.position_at > 0);
        assert!(from_page("not json").is_none());
    }

    #[test]
    fn only_the_marked_title_is_a_report() {
        // An ordinary page title carries no mark: handle_title returns before parsing anything.
        assert!("Some song - YouTube Music".strip_prefix(TITLE_MARK).is_none());
        assert!(format!("{TITLE_MARK}{{}}").strip_prefix(TITLE_MARK).is_some());
        assert!(TITLE_MARK.chars().all(|c| !c.is_control()), "a control character would be dropped from the title");
    }

    #[test]
    fn the_window_stays_on_the_web() {
        let url = |s: &str| s.parse::<tauri::Url>().unwrap();
        assert!(allowed(&url("https://music.youtube.com/search?q=a")));
        assert!(allowed(&url("https://accounts.google.com/signin")));
        assert!(!allowed(&url("file:///C:/Windows/win.ini")));
        assert!(!allowed(&url("ms-settings:privacy")));
    }
}
