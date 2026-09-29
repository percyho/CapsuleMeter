use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Manager, PhysicalPosition, RunEvent, WindowEvent};

/// 窗口位置状态：拖动过程中仅更新内存，退出时写盘一次
pub struct WindowPosState(pub Mutex<Option<PhysicalPosition<i32>>>);

/// 贴边吸附状态：记录最后一次窗口移动时间，用于拖动结束后的吸附检测
pub struct SnapState {
    pub last_move: Arc<Mutex<Option<Instant>>>,
    pub suppress_until: Arc<Mutex<Option<Instant>>>,
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

/// Suppress edge snapping briefly after programmatic window repositioning.
#[tauri::command]
pub fn suppress_snap_after_programmatic_move(state: tauri::State<SnapState>) {
    if let Ok(mut until) = state.suppress_until.lock() {
        *until = Some(Instant::now() + Duration::from_millis(800));
    }
}

/// 吸附判定延迟：拖动停止这么久之后才执行吸附
const SNAP_DELAY_MS: u64 = 250;
/// 吸附重试次数：拖动停止后系统可能补发 Moved 事件，需要重试直到真正空闲
const SNAP_RETRY: u32 = 5;
/// 吸附判定阈值：窗口边缘距屏幕工作区边缘小于该逻辑像素即吸附
const SNAP_MARGIN_LOGICAL: i32 = 24;

#[derive(Serialize, Deserialize)]
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

pub fn restore(app: &AppHandle) {
    let path = pos_file(app);
    let Ok(raw) = std::fs::read_to_string(&path) else {
        return;
    };
    let Ok(pos) = serde_json::from_str::<Pos>(&raw) else {
        return;
    };
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.set_position(PhysicalPosition::new(pos.x, pos.y));
    }
}

pub fn save(app: &AppHandle, pos: PhysicalPosition<i32>) {
    let path = pos_file(app);
    let raw = serde_json::to_string(&Pos { x: pos.x, y: pos.y }).unwrap_or_default();
    let _ = std::fs::write(path, raw);
}

/// 拖动结束后检查贴边吸附（Windows：基于显示器工作区，自动避开任务栏）
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

#[cfg(not(windows))]
fn work_area(_window: &tauri::WebviewWindow) -> Option<(i32, i32, i32, i32)> {
    None
}

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
                let is_suppressed = snap
                    .suppress_until
                    .lock()
                    .map(|until| match *until {
                        Some(deadline) => Instant::now() < deadline,
                        None => false,
                    })
                    .unwrap_or(false);
                if is_suppressed {
                    return;
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
                        let programmatic = app2
                            .try_state::<SnapState>()
                            .map(|s| {
                                s.suppress_until
                                    .lock()
                                    .map(|until| match *until {
                                        Some(deadline) => Instant::now() < deadline,
                                        None => false,
                                    })
                                    .unwrap_or(false)
                            })
                            .unwrap_or(false);
                        if programmatic {
                            break;
                        }
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
