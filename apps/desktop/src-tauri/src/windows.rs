//! Only this module knows about Windows handles. No injection into the Codex process.
use crate::{
    geometry::{self, Bounds},
    DragState, HitRegion, HostStatus, Shared,
};
use std::{
    ffi::c_void,
    mem::size_of,
    ptr::null_mut,
    sync::Arc,
    thread,
    time::{Duration, Instant},
};
use tauri::WebviewWindow;
use windows_sys::Win32::{
    Foundation::*,
    Graphics::{Dwm::*, Gdi::*},
    System::Threading::*,
    UI::{HiDpi::*, Shell::*, WindowsAndMessaging::*},
};

fn frameless_style(style: u32) -> u32 {
    (style & !(WS_CAPTION | WS_THICKFRAME | WS_SYSMENU | WS_MINIMIZEBOX | WS_MAXIMIZEBOX))
        | WS_POPUP
}

fn frameless_ex_style(style: u32) -> u32 {
    style & !(WS_EX_WINDOWEDGE | WS_EX_CLIENTEDGE | WS_EX_DLGMODALFRAME | WS_EX_STATICEDGE)
}

unsafe extern "system" fn frameless_events(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    id: usize,
    _data: usize,
) -> LRESULT {
    match message {
        WM_STYLECHANGING if lparam != 0 => {
            // Tao retains caption styles even with decorations=false, and can
            // restore them on focus/visibility changes. Filter every later write.
            let styles = &mut *(lparam as *mut STYLESTRUCT);
            if wparam as i32 == GWL_STYLE {
                styles.styleNew = frameless_style(styles.styleNew);
            } else if wparam as i32 == GWL_EXSTYLE {
                styles.styleNew = frameless_ex_style(styles.styleNew);
            }
        }
        WM_NCCALCSIZE | WM_NCPAINT => return 0,
        // Let Tao update focus, but don't let DefWindowProc repaint a caption.
        WM_NCACTIVATE => return DefSubclassProc(hwnd, message, wparam, -1),
        WM_NCDESTROY => {
            RemoveWindowSubclass(hwnd, Some(frameless_events), id);
        }
        _ => {}
    }
    DefSubclassProc(hwnd, message, wparam, lparam)
}

unsafe fn remove_native_frame(hwnd: HWND) -> std::io::Result<()> {
    if SetWindowSubclass(hwnd, Some(frameless_events), 2, 0) == 0 {
        return Err(std::io::Error::last_os_error());
    }
    for (index, style) in [
        (
            GWL_STYLE,
            frameless_style(GetWindowLongPtrW(hwnd, GWL_STYLE) as u32),
        ),
        (
            GWL_EXSTYLE,
            frameless_ex_style(GetWindowLongPtrW(hwnd, GWL_EXSTYLE) as u32),
        ),
    ] {
        SetLastError(0);
        if SetWindowLongPtrW(hwnd, index, style as isize) == 0 && GetLastError() != 0 {
            return Err(std::io::Error::last_os_error());
        }
    }
    if SetWindowPos(
        hwnd,
        null_mut(),
        0,
        0,
        0,
        0,
        SWP_FRAMECHANGED | SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
    ) == 0
    {
        return Err(std::io::Error::last_os_error());
    }
    Ok(())
}

// The OS move loop can start after the initiating IPC has returned. Mouse-button
// polling alone can therefore save the old position before the actual move.
unsafe extern "system" fn drag_events(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    id: usize,
    data: usize,
) -> LRESULT {
    if message == WM_NCDESTROY {
        RemoveWindowSubclass(hwnd, Some(drag_events), id);
        drop(Arc::from_raw(data as *const Shared));
    } else if message == WM_ENTERSIZEMOVE || message == WM_EXITSIZEMOVE {
        let state = &*(data as *const Shared);
        if let Ok(mut model) = state.model.lock() {
            model.dragging = Some(if message == WM_ENTERSIZEMOVE {
                DragState::Moving
            } else {
                DragState::Finished
            });
        }
    }
    DefSubclassProc(hwnd, message, wparam, lparam)
}

