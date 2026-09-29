mod codex_cli;
mod reset_credits;
mod usage;
mod window_pos;
use serde::Serialize;
use std::{
    process::Command,
    sync::{
        atomic::{AtomicU8, Ordering},
        Arc,
    },
};
use tauri::{menu::MenuItem, Emitter, Manager, PhysicalPosition};

#[derive(Clone)]
struct TrayIconMenuState {
    switcher: MenuItem<tauri::Wry>,
    selected_mode: Arc<AtomicU8>,
}

impl TrayIconMenuState {
    fn select(&self, mode: &str) -> Result<(), String> {
        let (selected_mode, next_label) = match mode {
            "usage" => (1, "Logo"),
            _ => (0, "用量环"),
        };
        self.switcher
            .set_text(next_label)
            .map_err(|error| format!("更新托盘切换菜单失败：{error}"))?;
        self.selected_mode.store(selected_mode, Ordering::Release);
        Ok(())
    }

    fn toggle(&self) -> Result<&'static str, String> {
        let next_mode = match self.selected_mode.load(Ordering::Acquire) {
            1 => "logo",
            _ => "usage",
        };
        self.select(next_mode)?;
        Ok(next_mode)
    }
}

#[cfg(target_os = "macos")]
#[derive(Clone)]
struct CapsuleWidgetMenuState {
    item: MenuItem<tauri::Wry>,
}

#[cfg(target_os = "macos")]
impl CapsuleWidgetMenuState {
    fn set_visible(&self, visible: bool) -> Result<(), String> {
        let label = if visible {
            "隐藏桌面胶囊小工具"
        } else {
            "显示桌面胶囊小工具"
        };
        self.item
            .set_text(label)
            .map_err(|error| format!("更新桌面小工具菜单失败：{error}"))
    }
}

#[derive(Serialize)]
struct UpdateInfo {
    current: String,
    latest: Option<String>,
    url: Option<String>,
    error: Option<String>,
}

#[tauri::command]
fn is_macos() -> bool {
    cfg!(target_os = "macos")
}

#[tauri::command]
fn set_capsule_widget_visible(
    app: tauri::AppHandle,
    visible: bool,
    x: Option<i32>,
    y: Option<i32>,
) -> Result<(), String> {
    let window = app
        .get_webview_window("capsule-widget")
        .ok_or_else(|| "桌面胶囊小工具窗口未初始化".to_string())?;
    if !visible {
        window
            .hide()
            .map_err(|error| format!("隐藏桌面小工具失败：{error}"))?;
        #[cfg(target_os = "macos")]
        {
            app.state::<CapsuleWidgetMenuState>().set_visible(false)?;
            let _ = app.emit("capsule-widget-state", false);
        }
        return Ok(());
    }

    if let (Some(x), Some(y)) = (x, y) {
        window
            .set_position(PhysicalPosition::new(x, y))
            .map_err(|error| format!("设置桌面小工具位置失败：{error}"))?;
    } else {
        let monitor = window
            .current_monitor()
            .map_err(|error| format!("获取桌面小工具屏幕失败：{error}"))?
            .or_else(|| window.primary_monitor().ok().flatten());
        if let Some(monitor) = monitor {
            let scale = window.scale_factor().unwrap_or(1.0);
            let monitor_position = monitor.position();
            let monitor_size = monitor.size();
            let inset = (164.0 * scale).round() as i32;
            let top = (42.0 * scale).round() as i32;
            window
                .set_position(PhysicalPosition::new(
                    monitor_position.x + monitor_size.width as i32 - inset,
                    monitor_position.y + top,
                ))
                .map_err(|error| format!("设置桌面小工具位置失败：{error}"))?;
        }
    }

    window
        .set_always_on_top(true)
        .map_err(|error| format!("设置桌面小工具置顶失败：{error}"))?;
    window
        .show()
        .map_err(|error| format!("显示桌面小工具失败：{error}"))?;
    #[cfg(target_os = "macos")]
    {
        app.state::<CapsuleWidgetMenuState>().set_visible(true)?;
        let _ = app.emit("capsule-widget-state", true);
    }
    window
        .set_focus()
        .map_err(|error| format!("聚焦桌面小工具失败：{error}"))
}

#[tauri::command]
fn show_settings_window(app: tauri::AppHandle) -> Result<(), String> {
    let window = app
        .get_webview_window("main")
        .ok_or_else(|| "设置窗口未初始化".to_string())?;
    window
        .show()
        .map_err(|error| format!("显示设置窗口失败：{error}"))?;
    window
        .set_focus()
        .map_err(|error| format!("聚焦设置窗口失败：{error}"))
}

