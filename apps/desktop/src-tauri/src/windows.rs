//! Only this module knows about Windows handles. No injection into the Codex process.
use crate::{
    geometry::{self, Bounds},
    DragState, HitRegion, HostStatus, Shared,
};
use std::{
    cell::Cell,
    ffi::c_void,
    mem::size_of,
    ptr::null_mut,
    sync::{atomic::Ordering, Arc},
    thread,
    time::{Duration, Instant},
};
use tauri::{Emitter, WebviewWindow};
use windows_sys::Win32::{
    Foundation::*,
    Graphics::{Dwm::*, Gdi::*},
    System::Threading::*,
    UI::{Accessibility::*, HiDpi::*, Shell::*, WindowsAndMessaging::*},
};

thread_local! {
    // Out-of-context WinEvents are delivered on the registering worker thread.
    static TRACKED_HOST: Cell<HWND> = const { Cell::new(null_mut()) };
    static HOST_MOVED: Cell<bool> = const { Cell::new(false) };
    static UPDATE_NOW: Cell<bool> = const { Cell::new(false) };
    static HOST_HIDDEN: Cell<bool> = const { Cell::new(false) };
    static FOLLOWING: Cell<bool> = const { Cell::new(true) };
}

const WAKE_TRACKER: u32 = WM_APP + 1;
const FALLBACK_INTERVAL: Duration = Duration::from_secs(1);
const FRAME_INTERVAL: Duration = Duration::from_millis(16);

pub fn wake(state: &Shared) {
    let thread = state.tracker_thread.load(Ordering::Acquire);
    if thread != 0 {
        unsafe { PostThreadMessageW(thread, WAKE_TRACKER, 0, 0) };
    }
}

unsafe extern "system" fn host_location_changed(
    _hook: HWINEVENTHOOK,
    event: u32,
    hwnd: HWND,
    object: i32,
    child: i32,
    _thread: u32,
    _time: u32,
) {
    if event == EVENT_OBJECT_LOCATIONCHANGE
        && object == OBJID_WINDOW
        && child == 0
        && !hwnd.is_null()
        && TRACKED_HOST.get() == hwnd
        && !HOST_HIDDEN.get()
    {
        // No locks or window calls in this reentrant callback. Coalesce queued
        // notifications and read the latest bounds in the normal update loop.
        HOST_MOVED.set(true);
    }
}

unsafe extern "system" fn host_state_changed(
    _hook: HWINEVENTHOOK,
    event: u32,
    hwnd: HWND,
    object: i32,
    child: i32,
    _thread: u32,
    _time: u32,
) {
    if !FOLLOWING.get() {
        return;
    }
    let global = event == EVENT_SYSTEM_FOREGROUND || event == EVENT_SYSTEM_DESKTOPSWITCH;
    let host = !hwnd.is_null() && hwnd == TRACKED_HOST.get();
    let window_event = event < EVENT_OBJECT_CREATE || (object == OBJID_WINDOW && child == 0);
    if host && window_event && event == EVENT_OBJECT_REORDER {
        if !HOST_HIDDEN.get() {
            HOST_MOVED.set(true);
        }
    } else if global || (host && window_event) {
        // Visibility and final drag positions must not wait for frame coalescing.
        UPDATE_NOW.set(true);
    }
}

struct StateEvents(Vec<HWINEVENTHOOK>);

impl StateEvents {
    fn new() -> Self {
        // Small event ranges avoid subscribing to accessibility events for text,
        // selection, controls, etc. Callbacks only mark work; they never lock.
        Self(
            [
                (EVENT_SYSTEM_FOREGROUND, EVENT_SYSTEM_FOREGROUND),
                (EVENT_SYSTEM_MOVESIZEEND, EVENT_SYSTEM_MOVESIZEEND),
                (EVENT_SYSTEM_MINIMIZESTART, EVENT_SYSTEM_MINIMIZEEND),
                (EVENT_SYSTEM_DESKTOPSWITCH, EVENT_SYSTEM_DESKTOPSWITCH),
                (EVENT_OBJECT_DESTROY, EVENT_OBJECT_REORDER),
                (EVENT_OBJECT_CLOAKED, EVENT_OBJECT_UNCLOAKED),
            ]
            .into_iter()
            .filter_map(|(first, last)| {
                let hook = unsafe {
                    SetWinEventHook(
                        first,
                        last,
                        null_mut(),
                        Some(host_state_changed),
                        0,
                        0,
                        WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS,
                    )
                };
                (!hook.is_null()).then_some(hook)
            })
            .collect(),
        )
    }
}

