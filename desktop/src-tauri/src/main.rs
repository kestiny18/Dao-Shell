#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod bridge;
mod directory;
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
    session_id: String,
    request_id: String,
    kind: String,
    input: String,
    on_event: Channel<serde_json::Value>,
) -> Result<dao_shell::session::Reply, String> {
    let bridge = state.inner().clone();
    bridge.reserve_for(&session_id, &request_id)?;
    tauri::async_runtime::spawn_blocking(move || {
        bridge.execute(&session_id, &request_id, &kind, input, on_event)
    })
    .await
    .map_err(|_| "桌面请求意外中断，请重启入口".to_string())?
}

#[tauri::command]
fn confirm_open(
    state: State<'_, Arc<Bridge>>,
    session_id: String,
    execution_id: String,
    request_id: String,
    approved: bool,
) -> Result<(), String> {
    state.confirm(&session_id, &execution_id, &request_id, approved)
}

#[tauri::command]
fn cancel(
    state: State<'_, Arc<Bridge>>,
    session_id: Option<String>,
    request_id: Option<String>,
) -> Result<(), String> {
    match (session_id, request_id) {
        (Some(s), Some(r)) => state.cancel_for(&s, &r),
        (None, None) => {
            state.cancel();
            Ok(())
        }
        _ => Err("请求标识缺失".into()),
    }
}

#[tauri::command]
fn load_workspace(
    state: State<'_, Arc<Bridge>>,
) -> Result<dao_shell::workspace::Workspace, String> {
    state.load_workspace()
}
#[tauri::command]
fn save_workspace(
    state: State<'_, Arc<Bridge>>,
    workspace: dao_shell::workspace::Workspace,
) -> Result<(), String> {
    state.save_workspace(workspace)
}
#[tauri::command]
fn register_session(state: State<'_, Arc<Bridge>>, session_id: String) -> Result<(), String> {
    state.register(session_id)
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

#[tauri::command]
async fn choose_directory(window: tauri::Window) -> Result<Option<String>, String> {
    #[cfg(windows)]
    let owner = window.hwnd().map_err(|e| e.to_string())?.0 as isize;
    #[cfg(not(windows))]
    let owner = {
        let _ = window;
        0
    };
    tauri::async_runtime::spawn_blocking(move || directory::choose(owner))
        .await
        .map_err(|_| "目录选择中断".to_string())?
}

#[tauri::command]
fn finish_close(window: tauri::Window) -> Result<(), String> {
    window.destroy().map_err(|e| e.to_string())
}

fn main() {
    tauri::Builder::default()
        .manage(Arc::new(Bridge::new()))
        .invoke_handler(tauri::generate_handler![
            finish_close,
            choose_directory,
            session_info,
            load_workspace,
            save_workspace,
            register_session,
            perform,
            confirm_open,
            cancel,
            get_settings,
            save_settings,
            test_connection,
            computer_overview
        ])
        .on_window_event(|window, event| {
            use tauri::{Emitter, Manager};
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                window.state::<Arc<Bridge>>().cancel();
                let _ = window.emit("workspace-close-requested", ());
            }
            if matches!(event, tauri::WindowEvent::Destroyed) {
                window.state::<Arc<Bridge>>().cancel();
            }
        })
        .run(tauri::generate_context!())
        .expect("unable to start Dao-Shell desktop");
}
