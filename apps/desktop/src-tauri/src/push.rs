use crate::{
    assets::{image_extension, Assets, MAX_IMAGE},
    content::{now, Document},
    Shared,
};
use axum::{
    extract::{DefaultBodyLimit, Multipart, Path, State},
    http::{HeaderMap, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::{collections::VecDeque, sync::Arc};
use tauri::{AppHandle, Emitter};

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    Preparing,
    Waiting,
    Showing,
    Completed,
    Failed,
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Message {
    pub id: String,
    pub content: Document,
    pub duration_seconds: u32,
    pub status: Status,
    pub error: Option<String>,
}
#[derive(Default)]
pub struct Queue {
    pending: VecDeque<Message>,
    history: VecDeque<Message>,
    revision: u64,
}
#[derive(Clone, Serialize)]
pub struct Snapshot {
    revision: u64,
    messages: Vec<Message>,
}
impl Queue {
    pub fn snapshot(&self) -> Snapshot {
        Snapshot {
            revision: self.revision,
            messages: self.pending.iter().cloned().collect(),
        }
    }
    pub fn len(&self) -> usize {
        self.pending.len()
    }
    pub fn get(&self, id: &str) -> Option<Message> {
        self.pending
            .iter()
            .chain(self.history.iter())
            .find(|m| m.id == id)
            .cloned()
    }
    pub fn enqueue(&mut self, input: Input) -> Result<String, String> {
        input.validate()?;
        if self.pending.len() >= 100 {
            return Err("queue_full".into());
        }
        let id = uuid::Uuid::new_v4().to_string();
        self.pending.push_back(Message {
            id: id.clone(),
            content: Document {
                id: id.clone(),
                title: input.title.unwrap_or_default(),
                text: input.text.unwrap_or_default(),
                observed_at: now(),
                ..Document::default()
            },
            duration_seconds: input.duration_seconds.unwrap_or(8),
            status: if input.image.is_some() {
                Status::Preparing
            } else {
                Status::Waiting
            },
            error: None,
        });
        self.revision += 1;
        Ok(id)
    }
    pub fn prepared(&mut self, id: &str, image: Result<String, String>) {
        if let Some(index) = self.pending.iter().position(|m| m.id == id) {
            match image {
                Ok(url) => {
                    let m = &mut self.pending[index];
                    m.content.image = Some(url);
                    m.status = Status::Waiting;
                    self.revision += 1;
                }
                Err(error) => {
                    self.finish_at(index, Some(error));
                }
            }
        }
    }
    pub fn showing(&mut self, id: &str) -> bool {
        if let Some(m) = self
            .pending
            .front_mut()
            .filter(|m| m.id == id && m.status == Status::Waiting)
        {
            m.status = Status::Showing;
            self.revision += 1;
            return true;
        }
        false
    }
    pub fn finish(&mut self, id: &str, error: Option<String>) -> bool {
        if self.pending.front().is_some_and(|m| m.id == id) {
            self.finish_at(0, error);
            return true;
        }
        false
    }
    fn finish_at(&mut self, index: usize, error: Option<String>) {
        if let Some(mut m) = self.pending.remove(index) {
            m.status = if error.is_some() {
                Status::Failed
            } else {
                Status::Completed
            };
            m.error = error;
            self.history.push_back(m);
            while self.history.len() > 200 {
                self.history.pop_front();
            }
            self.revision += 1;
        }
    }
    pub fn clear(&mut self) {
        while !self.pending.is_empty() {
            self.finish_at(0, Some("已清空队列".into()));
        }
    }
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Input {
    pub title: Option<String>,
    pub text: Option<String>,
    pub image: Option<ImageInput>,
    pub duration_seconds: Option<u32>,
}
#[derive(Clone, Deserialize)]
#[serde(untagged)]
pub enum ImageInput {
    Asset(AssetInput),
    Url(UrlInput),
}
#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AssetInput {
    pub asset_id: String,
}
#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UrlInput {
    pub url: String,
}
impl Input {
    fn validate(&self) -> Result<(), String> {
        if self.text.as_ref().is_none_or(|s| s.trim().is_empty()) && self.image.is_none() {
            return Err("至少提供 text 或 image".into());
        }
        if self.text.as_ref().is_some_and(|s| s.len() > 32000)
            || self.title.as_ref().is_some_and(|s| s.len() > 256)
            || self
                .duration_seconds
                .is_some_and(|s| !(1..=3600).contains(&s))
        {
            return Err("消息长度或展示时长无效".into());
        }
        match &self.image {
            Some(ImageInput::Url(i)) => {
                crate::content::validate_url(&i.url)?;
            }
            Some(ImageInput::Asset(i)) if !crate::assets::valid_id(&i.asset_id) => {
                return Err("图片标识无效".into())
            }
            _ => (),
        }
        Ok(())
    }
}

#[derive(Default)]
pub struct HttpServer {
    pub task: Option<tauri::async_runtime::JoinHandle<()>>,
    pub port: u16,
    pub running: bool,
    pub error: Option<String>,
}
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerStatus {
    pub running: bool,
    pub port: u16,
    pub error: Option<String>,
    pub visible: bool,
    pub queue_length: usize,
}
pub fn status(shared: &Shared) -> ServerStatus {
    let server = shared.server.lock().unwrap();
    ServerStatus {
        running: server.running,
        port: server.port,
        error: server.error.clone(),
        visible: shared.model.lock().unwrap().status.visible,
        queue_length: shared.queue.lock().unwrap().len(),
    }
}
pub fn notify(app: &AppHandle, shared: &Shared) {
    let snapshot = shared.queue.lock().unwrap().snapshot();
    let _ = app.emit_to("main", "queue-changed", snapshot);
}
#[derive(Clone)]
struct ApiState {
    app: AppHandle,
    shared: Arc<Shared>,
    token: String,
    port: u16,
}
type ApiError = (StatusCode, Json<serde_json::Value>);
fn error(status: StatusCode, message: impl Into<String>) -> ApiError {
    (status, Json(serde_json::json!({"error":message.into()})))
}

fn authorized(headers: &HeaderMap, token: &str, port: u16) -> bool {
    let expected = format!("Bearer {token}");
    let Some(actual) = headers.get("authorization").and_then(|v| v.to_str().ok()) else {
        return false;
    };
    let equal = actual.len() == expected.len()
        && actual
            .bytes()
            .zip(expected.bytes())
            .fold(0u8, |diff, (a, b)| diff | (a ^ b))
            == 0;
    let host = headers
        .get("host")
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default();
    equal
        && !headers.contains_key("origin")
        && [format!("127.0.0.1:{port}"), format!("localhost:{port}")].contains(&host.to_string())
}
async fn auth(
    State(state): State<ApiState>,
    request: axum::extract::Request,
    next: Next,
) -> Response {
    if !authorized(request.headers(), &state.token, state.port) {
        return error(
            StatusCode::UNAUTHORIZED,
            "需要本机 Bearer 令牌；不接受网页跨域调用",
        )
        .into_response();
    }
    next.run(request).await
}
async fn post_message(
    State(state): State<ApiState>,
    Json(input): Json<Input>,
) -> Result<impl IntoResponse, ApiError> {
    input
        .validate()
        .map_err(|e| error(StatusCode::BAD_REQUEST, e))?;
    let image = input.image.clone();
    if let Some(ImageInput::Asset(asset)) = &image {
        let path = state
            .shared
            .assets
            .asset_path(&asset.asset_id)
            .map_err(|e| error(StatusCode::BAD_REQUEST, e))?;
        let bytes =
            std::fs::read(path).map_err(|_| error(StatusCode::BAD_REQUEST, "读取图片失败"))?;
        image_extension(&bytes).map_err(|e| error(StatusCode::BAD_REQUEST, e))?;
    }
    let id = state
        .shared
        .queue
        .lock()
        .unwrap()
        .enqueue(input)
        .map_err(|e| {
            error(
                if e == "queue_full" {
                    StatusCode::TOO_MANY_REQUESTS
                } else {
                    StatusCode::BAD_REQUEST
                },
                e,
            )
        })?;
    notify(&state.app, &state.shared);
    if let Some(image) = image {
        let state = state.clone();
        let id = id.clone();
        tauri::async_runtime::spawn(async move {
            let result = match image {
                ImageInput::Asset(asset) => Ok(Assets::url(&asset.asset_id)),
                ImageInput::Url(image) => state
                    .shared
                    .assets
                    .download(&state.shared.client, &image.url)
                    .await
                    .map(|id| Assets::url(&id)),
            };
            state.shared.queue.lock().unwrap().prepared(&id, result);
            notify(&state.app, &state.shared);
        });
    }
    Ok((
        StatusCode::ACCEPTED,
        Json(serde_json::json!({"id":id, "accepted":true})),
    ))
}
async fn upload(
    State(state): State<ApiState>,
    mut multipart: Multipart,
) -> Result<Json<serde_json::Value>, ApiError> {
    let field = multipart
        .next_field()
        .await
        .map_err(|_| error(StatusCode::BAD_REQUEST, "无效 multipart 请求"))?
        .ok_or_else(|| error(StatusCode::BAD_REQUEST, "缺少图片文件"))?;
    let bytes = field
        .bytes()
        .await
        .map_err(|_| error(StatusCode::PAYLOAD_TOO_LARGE, "图片超过 8 MB"))?;
    let ext = image_extension(&bytes).map_err(|e| error(StatusCode::BAD_REQUEST, e))?;
    let id = state
        .shared
        .assets
        .store(&bytes, ext, true)
        .map_err(|e| error(StatusCode::BAD_REQUEST, e))?;
    Ok(Json(serde_json::json!({"assetId":id})))
}
async fn get_message(
    State(state): State<ApiState>,
    Path(id): Path<String>,
) -> Result<Json<Message>, ApiError> {
    state
        .shared
        .queue
        .lock()
        .unwrap()
        .get(&id)
        .map(Json)
        .ok_or_else(|| error(StatusCode::NOT_FOUND, "消息不存在或已过期"))
}
async fn get_status(State(state): State<ApiState>) -> Json<ServerStatus> {
    Json(status(&state.shared))
}

pub async fn configure(app: AppHandle, shared: Arc<Shared>) {
    let _control = shared.server_control.lock().await;
    let config = shared
        .model
        .lock()
        .unwrap()
        .settings
        .preferences
        .http
        .clone();
    let previous = shared.server.lock().unwrap().task.take();
    if let Some(task) = previous {
        task.abort();
        let _ = task.await;
    }
    {
        let mut server = shared.server.lock().unwrap();
        server.running = false;
        server.port = config.port;
        server.error = None;
    }
    if !config.enabled {
        return;
    }
    let listener =
        match tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, config.port)).await {
            Ok(listener) => listener,
            Err(_) => {
                shared.server.lock().unwrap().error =
                    Some(format!("端口 {} 无法监听，可能已被占用", config.port));
                return;
            }
        };
    let state = ApiState {
        app,
        shared: shared.clone(),
        token: shared.token.clone(),
        port: config.port,
    };
    let router = Router::new()
        .route("/v1/messages", post(post_message))
        .route("/v1/messages/{id}", get(get_message))
        .route("/v1/assets", post(upload))
        .route("/v1/status", get(get_status))
        .layer(DefaultBodyLimit::max(MAX_IMAGE + 65536))
        .route_layer(middleware::from_fn_with_state(state.clone(), auth))
        .with_state(state);
    let weak = Arc::downgrade(&shared);
    let task = tauri::async_runtime::spawn(async move {
        let result = axum::serve(listener, router).await;
        if let Some(shared) = weak.upgrade() {
            let mut server = shared.server.lock().unwrap();
            server.running = false;
            if result.is_err() {
                server.error = Some("HTTP 服务异常停止".into());
            }
        }
    });
    let mut server = shared.server.lock().unwrap();
    server.task = Some(task);
    server.running = true;
}