impl Drop for StateEvents {
    fn drop(&mut self) {
        for hook in &self.0 {
            unsafe { UnhookWinEvent(*hook) };
        }
    }
}

#[derive(Default)]
struct HostEvents {
    hwnd: HWND,
    hook: HWINEVENTHOOK,
}

impl HostEvents {
    fn track(&mut self, hwnd: HWND) {
        if self.hwnd == hwnd && !self.hook.is_null() {
            return;
        }
        self.clear();
        if hwnd.is_null() {
            return;
        }
        let mut pid = 0;
        let thread = unsafe { GetWindowThreadProcessId(hwnd, &mut pid) };
        if thread == 0 || pid == 0 {
            return;
        }
        self.hwnd = hwnd;
        TRACKED_HOST.set(hwnd);
        // Scoped to the host's process/thread, never injected into Codex.
        self.hook = unsafe {
            SetWinEventHook(
                EVENT_OBJECT_LOCATIONCHANGE,
                EVENT_OBJECT_LOCATIONCHANGE,
                null_mut(),
                Some(host_location_changed),
                pid,
                thread,
                WINEVENT_OUTOFCONTEXT,
            )
        };
        // If registration fails, periodic updates still work and track retries
        // at the next discovery tick.
    }

    fn clear(&mut self) {
        TRACKED_HOST.set(null_mut());
        HOST_MOVED.set(false);
        if !self.hook.is_null() {
            unsafe { UnhookWinEvent(self.hook) };
        }
        self.hook = null_mut();
        self.hwnd = null_mut();
    }

    #[cfg(test)]
    fn wait(&self, timeout: Duration) -> bool {
        self.wait_until(timeout, Instant::now())
    }

    fn wait_until(&self, timeout: Duration, next_frame: Instant) -> bool {
        let deadline = Instant::now() + timeout;
        loop {
            // SetWinEventHook requires a message pump on its registering thread.
            // Pump only here, outside the shared model lock and position update.
            let mut message = unsafe { std::mem::zeroed() };
            for _ in 0..128 {
                if unsafe { PeekMessageW(&mut message, null_mut(), 0, 0, PM_REMOVE) } == 0 {
                    break;
                }
                if message.message == WAKE_TRACKER {
                    UPDATE_NOW.set(true);
                }
                unsafe {
                    TranslateMessage(&message);
                    DispatchMessageW(&message);
                }
            }
            let now = Instant::now();
            if UPDATE_NOW.replace(false) || (HOST_MOVED.get() && now >= next_frame) {
                HOST_MOVED.set(false);
                return true;
            }
            let remaining = deadline.saturating_duration_since(now);
            if remaining.is_zero() {
                HOST_MOVED.set(false);
                return false;
            }
            let remaining = if HOST_MOVED.get() {
                remaining.min(next_frame.saturating_duration_since(now))
            } else {
                remaining
            };
            let result = unsafe {
                MsgWaitForMultipleObjectsEx(
                    0,
                    std::ptr::null(),
                    remaining.as_millis().max(1) as u32,
                    QS_ALLINPUT,
                    MWMO_INPUTAVAILABLE,
                )
            };
            if result == WAIT_FAILED {
                thread::sleep(remaining);
                return false;
            }
        }
    }
}

