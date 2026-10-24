//! Starting at sign-in, through `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`.

use windows::core::{w, PCWSTR};
use windows::Win32::System::Registry::{
    RegCloseKey, RegDeleteKeyValueW, RegGetValueW, RegOpenKeyExW, RegSetValueExW, HKEY,
    HKEY_CURRENT_USER, KEY_SET_VALUE, REG_SZ, RRF_RT_REG_SZ,
};

use crate::win::wide;

const RUN: PCWSTR = w!("Software\\Microsoft\\Windows\\CurrentVersion\\Run");
const NAME: PCWSTR = w!("ytaskbary");

pub fn is_registered() -> bool {
    let mut size = 0u32;
    unsafe { RegGetValueW(HKEY_CURRENT_USER, RUN, NAME, RRF_RT_REG_SZ, None, None, Some(&mut size)).is_ok() }
}

/// Registers this executable with the arguments it was started with, so
/// `--keyboard-hook` carries over.
pub fn register() -> bool {
    let Ok(exe) = std::env::current_exe() else { return false };
    let mut command = format!("\"{}\"", exe.display());
    for arg in std::env::args().skip(1) {
        command.push(' ');
        command.push_str(&arg);
    }
    let value = wide(&command);
    let bytes = unsafe { std::slice::from_raw_parts(value.as_ptr() as *const u8, value.len() * 2) };
    unsafe {
        let mut key = HKEY::default();
        if RegOpenKeyExW(HKEY_CURRENT_USER, RUN, 0, KEY_SET_VALUE, &mut key).is_err() {
            return false;
        }
        let ok = RegSetValueExW(key, NAME, 0, REG_SZ, Some(bytes)).is_ok();
        let _ = RegCloseKey(key);
        ok
    }
}

pub fn unregister() -> bool {
    unsafe { RegDeleteKeyValueW(HKEY_CURRENT_USER, RUN, NAME).is_ok() }
}
