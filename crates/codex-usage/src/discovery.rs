use crate::UsageError;
use std::{
    env, fs,
    path::{Path, PathBuf},
    time::SystemTime,
};

fn executable(path: &Path) -> bool {
    path.is_file()
        && (!cfg!(windows)
            || path
                .extension()
                .is_some_and(|s| s.to_string_lossy().eq_ignore_ascii_case("exe")))
}

pub fn find_cli(configured: &str) -> Result<PathBuf, UsageError> {
    let explicit = if configured.trim().is_empty() {
        env::var("CODEX_EXECUTABLE").unwrap_or_default()
    } else {
        configured.trim().to_owned()
    };
    if !explicit.is_empty() {
        let path = PathBuf::from(explicit);
        return if path.is_absolute() && executable(&path) {
            Ok(path)
        } else {
            Err(UsageError::new(
                "cli_missing",
                "指定的 CLI 不存在，请填写 codex.exe 的完整路径。",
            ))
        };
    }
    let filename = if cfg!(windows) { "codex.exe" } else { "codex" };
    for dir in env::split_paths(&env::var_os("PATH").unwrap_or_default()) {
        // Do not execute a lookalike from a relative PATH entry in the workspace.
        if dir.is_absolute() && executable(&dir.join(filename)) {
            return Ok(dir.join(filename));
        }
    }
    let mut candidates = Vec::new();
    if let Some(local) = env::var_os("LOCALAPPDATA") {
        let bin = PathBuf::from(local).join("OpenAI/Codex/bin");
        collect_children(&bin, Path::new("codex.exe"), &mut candidates);
    }
    if let Some(programs) = env::var_os("ProgramFiles") {
        let apps = PathBuf::from(programs).join("WindowsApps");
        if let Ok(entries) = fs::read_dir(apps) {
            for entry in entries.flatten() {
                if entry
                    .file_name()
                    .to_string_lossy()
                    .to_ascii_lowercase()
                    .contains("codex")
                {
                    candidates.push(entry.path().join("app/resources/codex.exe"));
                }
            }
        }
    }
    if let Some(roaming) = env::var_os("APPDATA") {
        let package = PathBuf::from(roaming).join("npm/node_modules/@openai/codex");
        candidates.push(package.join("vendor/x86_64-pc-windows-msvc/codex/codex.exe"));
        candidates.push(package.join(
            "node_modules/@openai/codex-win32-x64/vendor/x86_64-pc-windows-msvc/codex/codex.exe",
        ));
    }
    candidates.retain(|p| executable(p));
    candidates.sort_by_key(|p| {
        std::cmp::Reverse(
            fs::metadata(p)
                .and_then(|m| m.modified())
                .unwrap_or(SystemTime::UNIX_EPOCH),
        )
    });
    candidates.into_iter().next().ok_or_else(|| {
        UsageError::new(
            "cli_missing",
            "未找到 Codex CLI。请安装并登录 Codex，或在连接设置中指定 codex.exe。",
        )
    })
}

fn collect_children(root: &Path, suffix: &Path, candidates: &mut Vec<PathBuf>) {
    candidates.push(root.join(suffix));
    if let Ok(entries) = fs::read_dir(root) {
        for entry in entries.flatten() {
            candidates.push(entry.path().join(suffix));
        }
    }
}
