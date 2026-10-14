mod discovery;
#[cfg(windows)]
mod job;
mod protocol;

pub use discovery::find_cli;
use serde::Serialize;
use std::{path::Path, process::Stdio, time::Duration};
use tokio::{io::BufReader, process::Command, time::timeout};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub provider: String,
    pub plan: Option<String>,
    pub observed_at: u64,
    pub meters: Vec<QuotaMeter>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QuotaMeter {
    pub kind: &'static str,
    pub id: String,
    pub label: String,
    pub remaining_percent: f64,
    pub duration_minutes: u64,
    pub resets_at: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct UsageError {
    pub code: &'static str,
    pub message: &'static str,
}

impl UsageError {
    pub fn new(code: &'static str, message: &'static str) -> Self {
        Self { code, message }
    }

    pub(crate) fn protocol() -> Self {
        Self::new(
            "protocol",
            "Codex 返回的数据格式无法识别，请更新 Codex 或检查 CLI 路径。",
        )
    }
}

impl std::fmt::Display for UsageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.message)
    }
}

impl std::error::Error for UsageError {}

/// Uses the CLI's existing authentication; never opens auth.json or starts a model turn.
pub async fn read_usage(cli: &Path) -> Result<Snapshot, UsageError> {
    let mut command = Command::new(cli);
    command
        .arg("app-server")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true);
    #[cfg(windows)]
    command.creation_flags(0x08000000); // CREATE_NO_WINDOW

    let mut child = command.spawn().map_err(|_| {
        UsageError::new(
            "cli_missing",
            "无法启动 Codex CLI，请在连接设置中指定 codex.exe。",
        )
    })?;
    #[cfg(windows)]
    let _job = job::ChildJob::attach(child.id().ok_or_else(UsageError::protocol)?)?;
    let mut reader = BufReader::new(child.stdout.take().ok_or_else(UsageError::protocol)?);
    let mut writer = child.stdin.take().ok_or_else(UsageError::protocol)?;
    let result = timeout(
        Duration::from_secs(20),
        protocol::query(&mut reader, &mut writer),
    )
    .await
    .unwrap_or_else(|_| {
        Err(UsageError::new(
            "timeout",
            "额度查询超时，请检查网络后重试。",
        ))
    });

    drop(writer);
    let _ = child.start_kill();
    let _ = timeout(Duration::from_secs(2), child.wait()).await;
    result
}
