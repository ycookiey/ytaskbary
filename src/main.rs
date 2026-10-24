//! Nothing runs on a timer. The process sleeps in its message loop and wakes
//! only for window events:
//! - explorer shows a taskbar: hide it again (EVENT_OBJECT_SHOW, for explorer's
//!   process only);
//! - the foreground window changes: show the taskbars while the Start menu or
//!   its search is in front, hide them when another window comes to the front
//!   (EVENT_SYSTEM_FOREGROUND);
//! - explorer restarts (TaskbarCreated) or the monitors change: look for the
//!   taskbars again.

#![windows_subsystem = "windows"]

mod autostart;
mod classify;
mod game;
mod log;
mod taskbar;
mod win;

use std::cell::RefCell;
use std::mem;

use classify::{classify, is_taskbar_class, Foreground, WinTap};
use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{
    CloseHandle, GetLastError, ERROR_ALREADY_EXISTS, HANDLE, HINSTANCE, HWND, LPARAM, LRESULT,
    POINT, WAIT_OBJECT_0, WPARAM,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Threading::{CreateEventW, CreateMutexW, INFINITE};
use windows::Win32::UI::Accessibility::{SetWinEventHook, UnhookWinEvent, HWINEVENTHOOK};
use windows::Win32::UI::Input::KeyboardAndMouse::{VK_LWIN, VK_RWIN};
use windows::Win32::UI::Shell::{
    Shell_NotifyIconW, NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NOTIFYICONDATAW,
};
use windows::Win32::UI::WindowsAndMessaging::*;

const WM_APP_TRAY: u32 = WM_APP + 1;
const WM_APP_TAP: u32 = WM_APP + 2;
const ID_ENABLED: usize = 1;
const ID_AUTOSTART: usize = 2;
const ID_EXIT: usize = 3;
const OBJID_WINDOW: i32 = 0;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    /// Show the taskbars while the Start menu is the foreground window.
    StartMenu,
    /// Show the taskbars on a tap of the Win key.
    KeyboardHook,
}

struct App {
    hwnd: HWND,
    mode: Mode,
    enabled: bool,
    /// The taskbars are shown on purpose (Start menu open, or a Win tap).
    revealed: bool,
    taskbar_created: u32,
    show_hook: HWINEVENTHOOK,
    foreground_hook: HWINEVENTHOOK,
    keyboard_hook: HHOOK,
    tap: WinTap,
    games: Option<game::GameList>,
    rehides: u64,
}

thread_local! {
    static APP: RefCell<Option<App>> = const { RefCell::new(None) };
}

fn with_app<R>(f: impl FnOnce(&mut App) -> R) -> Option<R> {
    APP.with(|a| a.try_borrow_mut().ok().and_then(|mut a| a.as_mut().map(f)))
}

fn main() {
    let mode = if std::env::args().any(|a| a == "--keyboard-hook") { Mode::KeyboardHook } else { Mode::StartMenu };
    log::init();

    // One instance only: two would hide and show the taskbars against each other.
    let mutex = unsafe { CreateMutexW(None, true, w!("Local\\ytaskbary")) };
    if mutex.is_err() || unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
        log::write("already running; exiting");
        return;
    }

    let Some(hwnd) = create_window() else {
        log::write("could not create the window");
        return;
    };
    let taskbar_created = unsafe { RegisterWindowMessageW(w!("TaskbarCreated")) };
    let games = (mode == Mode::KeyboardHook).then(game::GameList::load);
    if let Some(g) = &games {
        log::write(&format!("game list: {} executables", g.len()));
    }
    APP.with(|a| {
        *a.borrow_mut() = Some(App {
            hwnd,
            mode,
            enabled: true,
            revealed: false,
            taskbar_created,
            show_hook: HWINEVENTHOOK::default(),
            foreground_hook: HWINEVENTHOOK::default(),
            keyboard_hook: HHOOK::default(),
            tap: WinTap::default(),
            games,
            rehides: 0,
        })
    });
    log::write(match mode {
        Mode::StartMenu => "start (shows the taskbars while the Start menu is in front)",
        Mode::KeyboardHook => "start (shows the taskbars on a tap of the Win key)",
    });

    add_tray_icon(hwnd);
    attach_hooks();
    conceal();
    run_loop();

    detach_hooks();
    taskbar::show_all();
    remove_tray_icon(hwnd);
    log::write("exit");
    if let Ok(m) = mutex {
        unsafe {
            let _ = CloseHandle(m);
        }
    }
}