pub fn set_regions(window: &WebviewWindow, regions: &[HitRegion]) -> Result<(), String> {
    let hwnd = window.hwnd().map_err(|_| "窗口不可用")?.0 as HWND;
    let dpi = window.scale_factor().map_err(|_| "无法读取缩放")?;
    unsafe {
        let combined = CreateRectRgn(0, 0, 0, 0);
        if combined.is_null() {
            return Err("无法创建交互区域".into());
        }
        for region in regions {
            let part = CreateRectRgn(
                (region.x * dpi).floor() as i32,
                (region.y * dpi).floor() as i32,
                ((region.x + region.width) * dpi).ceil() as i32,
                ((region.y + region.height) * dpi).ceil() as i32,
            );
            if part.is_null() {
                DeleteObject(combined);
                return Err("无法创建交互区域".into());
            }
            let result = CombineRgn(combined, combined, part, RGN_OR);
            DeleteObject(part);
            if result == 0 {
                DeleteObject(combined);
                return Err("无法合并交互区域".into());
            }
        }
        // On success Windows owns the region. Outside these areas clicks reach Codex.
        if SetWindowRgn(hwnd, combined, 1) == 0 {
            DeleteObject(combined);
            return Err("无法应用交互区域".into());
        }
    }
    Ok(())
}

fn bounds(hwnd: HWND) -> Option<Bounds> {
    let mut rect = RECT {
        left: 0,
        top: 0,
        right: 0,
        bottom: 0,
    };
    if unsafe { GetWindowRect(hwnd, &mut rect) } == 0 {
        return None;
    }
    Some(Bounds {
        left: rect.left,
        top: rect.top,
        right: rect.right,
        bottom: rect.bottom,
    })
}

fn surface_visible(hwnd: HWND) -> bool {
    let mut cloaked: u32 = 0;
    unsafe {
        IsWindowVisible(hwnd) != 0
            && IsIconic(hwnd) == 0
            && (DwmGetWindowAttribute(
                hwnd,
                DWMWA_CLOAKED as u32,
                &mut cloaked as *mut _ as *mut c_void,
                size_of::<u32>() as u32,
            ) != 0
                || cloaked == 0)
    }
}

fn is_codex(hwnd: HWND) -> bool {
    unsafe {
        if IsWindowVisible(hwnd) == 0 || !GetWindow(hwnd, GW_OWNER).is_null() {
            return false;
        }
        let mut pid = 0;
        GetWindowThreadProcessId(hwnd, &mut pid);
        let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if process.is_null() {
            return false;
        }
        let mut path = [0u16; 32768];
        let mut length = path.len() as u32;
        let success = QueryFullProcessImageNameW(process, 0, path.as_mut_ptr(), &mut length) != 0;
        CloseHandle(process);
        if !success {
            return false;
        }
        let image = String::from_utf16_lossy(&path[..length as usize]);
        if !is_codex_image(&image) {
            return false;
        }
        let mut class = [0u16; 256];
        let length = GetClassNameW(hwnd, class.as_mut_ptr(), class.len() as i32);
        String::from_utf16_lossy(&class[..length.max(0) as usize]).starts_with("Chrome_WidgetWin")
    }
}

fn is_codex_image(image: &str) -> bool {
    let path = image.replace('/', "\\").to_ascii_lowercase();
    let filename = path.rsplit('\\').next().unwrap_or_default();
    filename == "codex.exe"
        || (filename == "chatgpt.exe"
            && path
                .split('\\')
                .any(|part| part.starts_with("openai.codex_") && part.ends_with("__2p2nqsd0c76g0")))
}

unsafe extern "system" fn enumerate(hwnd: HWND, data: LPARAM) -> BOOL {
    if is_codex(hwnd) {
        let candidates = &mut *(data as *mut Vec<usize>);
        candidates.push(hwnd as usize);
    }
    1
}

