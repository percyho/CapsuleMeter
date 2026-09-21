mod usage;
mod window_pos;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
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
            window_pos::set_snap_enabled
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
