#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod geometry;
mod settings;
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

const WIDTH: f64 = 336.0;
const HEIGHT: f64 = 490.0;

#[derive(Default, Clone, Copy, Serialize)]
pub struct HostStatus {
    visible: bool,
    attached: bool,
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
    state.model.lock().unwrap().status
}

#[tauri::command]
fn set_preferences(
    preferences: Preferences,
    window: WebviewWindow,
    state: State<'_, Arc<Shared>>,
) -> Result<Preferences, String> {
    preferences.validate()?;
    let mut model = state.model.lock().unwrap();
    let mut settings = model.settings.clone();
    settings.preferences = preferences.clone();
    settings.save(&state.file)?;
    model.settings = settings;
    drop(model);
    window
        .set_size(tauri::LogicalSize::new(
            WIDTH * preferences.scale,
            HEIGHT * preferences.scale,
        ))
        .map_err(|_| "无法调整窗口大小")?;
    let _ = window.emit("preferences-changed", &preferences);
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
}

#[tauri::command]
fn begin_drag(window: WebviewWindow, state: State<'_, Arc<Shared>>) -> Result<(), String> {
    state.model.lock().unwrap().dragging = Some(DragState::Pending(Instant::now()));
    if window.start_dragging().is_err() {
        state.model.lock().unwrap().dragging = None;
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
    let follow = MenuItem::with_id(app, "follow", "跟随 Codex 窗口", true, None::<&str>)?;
    let desktop = MenuItem::with_id(app, "desktop", "显示在桌面", true, None::<&str>)?;
    let reset = MenuItem::with_id(app, "reset", "重置位置和大小", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "退出 Silverwolf", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&follow, &desktop, &reset, &quit])?;
    let mut builder = TrayIconBuilder::new()
        .tooltip("Silverwolf · Codex 额度")
        .menu(&menu)
        .show_menu_on_left_click(true);
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    builder
        .on_menu_event(|app, event| {
            if event.id.as_ref() == "quit" {
                app.exit(0);
                return;
            }
            let state = app.state::<Arc<Shared>>();
            let preferences = {
                let mut model = state.model.lock().unwrap();
                let mut next = model.settings.clone();
                match event.id.as_ref() {
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
                let _ = window.set_size(tauri::LogicalSize::new(
                    WIDTH * preferences.scale,
                    HEIGHT * preferences.scale,
                ));
                let _ = window.emit("preferences-changed", preferences);
            }
        })
        .build(app)?;
    Ok(())
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _, _| {
            // A second launch makes the existing widget reachable even without Codex.
            let state = app.state::<Arc<Shared>>();
            let preferences = {
                let mut model = state.model.lock().unwrap();
                model.settings.preferences.follow = false;
                model.settings.preferences.clone()
            };
            let _ = app.emit("preferences-changed", preferences);
        }))
        .setup(|app| {
            let file = app.path().app_config_dir()?.join("settings.json");
            let settings = Settings::load(&file);
            let window = app
                .get_webview_window("main")
                .ok_or("main window missing")?;
            window.set_size(tauri::LogicalSize::new(
                WIDTH * settings.preferences.scale,
                HEIGHT * settings.preferences.scale,
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
            });
            app.manage(state.clone());
            tray(app)?;
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
            set_hit_regions
        ])
        .run(tauri::generate_context!())
        .expect("Unable to start Silverwolf");
}
