// The Tasks and Notepad pills: a list of things to do and a page of notes, both yours, kept in
// personal.json next to settings.json. Nothing here talks to the network, a model or another
// program: the island reads and edits them through the commands below, and that is all.
//
// The file holds what you typed, so it is never logged; the pills only exist while a mode (or
// Settings) has them on, but the content stays in the file when they are off.

use std::path::PathBuf;
use std::sync::{LazyLock, Mutex, MutexGuard, PoisonError};

use serde::{Deserialize, Serialize};
use tauri::WebviewWindow;

pub const MAX_TASKS: usize = 200;
const MAX_TASK_CHARS: usize = 200;
const MAX_PAD_CHARS: usize = 20_000;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Task {
    pub id: u64,
    pub text: String,
    pub done: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Personal {
    /// Newest first.
    pub tasks: Vec<Task>,
    /// The notepad, as typed.
    pub pad: String,
    next_id: u64,
}

impl Personal {
    /// What a file (or the webview) may hold, brought back inside the rules.
    pub fn cleaned(mut self) -> Personal {
        self.tasks.retain(|t| !t.text.trim().is_empty());
        self.tasks.truncate(MAX_TASKS);
        for task in &mut self.tasks {
            task.text = one_line(&task.text);
        }
        self.pad = self.pad.chars().take(MAX_PAD_CHARS).collect();
        self.next_id = self.next_id.max(self.tasks.iter().map(|t| t.id).max().unwrap_or(0));
        self
    }

    /// Adds a task at the top. False when it is empty or the list is full.
    pub fn add(&mut self, text: &str) -> bool {
        let text = one_line(text);
        if text.is_empty() || self.tasks.len() >= MAX_TASKS {
            return false;
        }
        self.next_id += 1;
        self.tasks.insert(0, Task { id: self.next_id, text, done: false });
        true
    }

    pub fn toggle(&mut self, id: u64) {
        if let Some(task) = self.tasks.iter_mut().find(|t| t.id == id) {
            task.done = !task.done;
        }
    }

    pub fn delete(&mut self, id: u64) {
        self.tasks.retain(|t| t.id != id);
    }

    pub fn clear_done(&mut self) {
        self.tasks.retain(|t| !t.done);
    }

    pub fn set_pad(&mut self, text: &str) {
        self.pad = text.chars().take(MAX_PAD_CHARS).collect();
    }
}

fn one_line(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ").chars().take(MAX_TASK_CHARS).collect()
}

fn path() -> PathBuf {
    crate::platform::config_dir().join("personal.json")
}

fn load() -> Personal {
    std::fs::read(path()).ok().and_then(|b| serde_json::from_slice::<Personal>(&b).ok()).unwrap_or_default().cleaned()
}

fn save(personal: &Personal) {
    let path = path();
    let Some(dir) = path.parent() else { return };
    let temp = path.with_extension(format!("json.coucou-{}", std::process::id()));
    let written = crate::platform::ensure_private_dir(dir)
        .and_then(|()| serde_json::to_vec_pretty(personal).map_err(std::io::Error::other))
        .and_then(|json| std::fs::write(&temp, json))
        .and_then(|()| std::fs::rename(&temp, &path));
    if let Err(err) = written {
        let _ = std::fs::remove_file(&temp);
        // What failed, never what was typed.
        crate::log::line(format!("personal: could not save personal.json: {err}"));
    }
}

static DATA: LazyLock<Mutex<Personal>> = LazyLock::new(|| Mutex::new(load()));

fn data() -> MutexGuard<'static, Personal> {
    DATA.lock().unwrap_or_else(PoisonError::into_inner)
}

fn only(window: &WebviewWindow) -> Result<(), String> {
    if window.label() == crate::island::WINDOW_LABEL { Ok(()) } else { Err("not from this window".into()) }
}

/// Changes the data, keeps it on disk, and answers with how it stands now.
fn change(window: &WebviewWindow, edit: impl FnOnce(&mut Personal)) -> Result<Personal, String> {
    only(window)?;
    let mut d = data();
    edit(&mut d);
    save(&d);
    Ok(d.clone())
}

