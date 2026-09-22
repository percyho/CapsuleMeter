mod usage;
mod window_pos;
use serde::Serialize;
use std::process::Command;
use tauri::Manager;

#[derive(Serialize)]
struct UpdateInfo {
    current: String,
    latest: Option<String>,
    url: Option<String>,
    error: Option<String>,
}

#[tauri::command]
fn start_codex_login() -> Result<(), String> {
    #[cfg(windows)]
    Command::new("cmd")
        .args(["/C", "start", "", "codex", "login"])
        .spawn()
        .map_err(|e| format!("无法启动 codex login：{e}"))?;
    #[cfg(not(windows))]
    Command::new("codex")
        .arg("login")
        .spawn()
        .map_err(|e| format!("无法启动 codex login：{e}"))?;
    Ok(())
}

#[tauri::command]
fn diagnose_codex() -> Result<String, String> {
    let output = Command::new("codex")
        .arg("doctor")
        .output()
        .map_err(|e| format!("无法运行 codex doctor：{e}"))?;
    let text = if output.status.success() {
        output.stdout
    } else {
        output.stderr
    };
    Ok(String::from_utf8_lossy(&text).trim().to_string())
}

#[tauri::command]
async fn check_update(app: tauri::AppHandle) -> Result<UpdateInfo, String> {
    let current = app.package_info().version.to_string();
    let result = reqwest::Client::new()
        .get("https://gitee.com/api/v5/repos/siteweb/codexc-apsule/releases/latest")
        .header("User-Agent", "CodexCapsule")
        .send()
        .await;
    let response = match result {
        Ok(value) => value,
        Err(error) => {
            return Ok(UpdateInfo {
                current,
                latest: None,
                url: None,
                error: Some(error.to_string()),
            })
        }
    };
    let value: serde_json::Value = match response.json().await {
        Ok(value) => value,
        Err(error) => {
            return Ok(UpdateInfo {
                current,
                latest: None,
                url: None,
                error: Some(error.to_string()),
            })
        }
    };
    Ok(UpdateInfo {
        current,
        latest: value
            .get("tag_name")
            .and_then(|v| v.as_str())
            .map(str::to_string),
        url: value
            .get("html_url")
            .and_then(|v| v.as_str())
            .map(str::to_string),
        error: None,
    })
}

#[tauri::command]
fn open_history(app: tauri::AppHandle) {
    if let Some(w) = app.get_webview_window("history") {
        let _ = w.show();
        let _ = w.set_focus();
        return;
    }
    use tauri::WebviewWindowBuilder;
    let _ = WebviewWindowBuilder::new(
        &app,
        "history",
        tauri::WebviewUrl::App("history.html".into()),
    )
    .title("使用历史")
    .inner_size(920.0, 600.0)
    .resizable(true)
    .decorations(false)
    .build();
}

#[tauri::command]
fn write_history_csv(path: String, contents: String) -> Result<(), String> {
    std::fs::write(path, contents).map_err(|error| format!("写入 CSV 失败：{error}"))
}

fn usage_ring_icon(five_hour: f64, weekly: f64) -> tauri::image::Image<'static> {
    let size = 32u32;
    let mut rgba = vec![0u8; (size * size * 4) as usize];
    let tau = std::f64::consts::TAU;
    for y in 0..size {
        for x in 0..size {
            let dx = x as f64 - 15.5;
            let dy = y as f64 - 15.5;
            let distance = (dx * dx + dy * dy).sqrt();
            let angle = dx.atan2(-dy).rem_euclid(tau);
            let (color, alpha) = if (11.0..=14.5).contains(&distance) {
                let active = angle <= weekly.clamp(0.0, 100.0) / 100.0 * tau;
                (if active { [240, 92, 104] } else { [78, 84, 96] }, 255)
            } else if (6.0..=9.5).contains(&distance) {
                let active = angle <= five_hour.clamp(0.0, 100.0) / 100.0 * tau;
                (if active { [66, 164, 245] } else { [78, 84, 96] }, 255)
            } else {
                ([0, 0, 0], 0)
            };
            let index = ((y * size + x) * 4) as usize;
            rgba[index..index + 4].copy_from_slice(&[color[0], color[1], color[2], alpha]);
        }
    }
    tauri::image::Image::new_owned(rgba, size, size)
}

#[tauri::command]
fn update_tray_icon(
    app: tauri::AppHandle,
    mode: String,
    five_hour: f64,
    weekly: f64,
) -> Result<(), String> {
    let tray = app
        .tray_by_id("main-tray")
        .ok_or_else(|| "托盘尚未初始化".to_string())?;
    let icon = if mode == "usage" {
        usage_ring_icon(five_hour, weekly)
    } else {
        app.default_window_icon()
            .cloned()
            .ok_or_else(|| "应用 Logo 不可用".to_string())?
    };
    tray.set_icon(Some(icon))
        .map_err(|error| format!("更新托盘图标失败：{error}"))?;
    let tooltip = if mode == "usage" {
        format!(
            "Codex Capsule · 5 小时 {:.0}% · 每周 {:.0}%",
            five_hour, weekly
        )
    } else {
        "Codex Capsule".to_string()
    };
    tray.set_tooltip(Some(tooltip))
        .map_err(|error| format!("更新托盘提示失败：{error}"))
}

#[cfg(test)]
mod tests {
    use super::usage_ring_icon;

    #[test]
    fn usage_ring_has_expected_dimensions_and_visible_pixels() {
        let image = usage_ring_icon(50.0, 75.0);
        assert_eq!(image.width(), 32);
        assert_eq!(image.height(), 32);
        assert_eq!(image.rgba().len(), 32 * 32 * 4);
        assert!(image.rgba().chunks_exact(4).any(|pixel| pixel[3] > 0));
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
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
            open_history,
            write_history_csv,
            start_codex_login,
            diagnose_codex,
            check_update,
            update_tray_icon
        ])
        .setup(|app| {
            use tauri::{
                menu::{Menu, MenuItem},
                tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
                Emitter,
            };
            window_pos::restore(app.handle());
            let show = MenuItem::with_id(app, "show", "显示胶囊", true, None::<&str>)?;
            let refresh = MenuItem::with_id(app, "refresh", "立即刷新", true, None::<&str>)?;
            let settings = MenuItem::with_id(app, "settings", "打开设置", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show, &refresh, &settings, &quit])?;
            TrayIconBuilder::with_id("main-tray")
                .icon(app.default_window_icon().unwrap().clone())
                .tooltip("Codex Capsule")
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "show" => {
                        if let Some(w) = app.get_webview_window("main") {
                            let _ = w.show();
                            let _ = w.set_focus();
                        }
                    }
                    "refresh" => {
                        let _ = app.emit("tray-refresh", ());
                    }
                    "settings" => {
                        if let Some(w) = app.get_webview_window("main") {
                            let _ = w.show();
                            let _ = w.set_focus();
                        }
                        let _ = app.emit("tray-open-settings", ());
                    }
                    "quit" => app.exit(0),
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        if let Some(w) = tray.app_handle().get_webview_window("main") {
                            let _ = w.show();
                            let _ = w.set_focus();
                        }
                    }
                })
                .build(app)?;
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while running tauri application")
        .run(|app_handle, event| {
            window_pos::on_event(app_handle, event);
        });
}
