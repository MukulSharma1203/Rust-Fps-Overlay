#![windows_subsystem = "windows"]

use std::collections::{HashSet, VecDeque};
use std::ffi::OsStr;
use std::fs;
use std::os::windows::ffi::{OsStrExt, OsStringExt};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex, OnceLock, RwLock};
use std::thread;
use std::time::{Duration, Instant};
use windows::core::{GUID, PCWSTR, PWSTR};
use windows::Win32::Foundation::{
    CloseHandle, BOOL, COLORREF, HINSTANCE, HWND, LPARAM, LRESULT, POINT, SIZE, WPARAM,
};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, CreateCompatibleDC, CreateDIBSection, CreateFontW, DeleteDC, DeleteObject,
    EndPaint, GetDC, GetStockObject, ReleaseDC, SelectObject, SetBkMode, SetTextAlign,
    SetTextColor, TextOutW, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, CLEARTYPE_QUALITY,
    CLIP_DEFAULT_PRECIS, DEFAULT_CHARSET, DEFAULT_GUI_FONT, DEFAULT_PITCH, DIB_RGB_COLORS,
    FF_MODERN, FW_BOLD, OUT_DEFAULT_PRECIS, PAINTSTRUCT, TA_RIGHT, TRANSPARENT,
};
use windows::Win32::System::Diagnostics::Etw::{
    CloseTrace, ControlTraceW, EnableTraceEx2, OpenTraceW, ProcessTrace, StartTraceW,
    CONTROLTRACE_HANDLE, EVENT_RECORD, EVENT_TRACE_CONTROL_STOP, EVENT_TRACE_LOGFILEW,
    EVENT_TRACE_PROPERTIES, EVENT_TRACE_REAL_TIME_MODE, PROCESS_TRACE_MODE_EVENT_RECORD,
    PROCESS_TRACE_MODE_REAL_TIME, WNODE_FLAG_TRACED_GUID,
};
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Threading::{
    OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows::Win32::UI::Controls::Dialogs::{
    GetOpenFileNameW, OFN_FILEMUSTEXIST, OFN_NOCHANGEDIR, OFN_PATHMUSTEXIST, OPENFILENAMEW,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    RegisterHotKey, UnregisterHotKey, HOT_KEY_MODIFIERS,
};
use windows::Win32::UI::Shell::{
    Shell_NotifyIconW, NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NOTIFYICONDATAW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreatePopupMenu, CreateWindowExW, DefWindowProcW, DestroyMenu, DispatchMessageW,
    EnumChildWindows, GetCursorPos, GetForegroundWindow, GetMessageW, GetSystemMetrics,
    GetWindowTextLengthW, GetWindowTextW, GetWindowThreadProcessId, LoadIconW, PostQuitMessage,
    RegisterClassW, SendMessageW, SetForegroundWindow, SetTimer, SetWindowPos, SetWindowTextW,
    ShowWindow, TrackPopupMenu, TranslateMessage, UpdateLayeredWindow, BM_GETCHECK, BM_SETCHECK,
    BS_AUTOCHECKBOX, BS_PUSHBUTTON, CS_HREDRAW, CS_VREDRAW, ES_AUTOHSCROLL, HICON, HWND_TOPMOST,
    ICON_BIG, ICON_SMALL, LBN_SELCHANGE, LB_ADDSTRING, LB_GETCURSEL, LB_GETTEXT, LB_GETTEXTLEN,
    LB_RESETCONTENT, MF_SEPARATOR, MF_STRING, MSG, SM_CXSCREEN, SWP_NOACTIVATE, SWP_NOMOVE,
    SWP_NOSIZE, SW_HIDE, SW_RESTORE, SW_SHOW, SW_SHOWNOACTIVATE, TPM_BOTTOMALIGN, TPM_LEFTALIGN,
    TPM_RIGHTBUTTON, ULW_ALPHA, WM_CLOSE, WM_COMMAND, WM_CREATE, WM_DESTROY, WM_HOTKEY,
    WM_LBUTTONDBLCLK, WM_LBUTTONUP, WM_PAINT, WM_RBUTTONUP, WM_SETFONT, WM_SETICON, WM_TIMER,
    WM_USER, WNDCLASSW, WS_BORDER, WS_CAPTION, WS_CHILD, WS_EX_LAYERED, WS_EX_NOACTIVATE,
    WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_EX_TRANSPARENT, WS_MINIMIZEBOX, WS_OVERLAPPED, WS_POPUP,
    WS_SYSMENU, WS_VISIBLE, WS_VSCROLL,
};

const DXGI_GUID: GUID = GUID::from_u128(0xca11c036_0102_4a2d_a6ad_f03cfed5d3c9);
const D3D9_GUID: GUID = GUID::from_u128(0x783aca0a_790e_4d7f_8451_aa850511c6b9);
const DXGKRNL_GUID: GUID = GUID::from_u128(0x802ec45a_1e99_4b83_9920_87c98277ba9d);

const SESSION_NAME: &str = "RustFpsOverlaySession";
const HOTKEY_TOGGLE_VISIBILITY: i32 = 1; // F9
const TIMER_OVERLAY_ID: usize = 1;

// System Tray constants
const WM_TRAYICON: u32 = WM_USER + 1;
const TRAY_ICON_ID: u32 = 1;
const ID_TRAY_OPEN: usize = 201;
const ID_TRAY_TOGGLE: usize = 202;
const ID_TRAY_QUIT: usize = 203;

// GUI Control IDs
const ID_CHK_ENABLED: usize = 101;
const ID_CHK_ALL_APPS: usize = 102;
const ID_EDIT_EXE: usize = 104;
const ID_BTN_ADD: usize = 105;
const ID_BTN_BROWSE: usize = 106;
const ID_BTN_ADD_ACTIVE: usize = 107;
const ID_LIST_GAMES: usize = 108;
const ID_BTN_REMOVE: usize = 109;
const ID_LBL_STATUS: usize = 110;

struct FrameBuffers {
    api_ts: VecDeque<i64>,
    dxg_ts: VecDeque<i64>,
    last_api_arrival: Option<Instant>,
    last_dxg_arrival: Option<Instant>,
}

impl FrameBuffers {
    fn new() -> Self {
        Self {
            api_ts: VecDeque::with_capacity(512),
            dxg_ts: VecDeque::with_capacity(512),
            last_api_arrival: None,
            last_dxg_arrival: None,
        }
    }

    fn clear(&mut self) {
        self.api_ts.clear();
        self.dxg_ts.clear();
        self.last_api_arrival = None;
        self.last_dxg_arrival = None;
    }
}

static TARGET_PIDS: RwLock<Option<HashSet<u32>>> = RwLock::new(None);
static FRAME_BUFFERS: OnceLock<Mutex<FrameBuffers>> = OnceLock::new();

static CURRENT_FPS: AtomicU32 = AtomicU32::new(0);
static ACTIVE_SOURCE: AtomicU32 = AtomicU32::new(0);
static ETW_ACTIVE: AtomicBool = AtomicBool::new(false);
static MANUAL_ENABLED: AtomicBool = AtomicBool::new(true);
static ALWAYS_ON_TOP_ALL: AtomicBool = AtomicBool::new(false);
static OVERLAY_ACTUALLY_SHOWN: AtomicBool = AtomicBool::new(false);

fn frame_buffers() -> &'static Mutex<FrameBuffers> {
    FRAME_BUFFERS.get_or_init(|| Mutex::new(FrameBuffers::new()))
}