#[tauri::command]
pub fn personal_get(window: WebviewWindow) -> Result<Personal, String> {
    only(&window)?;
    Ok(data().clone())
}

#[tauri::command]
pub fn personal_add_task(window: WebviewWindow, text: String) -> Result<Personal, String> {
    change(&window, |d| {
        d.add(&text);
    })
}

#[tauri::command]
pub fn personal_toggle_task(window: WebviewWindow, id: u64) -> Result<Personal, String> {
    change(&window, |d| d.toggle(id))
}

#[tauri::command]
pub fn personal_delete_task(window: WebviewWindow, id: u64) -> Result<Personal, String> {
    change(&window, |d| d.delete(id))
}

#[tauri::command]
pub fn personal_clear_done(window: WebviewWindow) -> Result<Personal, String> {
    change(&window, Personal::clear_done)
}

#[tauri::command]
pub fn personal_set_pad(window: WebviewWindow, text: String) -> Result<(), String> {
    change(&window, |d| d.set_pad(&text)).map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tasks_come_in_on_top_one_line_each_and_empty_ones_are_refused() {
        let mut p = Personal::default();
        assert!(p.add("  ligar para\n o   Rafael "));
        assert!(p.add("comprar papel"));
        assert!(!p.add("   \n "));
        let texts: Vec<&str> = p.tasks.iter().map(|t| t.text.as_str()).collect();
        assert_eq!(texts, ["comprar papel", "ligar para o Rafael"]);
        assert_ne!(p.tasks[0].id, p.tasks[1].id);
    }

    #[test]
    fn ticking_deleting_and_clearing_touch_only_what_they_name() {
        let mut p = Personal::default();
        for t in ["a", "b", "c"] {
            p.add(t);
        }
        let b = p.tasks.iter().find(|t| t.text == "b").unwrap().id;
        let c = p.tasks.iter().find(|t| t.text == "c").unwrap().id;
        p.toggle(b);
        p.toggle(c);
        p.toggle(c);
        assert_eq!(p.tasks.iter().filter(|t| t.done).count(), 1);
        p.clear_done();
        assert_eq!(p.tasks.iter().map(|t| t.text.as_str()).collect::<Vec<_>>(), ["c", "a"]);
        p.delete(c);
        assert_eq!(p.tasks.len(), 1);
        p.toggle(9999);
        p.delete(9999);
        assert_eq!(p.tasks.len(), 1);
    }

    #[test]
    fn a_new_task_never_reuses_an_id_after_a_delete() {
        let mut p = Personal::default();
        p.add("a");
        let first = p.tasks[0].id;
        p.delete(first);
        p.add("b");
        assert!(p.tasks[0].id > first);
    }

    #[test]
    fn the_lists_stop_at_their_limits() {
        let mut p = Personal::default();
        for i in 0..MAX_TASKS + 5 {
            p.add(&format!("tarefa {i}"));
        }
        assert_eq!(p.tasks.len(), MAX_TASKS);
        assert!(p.add(&"x".repeat(1)) == false);
        p.set_pad(&"n".repeat(MAX_PAD_CHARS + 10));
        assert_eq!(p.pad.chars().count(), MAX_PAD_CHARS);
        let long = Personal { tasks: vec![Task { id: 1, text: "y".repeat(500), done: false }], ..Default::default() }.cleaned();
        assert_eq!(long.tasks[0].text.chars().count(), MAX_TASK_CHARS);
    }

    #[test]
    fn a_file_keeps_ids_moving_forward_and_survives_a_round_trip() {
        let mut p = Personal::default();
        p.add("a");
        p.add("b");
        p.set_pad("notas\ncom linhas");
        let json = serde_json::to_string(&p).unwrap();
        assert!(json.contains("\"tasks\"") && json.contains("\"pad\"") && json.contains("\"nextId\""));
        let mut back = serde_json::from_str::<Personal>(&json).unwrap().cleaned();
        assert_eq!(back, p);
        back.add("c");
        assert!(back.tasks[0].id > p.tasks[0].id);
        assert!(serde_json::from_str::<Personal>("{}").unwrap().tasks.is_empty());
    }
}
