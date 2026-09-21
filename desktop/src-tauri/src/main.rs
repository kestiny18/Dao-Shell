#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod bridge;
use bridge::Bridge;
use dao_shell::settings::{ConnectionTest, SettingsUpdate, SettingsView};
use std::sync::Arc;
use tauri::{State, ipc::Channel};

#[tauri::command]
fn session_info(state: State<'_, Arc<Bridge>>) -> Result<serde_json::Value, String> {
    state.info()
}

#[tauri::command]
async fn perform(
    state: State<'_, Arc<Bridge>>,
    kind: String,
    input: String,
    on_event: Channel<serde_json::Value>,
) -> Result<dao_shell::session::Reply, String> {
    let bridge = state.inner().clone();
    bridge.reserve()?;
    tauri::async_runtime::spawn_blocking(move || bridge.execute(&kind, input, on_event))
        .await
        .map_err(|_| "桌面请求意外中断，请重启入口".to_string())?
}

#[tauri::command]
fn confirm_open(
    state: State<'_, Arc<Bridge>>,
    request_id: String,
    approved: bool,
) -> Result<(), String> {
    state.confirm(&request_id, approved)
}

#[tauri::command]
fn cancel(state: State<'_, Arc<Bridge>>) {
    state.cancel();
}

#[tauri::command]
fn get_settings(state: State<'_, Arc<Bridge>>) -> Result<SettingsView, String> {
    state.settings()
}

#[tauri::command]
async fn save_settings(
    state: State<'_, Arc<Bridge>>,
    update: SettingsUpdate,
) -> Result<SettingsView, String> {
    let bridge = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || bridge.save(update))
        .await
        .map_err(|_| "配置请求中断".to_string())?
}
#[tauri::command]
async fn test_connection(
    state: State<'_, Arc<Bridge>>,
    request: ConnectionTest,
) -> Result<serde_json::Value, String> {
    let bridge = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || bridge.test(request))
        .await
        .map_err(|_| "连接检测中断".to_string())?
}
#[tauri::command]
async fn computer_overview(state: State<'_, Arc<Bridge>>) -> Result<serde_json::Value, String> {
    let bridge = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || bridge.overview())
        .await
        .map_err(|_| "概览读取中断".to_string())?
}

fn main() {
    tauri::Builder::default()
        .manage(Arc::new(Bridge::new()))
        .invoke_handler(tauri::generate_handler![
            session_info,
            perform,
            confirm_open,
            cancel,
            get_settings,
            save_settings,
            test_connection,
            computer_overview
        ])
        .on_window_event(|window, event| {
            if matches!(
                event,
                tauri::WindowEvent::CloseRequested { .. } | tauri::WindowEvent::Destroyed
            ) {
                use tauri::Manager;
                window.state::<Arc<Bridge>>().cancel();
            }
        })
        .run(tauri::generate_context!())
        .expect("unable to start Dao-Shell desktop");
}