fn to_wide(s: &str) -> Vec<u16> {
    OsStr::new(s).encode_wide().chain(std::iter::once(0)).collect()
}

fn config_file_path() -> PathBuf {
    if let Ok(appdata) = std::env::var("APPDATA") {
        let dir = PathBuf::from(appdata).join("RustFpsOverlay");
        let _ = fs::create_dir_all(&dir);
        return dir.join("games.txt");
    }
    let mut path = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("."));
    path.set_file_name("games.txt");
    path
}

fn load_game_list() -> Vec<String> {
    let primary = config_file_path();
    let content = fs::read_to_string(&primary)
        .or_else(|_| fs::read_to_string("C:\\fps-overlay\\target\\release\\games.txt"));

    if let Ok(text) = content {
        let games: Vec<String> = text
            .lines()
            .map(|l| l.trim().to_lowercase())
            .filter(|l| !l.is_empty())
            .collect();
        if !primary.exists() && !games.is_empty() {
            save_game_list(&games);
        }
        games
    } else {
        Vec::new()
    }
}

fn save_game_list(games: &[String]) {
    let joined = games.join("\r\n");
    let _ = fs::write(config_file_path(), joined);
}

fn get_process_exe_name(pid: u32) -> Option<String> {
    if pid == 0 {
        return None;
    }
    unsafe {
        if let Ok(handle) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) {
            let mut buf = [0u16; 260];
            let mut size = buf.len() as u32;
            let ok = QueryFullProcessImageNameW(
                handle,
                PROCESS_NAME_WIN32,
                PWSTR(buf.as_mut_ptr()),
                &mut size,
            )
            .is_ok();
            let _ = CloseHandle(handle);
            if ok && size > 0 {
                let full_path = String::from_utf16_lossy(&buf[..size as usize]);
                if let Some(name) = PathBuf::from(full_path).file_name() {
                    return Some(name.to_string_lossy().to_lowercase());
                }
            }
        }

        if let Ok(snap) = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) {
            let mut entry = PROCESSENTRY32W {
                dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
                ..Default::default()
            };
            if Process32FirstW(snap, &mut entry).is_ok() {
                loop {
                    if entry.th32ProcessID == pid {
                        let len = entry
                            .szExeFile
                            .iter()
                            .position(|&c| c == 0)
                            .unwrap_or(entry.szExeFile.len());
                        let name = String::from_utf16_lossy(&entry.szExeFile[..len]).to_lowercase();
                        let _ = CloseHandle(snap);
                        return Some(name);
                    }
                    if Process32NextW(snap, &mut entry).is_err() {
                        break;
                    }
                }
            }
            let _ = CloseHandle(snap);
        }
    }
    None
}

fn collect_matching_pids(fg_pids: &[u32], active_exe: &str, allowed_games: &HashSet<String>) -> HashSet<u32> {
    let mut pids = HashSet::new();
    let self_pid = std::process::id();

    for &p in fg_pids {
        if p != 0 && p != self_pid {
            pids.insert(p);
        }
    }

    unsafe {
        if let Ok(snap) = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) {
            let mut entry = PROCESSENTRY32W {
                dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
                ..Default::default()
            };
            if Process32FirstW(snap, &mut entry).is_ok() {
                loop {
                    let pid = entry.th32ProcessID;
                    if pid != 0 && pid != self_pid {
                        let len = entry
                            .szExeFile
                            .iter()
                            .position(|&c| c == 0)
                            .unwrap_or(entry.szExeFile.len());
                        let name = String::from_utf16_lossy(&entry.szExeFile[..len]).to_lowercase();
                        if (!active_exe.is_empty() && name == active_exe)
                            || allowed_games.contains(&name)
                        {
                            pids.insert(pid);
                        }
                    }
                    if Process32NextW(snap, &mut entry).is_err() {
                        break;
                    }
                }
            }
            let _ = CloseHandle(snap);
        }
    }
    pids
}

