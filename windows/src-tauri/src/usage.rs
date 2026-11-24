// How much each pill is used, so the island can adapt to the way its owner works: a tool that has
// not been opened for two weeks leaves the mode it was in, and the options outside the mode are
// offered most-used first (src/core/adapt.ts holds those rules; this file only keeps the counts).
//
// Kept in usage.json next to settings.json: per pill, how many times it was opened, when last, and
// when it first showed up; plus the notes about pills the island took out on its own, which the
// user can undo. No message, name or content ever goes here, only pill ids and times. Nothing
// leaves the computer.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{LazyLock, Mutex, MutexGuard, PoisonError};

use serde::{Deserialize, Serialize};
use tauri::WebviewWindow;

const MAX_PILLS: usize = 64;
const MAX_NOTICES: usize = 10;
const MAX_ID: usize = 64;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PillUse {
    /// Times it was opened.
    pub uses: u32,
    /// Unix ms of the last time, 0 when never.
    pub last: i64,
    /// Unix ms when the island first showed it.
    pub since: i64,
}

/// A pill the island took out of a mode on its own, waiting to be seen (and maybe undone).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Notice {
    pub pill: String,
    pub mode: String,
    pub at: i64,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Usage {
    pub pills: BTreeMap<String, PillUse>,
    /// Oldest first.
    pub notices: Vec<Notice>,
}

