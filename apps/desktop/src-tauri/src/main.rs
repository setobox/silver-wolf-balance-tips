#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod assets;
mod commands;
mod content;
mod geometry;
mod push;
mod settings;
mod theme_server;
#[cfg(windows)]
mod windows;

use codex_usage::{Snapshot, UsageError};
use serde::{Deserialize, Serialize};
use settings::{Preferences, Settings};
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
    time::Instant,
};
use tauri::{
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    Emitter, Manager, State, WebviewWindow,
};

#[derive(Default, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HostStatus {
    visible: bool,
    attached: bool,
    toolbar_left: bool,
    settings_open: bool,
}

pub struct Model {
    settings: Settings,
    ready: bool,
    dragging: Option<DragState>,
    status: HostStatus,
}

pub enum DragState {
    Pending(Instant),
    Moving,
    Finished,
}

pub struct Shared {
    model: Mutex<Model>,
    query: tokio::sync::Mutex<()>,
    file: PathBuf,
    assets: assets::Assets,
    client: reqwest::Client,
    queue: Mutex<push::Queue>,
    server: Mutex<push::HttpServer>,
    server_control: tokio::sync::Mutex<()>,
    token: String,
    requests: Mutex<std::collections::HashMap<String, tokio_util::sync::CancellationToken>>,
    settings_open: std::sync::atomic::AtomicBool,
    #[cfg(windows)]
    tracker_thread: std::sync::atomic::AtomicU32,
}

impl Shared {
    fn wake_tracker(&self) {
        #[cfg(windows)]
        windows::wake(self);
    }
}