unsafe extern "system" fn enum_child_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
    let target_ptr = lparam.0 as *mut (u32, u32);
    let (host_pid, found_child_pid) = *target_ptr;
    if found_child_pid != 0 {
        return BOOL(0);
    }
    let mut child_pid = 0u32;
    GetWindowThreadProcessId(hwnd, Some(&mut child_pid));
    if child_pid != 0 && child_pid != host_pid {
        (*target_ptr).1 = child_pid;
        return BOOL(0);
    }
    BOOL(1)
}

fn get_foreground_info() -> (u32, u32, String) {
    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.0.is_null() {
            return (0, 0, String::new());
        }
        let mut main_pid = 0u32;
        GetWindowThreadProcessId(hwnd, Some(&mut main_pid));
        let main_name = get_process_exe_name(main_pid).unwrap_or_default();

        if main_name == "applicationframehost.exe" {
            let mut pair = (main_pid, 0u32);
            let _ = EnumChildWindows(
                hwnd,
                Some(enum_child_proc),
                LPARAM(&mut pair as *mut _ as isize),
            );
            if pair.1 != 0 {
                let child_name = get_process_exe_name(pair.1).unwrap_or(main_name);
                return (main_pid, pair.1, child_name);
            }
        }

        (main_pid, 0, main_name)
    }
}

unsafe extern "system" fn etw_event_callback(record: *mut EVENT_RECORD) {
    if record.is_null() {
        return;
    }
    let header = &(*record).EventHeader;
    let pid = header.ProcessId;
    if pid == 0 {
        return;
    }

    let is_target = if let Ok(guard) = TARGET_PIDS.try_read() {
        guard.as_ref().map(|s| s.contains(&pid)).unwrap_or(false)
    } else {
        false
    };
    if !is_target {
        return;
    }

    let id = header.EventDescriptor.Id;
    let ts = header.TimeStamp;

    let is_api_present = (header.ProviderId == DXGI_GUID && (id == 42 || id == 55))
        || (header.ProviderId == D3D9_GUID && id == 1);
    let is_dxgkrnl_present = header.ProviderId == DXGKRNL_GUID && id == 184;

    if !is_api_present && !is_dxgkrnl_present {
        return;
    }

    if let Ok(mut fb) = frame_buffers().lock() {
        let now = Instant::now();
        if is_api_present {
            fb.api_ts.push_back(ts);
            if fb.api_ts.len() > 2000 {
                fb.api_ts.pop_front();
            }
            fb.last_api_arrival = Some(now);
        } else {
            fb.dxg_ts.push_back(ts);
            if fb.dxg_ts.len() > 2000 {
                fb.dxg_ts.pop_front();
            }
            fb.last_dxg_arrival = Some(now);
        }
    }
}

fn spawn_etw_fps_listener() {
    thread::spawn(|| unsafe {
        let wide_name = to_wide(SESSION_NAME);
        let name_bytes = wide_name.len() * 2;
        let prop_size = std::mem::size_of::<EVENT_TRACE_PROPERTIES>() + name_bytes + 64;
        let mut buf = vec![0u8; prop_size];

        let props = buf.as_mut_ptr() as *mut EVENT_TRACE_PROPERTIES;
        (*props).Wnode.BufferSize = prop_size as u32;
        (*props).Wnode.Flags = WNODE_FLAG_TRACED_GUID;
        (*props).Wnode.ClientContext = 1;
        (*props).LogFileMode = EVENT_TRACE_REAL_TIME_MODE;
        (*props).FlushTimer = 1;
        (*props).LoggerNameOffset = std::mem::size_of::<EVENT_TRACE_PROPERTIES>() as u32;

        let _ = ControlTraceW(
            CONTROLTRACE_HANDLE::default(),
            PCWSTR(wide_name.as_ptr()),
            props,
            EVENT_TRACE_CONTROL_STOP,
        );

        buf.fill(0);
        let props = buf.as_mut_ptr() as *mut EVENT_TRACE_PROPERTIES;
        (*props).Wnode.BufferSize = prop_size as u32;
        (*props).Wnode.Flags = WNODE_FLAG_TRACED_GUID;
        (*props).Wnode.ClientContext = 1;
        (*props).LogFileMode = EVENT_TRACE_REAL_TIME_MODE;
        (*props).FlushTimer = 1;
        (*props).LoggerNameOffset = std::mem::size_of::<EVENT_TRACE_PROPERTIES>() as u32;

        let mut session_handle = CONTROLTRACE_HANDLE::default();
        if StartTraceW(&mut session_handle, PCWSTR(wide_name.as_ptr()), props).is_err() {
            ETW_ACTIVE.store(false, Ordering::Relaxed);
            return;
        }
        ETW_ACTIVE.store(true, Ordering::Relaxed);

        let _ = EnableTraceEx2(session_handle, &DXGI_GUID, 1, 5, u64::MAX, 0, 0, None);
        let _ = EnableTraceEx2(session_handle, &D3D9_GUID, 1, 5, u64::MAX, 0, 0, None);
        let _ = EnableTraceEx2(session_handle, &DXGKRNL_GUID, 1, 5, 0x51, 0, 0, None);

        let mut log_file = EVENT_TRACE_LOGFILEW::default();
        let mut mutable_name = wide_name.clone();
        log_file.LoggerName = PWSTR(mutable_name.as_mut_ptr());
        log_file.Anonymous1.ProcessTraceMode =
            PROCESS_TRACE_MODE_REAL_TIME | PROCESS_TRACE_MODE_EVENT_RECORD;
        log_file.Anonymous2.EventRecordCallback = Some(etw_event_callback);

        let trace_handle = OpenTraceW(&mut log_file);
        if trace_handle.Value != u64::MAX {
            let handles = [trace_handle];
            let _ = ProcessTrace(&handles, None, None);
            let _ = CloseTrace(trace_handle);
        }
    });
}

