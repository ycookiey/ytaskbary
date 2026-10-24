//! Finding, hiding and showing the taskbars of every monitor.

use windows::core::w;
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::{
    FindWindowExW, FindWindowW, IsWindowVisible, ShowWindow, SW_HIDE, SW_SHOWNOACTIVATE,
};

/// The main taskbar and those of the other monitors.
pub fn all() -> Vec<HWND> {
    let mut out = Vec::new();
    unsafe {
        if let Ok(main) = FindWindowW(w!("Shell_TrayWnd"), None) {
            if !main.0.is_null() {
                out.push(main);
            }
        }
        let mut after = HWND::default();
        while let Ok(next) = FindWindowExW(None, after, w!("Shell_SecondaryTrayWnd"), None) {
            if next.0.is_null() {
                break;
            }
            out.push(next);
            after = next;
        }
    }
    out
}

/// Hides `hwnd` if it is visible. Hiding only a visible taskbar keeps this
/// from raising window events of its own while nothing changed.
pub fn hide(hwnd: HWND) -> bool {
    unsafe {
        if !IsWindowVisible(hwnd).as_bool() {
            return false;
        }
        let _ = ShowWindow(hwnd, SW_HIDE);
    }
    true
}

pub fn hide_all() -> usize {
    all().into_iter().filter(|&h| hide(h)).count()
}

/// Shows every taskbar where explorer last placed it.
pub fn show_all() {
    for hwnd in all() {
        unsafe {
            if !IsWindowVisible(hwnd).as_bool() {
                let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
            }
        }
    }
}
