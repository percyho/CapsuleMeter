use serde::Serialize;
use serde_json::{json, Value};
use std::{
    io::{BufRead, BufReader, Write},
    process::{Command, Stdio},
    sync::mpsc,
    time::Duration,
};

const RPC_TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResetCreditsSummary {
    pub available_count: i64,
    pub credits: Option<Vec<ResetCredit>>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResetCredit {
    pub id: String,
    pub title: Option<String>,
    pub description: Option<String>,
    pub granted_at: i64,
    pub expires_at: Option<i64>,
    pub reset_type: String,
    pub status: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConsumeResetResult {
    pub outcome: String,
}

fn wait_for_response(receiver: &mpsc::Receiver<String>, request_id: i64) -> Result<Value, String> {
    loop {
        let line = receiver
            .recv_timeout(RPC_TIMEOUT)
            .map_err(|_| "Codex App Server 响应超时".to_string())?;
        let message: Value = serde_json::from_str(&line)
            .map_err(|error| format!("解析 Codex App Server 响应失败：{error}"))?;
        if message.get("id").and_then(Value::as_i64) != Some(request_id) {
            continue;
        }
        if let Some(error) = message.get("error") {
            let detail = error
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or("未知错误");
            return Err(format!("Codex App Server 请求失败：{detail}"));
        }
        return message
            .get("result")
            .cloned()
            .ok_or_else(|| "Codex App Server 响应缺少 result".to_string());
    }
}

fn run_rpc(method: &str, params: Value) -> Result<Value, String> {
    #[cfg(windows)]
    let mut command = {
        let mut command = Command::new("cmd");
        command.args(["/D", "/S", "/C", "codex app-server --stdio"]);
        command
    };
    #[cfg(not(windows))]
    let mut command = Command::new(crate::codex_cli::executable());
    #[cfg(not(windows))]
    command.args(["app-server", "--stdio"]);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0800_0000);
    }
    let mut child = command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|error| format!("无法启动 Codex App Server：{error}"))?;
    let mut stdin = child
        .stdin
        .take()
        .ok_or_else(|| "无法连接 Codex App Server 输入流".to_string())?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "无法连接 Codex App Server 输出流".to_string())?;
    let (sender, receiver) = mpsc::channel();
    std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            if sender.send(line).is_err() {
                break;
            }
        }
    });

    let result = (|| {
        writeln!(
            stdin,
            "{}",
            json!({
                "method": "initialize",
                "id": 1,
                "params": {
                    "clientInfo": {
                        "name": "capsule_meter",
                        "title": "Capsule Meter",
                        "version": env!("CARGO_PKG_VERSION")
                    }
                }
            })
        )
        .map_err(|error| format!("初始化 Codex App Server 失败：{error}"))?;
        stdin.flush().map_err(|error| error.to_string())?;
        wait_for_response(&receiver, 1)?;

        writeln!(
            stdin,
            "{}",
            json!({ "method": "initialized", "params": {} })
        )
        .map_err(|error| error.to_string())?;
        writeln!(
            stdin,
            "{}",
            json!({ "method": method, "id": 2, "params": params })
        )
        .map_err(|error| error.to_string())?;
        stdin.flush().map_err(|error| error.to_string())?;
        wait_for_response(&receiver, 2)
    })();

    let _ = child.kill();
    let _ = child.wait();
    result
}

fn decode_summary(result: &Value) -> Result<ResetCreditsSummary, String> {
    let summary = result
        .get("rateLimitResetCredits")
        .ok_or_else(|| "Codex App Server 响应缺少 rateLimitResetCredits".to_string())?;
    let available_count = summary
        .get("availableCount")
        .and_then(Value::as_i64)
        .ok_or_else(|| "Codex App Server 响应缺少 availableCount".to_string())?
        .max(0);
    let credits = summary
        .get("credits")
        .and_then(Value::as_array)
        .map(|rows| {
            rows.iter()
                .filter_map(|row| {
                    Some(ResetCredit {
                        id: row.get("id")?.as_str()?.to_string(),
                        title: row.get("title").and_then(Value::as_str).map(str::to_string),
                        description: row
                            .get("description")
                            .and_then(Value::as_str)
                            .map(str::to_string),
                        granted_at: row.get("grantedAt")?.as_i64()?,
                        expires_at: row.get("expiresAt").and_then(Value::as_i64),
                        reset_type: row
                            .get("resetType")
                            .and_then(Value::as_str)
                            .unwrap_or("unknown")
                            .to_string(),
                        status: row
                            .get("status")
                            .and_then(Value::as_str)
                            .unwrap_or("unknown")
                            .to_string(),
                    })
                })
                .collect()
        });
    Ok(ResetCreditsSummary {
        available_count,
        credits,
    })
}

#[tauri::command]
pub async fn fetch_reset_credits() -> Result<ResetCreditsSummary, String> {
    tauri::async_runtime::spawn_blocking(|| {
        run_rpc(
            "account/rateLimits/read",
            json!({ "excludeResetCreditDetails": false }),
        )
        .and_then(|result| decode_summary(&result))
    })
    .await
    .map_err(|error| format!("读取重置机会失败：{error}"))?
}

#[cfg(test)]
mod tests {
    use super::decode_summary;
    use serde_json::json;

    #[test]
    fn decodes_available_reset_credit() {
        let summary = decode_summary(&json!({
            "rateLimitResetCredits": {
                "availableCount": 1,
                "credits": [{
                    "id": "credit-1",
                    "title": "Full reset",
                    "description": "Reset weekly and five-hour limits",
                    "grantedAt": 1_790_102_639_i64,
                    "expiresAt": 1_792_694_639_i64,
                    "resetType": "codexRateLimits",
                    "status": "available"
                }]
            }
        }))
        .expect("valid reset credit response");

        assert_eq!(summary.available_count, 1);
        assert_eq!(summary.credits.as_ref().map(Vec::len), Some(1));
    }

    #[test]
    fn rejects_response_without_reset_credit_data() {
        let error = decode_summary(&json!({ "rateLimits": {} }))
            .expect_err("missing reset credit data must not look like zero credits");

        assert!(error.contains("rateLimitResetCredits"));
    }
}

#[tauri::command]
pub async fn consume_reset_credit(
    idempotency_key: String,
    credit_id: Option<String>,
) -> Result<ConsumeResetResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let result = run_rpc(
            "account/rateLimitResetCredit/consume",
            json!({ "idempotencyKey": idempotency_key, "creditId": credit_id }),
        )?;
        let outcome = result
            .get("outcome")
            .and_then(Value::as_str)
            .ok_or_else(|| "重置响应缺少 outcome".to_string())?;
        Ok(ConsumeResetResult {
            outcome: outcome.to_string(),
        })
    })
    .await
    .map_err(|error| format!("使用重置失败：{error}"))?
}