fn compute_window_fps(queue: &mut VecDeque<i64>, last_arrival: Option<Instant>) -> Option<u32> {
    let arrived = last_arrival?;
    if arrived.elapsed() > Duration::from_millis(2500) {
        queue.clear();
        return None;
    }

    let latest = *queue.back()?;
    let cutoff = latest - 10_000_000;
    while let Some(&front) = queue.front() {
        if front < cutoff {
            queue.pop_front();
        } else {
            break;
        }
    }

    let n = queue.len();
    if n < 2 {
        return None;
    }
    let first = *queue.front()?;
    let span_secs = (latest - first) as f64 / 10_000_000.0;
    if span_secs < 0.05 {
        return None;
    }
    let fps = ((n - 1) as f64 / span_secs).round() as u32;
    Some(fps.min(999))
}

fn spawn_fps_worker(
    active_exe_shared: Arc<Mutex<String>>,
    allowed_games: Arc<Mutex<HashSet<String>>>,
) {
    thread::spawn(move || {
        let mut prev_primary_pid = 0u32;
        let mut last_non_self_exe = String::new();

        loop {
            thread::sleep(Duration::from_millis(250));

            let (fg_pid, child_pid, fg_exe) = get_foreground_info();
            let self_pid = std::process::id();

            if fg_pid != 0 && fg_pid != self_pid && !fg_exe.is_empty() {
                last_non_self_exe = fg_exe.clone();
                if let Ok(mut guard) = active_exe_shared.lock() {
                    *guard = fg_exe.clone();
                }

                if fg_pid != prev_primary_pid {
                    prev_primary_pid = fg_pid;
                    if let Ok(mut fb) = frame_buffers().lock() {
                        fb.clear();
                    }
                    CURRENT_FPS.store(0, Ordering::Relaxed);
                    ACTIVE_SOURCE.store(0, Ordering::Relaxed);
                }
            }

            let allowed_snapshot = allowed_games
                .lock()
                .map(|g| g.clone())
                .unwrap_or_default();

            let check_exe = if fg_pid == self_pid {
                last_non_self_exe.as_str()
            } else {
                fg_exe.as_str()
            };

            let matched_pids = collect_matching_pids(&[fg_pid, child_pid], check_exe, &allowed_snapshot);
            if let Ok(mut guard) = TARGET_PIDS.write() {
                *guard = Some(matched_pids);
            }

            if let Ok(mut fb) = frame_buffers().lock() {
                let last_api = fb.last_api_arrival;
                let last_dxg = fb.last_dxg_arrival;

                if let Some(fps) = compute_window_fps(&mut fb.api_ts, last_api) {
                    CURRENT_FPS.store(fps, Ordering::Relaxed);
                    ACTIVE_SOURCE.store(1, Ordering::Relaxed);
                } else if let Some(fps) = compute_window_fps(&mut fb.dxg_ts, last_dxg) {
                    CURRENT_FPS.store(fps, Ordering::Relaxed);
                    ACTIVE_SOURCE.store(2, Ordering::Relaxed);
                } else {
                    CURRENT_FPS.store(0, Ordering::Relaxed);
                    ACTIVE_SOURCE.store(0, Ordering::Relaxed);
                }
            }

            let is_allowed = if ALWAYS_ON_TOP_ALL.load(Ordering::Relaxed) {
                true
            } else {
                !check_exe.is_empty() && allowed_snapshot.contains(check_exe)
            };

            let should_show = MANUAL_ENABLED.load(Ordering::Relaxed) && is_allowed;
            OVERLAY_ACTUALLY_SHOWN.store(should_show, Ordering::Relaxed);
        }
    });
}

