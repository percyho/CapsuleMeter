use serde::{Deserialize, Serialize};
use std::path::PathBuf;

const USAGE_URL: &str = "https://chatgpt.com/backend-api/wham/usage";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct WindowData {
    /// 剩余百分比（数值，0-100）
    pub remaining_percent: Option<f64>,
    /// 已用百分比
    pub used_percent: Option<f64>,
    /// 重置时间（unix 秒）
    pub reset_at: Option<i64>,
    /// 距重置剩余秒数
    pub reset_after_seconds: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct UsageData {
    /// 套餐类型：plus / pro / free ...
    pub plan: Option<String>,
    /// 5 小时窗口
    pub five_hour: WindowData,
    /// 周窗口
    pub weekly: WindowData,
    /// 错误信息（无错误时为 null）
    pub error: Option<String>,
}

fn empty_window() -> WindowData {
    WindowData {
        remaining_percent: None,
        used_percent: None,
        reset_at: None,
        reset_after_seconds: None,
    }
}

#[derive(Deserialize)]
struct RawUsage {
    #[serde(rename = "plan_type")]
    plan_type: Option<String>,
    #[serde(rename = "rate_limit")]
    rate_limit: Option<RawRateLimit>,
}

#[derive(Deserialize)]
struct RawRateLimit {
    #[serde(rename = "primary_window")]
    primary_window: Option<RawWindow>,
    #[serde(rename = "secondary_window")]
    secondary_window: Option<RawWindow>,
}

#[derive(Deserialize)]
struct RawWindow {
    #[serde(rename = "used_percent")]
    used_percent: Option<f64>,
    #[serde(rename = "reset_after_seconds")]
    reset_after_seconds: Option<i64>,
    #[serde(rename = "reset_at")]
    reset_at: Option<i64>,
}

#[derive(Deserialize)]
struct AuthDotJson {
    tokens: Option<AuthTokens>,
}

#[derive(Deserialize)]
struct AuthTokens {
    #[serde(rename = "access_token")]
    access_token: Option<String>,
    #[serde(rename = "account_id")]
    account_id: Option<String>,
}

fn codex_home() -> PathBuf {
    if let Some(h) = std::env::var_os("CODEX_HOME") {
        return PathBuf::from(h);
    }
    let base = std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .unwrap_or_default();
    PathBuf::from(base).join(".codex")
}

fn load_auth() -> Result<(String, String), String> {
    let auth_path = codex_home().join("auth.json");
    let raw = std::fs::read_to_string(&auth_path)
        .map_err(|e| format!("无法读取 {}：{}", auth_path.display(), e))?;
    let auth: AuthDotJson =
        serde_json::from_str(&raw).map_err(|e| format!("解析 auth.json 失败：{}", e))?;
    let tokens = auth
        .tokens
        .ok_or_else(|| "auth.json 中缺少 tokens 字段，请先运行 codex login".to_string())?;
    let access = tokens
        .access_token
        .filter(|s| !s.is_empty())
        .ok_or_else(|| "缺少 access_token，请先运行 codex login".to_string())?;
    let account = tokens.account_id.unwrap_or_default();
    Ok((access, account))
}

/// 读取 Windows 系统代理（注册表 Internet Settings），返回 http://host:port
#[cfg(windows)]
fn system_proxy_url() -> Option<String> {
    use winreg::enums::HKEY_CURRENT_USER;
    use winreg::RegKey;
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let settings = hkcu
        .open_subkey("Software\\Microsoft\\Windows\\CurrentVersion\\Internet Settings")
        .ok()?;
    let enable: u32 = settings.get_value("ProxyEnable").ok()?;
    if enable != 1 {
        return None;
    }
    let server: String = settings.get_value("ProxyServer").ok()?;
    if server.is_empty() {
        return None;
    }
    // ProxyServer 可能是 "host:port" 或 "http=host:port;https=host:port"
    let addr = if server.contains('=') {
        server
            .split(';')
            .find_map(|kv| {
                kv.strip_prefix("https=")
                    .or_else(|| kv.strip_prefix("http="))
            })
            .unwrap_or("")
            .to_string()
    } else {
        server
    };
    if addr.is_empty() {
        return None;
    }
    Some(format!("http://{}", addr.trim_end_matches('/')))
}

#[cfg(not(windows))]
fn system_proxy_url() -> Option<String> {
    None
}

/// 构建 HTTP 客户端：自动使用 Windows 系统代理（如 Clash），未配置则直连
fn build_http_client() -> Result<reqwest::Client, String> {
    let mut builder = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .user_agent("codex-cli");
    if let Some(proxy) = system_proxy_url() {
        builder = builder.proxy(
            reqwest::Proxy::all(&proxy).map_err(|e| format!("代理配置失败 {}：{}", proxy, e))?,
        );
    }
    builder
        .build()
        .map_err(|e| format!("初始化 HTTP 客户端失败：{}", e))
}

fn to_window(w: Option<&RawWindow>) -> WindowData {
    let Some(w) = w else {
        return empty_window();
    };
    let used = w.used_percent.unwrap_or(0.0);
    let remaining = (100.0 - used).clamp(0.0, 100.0);
    WindowData {
        remaining_percent: Some(remaining),
        used_percent: Some(used),
        reset_at: w.reset_at,
        reset_after_seconds: w.reset_after_seconds,
    }
}

#[tauri::command]
pub async fn fetch_usage() -> Result<UsageData, String> {
    let (access, account) = match load_auth() {
        Ok(v) => v,
        Err(e) => {
            return Ok(UsageData {
                plan: None,
                five_hour: empty_window(),
                weekly: empty_window(),
                error: Some(e),
            })
        }
    };

    let client = build_http_client()?;

    let mut req = client.get(USAGE_URL).bearer_auth(&access);
    if !account.is_empty() {
        req = req.header("ChatGPT-Account-Id", &account);
    }

    let resp = match req.send().await {
        Ok(r) => r,
        Err(e) => {
            return Ok(UsageData {
                plan: None,
                five_hour: empty_window(),
                weekly: empty_window(),
                error: Some(format!("请求用量接口失败：{}", e)),
            })
        }
    };
    let status = resp.status();
    let body = match resp.text().await {
        Ok(b) => b,
        Err(e) => {
            return Ok(UsageData {
                plan: None,
                five_hour: empty_window(),
                weekly: empty_window(),
                error: Some(format!("读取响应失败：{}", e)),
            })
        }
    };

    if status == 401 {
        return Ok(UsageData {
            plan: None,
            five_hour: empty_window(),
            weekly: empty_window(),
            error: Some("登录已过期，请运行 codex login 或打开 Codex 应用刷新登录".to_string()),
        });
    }
    if !status.is_success() {
        return Ok(UsageData {
            plan: None,
            five_hour: empty_window(),
            weekly: empty_window(),
            error: Some(format!("用量接口返回 HTTP {}", status)),
        });
    }

    let raw: RawUsage = match serde_json::from_str(&body) {
        Ok(r) => r,
        Err(e) => {
            return Ok(UsageData {
                plan: None,
                five_hour: empty_window(),
                weekly: empty_window(),
                error: Some(format!("解析用量响应失败：{}", e)),
            })
        }
    };

    Ok(UsageData {
        plan: raw.plan_type,
        five_hour: to_window(
            raw.rate_limit
                .as_ref()
                .and_then(|r| r.primary_window.as_ref()),
        ),
        weekly: to_window(
            raw.rate_limit
                .as_ref()
                .and_then(|r| r.secondary_window.as_ref()),
        ),
        error: None,
    })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DailyUsage {
    pub date: Option<String>,
    pub used_percent: Option<f64>,
    pub tokens: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalyticsData {
    pub daily: Vec<DailyUsage>,
    pub total_tokens: Option<f64>,
    pub peak_tokens: Option<f64>,
    pub error: Option<String>,
}

#[tauri::command]
pub async fn fetch_analytics(days: u32) -> Result<AnalyticsData, String> {
    let (access, account) = match load_auth() {
        Ok(v) => v,
        Err(e) => {
            return Ok(AnalyticsData {
                daily: vec![],
                total_tokens: None,
                peak_tokens: None,
                error: Some(e),
            })
        }
    };
    let client = build_http_client()?;
    let url = format!(
        "https://chatgpt.com/backend-api/wham/analytics/daily-workspace-usage-counts?days={days}"
    );
    let mut req = client.get(&url).bearer_auth(&access);
    if !account.is_empty() {
        req = req.header("ChatGPT-Account-Id", &account);
    }
    let resp = match req.send().await {
        Ok(r) => r,
        Err(e) => {
            return Ok(AnalyticsData {
                daily: vec![],
                total_tokens: None,
                peak_tokens: None,
                error: Some(format!("请求失败: {e}")),
            })
        }
    };
    let status = resp.status();
    let body = match resp.text().await {
        Ok(b) => b,
        Err(e) => {
            return Ok(AnalyticsData {
                daily: vec![],
                total_tokens: None,
                peak_tokens: None,
                error: Some(e.to_string()),
            })
        }
    };
    if !status.is_success() {
        return Ok(AnalyticsData {
            daily: vec![],
            total_tokens: None,
            peak_tokens: None,
            error: Some(format!("HTTP {status}")),
        });
    }
    // 尝试解析通用结构
    let v: serde_json::Value = match serde_json::from_str(&body) {
        Ok(v) => v,
        Err(e) => {
            return Ok(AnalyticsData {
                daily: vec![],
                total_tokens: None,
                peak_tokens: None,
                error: Some(format!("解析失败: {e}")),
            })
        }
    };
    let mut daily = vec![];
    let mut total = 0f64;
    let mut peak = 0f64;
    if let Some(arr) = v.as_array() {
        for item in arr {
            let date = item
                .get("date")
                .and_then(|d| d.as_str())
                .map(|s| s.to_string());
            let used = item.get("used_percent").and_then(|x| x.as_f64());
            let tokens = item
                .get("tokens")
                .or_else(|| item.get("counts"))
                .and_then(|x| x.as_f64());
            if let Some(t) = tokens {
                total += t;
                if t > peak {
                    peak = t;
                }
            }
            daily.push(DailyUsage {
                date,
                used_percent: used,
                tokens,
            });
        }
    }
    Ok(AnalyticsData {
        daily,
        total_tokens: Some(total),
        peak_tokens: Some(peak),
        error: None,
    })
}