impl Drop for HostEvents {
    fn drop(&mut self) {
        self.clear();
    }
}

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
        wake(&*(data as *const Shared));
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
        wake(state);
    } else if matches!(message, WM_SIZE | WM_DPICHANGED | WM_DISPLAYCHANGE) {
        wake(&*(data as *const Shared));
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
        // Reject unrelated windows before opening their processes.
        let mut class = [0u16; 256];
        let length = GetClassNameW(hwnd, class.as_mut_ptr(), class.len() as i32);
        if !String::from_utf16_lossy(&class[..length.max(0) as usize])
            .starts_with("Chrome_WidgetWin")
        {
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
        is_codex_image(&image)
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
    let foreground = unsafe { GetForegroundWindow() };
    if is_codex(foreground) {
        return foreground;
    }
    if is_codex(previous) {
        return previous;
    }
    let mut candidates = Vec::<usize>::new();
    unsafe {
        EnumWindows(Some(enumerate), &mut candidates as *mut _ as LPARAM);
    }
    candidates
        .into_iter()
        .max_by_key(|h| {
            bounds(*h as HWND)
                .map(|b| i64::from(b.right - b.left) * i64::from(b.bottom - b.top))
                .unwrap_or(0)
        })
        .map(|h| h as HWND)
        // Keep listening to a hidden host so a later SHOW event restores the
        // widget immediately even when Codex does not become foreground.
        .unwrap_or_else(|| {
            if unsafe { IsWindow(previous) } != 0 {
                previous
            } else {
                null_mut()
            }
        })
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
    let window = window.clone();
    thread::spawn(move || {
        let hwnd = handle as HWND;
        // Create the queue before publishing the thread ID to command handlers.
        let mut message = unsafe { std::mem::zeroed() };
        unsafe { PeekMessageW(&mut message, null_mut(), 0, 0, PM_NOREMOVE) };
        state
            .tracker_thread
            .store(unsafe { GetCurrentThreadId() }, Ordering::Release);
        let _state_events = StateEvents::new();
        let mut host: HWND = null_mut();
        let mut host_events = HostEvents::default();
        let mut last_foreground = null_mut();
        let mut discovered = Instant::now() - Duration::from_secs(2);
        let mut last_position = None;
        let mut shown = false;
        let mut first_update = true;
        let mut updated = Instant::now();
        loop {
            if !first_update {
                host_events.wait_until(FALLBACK_INTERVAL, updated + FRAME_INTERVAL);
            }
            first_update = false;
            updated = Instant::now();
            if unsafe { IsWindow(hwnd) } == 0 {
                break;
            }
            let follow = state.model.lock().unwrap().settings.preferences.follow;
            let mode_changed = FOLLOWING.replace(follow) != follow;
            let foreground = unsafe { GetForegroundWindow() };
            if mode_changed
                || foreground != last_foreground
                || discovered.elapsed() >= FALLBACK_INTERVAL
                || (!host.is_null() && unsafe { IsWindow(host) } == 0)
            {
                host = if follow { find_host(host) } else { null_mut() };
                host_events.track(host);
                last_foreground = foreground;
                discovered = Instant::now();
            }
            let mut model = state.model.lock().unwrap();
            // Read again under the model lock in case a command changed modes.
            let follow = model.settings.preferences.follow;
            let attached = !host.is_null() && unsafe { IsWindow(host) } != 0;
            let visible = model.ready && (!follow || attached && surface_visible(host));
            HOST_HIDDEN.set(!visible);
            if !visible {
                model.status.visible = false;
                model.status.attached = follow && attached;
                // A hidden widget cannot keep an active interactive drag.
                model.dragging = None;
                drop(model);
                if shown {
                    unsafe { ShowWindow(hwnd, SW_HIDE) };
                    shown = false;
                }
                last_position = None;
                // No bounds, DPI, monitor, anchor or toolbar work while hidden.
                continue;
            }
            let Some(own) = bounds(hwnd) else {
                break;
            };
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
            let screen = desktop_bounds(hwnd, position.or(Some((own.left, own.top))));
            let boundary = if follow {
                host_bounds.map_or(screen.right, |b| b.right.min(screen.right))
            } else {
                screen.right
            };
            let right = position.map_or(own.right, |(x, _)| x + own.right - own.left);
            let widget_dpi = unsafe { GetDpiForWindow(hwnd) }.max(96) as f64 / 96.0;
            let toolbar_left = geometry::toolbar_left(
                right,
                boundary,
                widget_dpi * model.settings.preferences.scale,
                model.status.toolbar_left,
            );
            let side_changed = toolbar_left != model.status.toolbar_left;
            model.status = HostStatus {
                visible,
                attached: follow && attached,
                toolbar_left,
                settings_open: false,
            };
            drop(model);
            if side_changed {
                let _ = window.emit("toolbar-side-changed", toolbar_left);
            }

            unsafe {
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
                // Place first, then show: restoring never flashes at stale coordinates.
                if !shown && position.is_some() && last_position == position {
                    ShowWindow(hwnd, SW_SHOWNOACTIVATE);
                    shown = true;
                }
            }
        }
        state.tracker_thread.store(0, Ordering::Release);
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestWindow(HWND);

    impl TestWindow {
        fn new() -> Self {
            let class: Vec<u16> = "STATIC\0".encode_utf16().collect();
            let hwnd = unsafe {
                CreateWindowExW(
                    0,
                    class.as_ptr(),
                    null_mut(),
                    WS_POPUP,
                    0,
                    0,
                    100,
                    100,
                    null_mut(),
                    null_mut(),
                    null_mut(),
                    null_mut(),
                )
            };
            assert!(!hwnd.is_null());
            Self(hwnd)
        }

        fn notify(&self, object: i32, child: i32) {
            unsafe { NotifyWinEvent(EVENT_OBJECT_LOCATIONCHANGE, self.0, object, child) };
        }
    }

    impl Drop for TestWindow {
        fn drop(&mut self) {
            unsafe { DestroyWindow(self.0) };
        }
    }

    #[test]
    fn host_notifications_filter_controls_and_rebind_without_stale_updates() {
        let first = TestWindow::new();
        let second = TestWindow::new();
        let mut events = HostEvents::default();
        events.track(first.0);
        assert!(!events.hook.is_null());

        first.notify(OBJID_CLIENT, 0);
        first.notify(OBJID_WINDOW, 1);
        second.notify(OBJID_WINDOW, 0);
        assert!(!events.wait(Duration::from_millis(20)));

        first.notify(OBJID_WINDOW, 0);
        first.notify(OBJID_WINDOW, 0);
        assert!(events.wait(Duration::from_secs(1)));
        assert!(!events.wait(Duration::ZERO));

        events.track(second.0);
        assert!(!events.hook.is_null());
        first.notify(OBJID_WINDOW, 0);
        assert!(!events.wait(Duration::from_millis(20)));
        second.notify(OBJID_WINDOW, 0);
        assert!(events.wait(Duration::from_secs(1)));

        events.track(null_mut());
        second.notify(OBJID_WINDOW, 0);
        assert!(events.hook.is_null());
        assert!(!events.wait(Duration::from_millis(20)));
    }

    #[test]
    fn host_movement_wakes_the_worker_and_reads_the_latest_bounds() {
        let (host_tx, host_rx) = std::sync::mpsc::channel();
        let (move_tx, move_rx) = std::sync::mpsc::channel();
        let host_thread = thread::spawn(move || {
            let window = TestWindow::new();
            host_tx.send(window.0 as usize).unwrap();
            while let Ok((x, y)) = move_rx.recv() {
                assert_ne!(
                    unsafe {
                        SetWindowPos(
                            window.0,
                            null_mut(),
                            x,
                            y,
                            0,
                            0,
                            SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
                        )
                    },
                    0
                );
                // Hidden test windows need an explicit accessibility notification.
                window.notify(OBJID_WINDOW, 0);
                host_tx.send(window.0 as usize).unwrap();
            }
        });
        let host = host_rx.recv().unwrap() as HWND;
        let mut events = HostEvents::default();
        events.track(host);
        assert!(!events.hook.is_null());
        for (x, y) in [(40, 70), (80, 100), (140, 150)] {
            move_tx.send((x, y)).unwrap();
            // Let any native and explicit notifications finish queuing before
            // draining them, so a late duplicate cannot satisfy the next move.
            host_rx.recv().unwrap();
            assert!(events.wait(Duration::from_secs(1)));
            let rect = bounds(host).unwrap();
            assert_eq!((rect.left, rect.top), (x, y));
        }
        drop(events);
        assert!(TRACKED_HOST.get().is_null());
        drop(move_tx);
        host_thread.join().unwrap();
    }

    #[test]
    fn hidden_hosts_ignore_motion_but_restore_and_drag_end_wake_immediately() {
        let host = TestWindow::new();
        let unrelated = TestWindow::new();
        let mut events = HostEvents::default();
        events.track(host.0);
        FOLLOWING.set(true);
        HOST_HIDDEN.set(true);

        host.notify(OBJID_WINDOW, 0);
        assert!(!events.wait(Duration::from_millis(20)));
        unsafe {
            host_state_changed(
                null_mut(),
                EVENT_OBJECT_SHOW,
                unrelated.0,
                OBJID_WINDOW,
                0,
                0,
                0,
            );
            host_state_changed(null_mut(), EVENT_OBJECT_SHOW, host.0, OBJID_CLIENT, 0, 0, 0);
        }
        assert!(!events.wait(Duration::ZERO));

        // State transitions bypass even a distant movement deadline.
        for event in [
            EVENT_OBJECT_SHOW,
            EVENT_OBJECT_HIDE,
            EVENT_OBJECT_UNCLOAKED,
            EVENT_SYSTEM_MINIMIZEEND,
            EVENT_SYSTEM_MOVESIZEEND,
            EVENT_OBJECT_DESTROY,
        ] {
            unsafe { host_state_changed(null_mut(), event, host.0, OBJID_WINDOW, 0, 0, 0) };
            assert!(events.wait_until(Duration::ZERO, Instant::now() + Duration::from_secs(30)));
            assert!(!events.wait(Duration::ZERO));
        }
        HOST_HIDDEN.set(false);
        host.notify(OBJID_WINDOW, 0);
        assert!(events.wait(Duration::from_secs(1)));

        // Desktop mode does not wake for unrelated foreground switches.
        FOLLOWING.set(false);
        unsafe { host_state_changed(null_mut(), EVENT_SYSTEM_FOREGROUND, unrelated.0, 0, 0, 0, 0) };
        assert!(!events.wait(Duration::ZERO));
        FOLLOWING.set(true);
    }

    #[test]
    fn movement_bursts_wait_for_one_frame_and_commands_bypass_the_frame_deadline() {
        let host = TestWindow::new();
        let mut events = HostEvents::default();
        events.track(host.0);
        for _ in 0..30 {
            host.notify(OBJID_WINDOW, 0);
        }
        let frame = Instant::now() + FRAME_INTERVAL;
        assert!(events.wait_until(Duration::from_secs(1), frame));
        assert!(Instant::now() >= frame);
        assert!(!events.wait(Duration::ZERO));

        // Commands posted from another thread wake the same native message pump.
        let worker = unsafe { GetCurrentThreadId() };
        thread::spawn(move || {
            assert_ne!(unsafe { PostThreadMessageW(worker, WAKE_TRACKER, 0, 0) }, 0);
        })
        .join()
        .unwrap();
        assert!(events.wait_until(Duration::ZERO, Instant::now() + Duration::from_secs(30)));
        assert!(!events.wait(Duration::ZERO));
    }

    #[test]
    fn state_hooks_register_and_idle_wait_does_not_poll_at_fifty_milliseconds() {
        let hooks = StateEvents::new();
        assert_eq!(hooks.0.len(), 6);
        // Ignore ambient foreground activity for this idle scheduling check.
        FOLLOWING.set(false);
        let events = HostEvents::default();
        let started = Instant::now();
        assert!(!events.wait(Duration::from_millis(120)));
        assert!(started.elapsed() >= Duration::from_millis(120));
        FOLLOWING.set(true);
    }

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