unsafe fn render_overlay(hwnd: HWND) {
    let fps = CURRENT_FPS.load(Ordering::Relaxed);
    let text = format!("{} FPS", fps);

    let width = 120i32;
    let height = 22i32;

    let screen_w = GetSystemMetrics(SM_CXSCREEN);
    let x_pos = (screen_w - width - 6).max(0);
    let y_pos = 4i32;

    let screen_dc = GetDC(HWND::default());
    let mem_dc = CreateCompatibleDC(screen_dc);

    let bmi = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: width,
            biHeight: -height,
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            ..Default::default()
        },
        ..Default::default()
    };

    let mut bits_ptr: *mut std::ffi::c_void = std::ptr::null_mut();
    let dib = CreateDIBSection(mem_dc, &bmi, DIB_RGB_COLORS, &mut bits_ptr, None, 0)
        .expect("Failed to create DIB");
    let old_bmp = SelectObject(mem_dc, dib);

    if !bits_ptr.is_null() {
        let pixel_count = (width * height) as usize;
        std::ptr::write_bytes(bits_ptr as *mut u32, 0, pixel_count);
    }

    let font_name = to_wide("Consolas");
    let font = CreateFontW(
        14,
        0,
        0,
        0,
        FW_BOLD.0 as i32,
        0,
        0,
        0,
        DEFAULT_CHARSET.0 as u32,
        OUT_DEFAULT_PRECIS.0 as u32,
        CLIP_DEFAULT_PRECIS.0 as u32,
        CLEARTYPE_QUALITY.0 as u32,
        (DEFAULT_PITCH.0 | FF_MODERN.0) as u32,
        PCWSTR(font_name.as_ptr()),
    );
    let old_font = SelectObject(mem_dc, font);
    let _ = SetBkMode(mem_dc, TRANSPARENT);
    let _ = SetTextAlign(mem_dc, TA_RIGHT);

    let right_x = width - 4;
    let wide_line: Vec<u16> = OsStr::new(&text).encode_wide().collect();

    SetTextColor(mem_dc, COLORREF(0x00080808));
    let _ = TextOutW(mem_dc, right_x + 1, 3, &wide_line);

    SetTextColor(mem_dc, COLORREF(0x00FFFFFF));
    let _ = TextOutW(mem_dc, right_x, 2, &wide_line);

    if !bits_ptr.is_null() {
        let pixel_count = (width * height) as usize;
        let pixels = std::slice::from_raw_parts_mut(bits_ptr as *mut u32, pixel_count);
        for px in pixels.iter_mut() {
            let rgb = *px & 0x00FFFFFF;
            if rgb != 0 {
                *px = 0xFF000000 | rgb;
            }
        }
    }

    let pt_dst = POINT { x: x_pos, y: y_pos };
    let size = SIZE {
        cx: width,
        cy: height,
    };
    let pt_src = POINT { x: 0, y: 0 };
    let blend = windows::Win32::Graphics::Gdi::BLENDFUNCTION {
        BlendOp: 0,
        BlendFlags: 0,
        SourceConstantAlpha: 255,
        AlphaFormat: 1,
    };

    let _ = UpdateLayeredWindow(
        hwnd,
        screen_dc,
        Some(&pt_dst),
        Some(&size),
        mem_dc,
        Some(&pt_src),
        COLORREF(0),
        Some(&blend),
        ULW_ALPHA,
    );

    SelectObject(mem_dc, old_font);
    let _ = DeleteObject(font);
    SelectObject(mem_dc, old_bmp);
    let _ = DeleteObject(dib);
    let _ = DeleteDC(mem_dc);
    ReleaseDC(HWND::default(), screen_dc);
}

unsafe fn add_tray_icon(hwnd: HWND, icon: HICON) {
    let mut nid = NOTIFYICONDATAW {
        cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
        hWnd: hwnd,
        uID: TRAY_ICON_ID,
        uFlags: NIF_ICON | NIF_MESSAGE | NIF_TIP,
        uCallbackMessage: WM_TRAYICON,
        hIcon: icon,
        ..Default::default()
    };
    let tip = to_wide("Rust FPS Overlay");
    let len = tip.len().min(nid.szTip.len());
    nid.szTip[..len].copy_from_slice(&tip[..len]);
    let _ = Shell_NotifyIconW(NIM_ADD, &nid);
}

unsafe fn remove_tray_icon(hwnd: HWND) {
    let nid = NOTIFYICONDATAW {
        cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
        hWnd: hwnd,
        uID: TRAY_ICON_ID,
        ..Default::default()
    };
    let _ = Shell_NotifyIconW(NIM_DELETE, &nid);
}

struct AppState {
    active_exe: Arc<Mutex<String>>,
    allowed_games: Arc<Mutex<HashSet<String>>>,
    overlay_hwnd: HWND,
    main_hwnd: HWND,
    chk_enabled: HWND,
    chk_all_apps: HWND,
    edit_exe: HWND,
    list_games: HWND,
    lbl_status: HWND,
}

static mut APP_STATE: Option<AppState> = None;

unsafe fn sync_listbox_from_set(state: &AppState) {
    SendMessageW(state.list_games, LB_RESETCONTENT, WPARAM(0), LPARAM(0));
    if let Ok(set) = state.allowed_games.lock() {
        let mut sorted: Vec<String> = set.iter().cloned().collect();
        sorted.sort();
        for item in sorted {
            let wide = to_wide(&item);
            SendMessageW(
                state.list_games,
                LB_ADDSTRING,
                WPARAM(0),
                LPARAM(wide.as_ptr() as isize),
            );
        }
    }
}

unsafe fn add_game_to_list(state: &AppState, raw_name: &str) {
    let cleaned = raw_name.trim().to_lowercase();
    if cleaned.is_empty() {
        return;
    }
    let exe_name = if cleaned.ends_with(".exe") {
        cleaned
    } else {
        format!("{}.exe", cleaned)
    };

    if let Ok(mut set) = state.allowed_games.lock() {
        set.insert(exe_name);
        let vec_list: Vec<String> = set.iter().cloned().collect();
        save_game_list(&vec_list);
    }
    sync_listbox_from_set(state);
}

unsafe fn browse_for_exe(owner: HWND) -> Option<String> {
    let mut file_buf = [0u16; 260];
    let filter = to_wide("Executable Files (*.exe)\0*.exe\0All Files (*.*)\0*.*\0\0");
    let title = to_wide("Select Game or App Executable");

    let mut ofn = OPENFILENAMEW {
        lStructSize: std::mem::size_of::<OPENFILENAMEW>() as u32,
        hwndOwner: owner,
        lpstrFilter: PCWSTR(filter.as_ptr()),
        lpstrFile: PWSTR(file_buf.as_mut_ptr()),
        nMaxFile: file_buf.len() as u32,
        lpstrTitle: PCWSTR(title.as_ptr()),
        Flags: OFN_FILEMUSTEXIST | OFN_PATHMUSTEXIST | OFN_NOCHANGEDIR,
        ..Default::default()
    };

    if GetOpenFileNameW(&mut ofn).as_bool() {
        let len = file_buf.iter().position(|&c| c == 0).unwrap_or(file_buf.len());
        let path_str = std::ffi::OsString::from_wide(&file_buf[..len]);
        PathBuf::from(path_str)
            .file_name()
            .map(|n| n.to_string_lossy().to_lowercase())
    } else {
        None
    }
}