/// The message loop. In keyboard-hook mode it also waits on a change of the
/// game list, so a game added while running is picked up.
fn run_loop() {
    let event = with_app(|a| a.games.is_some()).unwrap_or(false).then(|| unsafe {
        CreateEventW(None, false, false, None).unwrap_or_default()
    });
    let watching = event.map(|e| with_app(|a| a.games.as_ref().is_some_and(|g| g.watch(e))).unwrap_or(false)).unwrap_or(false);
    let handles: Vec<HANDLE> = if watching { event.into_iter().collect() } else { Vec::new() };
    let mut msg = MSG::default();
    loop {
        let r = unsafe { MsgWaitForMultipleObjects(Some(&handles), false, INFINITE, QS_ALLINPUT) };
        if !handles.is_empty() && r == WAIT_OBJECT_0 {
            with_app(|a| {
                if let Some(g) = a.games.as_mut() {
                    g.reload();
                    log::write(&format!("game list reloaded: {} executables", g.len()));
                    g.watch(handles[0]);
                }
            });
            continue;
        }
        while unsafe { PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE) }.as_bool() {
            if msg.message == WM_QUIT {
                if let Some(e) = event {
                    unsafe {
                        let _ = CloseHandle(e);
                    }
                }
                return;
            }
            unsafe {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
    }
}

fn create_window() -> Option<HWND> {
    unsafe {
        let instance: HINSTANCE = GetModuleHandleW(None).ok()?.into();
        let class = w!("ytaskbary");
        let wc = WNDCLASSW { lpfnWndProc: Some(window_proc), hInstance: instance, lpszClassName: class, ..Default::default() };
        if RegisterClassW(&wc) == 0 {
            return None;
        }
        // A hidden top-level window (not message-only): TaskbarCreated is broadcast
        // to top-level windows only.
        CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            class,
            w!("ytaskbary"),
            WS_OVERLAPPED,
            0,
            0,
            0,
            0,
            None,
            None,
            instance,
            None,
        )
        .ok()
    }
}

/// Starts receiving the window events. The show event is limited to explorer's
/// process; it is registered again when explorer restarts.
fn attach_hooks() {
    let explorer = taskbar::all().first().map(|&h| win::process_id(h)).unwrap_or(0);
    with_app(|a| unsafe {
        if !a.show_hook.is_invalid() {
            let _ = UnhookWinEvent(a.show_hook);
            a.show_hook = HWINEVENTHOOK::default();
        }
        if explorer != 0 {
            a.show_hook = SetWinEventHook(EVENT_OBJECT_SHOW, EVENT_OBJECT_SHOW, None, Some(on_win_event), explorer, 0, WINEVENT_OUTOFCONTEXT);
        }
        if a.foreground_hook.is_invalid() {
            a.foreground_hook = SetWinEventHook(
                EVENT_SYSTEM_FOREGROUND,
                EVENT_SYSTEM_FOREGROUND,
                None,
                Some(on_win_event),
                0,
                0,
                WINEVENT_OUTOFCONTEXT,
            );
        }
        log::write(&format!(
            "hooks: show={} (explorer pid {explorer}) foreground={}",
            !a.show_hook.is_invalid(),
            !a.foreground_hook.is_invalid()
        ));
    });
    update_keyboard_hook();
}

fn detach_hooks() {
    with_app(|a| unsafe {
        for h in [&mut a.show_hook, &mut a.foreground_hook] {
            if !h.is_invalid() {
                let _ = UnhookWinEvent(*h);
                *h = HWINEVENTHOOK::default();
            }
        }
        if !a.keyboard_hook.is_invalid() {
            let _ = UnhookWindowsHookEx(a.keyboard_hook);
            a.keyboard_hook = HHOOK::default();
        }
    });
}

