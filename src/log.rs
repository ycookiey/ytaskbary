//! A plain log file for checking the behavior: `%LOCALAPPDATA%\ytaskbary\ytaskbary.log`.
//! It starts over when it grows past 256 KB.

use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

static FILE: Mutex<Option<File>> = Mutex::new(None);
const LIMIT: u64 = 256 * 1024;

pub fn dir() -> Option<PathBuf> {
    std::env::var_os("LOCALAPPDATA").map(|d| PathBuf::from(d).join("ytaskbary"))
}

pub fn init() {
    let Some(dir) = dir() else { return };
    if fs::create_dir_all(&dir).is_err() {
        return;
    }
    let path = dir.join("ytaskbary.log");
    let too_big = fs::metadata(&path).map(|m| m.len() > LIMIT).unwrap_or(false);
    let file = OpenOptions::new()
        .create(true)
        .write(true)
        .append(!too_big)
        .truncate(too_big)
        .open(path)
        .ok();
    *FILE.lock().unwrap() = file;
}

pub fn write(line: &str) {
    let mut guard = FILE.lock().unwrap();
    let Some(file) = guard.as_mut() else { return };
    let secs = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let _ = writeln!(file, "{secs} {line}");
}