unsafe extern "system" fn overlay_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_TIMER => {
            if let Some(ref state) = APP_STATE {
                let show = OVERLAY_ACTUALLY_SHOWN.load(Ordering::Relaxed);
                let active_name = state
                    .active_exe
                    .lock()
                    .map(|g| g.clone())
                    .unwrap_or_default();
                let etw_ok = ETW_ACTIVE.load(Ordering::Relaxed);
                let src_tag = match ACTIVE_SOURCE.load(Ordering::Relaxed) {
                    1 => "DXGI/D3D9",
                    2 => "DxgKrnl",
                    _ => "Idle",
                };

                if show {
                    let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
                    render_overlay(hwnd);
                    let _ = SetWindowPos(
                        hwnd,
                        HWND_TOPMOST,
                        0,
                        0,
                        0,
                        0,
                        SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
                    );
                    let status_text = if etw_ok {
                        format!(
                            "Active: {} | {} FPS ({})",
                            if active_name.is_empty() { "Desktop" } else { &active_name },
                            CURRENT_FPS.load(Ordering::Relaxed),
                            src_tag
                        )
                    } else {
                        "ETW Inactive — Run as Administrator for FPS".to_string()
                    };
                    let _ = SetWindowTextW(state.lbl_status, PCWSTR(to_wide(&status_text).as_ptr()));
                } else {
                    let _ = ShowWindow(hwnd, SW_HIDE);
                    let status_text = if etw_ok {
                        format!(
                            "Last App: {} (Not in list)",
                            if active_name.is_empty() { "None" } else { &active_name }
                        )
                    } else {
                        "ETW Inactive — Run as Administrator for FPS".to_string()
                    };
                    let _ = SetWindowTextW(state.lbl_status, PCWSTR(to_wide(&status_text).as_ptr()));
                }
            }
            LRESULT(0)
        }
        WM_HOTKEY => {
            if let Some(ref state) = APP_STATE {
                if wparam.0 as i32 == HOTKEY_TOGGLE_VISIBILITY {
                    let next = !MANUAL_ENABLED.load(Ordering::Relaxed);
                    MANUAL_ENABLED.store(next, Ordering::Relaxed);
                    SendMessageW(
                        state.chk_enabled,
                        BM_SETCHECK,
                        WPARAM(if next { 1 } else { 0 }),
                        LPARAM(0),
                    );
                }
            }
            LRESULT(0)
        }
        WM_PAINT => {
            let mut ps = PAINTSTRUCT::default();
            let _ = BeginPaint(hwnd, &mut ps);
            let _ = EndPaint(hwnd, &mut ps);
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

unsafe extern "system" fn main_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_CREATE => {
            let font = GetStockObject(DEFAULT_GUI_FONT);
            let instance = GetModuleHandleW(None).unwrap();

            let make_ctrl =
                |class: &str, text: &str, style: u32, x: i32, y: i32, w: i32, h: i32, id: usize| -> HWND {
                    let h_ctrl = CreateWindowExW(
                        Default::default(),
                        PCWSTR(to_wide(class).as_ptr()),
                        PCWSTR(to_wide(text).as_ptr()),
                        windows::Win32::UI::WindowsAndMessaging::WINDOW_STYLE(
                            WS_CHILD.0 | WS_VISIBLE.0 | style,
                        ),
                        x,
                        y,
                        w,
                        h,
                        hwnd,
                        windows::Win32::UI::WindowsAndMessaging::HMENU(id as *mut _),
                        instance,
                        None,
                    )
                    .unwrap();
                    SendMessageW(h_ctrl, WM_SETFONT, WPARAM(font.0 as usize), LPARAM(1));
                    h_ctrl
                };

            let chk_enabled = make_ctrl(
                "BUTTON",
                "Enable Overlay (F9)",
                BS_AUTOCHECKBOX as u32,
                16,
                14,
                160,
                22,
                ID_CHK_ENABLED,
            );
            SendMessageW(chk_enabled, BM_SETCHECK, WPARAM(1), LPARAM(0));

            let chk_all_apps = make_ctrl(
                "BUTTON",
                "Always On Top (All Apps)",
                BS_AUTOCHECKBOX as u32,
                190,
                14,
                190,
                22,
                ID_CHK_ALL_APPS,
            );

            let _ = make_ctrl(
                "STATIC",
                "Show FPS only when these games/apps are focused:",
                0,
                16,
                46,
                365,
                18,
                0,
            );

            let edit_exe = CreateWindowExW(
                windows::Win32::UI::WindowsAndMessaging::WINDOW_EX_STYLE(0x00000200),
                PCWSTR(to_wide("EDIT").as_ptr()),
                PCWSTR(to_wide("").as_ptr()),
                windows::Win32::UI::WindowsAndMessaging::WINDOW_STYLE(
                    WS_CHILD.0 | WS_VISIBLE.0 | ES_AUTOHSCROLL as u32,
                ),
                16,
                68,
                200,
                24,
                hwnd,
                windows::Win32::UI::WindowsAndMessaging::HMENU(ID_EDIT_EXE as *mut _),
                instance,
                None,
            )
            .unwrap();
            SendMessageW(edit_exe, WM_SETFONT, WPARAM(font.0 as usize), LPARAM(1));

            let _ = make_ctrl("BUTTON", "Add .exe", BS_PUSHBUTTON as u32, 224, 68, 72, 24, ID_BTN_ADD);
            let _ = make_ctrl("BUTTON", "Browse...", BS_PUSHBUTTON as u32, 304, 68, 76, 24, ID_BTN_BROWSE);

            let list_games = CreateWindowExW(
                windows::Win32::UI::WindowsAndMessaging::WINDOW_EX_STYLE(0x00000200),
                PCWSTR(to_wide("LISTBOX").as_ptr()),
                PCWSTR(to_wide("").as_ptr()),
                windows::Win32::UI::WindowsAndMessaging::WINDOW_STYLE(
                    WS_CHILD.0 | WS_VISIBLE.0 | WS_VSCROLL.0 | WS_BORDER.0 | (LBN_SELCHANGE as u32),
                ),
                16,
                100,
                364,
                155,
                hwnd,
                windows::Win32::UI::WindowsAndMessaging::HMENU(ID_LIST_GAMES as *mut _),
                instance,
                None,
            )
            .unwrap();
            SendMessageW(list_games, WM_SETFONT, WPARAM(font.0 as usize), LPARAM(1));

            let _ = make_ctrl(
                "BUTTON",
                "Remove Selected",
                BS_PUSHBUTTON as u32,
                16,
                264,
                130,
                26,
                ID_BTN_REMOVE,
            );
            let _ = make_ctrl(
                "BUTTON",
                "Add Last Focused App",
                BS_PUSHBUTTON as u32,
                220,
                264,
                160,
                26,
                ID_BTN_ADD_ACTIVE,
            );

            let lbl_status = make_ctrl(
                "STATIC",
                "Starting ETW...",
                0,
                16,
                300,
                364,
                20,
                ID_LBL_STATUS,
            );

            if let Some(ref mut state) = APP_STATE {
                state.main_hwnd = hwnd;
                state.chk_enabled = chk_enabled;
                state.chk_all_apps = chk_all_apps;
                state.edit_exe = edit_exe;
                state.list_games = list_games;
                state.lbl_status = lbl_status;
                sync_listbox_from_set(state);
            }
            LRESULT(0)
        }
        WM_TRAYICON => {
            let event = (lparam.0 & 0xFFFF) as u32;
            if event == WM_LBUTTONUP || event == WM_LBUTTONDBLCLK {
                let _ = ShowWindow(hwnd, SW_RESTORE);
                let _ = SetForegroundWindow(hwnd);
            } else if event == WM_RBUTTONUP {
                let mut pt = POINT::default();
                let _ = GetCursorPos(&mut pt);
                if let Ok(menu) = CreatePopupMenu() {
                    let _ = AppendMenuW(
                        menu,
                        MF_STRING,
                        ID_TRAY_OPEN,
                        PCWSTR(to_wide("Open Settings").as_ptr()),
                    );
                    let _ = AppendMenuW(
                        menu,
                        MF_STRING,
                        ID_TRAY_TOGGLE,
                        PCWSTR(to_wide("Toggle Overlay (F9)").as_ptr()),
                    );
                    let _ = AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null());
                    let _ = AppendMenuW(
                        menu,
                        MF_STRING,
                        ID_TRAY_QUIT,
                        PCWSTR(to_wide("Quit").as_ptr()),
                    );
                    let _ = SetForegroundWindow(hwnd);
                    let _ = TrackPopupMenu(
                        menu,
                        TPM_RIGHTBUTTON | TPM_BOTTOMALIGN | TPM_LEFTALIGN,
                        pt.x,
                        pt.y,
                        0,
                        hwnd,
                        None,
                    );
                    let _ = DestroyMenu(menu);
                }
            }
            LRESULT(0)
        }
        WM_COMMAND => {
            let id = (wparam.0 & 0xFFFF) as usize;
            if let Some(ref state) = APP_STATE {
                match id {
                    ID_TRAY_OPEN => {
                        let _ = ShowWindow(hwnd, SW_RESTORE);
                        let _ = SetForegroundWindow(hwnd);
                    }
                    ID_TRAY_TOGGLE => {
                        let next = !MANUAL_ENABLED.load(Ordering::Relaxed);
                        MANUAL_ENABLED.store(next, Ordering::Relaxed);
                        SendMessageW(
                            state.chk_enabled,
                            BM_SETCHECK,
                            WPARAM(if next { 1 } else { 0 }),
                            LPARAM(0),
                        );
                    }
                    ID_TRAY_QUIT => {
                        remove_tray_icon(hwnd);
                        let _ = UnregisterHotKey(state.overlay_hwnd, HOTKEY_TOGGLE_VISIBILITY);
                        PostQuitMessage(0);
                    }
                    ID_CHK_ENABLED => {
                        let checked =
                            SendMessageW(state.chk_enabled, BM_GETCHECK, WPARAM(0), LPARAM(0)).0 == 1;
                        MANUAL_ENABLED.store(checked, Ordering::Relaxed);
                    }
                    ID_CHK_ALL_APPS => {
                        let checked =
                            SendMessageW(state.chk_all_apps, BM_GETCHECK, WPARAM(0), LPARAM(0)).0 == 1;
                        ALWAYS_ON_TOP_ALL.store(checked, Ordering::Relaxed);
                    }
                    ID_BTN_ADD => {
                        let len = GetWindowTextLengthW(state.edit_exe);
                        if len > 0 {
                            let mut buf = vec![0u16; (len + 1) as usize];
                            GetWindowTextW(state.edit_exe, &mut buf);
                            let txt = String::from_utf16_lossy(&buf[..len as usize]);
                            add_game_to_list(state, &txt);
                            let _ = SetWindowTextW(state.edit_exe, PCWSTR(to_wide("").as_ptr()));
                        }
                    }
                    ID_BTN_BROWSE => {
                        if let Some(exe) = browse_for_exe(hwnd) {
                            add_game_to_list(state, &exe);
                        }
                    }
                    ID_BTN_ADD_ACTIVE => {
                        let last_exe = state
                            .active_exe
                            .lock()
                            .map(|g| g.clone())
                            .unwrap_or_default();
                        if !last_exe.is_empty() {
                            add_game_to_list(state, &last_exe);
                        }
                    }
                    ID_BTN_REMOVE => {
                        let sel =
                            SendMessageW(state.list_games, LB_GETCURSEL, WPARAM(0), LPARAM(0)).0;
                        if sel >= 0 {
                            let len = SendMessageW(
                                state.list_games,
                                LB_GETTEXTLEN,
                                WPARAM(sel as usize),
                                LPARAM(0),
                            )
                            .0;
                            if len > 0 {
                                let mut buf = vec![0u16; (len + 1) as usize];
                                SendMessageW(
                                    state.list_games,
                                    LB_GETTEXT,
                                    WPARAM(sel as usize),
                                    LPARAM(buf.as_mut_ptr() as isize),
                                );
                                let txt = String::from_utf16_lossy(&buf[..len as usize]);
                                if let Ok(mut set) = state.allowed_games.lock() {
                                    set.remove(&txt);
                                    let vec_list: Vec<String> = set.iter().cloned().collect();
                                    save_game_list(&vec_list);
                                }
                                sync_listbox_from_set(state);
                            }
                        }
                    }
                    _ => {}
                }
            }
            LRESULT(0)
        }
        // Pressing the 'X' button hides to the system tray instead of exiting
        WM_CLOSE => {
            let _ = ShowWindow(hwnd, SW_HIDE);
            LRESULT(0)
        }
        WM_DESTROY => {
            remove_tray_icon(hwnd);
            if let Some(ref state) = APP_STATE {
                let _ = UnregisterHotKey(state.overlay_hwnd, HOTKEY_TOGGLE_VISIBILITY);
            }
            PostQuitMessage(0);
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

fn main() {
    let initial_games: HashSet<String> = load_game_list().into_iter().collect();
    let allowed_games = Arc::new(Mutex::new(initial_games));
    let active_exe = Arc::new(Mutex::new(String::new()));

    spawn_etw_fps_listener();
    spawn_fps_worker(active_exe.clone(), allowed_games.clone());

    unsafe {
        let instance: HINSTANCE = GetModuleHandleW(None).unwrap().into();
        let app_icon = LoadIconW(instance, PCWSTR(1 as *const u16)).unwrap_or_default();

        // 1. Transparent click-through HUD overlay
        let overlay_class = to_wide("RustFpsOverlayHudClass");
        let wc_overlay = WNDCLASSW {
            lpfnWndProc: Some(overlay_wnd_proc),
            hInstance: instance,
            lpszClassName: PCWSTR(overlay_class.as_ptr()),
            ..Default::default()
        };
        RegisterClassW(&wc_overlay);

        let overlay_hwnd = CreateWindowExW(
            WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
            PCWSTR(overlay_class.as_ptr()),
            PCWSTR(to_wide("Rust FPS HUD").as_ptr()),
            WS_POPUP,
            0,
            0,
            120,
            22,
            None,
            None,
            instance,
            None,
        )
        .expect("Failed to create overlay HUD");

        // 2. GUI control window with embedded Rust icon
        let main_class = to_wide("RustFpsOverlayControlClass");
        let wc_main = WNDCLASSW {
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(main_wnd_proc),
            hInstance: instance,
            hIcon: app_icon,
            hbrBackground: windows::Win32::Graphics::Gdi::HBRUSH(
                (windows::Win32::Graphics::Gdi::COLOR_WINDOW.0 + 1) as *mut _,
            ),
            lpszClassName: PCWSTR(main_class.as_ptr()),
            ..Default::default()
        };
        RegisterClassW(&wc_main);

        APP_STATE = Some(AppState {
            active_exe,
            allowed_games,
            overlay_hwnd,
            main_hwnd: HWND::default(),
            chk_enabled: HWND::default(),
            chk_all_apps: HWND::default(),
            edit_exe: HWND::default(),
            list_games: HWND::default(),
            lbl_status: HWND::default(),
        });

        let main_hwnd = CreateWindowExW(
            Default::default(),
            PCWSTR(main_class.as_ptr()),
            PCWSTR(to_wide("Rust FPS Overlay").as_ptr()),
            WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_MINIMIZEBOX | WS_VISIBLE,
            200,
            200,
            412,
            365,
            None,
            None,
            instance,
            None,
        )
        .expect("Failed to create control window");

        SendMessageW(main_hwnd, WM_SETICON, WPARAM(ICON_BIG as usize), LPARAM(app_icon.0 as isize));
        SendMessageW(main_hwnd, WM_SETICON, WPARAM(ICON_SMALL as usize), LPARAM(app_icon.0 as isize));
        add_tray_icon(main_hwnd, app_icon);

        let _ = ShowWindow(main_hwnd, SW_SHOW);

        let _ = RegisterHotKey(overlay_hwnd, HOTKEY_TOGGLE_VISIBILITY, HOT_KEY_MODIFIERS(0), 0x78); // F9
        let _ = SetTimer(overlay_hwnd, TIMER_OVERLAY_ID, 250, None);

        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).into() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}