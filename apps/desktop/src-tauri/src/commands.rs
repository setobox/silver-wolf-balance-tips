use crate::{
    assets::{ImportFile, RuntimeTheme},
    content::{ContentConfig, Document, ProviderKind},
    push,
    settings::Appearance,
    Shared,
};
use std::sync::{atomic::Ordering, Arc};
use tauri::{AppHandle, Emitter, Manager, State, WebviewWindow};

#[tauri::command]
pub fn get_theme(appearance: Option<Appearance>, state: State<'_, Arc<Shared>>) -> RuntimeTheme {
    state.assets.theme(&appearance.unwrap_or_else(|| {
        state
            .model
            .lock()
            .unwrap()
            .settings
            .preferences
            .appearance
            .clone()
    }))
}
#[tauri::command]
pub async fn import_asset(
    name: String,
    bytes: Vec<u8>,
    kind: String,
    state: State<'_, Arc<Shared>>,
) -> Result<String, String> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || state.assets.import(&name, &bytes, &kind))
        .await
        .map_err(|_| "导入任务失败")?
}
#[tauri::command]
pub async fn import_theme(
    files: Option<Vec<ImportFile>>,
    archive: Option<Vec<u8>>,
    state: State<'_, Arc<Shared>>,
) -> Result<String, String> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || match (files, archive) {
        (Some(files), None) => state.assets.import_theme(files),
        (None, Some(bytes)) => state.assets.import_zip(bytes),
        _ => Err("请选择主题目录或 ZIP 文件".into()),
    })
    .await
    .map_err(|_| "导入主题失败")?
}
#[tauri::command]
pub async fn read_source(
    config: ContentConfig,
    request_id: String,
    codex_path: Option<String>,
    window: WebviewWindow,
    state: State<'_, Arc<Shared>>,
) -> Result<Document, String> {
    config.validate()?;
    if request_id.len() > 100 {
        return Err("请求标识过长".into());
    }
    let key = format!("{}:{request_id}", window.label());
    let cancellation = tokio_util::sync::CancellationToken::new();
    {
        let mut requests = state.requests.lock().unwrap();
        if requests.len() >= 16 {
            return Err("正在处理过多内容请求".into());
        }
        if let Some(old) = requests.insert(key.clone(), cancellation.clone()) {
            old.cancel();
        }
    }
    let read = async {
        match config.provider {
            ProviderKind::Codex => {
                let _query = state.query.try_lock().map_err(|_| "正在刷新额度，请稍候")?;
                let path = codex_path.unwrap_or_else(|| {
                    state
                        .model
                        .lock()
                        .unwrap()
                        .settings
                        .preferences
                        .codex_path
                        .clone()
                });
                if path.len() > 4096 || path.contains('\0') {
                    return Err("CLI 路径无效".into());
                }
                let cli = codex_usage::find_cli(&path).map_err(|e| e.to_string())?;
                let snapshot = codex_usage::read_usage(&cli)
                    .await
                    .map_err(|e| e.to_string())?;
                Ok(Document {
                    id: request_id.clone(),
                    title: "剩余额度".into(),
                    observed_at: snapshot.observed_at,
                    usage: Some(serde_json::to_value(snapshot).map_err(|_| "额度解析失败")?),
                    ..Document::default()
                })
            }
            ProviderKind::Local => {
                let entry = config.entries.first().ok_or("请先添加文字或图片")?;
                Ok(Document {
                    id: entry.id.clone(),
                    text: entry.text.clone(),
                    image: entry
                        .image
                        .as_ref()
                        .map(|id| crate::assets::Assets::url(id)),
                    observed_at: crate::content::now(),
                    ..Document::default()
                })
            }
            _ => {
                let mut content = crate::content::fetch(&config, &state.client).await?;
                if let Some(url) = content.image.take() {
                    content.image = Some(crate::assets::Assets::url(
                        &state.assets.download(&state.client, &url).await?,
                    ));
                }
                Ok(content)
            }
        }
    };
    let result = tokio::select! { _ = cancellation.cancelled() => Err("请求已取消".into()), result = read => result };
    state.requests.lock().unwrap().remove(&key);
    result
}
#[tauri::command]
pub fn cancel_source(request_id: String, window: WebviewWindow, state: State<'_, Arc<Shared>>) {
    if let Some(request) = state
        .requests
        .lock()
        .unwrap()
        .get(&format!("{}:{request_id}", window.label()))
    {
        request.cancel();
    }
}
#[tauri::command]
pub fn get_queue(state: State<'_, Arc<Shared>>) -> push::Snapshot {
    state.queue.lock().unwrap().snapshot()
}
#[tauri::command]
pub fn mark_showing(id: String, app: AppHandle, state: State<'_, Arc<Shared>>) -> bool {
    if !state.model.lock().unwrap().status.visible || state.settings_open.load(Ordering::Relaxed) {
        return false;
    }
    let changed = state.queue.lock().unwrap().showing(&id);
    if changed {
        push::notify(&app, &state);
    }
    changed
}
#[tauri::command]
pub fn finish_message(
    id: String,
    error: Option<String>,
    app: AppHandle,
    state: State<'_, Arc<Shared>>,
) {
    let changed = state
        .queue
        .lock()
        .unwrap()
        .finish(&id, error.map(|s| s.chars().take(256).collect()));
    if changed {
        push::notify(&app, &state);
    }
}
#[tauri::command]
pub fn clear_queue(app: AppHandle, state: State<'_, Arc<Shared>>) {
    state.queue.lock().unwrap().clear();
    push::notify(&app, &state);
}
#[tauri::command]
pub fn get_server_status(state: State<'_, Arc<Shared>>) -> push::ServerStatus {
    push::status(&state)
}
#[tauri::command]
pub fn get_http_token(state: State<'_, Arc<Shared>>) -> String {
    state.token.clone()
}
#[tauri::command]
pub async fn open_settings(app: AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("settings") {
        window.show().map_err(|_| "无法显示设置")?;
        window.set_focus().map_err(|_| "无法聚焦设置")?;
    } else {
        tauri::WebviewWindowBuilder::new(
            &app,
            "settings",
            tauri::WebviewUrl::App("index.html?view=settings".into()),
        )
        .title("Silverwolf · 挂件设置")
        .inner_size(880.0, 680.0)
        .min_inner_size(700.0, 520.0)
        .on_navigation(crate::trusted_navigation)
        .build()
        .map_err(|_| "无法创建设置窗口")?;
    }
    set_settings_activity(true, app.clone(), app.state::<Arc<Shared>>());
    Ok(())
}
#[tauri::command]
pub fn set_settings_activity(open: bool, app: AppHandle, state: State<'_, Arc<Shared>>) {
    state.settings_open.store(open, Ordering::Relaxed);
    let _ = app.emit_to("main", "settings-activity", open);
}