fn valid(id: &str) -> bool {
    !id.is_empty() && id.len() <= MAX_ID && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

impl Usage {
    /// What a file may hold, brought back inside the rules.
    pub fn cleaned(mut self) -> Usage {
        self.pills.retain(|id, _| valid(id));
        while self.pills.len() > MAX_PILLS {
            self.pills.pop_last();
        }
        self.notices.retain(|n| valid(&n.pill) && valid(&n.mode));
        let extra = self.notices.len().saturating_sub(MAX_NOTICES);
        self.notices.drain(0..extra);
        self
    }

    fn slot(&mut self, id: &str) -> Option<&mut PillUse> {
        if !valid(id) || (!self.pills.contains_key(id) && self.pills.len() >= MAX_PILLS) {
            return None;
        }
        Some(self.pills.entry(id.to_string()).or_default())
    }

    /// The pill was opened.
    pub fn touch(&mut self, id: &str, now: i64) {
        if let Some(u) = self.slot(id) {
            u.uses = u.uses.saturating_add(1);
            u.last = now;
            if u.since == 0 {
                u.since = now;
            }
        }
    }

    /// These pills are on the island now: the ones never seen before start their clock.
    pub fn seen(&mut self, ids: &[String], now: i64) {
        for id in ids {
            if let Some(u) = self.slot(id) {
                if u.since == 0 {
                    u.since = now;
                }
            }
        }
    }

    pub fn notice(&mut self, pill: &str, mode: &str, now: i64) {
        if !valid(pill) || !valid(mode) {
            return;
        }
        self.notices.retain(|n| !(n.pill == pill && n.mode == mode));
        self.notices.push(Notice { pill: pill.into(), mode: mode.into(), at: now });
        let extra = self.notices.len().saturating_sub(MAX_NOTICES);
        self.notices.drain(0..extra);
    }

    pub fn dismiss(&mut self, pill: &str, mode: &str) {
        self.notices.retain(|n| !(n.pill == pill && n.mode == mode));
    }
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

fn path() -> PathBuf {
    crate::platform::config_dir().join("usage.json")
}

fn load() -> Usage {
    std::fs::read(path()).ok().and_then(|b| serde_json::from_slice::<Usage>(&b).ok()).unwrap_or_default().cleaned()
}

fn save(usage: &Usage) {
    let path = path();
    let Some(dir) = path.parent() else { return };
    let temp = path.with_extension(format!("json.coucou-{}", std::process::id()));
    let written = crate::platform::ensure_private_dir(dir)
        .and_then(|()| serde_json::to_vec_pretty(usage).map_err(std::io::Error::other))
        .and_then(|json| std::fs::write(&temp, json))
        .and_then(|()| std::fs::rename(&temp, &path));
    if let Err(err) = written {
        let _ = std::fs::remove_file(&temp);
        crate::log::line(format!("usage: could not save usage.json: {err}"));
    }
}

static DATA: LazyLock<Mutex<Usage>> = LazyLock::new(|| Mutex::new(load()));

fn data() -> MutexGuard<'static, Usage> {
    DATA.lock().unwrap_or_else(PoisonError::into_inner)
}

fn only(window: &WebviewWindow) -> Result<(), String> {
    if window.label() == crate::island::WINDOW_LABEL { Ok(()) } else { Err("not from this window".into()) }
}

fn change(window: &WebviewWindow, edit: impl FnOnce(&mut Usage)) -> Result<Usage, String> {
    only(window)?;
    let mut d = data();
    edit(&mut d);
    save(&d);
    Ok(d.clone())
}

#[tauri::command]
pub fn usage_get(window: WebviewWindow) -> Result<Usage, String> {
    only(&window)?;
    Ok(data().clone())
}

#[tauri::command]
pub fn usage_touch(window: WebviewWindow, id: String) -> Result<Usage, String> {
    change(&window, |u| u.touch(&id, now_ms()))
}

#[tauri::command]
pub fn usage_seen(window: WebviewWindow, ids: Vec<String>) -> Result<Usage, String> {
    change(&window, |u| u.seen(&ids, now_ms()))
}

#[tauri::command]
pub fn usage_notice(window: WebviewWindow, pill: String, mode: String) -> Result<Usage, String> {
    change(&window, |u| u.notice(&pill, &mode, now_ms()))
}

#[tauri::command]
pub fn usage_dismiss(window: WebviewWindow, pill: String, mode: String) -> Result<Usage, String> {
    change(&window, |u| u.dismiss(&pill, &mode))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opening_a_pill_counts_it_and_starts_its_clock_once() {
        let mut u = Usage::default();
        u.touch("integration_tasks", 1_000);
        u.touch("integration_tasks", 5_000);
        let t = &u.pills["integration_tasks"];
        assert_eq!((t.uses, t.last, t.since), (2, 5_000, 1_000));
    }

    #[test]
    fn a_pill_that_is_only_seen_has_a_clock_but_no_use() {
        let mut u = Usage::default();
        u.seen(&["integration_pad".into(), "integration_tasks".into()], 7_000);
        u.seen(&["integration_pad".into()], 9_000);
        let p = &u.pills["integration_pad"];
        assert_eq!((p.uses, p.last, p.since), (0, 0, 7_000), "the first sight stays");
        u.touch("integration_pad", 20_000);
        assert_eq!(u.pills["integration_pad"].since, 7_000);
    }

    #[test]
    fn notices_are_kept_once_per_pill_and_mode_and_can_be_dismissed() {
        let mut u = Usage::default();
        u.notice("integration_pad", "work", 1);
        u.notice("integration_pad", "work", 2);
        u.notice("integration_pad", "rest", 3);
        assert_eq!(u.notices.len(), 2);
        assert_eq!((u.notices[0].mode.as_str(), u.notices[0].at), ("work", 2), "a repeat replaces the older note");
        assert_eq!(u.notices[1].mode, "rest");
        u.dismiss("integration_pad", "work");
        assert_eq!(u.notices.iter().map(|n| n.mode.as_str()).collect::<Vec<_>>(), ["rest"]);
        for i in 0..30 {
            u.notice(&format!("pill_{i}"), "work", i);
        }
        assert_eq!(u.notices.len(), MAX_NOTICES);
        assert_eq!(u.notices.last().unwrap().pill, "pill_29");
    }

    #[test]
    fn odd_ids_and_too_many_pills_are_ignored() {
        let mut u = Usage::default();
        u.touch("", 1);
        u.touch("has space", 1);
        u.touch(&"x".repeat(MAX_ID + 1), 1);
        u.notice("ok", "bad mode!", 1);
        assert!(u.pills.is_empty() && u.notices.is_empty());
        for i in 0..MAX_PILLS + 10 {
            u.touch(&format!("pill_{i}"), 1);
        }
        assert_eq!(u.pills.len(), MAX_PILLS);
        u.touch("pill_0", 2);
        assert_eq!(u.pills["pill_0"].uses, 2, "a known pill still counts when the table is full");
    }

    #[test]
    fn a_file_round_trips_and_a_bad_one_is_brought_back_in_line() {
        let mut u = Usage::default();
        u.touch("integration_tasks", 10);
        u.notice("integration_pad", "work", 11);
        let json = serde_json::to_string(&u).unwrap();
        assert!(json.contains("\"uses\"") && json.contains("\"notices\""));
        assert_eq!(serde_json::from_str::<Usage>(&json).unwrap().cleaned(), u);
        let odd = serde_json::from_str::<Usage>(r#"{"pills":{"bad id":{"uses":3}},"notices":[{"pill":"a b","mode":"x"}]}"#).unwrap().cleaned();
        assert!(odd.pills.is_empty() && odd.notices.is_empty());
        assert_eq!(serde_json::from_str::<Usage>("{}").unwrap(), Usage::default());
    }
}