fn find_host(previous: HWND) -> HWND {
    let mut candidates = Vec::<usize>::new();
    unsafe {
        EnumWindows(Some(enumerate), &mut candidates as *mut _ as LPARAM);
    }
    let foreground = unsafe { GetForegroundWindow() } as usize;
    if candidates.contains(&foreground) {
        return foreground as HWND;
    }
    if candidates.contains(&(previous as usize)) {
        return previous;
    }
    candidates
        .into_iter()
        .max_by_key(|h| {
            bounds(*h as HWND)
                .map(|b| i64::from(b.right - b.left) * i64::from(b.bottom - b.top))
                .unwrap_or(0)
        })
        .map(|h| h as HWND)
        .unwrap_or(null_mut())
}

fn desktop_bounds(hwnd: HWND, position: Option<(i32, i32)>) -> Bounds {
    unsafe {
        let monitor = match position {
            Some((x, y)) => MonitorFromPoint(POINT { x, y }, MONITOR_DEFAULTTONEAREST),
            None => MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST),
        };
        let mut info = MONITORINFO {
            cbSize: size_of::<MONITORINFO>() as u32,
            ..std::mem::zeroed()
        };
        if GetMonitorInfoW(monitor, &mut info) != 0 {
            return Bounds {
                left: info.rcWork.left,
                top: info.rcWork.top,
                right: info.rcWork.right,
                bottom: info.rcWork.bottom,
            };
        }
    }
    Bounds {
        left: 0,
        top: 0,
        right: 1920,
        bottom: 1080,
    }
}