#[tauri::command]
fn start_codex_login() -> Result<(), String> {
    #[cfg(windows)]
    Command::new("cmd")
        .args(["/C", "start", "", "codex", "login"])
        .spawn()
        .map_err(|e| format!("无法启动 codex login：{e}"))?;
    #[cfg(not(windows))]
    Command::new(codex_cli::executable())
        .arg("login")
        .spawn()
        .map_err(|e| format!("无法启动 codex login：{e}"))?;
    Ok(())
}

#[tauri::command]
fn diagnose_codex() -> Result<String, String> {
    let output = Command::new(codex_cli::executable())
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
        .header("User-Agent", "CapsuleMeter")
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
fn open_history(app: tauri::AppHandle) -> Result<(), String> {
    let window = app
        .get_webview_window("history")
        .ok_or_else(|| "使用历史窗口未初始化，请重启应用后重试".to_string())?;
    window
        .show()
        .map_err(|error| format!("显示使用历史窗口失败：{error}"))?;
    window
        .set_focus()
        .map_err(|error| format!("聚焦使用历史窗口失败：{error}"))?;
    let _ = app.emit("history-window-shown", ());
    Ok(())
}

#[tauri::command]
fn write_history_csv(path: String, contents: String) -> Result<(), String> {
    std::fs::write(path, contents).map_err(|error| format!("写入 CSV 失败：{error}"))
}

#[tauri::command]
fn quit_app(app: tauri::AppHandle) {
    app.exit(0);
}

fn usage_ring_icon(five_hour: f64, weekly: f64) -> tauri::image::Image<'static> {
    let size = 32u32;
    let mut rgba = vec![0u8; (size * size * 4) as usize];
    let tau = std::f64::consts::TAU;
    let track_color = [209, 213, 219];
    for y in 0..size {
        for x in 0..size {
            let dx = x as f64 - 15.5;
            let dy = y as f64 - 15.5;
            let distance = (dx * dx + dy * dy).sqrt();
            let angle = dx.atan2(-dy).rem_euclid(tau);
            let (color, alpha) = if (11.0..=14.5).contains(&distance) {
                let active = angle <= weekly.clamp(0.0, 100.0) / 100.0 * tau;
                (if active { [240, 92, 104] } else { track_color }, 255)
            } else if (6.0..=9.5).contains(&distance) {
                let active = angle <= five_hour.clamp(0.0, 100.0) / 100.0 * tau;
                (if active { [66, 164, 245] } else { track_color }, 255)
            } else {
                ([0, 0, 0], 0)
            };
            let index = ((y * size + x) * 4) as usize;
            rgba[index..index + 4].copy_from_slice(&[color[0], color[1], color[2], alpha]);
        }
    }
    tauri::image::Image::new_owned(rgba, size, size)
}

