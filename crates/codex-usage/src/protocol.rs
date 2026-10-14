use crate::{QuotaMeter, Snapshot, UsageError};
use serde_json::{json, Value};
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::io::{AsyncBufRead, AsyncBufReadExt, AsyncReadExt, AsyncWrite, AsyncWriteExt};

const MAX_LINE: usize = 1024 * 1024;

async fn send<W: AsyncWrite + Unpin>(writer: &mut W, message: Value) -> Result<(), UsageError> {
    let mut bytes = serde_json::to_vec(&message).map_err(|_| UsageError::protocol())?;
    bytes.push(b'\n');
    writer
        .write_all(&bytes)
        .await
        .map_err(|_| UsageError::protocol())?;
    writer.flush().await.map_err(|_| UsageError::protocol())
}

async fn response<R: AsyncBufRead + Unpin>(reader: &mut R, id: u64) -> Result<Value, UsageError> {
    loop {
        let mut bytes = Vec::new();
        // Limit allocation before parsing, including a child that never emits a newline.
        let count = (&mut *reader)
            .take((MAX_LINE + 1) as u64)
            .read_until(b'\n', &mut bytes)
            .await
            .map_err(|_| UsageError::protocol())?;
        if count == 0 || count > MAX_LINE {
            return Err(UsageError::protocol());
        }
        if bytes.iter().all(u8::is_ascii_whitespace) {
            continue;
        }
        let message: Value = serde_json::from_slice(&bytes).map_err(|_| UsageError::protocol())?;
        if message.get("id").and_then(Value::as_u64) != Some(id) {
            continue;
        }
        if message.get("error").is_some() {
            // Server errors may contain private paths or identity; never expose raw payloads.
            return Err(UsageError::new(
                "query_failed",
                "Codex 无法查询额度，请检查登录状态、网络和 CLI 版本。",
            ));
        }
        return message
            .get("result")
            .cloned()
            .ok_or_else(UsageError::protocol);
    }
}

pub(crate) async fn query<R: AsyncBufRead + Unpin, W: AsyncWrite + Unpin>(
    reader: &mut R,
    writer: &mut W,
) -> Result<Snapshot, UsageError> {
    send(writer, json!({"id": 1, "method": "initialize", "params": {"clientInfo": {"name": "silverwolf_balance", "title": "Silverwolf", "version": env!("CARGO_PKG_VERSION")}}})).await?;
    response(reader, 1).await?;
    send(writer, json!({"method": "initialized", "params": {}})).await?;
    send(
        writer,
        json!({"id": 2, "method": "account/read", "params": {"refreshToken": false}}),
    )
    .await?;
    let account_result = response(reader, 2).await?;
    let account = &account_result["account"];
    match account["type"].as_str() {
        Some("apiKey") => {
            return Err(UsageError::new(
                "api_key_login",
                "当前为 API Key 登录。请在 Codex 中使用 ChatGPT 账户登录以查询订阅额度。",
            ))
        }
        Some("chatgpt") => {}
        _ => {
            return Err(UsageError::new(
                "not_logged_in",
                "请先在 Codex 中登录 ChatGPT 账户，再刷新额度。",
            ))
        }
    }
    send(
        writer,
        json!({"id": 3, "method": "account/rateLimits/read", "params": {}}),
    )
    .await?;
    let result = response(reader, 3).await?;
    parse_limits(&result, account["planType"].as_str())
}