pub fn start(window: &WebviewWindow, state: Arc<Shared>) -> tauri::Result<()> {
    // Transfer a numeric handle, never a borrowed window pointer, to the worker.
    let handle = window.hwnd()?.0 as usize;
    unsafe { remove_native_frame(handle as HWND)? };
    let events = Arc::into_raw(state.clone());
    if unsafe { SetWindowSubclass(handle as HWND, Some(drag_events), 1, events as usize) } == 0 {
        unsafe { drop(Arc::from_raw(events)) };
        return Err(std::io::Error::last_os_error().into());
    }
    thread::spawn(move || {
        let hwnd = handle as HWND;
        let mut host = null_mut();
        let mut discovered = Instant::now() - Duration::from_secs(2);
        let mut last_position = None;
        let mut shown = false;
        loop {
            if unsafe { IsWindow(hwnd) } == 0 {
                break;
            }
            if discovered.elapsed() >= Duration::from_secs(1) {
                host = find_host(host);
                discovered = Instant::now();
            }
            let Some(own) = bounds(hwnd) else {
                break;
            };
            let mut model = state.model.lock().unwrap();
            let follow = model.settings.preferences.follow;
            let attached = !host.is_null() && unsafe { IsWindow(host) } != 0;
            let visible = model.ready && (!follow || attached && surface_visible(host));
            let dpi = unsafe { GetDpiForWindow(if follow && attached { host } else { hwnd }) }
                .max(96) as f64
                / 96.0;
            let host_bounds = if attached { bounds(host) } else { None };
            if matches!(model.dragging, Some(DragState::Pending(start)) if start.elapsed() > Duration::from_secs(2))
            {
                model.dragging = None;
            }
            if matches!(model.dragging, Some(DragState::Finished)) {
                model.dragging = None;
                if follow {
                    if let Some(b) = host_bounds {
                        model.settings.right = (f64::from(b.right - own.right) / dpi).max(0.0);
                        model.settings.bottom = (f64::from(b.bottom - own.bottom) / dpi).max(0.0);
                    }
                } else {
                    model.settings.desktop_position = Some((own.left, own.top));
                }
                let _ = model.settings.save(&state.file);
                last_position = None;
            }
            let position = if visible && model.dragging.is_none() {
                let width = own.right - own.left;
                let height = own.bottom - own.top;
                if follow {
                    host_bounds.map(|b| {
                        geometry::anchored(
                            b,
                            width,
                            height,
                            model.settings.right,
                            model.settings.bottom,
                            dpi,
                        )
                    })
                } else {
                    let area = desktop_bounds(hwnd, model.settings.desktop_position);
                    let (x, y) = model
                        .settings
                        .desktop_position
                        .unwrap_or((area.right - width - 16, area.bottom - height - 16));
                    Some(geometry::clamp(area, width, height, x, y))
                }
            } else {
                None
            };
            model.status = HostStatus {
                visible,
                attached: follow && attached,
            };
            drop(model);

            unsafe {
                if shown != visible {
                    ShowWindow(hwnd, if visible { SW_SHOWNOACTIVATE } else { SW_HIDE });
                    shown = visible;
                    last_position = None;
                }
                if let Some((x, y)) = position {
                    // Place directly above Codex, below the window covering it. Never globally topmost.
                    let previous = if follow {
                        GetWindow(host, GW_HWNDPREV)
                    } else {
                        hwnd
                    };
                    let reorder = follow && previous != hwnd;
                    let insert_after = if !previous.is_null()
                        && (GetWindowLongPtrW(previous, GWL_EXSTYLE) as u32 & WS_EX_TOPMOST) != 0
                    {
                        HWND_TOP
                    } else {
                        previous
                    };
                    if last_position != Some((x, y)) || reorder {
                        let flags =
                            SWP_NOACTIVATE | SWP_NOSIZE | if reorder { 0 } else { SWP_NOZORDER };
                        if SetWindowPos(
                            hwnd,
                            if reorder { insert_after } else { null_mut() },
                            x,
                            y,
                            0,
                            0,
                            flags,
                        ) != 0
                        {
                            last_position = Some((x, y));
                        }
                    }
                }
            }
            thread::sleep(Duration::from_millis(50));
        }
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_frame_stays_removed_after_style_and_activation_changes() {
        unsafe {
            let class: Vec<u16> = "STATIC\0".encode_utf16().collect();
            let hwnd = CreateWindowExW(
                WS_EX_WINDOWEDGE,
                class.as_ptr(),
                null_mut(),
                WS_OVERLAPPEDWINDOW,
                0,
                0,
                336,
                490,
                null_mut(),
                null_mut(),
                null_mut(),
                null_mut(),
            );
            assert!(!hwnd.is_null());
            remove_native_frame(hwnd).unwrap();
            // Simulate the window library restoring its cached styles.
            SetWindowLongPtrW(hwnd, GWL_STYLE, WS_OVERLAPPEDWINDOW as isize);
            SetWindowLongPtrW(
                hwnd,
                GWL_EXSTYLE,
                (WS_EX_WINDOWEDGE | WS_EX_TOOLWINDOW) as isize,
            );
            SendMessageW(hwnd, WM_NCACTIVATE, 1, 0);
            SendMessageW(hwnd, WM_NCACTIVATE, 0, 0);
            SetWindowPos(
                hwnd,
                null_mut(),
                0,
                0,
                0,
                0,
                SWP_FRAMECHANGED | SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
            );
            let style = GetWindowLongPtrW(hwnd, GWL_STYLE) as u32;
            assert_eq!(style & WS_OVERLAPPEDWINDOW, 0);
            assert_ne!(style & WS_POPUP, 0);
            assert_eq!(
                GetWindowLongPtrW(hwnd, GWL_EXSTYLE) as u32,
                WS_EX_TOOLWINDOW
            );
            let mut client: RECT = std::mem::zeroed();
            assert_ne!(GetClientRect(hwnd, &mut client), 0);
            assert_eq!((client.right, client.bottom), (336, 490));
            assert_ne!(DestroyWindow(hwnd), 0);
        }
    }

    #[test]
    fn recognizes_codex_and_its_renamed_store_app_only() {
        assert!(is_codex_image(r"C:\Apps\Codex.exe"));
        assert!(is_codex_image(
            r"C:\Program Files\WindowsApps\OpenAI.Codex_26.1007.2314.0_x64__2p2nqsd0c76g0\app\ChatGPT.exe"
        ));
        assert!(!is_codex_image(r"C:\Unrelated\ChatGPT.exe"));
        assert!(!is_codex_image(r"C:\Unrelated\codex-helper.exe"));
    }
}