#[derive(Deserialize)]
pub struct HitRegion {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

#[tauri::command]
fn get_preferences(state: State<'_, Arc<Shared>>) -> Preferences {
    state.model.lock().unwrap().settings.preferences.clone()
}

#[tauri::command]
fn get_host_status(state: State<'_, Arc<Shared>>) -> HostStatus {
    let mut status = state.model.lock().unwrap().status;
    status.settings_open = state
        .settings_open
        .load(std::sync::atomic::Ordering::Relaxed);
    status
}

#[tauri::command]
async fn set_preferences(
    preferences: Preferences,
    app: tauri::AppHandle,
    state: State<'_, Arc<Shared>>,
) -> Result<Preferences, String> {
    preferences.validate()?;
    let http_changed = {
        let mut model = state.model.lock().unwrap();
        let changed = model.settings.preferences.http != preferences.http;
        let mut settings = model.settings.clone();
        settings.preferences = preferences.clone();
        settings.save(&state.file)?;
        model.settings = settings;
        changed
    };
    let window = app.get_webview_window("main").ok_or("主窗口不存在")?;
    let layout = state.assets.theme(&preferences.appearance).layout;
    window
        .set_size(tauri::LogicalSize::new(
            layout.width * preferences.scale,
            layout.height * preferences.scale,
        ))
        .map_err(|_| "无法调整窗口大小")?;
    let _ = app.emit("preferences-changed", &preferences);
    state.wake_tracker();
    if http_changed {
        push::configure(app, state.inner().clone()).await;
    }
    Ok(preferences)
}

#[tauri::command]
async fn read_usage(state: State<'_, Arc<Shared>>) -> Result<Snapshot, UsageError> {
    let _guard = state
        .query
        .try_lock()
        .map_err(|_| UsageError::new("busy", "正在刷新，请稍候。"))?;
    let configured = state
        .model
        .lock()
        .unwrap()
        .settings
        .preferences
        .codex_path
        .clone();
    let cli = codex_usage::find_cli(&configured)?;
    codex_usage::read_usage(&cli).await
}

#[tauri::command]
fn ready(state: State<'_, Arc<Shared>>) {
    state.model.lock().unwrap().ready = true;
    state.wake_tracker();
}

#[tauri::command]
fn begin_drag(window: WebviewWindow, state: State<'_, Arc<Shared>>) -> Result<(), String> {
    state.model.lock().unwrap().dragging = Some(DragState::Pending(Instant::now()));
    if window.start_dragging().is_err() {
        state.model.lock().unwrap().dragging = None;
        state.wake_tracker();
        return Err("无法开始拖动窗口".into());
    }
    Ok(())
}

#[tauri::command]
fn set_hit_regions(regions: Vec<HitRegion>, window: WebviewWindow) -> Result<(), String> {
    if regions.is_empty()
        || regions.len() > 4
        || regions.iter().any(|r| {
            [r.x, r.y, r.width, r.height]
                .iter()
                .any(|v| !v.is_finite() || v.abs() > 10000.0)
                || r.width <= 0.0
                || r.height <= 0.0
        })
    {
        return Err("交互区域无效".into());
    }
    #[cfg(windows)]
    windows::set_regions(&window, &regions)?;
    Ok(())
}

fn tray(app: &tauri::App) -> tauri::Result<()> {
    let settings = MenuItem::with_id(app, "settings", "挂件设置", true, None::<&str>)?;
    let theme_reset = MenuItem::with_id(app, "theme-reset", "恢复默认外观", true, None::<&str>)?;
    let follow = MenuItem::with_id(app, "follow", "跟随 Codex 窗口", true, None::<&str>)?;
    let desktop = MenuItem::with_id(app, "desktop", "显示在桌面", true, None::<&str>)?;
    let reset = MenuItem::with_id(app, "reset", "重置位置和大小", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "退出 Silverwolf", true, None::<&str>)?;
    let menu = Menu::with_items(
        app,
        &[&settings, &follow, &desktop, &reset, &theme_reset, &quit],
    )?;
    let mut builder = TrayIconBuilder::new()
        .tooltip("Silverwolf · 桌面角色挂件")
        .menu(&menu)
        .show_menu_on_left_click(true);
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    builder
        .on_menu_event(|app, event| {
            if event.id.as_ref() == "settings" {
                let app = app.clone();
                tauri::async_runtime::spawn(async move {
                    let _ = commands::open_settings(app).await;
                });
                return;
            }
            if event.id.as_ref() == "quit" {
                app.exit(0);
                return;
            }
            let state = app.state::<Arc<Shared>>();
            let preferences = {
                let mut model = state.model.lock().unwrap();
                let mut next = model.settings.clone();
                match event.id.as_ref() {
                    "theme-reset" => next.preferences.appearance = settings::Appearance::default(),
                    "follow" => next.preferences.follow = true,
                    "desktop" => next.preferences.follow = false,
                    "reset" => {
                        next.right = 16.0;
                        next.bottom = 16.0;
                        next.desktop_position = None;
                        next.preferences.scale = 1.0;
                    }
                    _ => return,
                }
                if next.save(&state.file).is_err() {
                    return;
                }
                model.settings = next;
                model.settings.preferences.clone()
            };
            if let Some(window) = app.get_webview_window("main") {
                let layout = state.assets.theme(&preferences.appearance).layout;
                let _ = window.set_size(tauri::LogicalSize::new(
                    layout.width * preferences.scale,
                    layout.height * preferences.scale,
                ));
                let _ = app.emit("preferences-changed", preferences);
            }
            state.wake_tracker();
        })
        .build(app)?;
    Ok(())
}

fn trusted_navigation(url: &tauri::Url) -> bool {
    url.scheme() == "tauri"
        || (url.scheme() == "http"
            && (url.host_str() == Some("tauri.localhost")
                || url.host_str() == Some("127.0.0.1") && url.port() == Some(1420)))
}

fn resource_response(
    app: &tauri::AppHandle,
    request: tauri::http::Request<Vec<u8>>,
) -> tauri::http::Response<Vec<u8>> {
    let path = percent_encoding::percent_decode_str(request.uri().path()).decode_utf8_lossy();
    let content_type = assets::mime(&path);
    let data = if path.starts_with("/assets/") {
        app.try_state::<Arc<Shared>>()
            .ok_or_else(|| "应用尚未就绪".to_string())
            .and_then(|state| state.assets.read_resource(&path))
    } else {
        Err("无效资源".into())
    };
    let (status, data) = match data {
        Ok(data) => (200, data),
        Err(_) => (404, Vec::new()),
    };
    tauri::http::Response::builder()
        .status(status)
        .header("Content-Type", content_type)
        .header("X-Content-Type-Options", "nosniff")
        .header("Access-Control-Allow-Origin", "*")
        .header("Cache-Control", "no-store")
        .header("Content-Security-Policy", "default-src 'none'; sandbox")
        .body(data)
        .expect("static response headers")
}

fn main() {
    tauri::Builder::default()
        .register_uri_scheme_protocol("media", |context, request| {
            resource_response(context.app_handle(), request)
        })
        .on_window_event(|window, event| {
            if window.label() == "settings" && matches!(event, tauri::WindowEvent::Destroyed) {
                let app = window.app_handle();
                if let Some(state) = app.try_state::<Arc<Shared>>() {
                    commands::set_settings_activity(false, app.clone(), state);
                }
            }
        })
        .plugin(tauri_plugin_single_instance::init(|app, _, _| {
            // A second launch makes the existing widget reachable even without Codex.
            let state = app.state::<Arc<Shared>>();
            let preferences = {
                let mut model = state.model.lock().unwrap();
                model.settings.preferences.follow = false;
                model.settings.preferences.clone()
            };
            let _ = app.emit("preferences-changed", preferences);
            state.wake_tracker();
        }))
        .setup(|app| {
            // Development validation can use a disposable profile without touching user settings.
            let development_profile = if cfg!(debug_assertions) {
                std::env::var_os("SILVERWOLF_DEV_DATA")
                    .map(PathBuf::from)
                    .filter(|p| p.is_absolute())
            } else {
                None
            };
            let file = development_profile
                .clone()
                .unwrap_or(app.path().app_config_dir()?)
                .join("settings.json");
            let settings = Settings::load(&file);
            let mut assets =
                assets::Assets::new(development_profile.unwrap_or(app.path().app_data_dir()?))?;
            let theme_listener = std::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))?;
            assets.theme_base = format!(
                "http://127.0.0.1:{}/{}",
                theme_listener.local_addr()?.port(),
                uuid::Uuid::new_v4().simple()
            );
            let layout = assets.theme(&settings.preferences.appearance).layout;
            let token_file = file.with_file_name("http-token");
            let token = std::fs::read_to_string(&token_file)
                .ok()
                .filter(|s| s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit()))
                .unwrap_or_else(|| {
                    format!(
                        "{}{}",
                        uuid::Uuid::new_v4().simple(),
                        uuid::Uuid::new_v4().simple()
                    )
                });
            std::fs::create_dir_all(file.parent().unwrap())?;
            std::fs::write(token_file, &token)?;
            let window = app
                .get_webview_window("main")
                .ok_or("main window missing")?;
            window.set_size(tauri::LogicalSize::new(
                layout.width * settings.preferences.scale,
                layout.height * settings.preferences.scale,
            ))?;
            let state = Arc::new(Shared {
                model: Mutex::new(Model {
                    settings,
                    ready: false,
                    dragging: None,
                    status: HostStatus::default(),
                }),
                query: tokio::sync::Mutex::new(()),
                file,
                assets,
                token,
                client: reqwest::Client::builder()
                    .timeout(std::time::Duration::from_secs(10))
                    .redirect(reqwest::redirect::Policy::limited(3))
                    .build()?,
                queue: Mutex::new(push::Queue::default()),
                server: Mutex::new(push::HttpServer::default()),
                server_control: tokio::sync::Mutex::new(()),
                requests: Mutex::new(std::collections::HashMap::new()),
                settings_open: std::sync::atomic::AtomicBool::new(false),
                #[cfg(windows)]
                tracker_thread: std::sync::atomic::AtomicU32::new(0),
            });
            app.manage(state.clone());
            theme_server::start(theme_listener, state.clone())?;
            tray(app)?;
            tauri::async_runtime::spawn(push::configure(app.handle().clone(), state.clone()));
            #[cfg(windows)]
            windows::start(&window, state)?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_preferences,
            get_host_status,
            set_preferences,
            read_usage,
            ready,
            begin_drag,
            set_hit_regions,
            commands::get_theme,
            commands::import_asset,
            commands::import_theme,
            commands::read_source,
            commands::cancel_source,
            commands::get_queue,
            commands::mark_showing,
            commands::finish_message,
            commands::clear_queue,
            commands::get_server_status,
            commands::get_http_token,
            commands::open_settings,
            commands::set_settings_activity
        ])
        .run(tauri::generate_context!())
        .expect("Unable to start Silverwolf");
}
