use serde::Serialize;
#[cfg(not(target_os = "macos"))]
use serde::Deserialize;
use std::path::PathBuf;
use std::sync::Mutex;
#[cfg(not(target_os = "macos"))]
use std::sync::Arc;
#[cfg(not(target_os = "macos"))]
use std::time::{Duration, Instant};
use tauri::{AppHandle, Manager, PhysicalPosition, WebviewWindow};
#[cfg(not(target_os = "macos"))]
use tauri::{RunEvent, WindowEvent};

/// 窗口位置状态：拖动过程中仅更新内存，退出时写盘一次
#[cfg(not(target_os = "macos"))]
pub struct WindowPosState(pub Mutex<Option<PhysicalPosition<i32>>>);

/// 贴边吸附状态：记录最后一次窗口移动时间，用于拖动结束后的吸附检测
#[cfg(not(target_os = "macos"))]
pub struct SnapState {
    pub last_move: Arc<Mutex<Option<Instant>>>,
}

/// 贴边吸附功能开关（由设置面板控制）
pub struct SnapEnabled(pub Mutex<bool>);

/// 设置吸附开关状态
#[tauri::command]
pub fn set_snap_enabled(state: tauri::State<SnapEnabled>, enabled: bool) {
    if let Ok(mut inner) = state.0.lock() {
        *inner = enabled;
    }
}

/// 吸附判定延迟：拖动停止这么久之后才执行吸附
#[cfg(not(target_os = "macos"))]
const SNAP_DELAY_MS: u64 = 250;
/// 吸附重试次数：拖动停止后系统可能补发 Moved 事件，需要重试直到真正空闲
#[cfg(not(target_os = "macos"))]
const SNAP_RETRY: u32 = 5;
/// 吸附判定阈值：窗口边缘距屏幕工作区边缘小于该逻辑像素即吸附
#[cfg(not(target_os = "macos"))]
const SNAP_MARGIN_LOGICAL: i32 = 24;

#[derive(Serialize)]
#[cfg_attr(not(target_os = "macos"), derive(Deserialize))]
struct Pos {
    x: i32,
    y: i32,
}

fn pos_file(app: &AppHandle) -> PathBuf {
    let dir = app
        .path()
        .app_data_dir()
        .unwrap_or_else(|_| std::env::temp_dir().join("codex-capsule"));
    let _ = std::fs::create_dir_all(&dir);
    dir.join("window_pos.json")
}

fn clamp_axis(value: i32, origin: i32, display_size: u32, window_size: u32) -> i32 {
    let lower = i64::from(origin);
    let upper = (lower + i64::from(display_size) - i64::from(window_size)).max(lower);
    i64::from(value).clamp(lower, upper) as i32
}

fn distance_to_axis(value: i32, origin: i32, display_size: u32) -> i64 {
    let value = i64::from(value);
    let start = i64::from(origin);
    let end = start + i64::from(display_size);
    if value < start {
        start - value
    } else if value > end {
        value - end
    } else {
        0
    }
}

fn clamp_to_monitor(
    window: &WebviewWindow,
    position: PhysicalPosition<i32>,
) -> Option<PhysicalPosition<i32>> {
    let window_size = window.outer_size().ok()?;
    let monitors = window.available_monitors().ok()?;
    let monitor = monitors.iter().min_by_key(|monitor| {
        let origin = monitor.position();
        let size = monitor.size();
        let dx = distance_to_axis(position.x, origin.x, size.width);
        let dy = distance_to_axis(position.y, origin.y, size.height);
        dx * dx + dy * dy
    })?;

    let origin = monitor.position();
    let size = monitor.size();
    Some(PhysicalPosition::new(
        clamp_axis(position.x, origin.x, size.width, window_size.width),
        clamp_axis(position.y, origin.y, size.height, window_size.height),
    ))
}

/// Keep the capsule on a connected display when restoring or showing it.
pub fn ensure_visible(window: &WebviewWindow) -> Option<PhysicalPosition<i32>> {
    let current = window.outer_position().ok()?;
    let visible = clamp_to_monitor(window, current).unwrap_or(current);
    if visible != current {
        let _ = window.set_position(visible);
    }
    Some(visible)
}

pub fn show_main(app: &AppHandle, window: &WebviewWindow) {
    #[cfg(target_os = "macos")]
    let _ = app.show();
    if let Some(position) = ensure_visible(window) {
        save(app, position);
    }
    let _ = window.unminimize();
    let _ = window.show();
    let _ = window.set_focus();
}

#[cfg(not(target_os = "macos"))]
pub fn restore(app: &AppHandle) {
    let path = pos_file(app);
    let Ok(raw) = std::fs::read_to_string(&path) else {
        return;
    };
    let Ok(pos) = serde_json::from_str::<Pos>(&raw) else {
        return;
    };
    if let Some(window) = app.get_webview_window("main") {
        let requested = PhysicalPosition::new(pos.x, pos.y);
        let restored = clamp_to_monitor(&window, requested).unwrap_or(requested);
        let _ = window.set_position(restored);
        if restored != requested {
            save(app, restored);
        }
    }
}

