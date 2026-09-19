#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod bridge;
use bridge::Bridge;
use dao_shell::session::Action;
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
    let action = match kind.as_str() {
        "say" => Action::Say(input),
        "search" => Action::Search(input),
        "open" => Action::Open(input),
        "reset" => Action::Reset,
        _ => return Err("不支持的请求".into()),
    };
    let bridge = state.inner().clone();
    bridge.reserve()?;
    tauri::async_runtime::spawn_blocking(move || bridge.execute(action, on_event))
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

fn main() {
    tauri::Builder::default()
        .manage(Arc::new(Bridge::new()))
        .invoke_handler(tauri::generate_handler![
            session_info,
            perform,
            confirm_open,
            cancel
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
