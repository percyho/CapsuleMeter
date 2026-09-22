mod usage;
mod window_pos;
use tauri::Manager;

#[tauri::command]
fn open_history(app: tauri::AppHandle) {
    if let Some(w) = app.get_webview_window("history") {
        let _ = w.show();
        let _ = w.set_focus();
        return;
    }
    use tauri::WebviewWindowBuilder;
    let _ = WebviewWindowBuilder::new(&app, "history", tauri::WebviewUrl::App("history.html".into()))
        .title("使用历史")
        .inner_size(920.0, 600.0)
        .resizable(true)
        .build();
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .manage(window_pos::WindowPosState(Default::default()))
        .manage(window_pos::SnapState {
            last_move: Default::default(),
        })
        .manage(window_pos::SnapEnabled(Default::default()))
        .invoke_handler(tauri::generate_handler![
            usage::fetch_usage,
            usage::fetch_analytics,
            window_pos::set_snap_enabled,
            open_history
        ])
        .setup(|app| {
            window_pos::restore(app.handle());
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while running tauri application")
        .run(|app_handle, event| {
            window_pos::on_event(app_handle, event);
        });
}