pub fn save(app: &AppHandle, pos: PhysicalPosition<i32>) {
    let path = pos_file(app);
    let raw = serde_json::to_string(&Pos { x: pos.x, y: pos.y }).unwrap_or_default();
    let _ = std::fs::write(path, raw);
}

/// 拖动结束后检查贴边吸附（Windows：基于显示器工作区，自动避开任务栏）
#[cfg(not(target_os = "macos"))]
fn snap_window(app: &AppHandle) {
    let Some(window) = app.get_webview_window("main") else {
        return;
    };
    let Ok(pos) = window.outer_position() else {
        return;
    };
    let Ok(size) = window.outer_size() else {
        return;
    };
    let Some(work) = work_area(&window) else {
        return;
    };
    let scale = window.scale_factor().unwrap_or(1.0);
    let margin = (SNAP_MARGIN_LOGICAL as f64 * scale) as i32;

    let (wl, wt, wr, wb) = work;
    let x = pos.x;
    let y = pos.y;
    let w = size.width as i32;
    let h = size.height as i32;

    let mut nx = x;
    let mut ny = y;
    if x <= wl + margin {
        nx = wl;
    } else if x + w >= wr - margin {
        nx = wr - w;
    }
    if y <= wt + margin {
        ny = wt;
    } else if y + h >= wb - margin {
        ny = wb - h;
    }

    if (nx, ny) != (x, y) {
        let _ = window.set_position(PhysicalPosition::new(nx, ny));
    }
}

/// 获取窗口所在显示器的工作区（排除任务栏），返回 (left, top, right, bottom)
#[cfg(windows)]
fn work_area(window: &tauri::WebviewWindow) -> Option<(i32, i32, i32, i32)> {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use windows::Win32::Foundation::HWND;
    use windows::Win32::Graphics::Gdi::{
        GetMonitorInfoW, MonitorFromWindow, MONITORINFO, MONITOR_DEFAULTTONEAREST,
    };

    let handle = window.window_handle().ok()?;
    let hwnd = match handle.as_raw() {
        RawWindowHandle::Win32(wh) => HWND(wh.hwnd.get() as *mut _),
        _ => return None,
    };
    unsafe {
        let monitor = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
        if monitor.is_invalid() {
            return None;
        }
        let mut info: MONITORINFO = std::mem::zeroed();
        info.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
        if GetMonitorInfoW(monitor, &mut info).as_bool() {
            let r = info.rcWork;
            Some((r.left, r.top, r.right, r.bottom))
        } else {
            None
        }
    }
}

#[cfg(all(not(windows), not(target_os = "macos")))]
fn work_area(_window: &tauri::WebviewWindow) -> Option<(i32, i32, i32, i32)> {
    None
}

#[cfg(not(target_os = "macos"))]
pub fn on_event(app: &AppHandle, event: RunEvent) {
    match event {
        RunEvent::WindowEvent {
            label,
            event: WindowEvent::Moved(pos),
            ..
        } if label == "main" => {
            // 记录最新位置（退出时保存）
            if let Some(state) = app.try_state::<WindowPosState>() {
                if let Ok(mut inner) = state.0.lock() {
                    *inner = Some(pos);
                }
            }
            // 节流：拖动停止一段时间后才执行吸附
            if let Some(snap) = app.try_state::<SnapState>() {
                if let Ok(mut last) = snap.last_move.lock() {
                    *last = Some(Instant::now());
                }
            }
            let app2 = app.clone();
            std::thread::spawn(move || {
                // 拖动停止后重试检测：系统可能补发 Moved 事件，导致一次检测误判为"仍在移动"
                for _ in 0..SNAP_RETRY {
                    std::thread::sleep(Duration::from_millis(SNAP_DELAY_MS));
                    let snap_arc = app2
                        .try_state::<SnapState>()
                        .map(|s| Arc::clone(&s.last_move));
                    let idle = match snap_arc {
                        Some(arc) => match arc.lock() {
                            Ok(l) => l
                                .map(|t| t.elapsed().as_millis() >= SNAP_DELAY_MS as u128)
                                .unwrap_or(false),
                            Err(_) => false,
                        },
                        None => false,
                    };
                    if idle {
                        // 仅在吸附开关打开时执行贴边吸附
                        let snap_on = app2
                            .try_state::<SnapEnabled>()
                            .map(|s| s.0.lock().map(|g| *g).unwrap_or(false))
                            .unwrap_or(false);
                        if snap_on {
                            snap_window(&app2);
                        }
                        break;
                    }
                }
            });
        }
        RunEvent::Exit => {
            if let Some(state) = app.try_state::<WindowPosState>() {
                if let Ok(mut inner) = state.0.lock() {
                    if let Some(pos) = inner.take() {
                        save(app, pos);
                    }
                }
            }
        }
        _ => {}
    }
}
