//! Whether the foreground window belongs to a game, by the list of games that
//! Windows keeps for the Xbox Game Bar (`HKCU\System\GameConfigStore\Children`).
//!
//! Only entries with an executable path (`MatchedExeFullPath`) are used: the
//! others cannot be matched with a running process. The key is undocumented,
//! so when it cannot be read no window counts as a game.

use std::collections::HashSet;

use windows::core::{w, PCWSTR, PWSTR};
use windows::Win32::Foundation::{CloseHandle, HANDLE};
use windows::Win32::Storage::FileSystem::{
    CreateFileW, GetFinalPathNameByHandleW, FILE_FLAGS_AND_ATTRIBUTES, FILE_NAME_NORMALIZED,
    FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
};
use windows::Win32::System::Registry::{
    RegCloseKey, RegEnumKeyExW, RegGetValueW, RegNotifyChangeKeyValue, RegOpenKeyExW, HKEY,
    HKEY_CURRENT_USER, KEY_NOTIFY, KEY_READ, REG_NOTIFY_CHANGE_LAST_SET, REG_NOTIFY_CHANGE_NAME,
    RRF_RT_REG_SZ,
};

const CHILDREN: PCWSTR = w!("System\\GameConfigStore\\Children");

/// The game list, as lowercase paths of the executables' final files.
pub struct GameList {
    paths: HashSet<String>,
    notify_key: Option<HKEY>,
}

impl GameList {
    pub fn load() -> Self {
        let notify_key = unsafe {
            let mut key = HKEY::default();
            RegOpenKeyExW(HKEY_CURRENT_USER, CHILDREN, 0, KEY_NOTIFY, &mut key)
                .ok()
                .ok()
                .map(|_| key)
        };
        Self { paths: read_paths(), notify_key }
    }

    pub fn reload(&mut self) {
        self.paths = read_paths();
    }

    pub fn len(&self) -> usize {
        self.paths.len()
    }

    /// Signals `event` once when the list changes; call again after each signal.
    pub fn watch(&self, event: HANDLE) -> bool {
        let Some(key) = self.notify_key else {
            return false;
        };
        unsafe {
            RegNotifyChangeKeyValue(key, true, REG_NOTIFY_CHANGE_NAME | REG_NOTIFY_CHANGE_LAST_SET, event, true).is_ok()
        }
    }

    pub fn contains(&self, exe_path: &str) -> bool {
        !self.paths.is_empty() && self.paths.contains(&normalize(exe_path))
    }
}

impl Drop for GameList {
    fn drop(&mut self) {
        if let Some(key) = self.notify_key.take() {
            unsafe {
                let _ = RegCloseKey(key);
            }
        }
    }
}

fn read_paths() -> HashSet<String> {
    let mut out = HashSet::new();
    unsafe {
        let mut root = HKEY::default();
        if RegOpenKeyExW(HKEY_CURRENT_USER, CHILDREN, 0, KEY_READ, &mut root).is_err() {
            return out;
        }
        let mut index = 0u32;
        loop {
            let mut name = [0u16; 256];
            let mut len = name.len() as u32;
            let result = RegEnumKeyExW(root, index, PWSTR(name.as_mut_ptr()), &mut len, None, PWSTR::null(), None, None);
            if result.is_err() {
                break;
            }
            index += 1;
            if let Some(path) = read_string(root, &name[..len as usize], "MatchedExeFullPath") {
                if !path.is_empty() {
                    out.insert(normalize(&path));
                }
            }
        }
        let _ = RegCloseKey(root);
    }
    out
}

fn read_string(root: HKEY, subkey: &[u16], value: &str) -> Option<String> {
    let mut subkey: Vec<u16> = subkey.to_vec();
    subkey.push(0);
    let value: Vec<u16> = value.encode_utf16().chain(std::iter::once(0)).collect();
    let mut buf = [0u16; 1024];
    let mut size = (buf.len() * 2) as u32;
    unsafe {
        RegGetValueW(
            root,
            PCWSTR(subkey.as_ptr()),
            PCWSTR(value.as_ptr()),
            RRF_RT_REG_SZ,
            None,
            Some(buf.as_mut_ptr() as *mut _),
            Some(&mut size),
        )
        .ok()
        .ok()?;
    }
    let chars = (size as usize / 2).saturating_sub(1);
    Some(String::from_utf16_lossy(&buf[..chars.min(buf.len())]))
}

/// The form used for matching: the final path behind junctions and symbolic
/// links, in lowercase. The game list and a running process can name the same
/// file through different junctions. A path that cannot be opened is only
/// lowercased.
fn normalize(path: &str) -> String {
    final_path(path).unwrap_or_else(|| path.to_string()).to_lowercase()
}

fn final_path(path: &str) -> Option<String> {
    let wide: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();
    unsafe {
        let handle = CreateFileW(
            PCWSTR(wide.as_ptr()),
            0,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            None,
            OPEN_EXISTING,
            FILE_FLAGS_AND_ATTRIBUTES(0),
            None,
        )
        .ok()?;
        let mut buf = [0u16; 1024];
        let len = GetFinalPathNameByHandleW(handle, &mut buf, FILE_NAME_NORMALIZED) as usize;
        let _ = CloseHandle(handle);
        if len == 0 || len >= buf.len() {
            return None;
        }
        let s = String::from_utf16_lossy(&buf[..len]);
        Some(s.strip_prefix("\\\\?\\").unwrap_or(&s).to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_lowercases_paths_that_cannot_be_opened() {
        assert_eq!(normalize("C:\\No\\Such\\Game.EXE"), "c:\\no\\such\\game.exe");
    }

    #[test]
    fn normalize_gives_one_form_for_an_existing_file() {
        let exe = std::env::current_exe().unwrap();
        let a = normalize(exe.to_str().unwrap());
        let b = normalize(&exe.to_str().unwrap().to_uppercase());
        assert_eq!(a, b);
    }
}