fn parse_limits(result: &Value, account_plan: Option<&str>) -> Result<Snapshot, UsageError> {
    if !result.is_object()
        || (result.get("rateLimits").is_none() && !result["rateLimitsByLimitId"].is_object())
    {
        return Err(UsageError::protocol());
    }
    if (!result["rateLimits"].is_null() && !result["rateLimits"].is_object())
        || (!result["rateLimitsByLimitId"].is_null() && !result["rateLimitsByLimitId"].is_object())
    {
        return Err(UsageError::protocol());
    }
    let mut meters = Vec::new();
    let mut plan = account_plan.map(str::to_owned);
    let buckets: Vec<(String, &Value)> = match result["rateLimitsByLimitId"].as_object() {
        Some(map) if !map.is_empty() => map
            .iter()
            .map(|(key, value)| (key.clone(), value))
            .collect(),
        _ => vec![("codex".into(), &result["rateLimits"])],
    };
    for (id, bucket) in buckets {
        if !bucket.is_null() && !bucket.is_object() {
            return Err(UsageError::protocol());
        }
        if id == "codex" {
            plan = bucket["planType"].as_str().map(str::to_owned).or(plan);
        }
        let label =
            bucket["limitName"]
                .as_str()
                .unwrap_or(if id == "codex" { "Codex" } else { &id });
        for slot in ["primary", "secondary"] {
            let window = &bucket[slot];
            if window.is_null() {
                continue;
            }
            let used = window["usedPercent"]
                .as_f64()
                .filter(|v| v.is_finite() && (0.0..=100.0).contains(v))
                .ok_or_else(UsageError::protocol)?;
            let minutes = window["windowDurationMins"]
                .as_u64()
                .filter(|v| *v > 0)
                .ok_or_else(UsageError::protocol)?;
            let resets_at = if window["resetsAt"].is_null() {
                None
            } else {
                Some(
                    window["resetsAt"]
                        .as_u64()
                        .and_then(|s| s.checked_mul(1000))
                        .filter(|ms| *ms <= 8_640_000_000_000_000)
                        .ok_or_else(UsageError::protocol)?,
                )
            };
            meters.push(QuotaMeter {
                kind: "quota",
                id: format!("{id}:{slot}"),
                label: label.into(),
                remaining_percent: 100.0 - used,
                duration_minutes: minutes,
                resets_at,
            });
        }
    }
    meters.sort_by_key(|m| m.duration_minutes);
    Ok(Snapshot {
        provider: "codex".into(),
        plan,
        observed_at: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64,
        meters,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{duplex, split, BufReader};

    #[test]
    fn supports_dynamic_windows_buckets_and_missing_resets() {
        let value = json!({"rateLimitsByLimitId": {
            "codex": {"planType": "go", "primary": {"usedPercent": 25.5, "windowDurationMins": 43200, "resetsAt": 1800000000}},
            "review": {"limitName": "Review", "secondary": {"usedPercent": 100, "windowDurationMins": 37, "resetsAt": null}}
        }});
        let result = parse_limits(&value, Some("plus")).unwrap();
        assert_eq!(result.plan.as_deref(), Some("go"));
        assert_eq!(result.meters.len(), 2);
        assert_eq!(result.meters[0].remaining_percent, 0.0);
        assert_eq!(result.meters[0].resets_at, None);
        assert_eq!(result.meters[1].remaining_percent, 74.5);
        assert_eq!(result.meters[1].resets_at, Some(1800000000000));
    }

    #[test]
    fn legacy_and_empty_are_valid_but_bad_numbers_are_not() {
        assert!(
            parse_limits(&json!({"rateLimits": {"primary": null}}), None)
                .unwrap()
                .meters
                .is_empty()
        );
        let bad =
            json!({"rateLimits": {"primary": {"usedPercent": 101, "windowDurationMins": 300}}});
        assert_eq!(parse_limits(&bad, None).unwrap_err().code, "protocol");
        assert_eq!(parse_limits(&json!({}), None).unwrap_err().code, "protocol");
    }

    #[tokio::test]
    async fn handshake_ignores_notifications_and_never_starts_a_turn() {
        let (client, server) = duplex(8192);
        let (read, mut write) = split(client);
        let (server_read, mut server_write) = split(server);
        let server_task = tokio::spawn(async move {
            let mut requests = BufReader::new(server_read).lines();
            for expected in [
                "initialize",
                "initialized",
                "account/read",
                "account/rateLimits/read",
            ] {
                let line = requests.next_line().await.unwrap().unwrap();
                let request: Value = serde_json::from_str(&line).unwrap();
                assert_eq!(request["method"], expected);
                let reply = match expected {
                    "initialize" => json!({"id": 1, "result": {}}),
                    "initialized" => continue,
                    "account/read" => {
                        json!({"id": 2, "result": {"account": {"type": "chatgpt", "email": "private@example.invalid", "planType": "plus"}}})
                    }
                    _ => {
                        json!({"id": 3, "result": {"rateLimits": {"primary": {"usedPercent": 42, "windowDurationMins": 300, "resetsAt": null}}}})
                    }
                };
                send(
                    &mut server_write,
                    json!({"method": "account/updated", "params": {}}),
                )
                .await
                .unwrap();
                send(&mut server_write, reply).await.unwrap();
            }
        });
        let snapshot = query(&mut BufReader::new(read), &mut write).await.unwrap();
        assert_eq!(snapshot.meters[0].remaining_percent, 58.0);
        assert!(!serde_json::to_string(&snapshot)
            .unwrap()
            .contains("private@example"));
        server_task.await.unwrap();
    }

    #[tokio::test]
    async fn api_key_login_stops_before_quota_request() {
        let input = b"{\"id\":1,\"result\":{}}\n{\"id\":2,\"result\":{\"account\":{\"type\":\"apiKey\"}}}\n";
        let mut output = Vec::new();
        let error = query(&mut BufReader::new(&input[..]), &mut output)
            .await
            .unwrap_err();
        assert_eq!(error.code, "api_key_login");
        assert!(!String::from_utf8(output)
            .unwrap()
            .contains("rateLimits/read"));
    }

    #[tokio::test]
    async fn rejects_unbounded_frames_and_redacts_server_errors() {
        let input = vec![b'x'; MAX_LINE + 1];
        assert_eq!(
            response(&mut BufReader::new(input.as_slice()), 1)
                .await
                .unwrap_err()
                .code,
            "protocol"
        );
        let private = b"{\"id\":1,\"error\":{\"message\":\"secret-token\"}}\n";
        let error = response(&mut BufReader::new(&private[..]), 1)
            .await
            .unwrap_err();
        assert!(!error.to_string().contains("secret-token"));
    }
}
