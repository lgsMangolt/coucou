// The commands of the two WhatsApp accounts. src/whatsapp/mod.rs is compiled twice (lib.rs): as
// `whatsapp`, the first account, and as `whatsapp2`, the second, each with its own state, files,
// gateway, port and keystore entries. The functions there are plain; the commands the pages call
// are here, one set per account. `whatsapp_*` is the first account's (the names the pages always
// had), `whatsapp2_*` the second's.

use tauri::WebviewWindow;

use crate::{whatsapp, whatsapp2};

// ── The first account ──
#[tauri::command]
pub fn whatsapp_island(window: WebviewWindow) -> Result<whatsapp::IslandView, String> {
    whatsapp::whatsapp_island(window)
}

#[tauri::command]
pub fn whatsapp_open(window: WebviewWindow, chat: String) -> Result<(), String> {
    whatsapp::whatsapp_open(window, chat)
}

#[tauri::command]
pub fn whatsapp_close(window: WebviewWindow, chat: String) -> Result<(), String> {
    whatsapp::whatsapp_close(window, chat)
}

#[tauri::command]
pub fn whatsapp_open_app(window: WebviewWindow) -> Result<bool, String> {
    whatsapp::whatsapp_open_app(window)
}

#[tauri::command]
pub async fn whatsapp_send(window: WebviewWindow, chat: String, text: String) -> Result<bool, String> {
    whatsapp::whatsapp_send(window, chat, text).await
}

#[tauri::command]
pub async fn whatsapp_suggest(window: WebviewWindow, chat: String) -> Result<String, String> {
    whatsapp::whatsapp_suggest(window, chat).await
}

#[tauri::command]
pub fn whatsapp_ignore(window: WebviewWindow, chat: String) -> Result<(), String> {
    whatsapp::whatsapp_ignore(window, chat)
}

#[tauri::command]
pub fn whatsapp_unignore(window: WebviewWindow, chat: String) -> Result<(), String> {
    whatsapp::whatsapp_unignore(window, chat)
}

#[tauri::command]
pub fn whatsapp_settings(window: WebviewWindow) -> Result<whatsapp::SettingsView, String> {
    whatsapp::whatsapp_settings(window)
}

#[tauri::command]
pub fn whatsapp_set_options(window: WebviewWindow, enabled: bool, mute_groups: bool, mark_read_on_open: bool) -> Result<(), String> {
    whatsapp::whatsapp_set_options(window, enabled, mute_groups, mark_read_on_open)
}

#[tauri::command]
pub fn whatsapp_save_gateway(window: WebviewWindow, gateway: String, url: String, credential: Option<String>, port: u16, auto_start: bool, gowa_path: String) -> Result<(), String> {
    whatsapp::whatsapp_save_gateway(window, gateway, url, credential, port, auto_start, gowa_path)
}

#[tauri::command]
pub async fn whatsapp_test(window: WebviewWindow) -> Result<whatsapp::TestResult, String> {
    whatsapp::whatsapp_test(window).await
}

#[tauri::command]
pub fn whatsapp_install(window: WebviewWindow) -> Result<(), String> {
    whatsapp::whatsapp_install(window)
}

#[tauri::command]
pub async fn whatsapp_ledger_save(window: WebviewWindow, id: u64) -> Result<String, String> {
    whatsapp::whatsapp_ledger_save(window, id).await
}

#[tauri::command]
pub fn whatsapp_ledger_discard(window: WebviewWindow, id: u64) -> Result<(), String> {
    whatsapp::whatsapp_ledger_discard(window, id)
}

#[tauri::command]
pub async fn whatsapp_order_quote(window: WebviewWindow, id: u64) -> Result<(), String> {
    whatsapp::whatsapp_order_quote(window, id).await
}

#[tauri::command]
pub fn whatsapp_order_discard(window: WebviewWindow, id: u64) -> Result<(), String> {
    whatsapp::whatsapp_order_discard(window, id)
}

#[tauri::command]
pub async fn whatsapp_order_file(window: WebviewWindow, id: u64) -> Result<String, String> {
    whatsapp::whatsapp_order_file(window, id).await
}

#[tauri::command]
pub fn whatsapp_set_ledger(window: WebviewWindow, enabled: bool, dir: String, chat: String) -> Result<(), String> {
    whatsapp::whatsapp_set_ledger(window, enabled, dir, chat)
}