fn monochrome_template_icon(source: &tauri::image::Image<'_>) -> tauri::image::Image<'static> {
    let mut rgba = Vec::with_capacity(source.rgba().len());
    for pixel in source.rgba().chunks_exact(4) {
        let luminance = (u32::from(pixel[0]) * 299
            + u32::from(pixel[1]) * 587
            + u32::from(pixel[2]) * 114)
            / 1000;
        // Keep the bright logo strokes and dots while dropping the dark app-icon fill.
        let mask = luminance.saturating_sub(40).min(50) * 255 / 50;
        let alpha = u32::from(pixel[3]) * mask / 255;
        rgba.extend_from_slice(&[255, 255, 255, alpha as u8]);
    }
    tauri::image::Image::new_owned(rgba, source.width(), source.height())
}

fn tray_reset_hint(seconds: Option<i64>) -> String {
    let Some(seconds) = seconds else {
        return String::new();
    };
    if seconds <= 0 {
        return " · 即将重置".to_string();
    }

    let minutes = seconds / 60;
    let days = minutes / (24 * 60);
    let hours = (minutes / 60) % 24;
    let remaining_minutes = minutes % 60;
    let duration = if days > 0 {
        format!("{days}天{hours}小时")
    } else if hours > 0 {
        format!("{hours}小时{remaining_minutes}分")
    } else {
        format!("{}分", remaining_minutes.max(1))
    };
    format!(" · {duration}后重置")
}

fn tray_usage_hint(label: &str, remaining: Option<f64>, reset_after: Option<i64>) -> String {
    let remaining = remaining
        .map(|value| format!("{value:.0}%"))
        .unwrap_or_else(|| "—".to_string());
    format!("{label} {remaining}{}", tray_reset_hint(reset_after))
}

fn tray_tooltip(
    five_hour: Option<f64>,
    weekly: Option<f64>,
    five_hour_reset_after_seconds: Option<i64>,
    weekly_reset_after_seconds: Option<i64>,
) -> String {
    format!(
        "{} | {}",
        tray_usage_hint("5小时", five_hour, five_hour_reset_after_seconds),
        tray_usage_hint("每周", weekly, weekly_reset_after_seconds),
    )
}

#[tauri::command]
fn update_tray_icon(
    app: tauri::AppHandle,
    mode: String,
    five_hour: Option<f64>,
    weekly: Option<f64>,
    five_hour_reset_after_seconds: Option<i64>,
    weekly_reset_after_seconds: Option<i64>,
) -> Result<(), String> {
    let normalized_mode = match mode.as_str() {
        "usage" => "usage",
        _ => "logo",
    };
    let tray = app
        .tray_by_id("main-tray")
        .ok_or_else(|| "托盘尚未初始化".to_string())?;
    let icon = match normalized_mode {
        "usage" => usage_ring_icon(five_hour.unwrap_or(0.0), weekly.unwrap_or(0.0)),
        _ => monochrome_template_icon(
            app.default_window_icon()
                .ok_or_else(|| "应用 Logo 不可用".to_string())?,
        ),
    };
    tray.set_icon_with_as_template(
        Some(icon),
        cfg!(target_os = "macos") && normalized_mode == "logo",
    )
        .map_err(|error| format!("更新托盘图标失败：{error}"))?;
    let tooltip = tray_tooltip(
        five_hour,
        weekly,
        five_hour_reset_after_seconds,
        weekly_reset_after_seconds,
    );
    tray.set_tooltip(Some(tooltip))
        .map_err(|error| format!("更新托盘提示失败：{error}"))?;
    app.state::<TrayIconMenuState>().select(normalized_mode)
}

#[cfg(test)]
mod tests {
    use super::{tray_reset_hint, tray_tooltip, tray_usage_hint, usage_ring_icon};

    const TRACK_COLOR: [u8; 4] = [209, 213, 219, 255];

    fn count_pixels(image: &tauri::image::Image<'_>, color: [u8; 4]) -> usize {
        image
            .rgba()
            .chunks_exact(4)
            .filter(|pixel| *pixel == color)
            .count()
    }

    #[test]
    fn usage_ring_has_expected_dimensions_and_visible_pixels() {
        let image = usage_ring_icon(50.0, 75.0);
        assert_eq!(image.width(), 32);
        assert_eq!(image.height(), 32);
        assert_eq!(image.rgba().len(), 32 * 32 * 4);
        assert!(image.rgba().chunks_exact(4).any(|pixel| pixel[3] > 0));
    }

    #[test]
    fn usage_ring_arc_changes_with_remaining_usage() {
        let low_usage = usage_ring_icon(25.0, 25.0);
        let high_usage = usage_ring_icon(75.0, 75.0);

        assert!(count_pixels(&low_usage, TRACK_COLOR) > count_pixels(&high_usage, TRACK_COLOR));
    }

    #[test]
    fn usage_ring_uses_light_gray_tracks() {
        let image = usage_ring_icon(0.0, 0.0);

        assert!(count_pixels(&image, TRACK_COLOR) > 0);
    }

    #[test]
    fn tray_tooltip_includes_remaining_usage_and_reset_context() {
        assert_eq!(
            tray_usage_hint("5小时", Some(82.4), Some(7_500)),
            "5小时 82% · 2小时5分后重置"
        );
        assert_eq!(tray_reset_hint(None), "");
        assert_eq!(tray_reset_hint(Some(0)), " · 即将重置");
        let tooltip = tray_tooltip(Some(82.4), Some(57.0), Some(7_500), Some(259_200));
        assert_eq!(
            tooltip,
            "5小时 82% · 2小时5分后重置 | 每周 57% · 3天0小时后重置"
        );
        assert!(!tooltip.contains("Codex"));
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .manage(window_pos::SnapEnabled(Default::default()));
    #[cfg(not(target_os = "macos"))]
    let builder = builder
        .manage(window_pos::WindowPosState(Default::default()))
        .manage(window_pos::SnapState {
            last_move: Default::default(),
        });
    builder
        .invoke_handler(tauri::generate_handler![
            usage::fetch_usage,
            usage::fetch_analytics,
            reset_credits::fetch_reset_credits,
            reset_credits::consume_reset_credit,
            window_pos::set_snap_enabled,
            open_history,
            write_history_csv,
            start_codex_login,
            diagnose_codex,
            check_update,
            is_macos,
            set_capsule_widget_visible,
            show_settings_window,
            update_tray_icon,
            quit_app
        ])
        .setup(|app| {
            use tauri::{
                menu::{Menu, MenuItem, PredefinedMenuItem},
                tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
                Emitter,
            };
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);
            #[cfg(not(target_os = "macos"))]
            window_pos::restore(app.handle());
            if let Some(history_window) = app.get_webview_window("history") {
                let window_to_hide = history_window.clone();
                history_window.on_window_event(move |event| {
                    if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                        api.prevent_close();
                        let _ = window_to_hide.hide();
                    }
                });
            }
            #[cfg(target_os = "macos")]
            if let Some(main_window) = app.get_webview_window("main") {
                let window_to_hide = main_window.clone();
                main_window.on_window_event(move |event| {
                    if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                        api.prevent_close();
                        let _ = window_to_hide.hide();
                    }
                });
            }
            let show_label = if cfg!(target_os = "macos") {
                "打开用量面板"
            } else {
                "显示胶囊"
            };
            let show = MenuItem::with_id(app, "show", show_label, true, None::<&str>)?;
            let refresh = MenuItem::with_id(app, "refresh", "立即刷新", true, None::<&str>)?;
            #[cfg(target_os = "macos")]
            let capsule_widget = MenuItem::with_id(
                app,
                "capsule-widget",
                "显示桌面胶囊小工具",
                true,
                None::<&str>,
            )?;
            #[cfg(target_os = "macos")]
            app.manage(CapsuleWidgetMenuState {
                item: capsule_widget.clone(),
            });
            #[cfg(not(target_os = "macos"))]
            let settings = MenuItem::with_id(app, "settings", "打开设置", true, None::<&str>)?;
            #[cfg(not(target_os = "macos"))]
            let separator = PredefinedMenuItem::separator(app)?;
            #[cfg(target_os = "macos")]
            let separator = PredefinedMenuItem::separator(app)?;
            let tray_icon_menu = TrayIconMenuState {
                switcher: MenuItem::with_id(app, "tray-icon-switch", "用量环", true, None::<&str>)?,
                selected_mode: Arc::new(AtomicU8::new(0)),
            };
            let quit = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
            #[cfg(target_os = "macos")]
            let menu = Menu::with_items(
                app,
                &[
                    &show,
                    &refresh,
                    &capsule_widget,
                    &separator,
                    &tray_icon_menu.switcher,
                    &quit,
                ],
            )?;
            #[cfg(not(target_os = "macos"))]
            let menu = Menu::with_items(
                app,
                &[
                    &show,
                    &refresh,
                    &settings,
                    &separator,
                    &tray_icon_menu.switcher,
                    &quit,
                ],
            )?;
            let tray_icon_menu_for_event = tray_icon_menu.clone();
            let app_icon = app.default_window_icon().unwrap();
            TrayIconBuilder::with_id("main-tray")
                .icon(monochrome_template_icon(app_icon))
                .icon_as_template(true)
                .tooltip("正在加载用量…")
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(move |app, event| match event.id.as_ref() {
                    "show" => {
                        if let Some(w) = app.get_webview_window("main") {
                            window_pos::show_main(app, &w);
                        }
                        let _ = app.emit("tray-show", ());
                    }
                    "refresh" => {
                        let _ = app.emit("tray-refresh", ());
                    }
                    #[cfg(target_os = "macos")]
                    "capsule-widget" => {
                        if let Some(widget) = app.get_webview_window("capsule-widget") {
                            let visible = widget.is_visible().unwrap_or(false);
                            let next_visible = !visible;
                            let result = if next_visible {
                                widget.show()
                            } else {
                                widget.hide()
                            };
                            if result.is_ok() {
                                if next_visible {
                                    let _ = widget.set_focus();
                                }
                                let _ = app
                                    .state::<CapsuleWidgetMenuState>()
                                    .set_visible(next_visible);
                                let _ = app.emit("capsule-widget-state", next_visible);
                            }
                        }
                    }
                    "settings" => {
                        if let Some(w) = app.get_webview_window("main") {
                            window_pos::show_main(app, &w);
                        }
                        let _ = app.emit("tray-open-settings", ());
                    }
                    "tray-icon-switch" => {
                        if let Ok(mode) = tray_icon_menu_for_event.toggle() {
                            let _ = app.emit("tray-icon-mode-change", mode);
                        }
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
                            window_pos::show_main(tray.app_handle(), &w);
                        }
                        let _ = tray.app_handle().emit("tray-show", ());
                    }
                })
                .build(app)?;
            app.manage(tray_icon_menu);
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while running tauri application")
        .run(|_app_handle, _event| {
            #[cfg(not(target_os = "macos"))]
            window_pos::on_event(_app_handle, _event);
        });
}