/// In keyboard-hook mode, keeps the hook only while it is needed: enabled, and
/// no game in the foreground. While a low-level hook is set, Windows waits for
/// it on every key press.
fn update_keyboard_hook() {
    let foreground_exe = win::process_path(unsafe { GetForegroundWindow() });
    with_app(|a| {
        if a.mode != Mode::KeyboardHook {
            return;
        }
        let in_game = foreground_exe.as_deref().is_some_and(|p| a.games.as_ref().is_some_and(|g| g.contains(p)));
        let want = a.enabled && !in_game;
        let have = !a.keyboard_hook.is_invalid();
        unsafe {
            if want && !have {
                let instance: HINSTANCE = GetModuleHandleW(None).map(Into::into).unwrap_or_default();
                if let Ok(h) = SetWindowsHookExW(WH_KEYBOARD_LL, Some(on_keyboard), instance, 0) {
                    a.keyboard_hook = h;
                }
                a.tap = WinTap::default();
                log::write(&format!("keyboard hook on: {}", !a.keyboard_hook.is_invalid()));
            } else if !want && have {
                let _ = UnhookWindowsHookEx(a.keyboard_hook);
                a.keyboard_hook = HHOOK::default();
                log::write(if in_game { "keyboard hook off (game in front)" } else { "keyboard hook off" });
            }
        }
    });
}

fn reveal() {
    let changed = with_app(|a| {
        let changed = a.enabled && !a.revealed;
        if changed {
            a.revealed = true;
        }
        changed
    });
    if changed == Some(true) {
        taskbar::show_all();
        log::write("reveal");
    }
}

fn conceal() {
    let enabled = with_app(|a| {
        a.revealed = false;
        a.enabled
    });
    if enabled == Some(true) {
        let n = taskbar::hide_all();
        if n > 0 {
            log::write(&format!("conceal ({n} taskbars)"));
        }
    }
}

unsafe extern "system" fn on_win_event(
    _hook: HWINEVENTHOOK,
    event: u32,
    hwnd: HWND,
    id_object: i32,
    _id_child: i32,
    _thread: u32,
    _time: u32,
) {
    if hwnd.0.is_null() || id_object != OBJID_WINDOW {
        return;
    }
    if event == EVENT_OBJECT_SHOW {
        on_show(hwnd);
    } else if event == EVENT_SYSTEM_FOREGROUND {
        on_foreground(hwnd);
    }
}

/// explorer showed one of its windows. If it is a taskbar that should be
/// hidden, hide it again.
fn on_show(hwnd: HWND) {
    let wanted_hidden = with_app(|a| a.enabled && !a.revealed).unwrap_or(false);
    if !wanted_hidden || !is_taskbar_class(&win::class_name(hwnd)) {
        return;
    }
    if taskbar::hide(hwnd) {
        let count = with_app(|a| {
            a.rehides += 1;
            a.rehides
        })
        .unwrap_or(0);
        // Every time at first, then sparsely, in case explorer keeps showing it.
        if count <= 20 || count % 100 == 0 {
            log::write(&format!("explorer showed a taskbar; hidden again (#{count})"));
        }
    }
}

fn on_foreground(hwnd: HWND) {
    let class = win::class_name(hwnd);
    let exe = win::process_path(hwnd).unwrap_or_default();
    let kind = classify(&class, win::file_name(&exe));
    let mode = with_app(|a| a.mode);
    match kind {
        Foreground::StartSurface => {
            if mode == Some(Mode::StartMenu) {
                reveal();
            }
        }
        Foreground::Taskbar => {}
        Foreground::Other => {
            if with_app(|a| a.revealed).unwrap_or(false) {
                conceal();
            }
        }
    }
    update_keyboard_hook();
}

unsafe extern "system" fn on_keyboard(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code >= 0 {
        let info = &*(lparam.0 as *const KBDLLHOOKSTRUCT);
        let vk = info.vkCode as u16;
        let is_win = vk == VK_LWIN.0 || vk == VK_RWIN.0;
        let msg = wparam.0 as u32;
        let is_down = msg == WM_KEYDOWN || msg == WM_SYSKEYDOWN;
        let is_up = msg == WM_KEYUP || msg == WM_SYSKEYUP;
        if is_down || is_up {
            // Only post here: the hook must return quickly.
            if let Some((true, hwnd)) = with_app(|a| (a.tap.feed(is_win, is_down), a.hwnd)) {
                let _ = PostMessageW(hwnd, WM_APP_TAP, WPARAM(0), LPARAM(0));
            }
        }
    }
    CallNextHookEx(None, code, wparam, lparam)
}