#[cfg(test)]
mod tests {
    use super::*;
    fn input() -> Input {
        Input {
            title: None,
            text: Some("hello".into()),
            image: None,
            duration_seconds: None,
        }
    }
    #[test]
    fn fifo_limits_and_late_completions_are_safe() {
        let mut q = Queue::default();
        let first = q.enqueue(input()).unwrap();
        let second = q.enqueue(input()).unwrap();
        assert!(!q.showing(&second));
        assert!(q.showing(&first));
        assert!(q.finish(&first, None));
        assert!(!q.finish(&first, None));
        assert_eq!(q.snapshot().messages[0].id, second);
        assert_eq!(q.get(&first).unwrap().status, Status::Completed);
        for _ in 1..100 {
            q.enqueue(input()).unwrap();
        }
        assert_eq!(q.enqueue(input()).unwrap_err(), "queue_full");
        q.clear();
        assert_eq!(q.len(), 0);
        q.prepared(&second, Ok("late".into()));
        assert_eq!(q.len(), 0);
    }
    #[test]
    fn image_failure_does_not_block_following_messages() {
        let mut q = Queue::default();
        let id = q
            .enqueue(Input {
                image: Some(ImageInput::Url(UrlInput {
                    url: "https://example.org/a.png".into(),
                })),
                ..input()
            })
            .unwrap();
        q.enqueue(input()).unwrap();
        q.prepared(&id, Err("download failed".into()));
        assert_eq!(q.len(), 1);
        assert_eq!(q.get(&id).unwrap().status, Status::Failed);
        assert!(q
            .enqueue(Input {
                duration_seconds: Some(0),
                ..input()
            })
            .is_err());
    }
    #[test]
    fn auth_rejects_browser_origins_and_rebinding_hosts() {
        let mut headers = HeaderMap::new();
        headers.insert("host", "127.0.0.1:38427".parse().unwrap());
        headers.insert("authorization", "Bearer secret".parse().unwrap());
        assert!(authorized(&headers, "secret", 38427));
        assert!(!authorized(&headers, "wrong", 38427));
        headers.insert("origin", "https://example.org".parse().unwrap());
        assert!(!authorized(&headers, "secret", 38427));
        headers.remove("origin");
        headers.insert("host", "evil.example:38427".parse().unwrap());
        assert!(!authorized(&headers, "secret", 38427));
    }
}