#[tauri::command]
pub fn whatsapp_set_ledger_sheet(window: WebviewWindow, url: String, code: Option<String>) -> Result<(), String> {
    whatsapp::whatsapp_set_ledger_sheet(window, url, code)
}

#[tauri::command]
pub async fn whatsapp_ledger_sheet_test(window: WebviewWindow) -> Result<(), String> {
    whatsapp::whatsapp_ledger_sheet_test(window).await
}

#[tauri::command]
pub fn whatsapp_set_group_limit(window: WebviewWindow, limit: u32) -> Result<(), String> {
    whatsapp::whatsapp_set_group_limit(window, limit)
}

// ── The second account ──
#[tauri::command]
pub fn whatsapp2_island(window: WebviewWindow) -> Result<whatsapp2::IslandView, String> {
    whatsapp2::whatsapp_island(window)
}

#[tauri::command]
pub fn whatsapp2_open(window: WebviewWindow, chat: String) -> Result<(), String> {
    whatsapp2::whatsapp_open(window, chat)
}

#[tauri::command]
pub fn whatsapp2_close(window: WebviewWindow, chat: String) -> Result<(), String> {
    whatsapp2::whatsapp_close(window, chat)
}

#[tauri::command]
pub fn whatsapp2_open_app(window: WebviewWindow) -> Result<bool, String> {
    whatsapp2::whatsapp_open_app(window)
}

#[tauri::command]
pub async fn whatsapp2_send(window: WebviewWindow, chat: String, text: String) -> Result<bool, String> {
    whatsapp2::whatsapp_send(window, chat, text).await
}

#[tauri::command]
pub async fn whatsapp2_suggest(window: WebviewWindow, chat: String) -> Result<String, String> {
    whatsapp2::whatsapp_suggest(window, chat).await
}

#[tauri::command]
pub fn whatsapp2_ignore(window: WebviewWindow, chat: String) -> Result<(), String> {
    whatsapp2::whatsapp_ignore(window, chat)
}

#[tauri::command]
pub fn whatsapp2_unignore(window: WebviewWindow, chat: String) -> Result<(), String> {
    whatsapp2::whatsapp_unignore(window, chat)
}

#[tauri::command]
pub fn whatsapp2_settings(window: WebviewWindow) -> Result<whatsapp2::SettingsView, String> {
    whatsapp2::whatsapp_settings(window)
}

#[tauri::command]
pub fn whatsapp2_set_options(window: WebviewWindow, enabled: bool, mute_groups: bool, mark_read_on_open: bool) -> Result<(), String> {
    whatsapp2::whatsapp_set_options(window, enabled, mute_groups, mark_read_on_open)
}

#[tauri::command]
pub fn whatsapp2_save_gateway(window: WebviewWindow, gateway: String, url: String, credential: Option<String>, port: u16, auto_start: bool, gowa_path: String) -> Result<(), String> {
    whatsapp2::whatsapp_save_gateway(window, gateway, url, credential, port, auto_start, gowa_path)
}

#[tauri::command]
pub async fn whatsapp2_test(window: WebviewWindow) -> Result<whatsapp2::TestResult, String> {
    whatsapp2::whatsapp_test(window).await
}

#[tauri::command]
pub fn whatsapp2_install(window: WebviewWindow) -> Result<(), String> {
    whatsapp2::whatsapp_install(window)
}

#[tauri::command]
pub fn whatsapp2_set_group_limit(window: WebviewWindow, limit: u32) -> Result<(), String> {
    whatsapp2::whatsapp_set_group_limit(window, limit)
}

#[tauri::command]
pub async fn whatsapp2_order_quote(window: WebviewWindow, id: u64) -> Result<(), String> {
    whatsapp2::whatsapp_order_quote(window, id).await
}

#[tauri::command]
pub fn whatsapp2_order_discard(window: WebviewWindow, id: u64) -> Result<(), String> {
    whatsapp2::whatsapp_order_discard(window, id)
}

#[tauri::command]
pub async fn whatsapp2_order_file(window: WebviewWindow, id: u64) -> Result<String, String> {
    whatsapp2::whatsapp_order_file(window, id).await
}