unsafe extern "system" fn window_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    let taskbar_created = with_app(|a| a.taskbar_created).unwrap_or(0);
    match msg {
        WM_APP_TAP => {
            reveal();
            LRESULT(0)
        }
        WM_APP_TRAY => {
            let event = (lparam.0 & 0xFFFF) as u32;
            if event == WM_RBUTTONUP || event == WM_LBUTTONUP {
                show_menu(hwnd);
            }
            LRESULT(0)
        }
        WM_COMMAND => {
            match wparam.0 & 0xFFFF {
                ID_ENABLED => toggle_enabled(),
                ID_AUTOSTART => {
                    let ok = if autostart::is_registered() { autostart::unregister() } else { autostart::register() };
                    log::write(&format!("start at sign-in toggled: {ok}"));
                }
                ID_EXIT => PostQuitMessage(0),
                _ => {}
            }
            LRESULT(0)
        }
        WM_DISPLAYCHANGE => {
            // A monitor added or removed brings a taskbar with it.
            if !with_app(|a| a.revealed).unwrap_or(true) {
                conceal();
            }
            DefWindowProcW(hwnd, msg, wparam, lparam)
        }
        WM_ENDSESSION => {
            if wparam.0 != 0 {
                taskbar::show_all();
                log::write("session ending; taskbars shown");
            }
            LRESULT(0)
        }
        WM_CLOSE => {
            PostQuitMessage(0);
            LRESULT(0)
        }
        _ if taskbar_created != 0 && msg == taskbar_created => {
            log::write("explorer restarted");
            add_tray_icon(hwnd);
            attach_hooks();
            if !with_app(|a| a.revealed).unwrap_or(true) {
                conceal();
            }
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

fn toggle_enabled() {
    let enabled = with_app(|a| {
        a.enabled = !a.enabled;
        a.revealed = false;
        a.enabled
    })
    .unwrap_or(true);
    if enabled {
        conceal();
    } else {
        taskbar::show_all();
    }
    update_keyboard_hook();
    log::write(if enabled { "enabled" } else { "disabled" });
}

fn notify_data(hwnd: HWND) -> NOTIFYICONDATAW {
    NOTIFYICONDATAW { cbSize: mem::size_of::<NOTIFYICONDATAW>() as u32, hWnd: hwnd, uID: 1, ..Default::default() }
}

fn add_tray_icon(hwnd: HWND) {
    let mut nid = notify_data(hwnd);
    nid.uFlags = NIF_ICON | NIF_MESSAGE | NIF_TIP;
    nid.uCallbackMessage = WM_APP_TRAY;
    // TODO: embed an icon of its own; the generic application icon is used for now.
    nid.hIcon = unsafe { LoadIconW(None, IDI_APPLICATION).unwrap_or_default() };
    let tip = win::wide("ytaskbary");
    let len = tip.len().min(nid.szTip.len());
    nid.szTip[..len].copy_from_slice(&tip[..len]);
    unsafe {
        let _ = Shell_NotifyIconW(NIM_ADD, &nid);
    }
}

fn remove_tray_icon(hwnd: HWND) {
    let nid = notify_data(hwnd);
    unsafe {
        let _ = Shell_NotifyIconW(NIM_DELETE, &nid);
    }
}

fn show_menu(hwnd: HWND) {
    let enabled = with_app(|a| a.enabled).unwrap_or(true);
    let autostart = autostart::is_registered();
    unsafe {
        let Ok(menu) = CreatePopupMenu() else { return };
        let check = |on: bool| if on { MF_STRING | MF_CHECKED } else { MF_STRING | MF_UNCHECKED };
        let _ = AppendMenuW(menu, check(enabled), ID_ENABLED, w!("Enabled"));
        let _ = AppendMenuW(menu, check(autostart), ID_AUTOSTART, w!("Start at sign-in"));
        let _ = AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null());
        let _ = AppendMenuW(menu, MF_STRING, ID_EXIT, w!("Exit"));
        let mut pt = POINT::default();
        let _ = GetCursorPos(&mut pt);
        // Needed for the menu to close when clicking elsewhere.
        let _ = SetForegroundWindow(hwnd);
        let _ = TrackPopupMenu(menu, TPM_RIGHTBUTTON | TPM_BOTTOMALIGN, pt.x, pt.y, 0, hwnd, None);
        let _ = PostMessageW(hwnd, WM_NULL, WPARAM(0), LPARAM(0));
        let _ = DestroyMenu(menu);
    }
}
