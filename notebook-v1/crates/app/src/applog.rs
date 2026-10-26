//! A small log written to a file the user can open (Files › On My iPad › Notebook),
//! so start-up problems on a device can be diagnosed without Xcode.

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;

struct FileLogger(Mutex<Option<File>>);

impl log::Log for FileLogger {
    fn enabled(&self, m: &log::Metadata<'_>) -> bool {
        let t = m.target();
        // Graphics libraries are chatty: keep only their warnings and errors.
        let noisy = t.starts_with("wgpu") || t.starts_with("naga");
        m.level()
            <= if noisy {
                log::Level::Warn
            } else {
                log::Level::Info
            }
    }

    fn log(&self, r: &log::Record<'_>) {
        if !self.enabled(r.metadata()) {
            return;
        }
        let line = format!("[{}] {}: {}\n", r.level(), r.target(), r.args());
        eprint!("{line}");
        if let Ok(mut f) = self.0.lock() {
            if let Some(f) = f.as_mut() {
                let _ = f.write_all(line.as_bytes());
                let _ = f.flush();
            }
        }
    }

    fn flush(&self) {}
}

pub fn documents_dir() -> PathBuf {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_default();
    if cfg!(target_os = "ios") {
        home.join("Documents")
    } else {
        std::env::temp_dir()
    }
}

pub fn init() {
    let dir = documents_dir();
    let _ = std::fs::create_dir_all(&dir);
    let file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(dir.join("Notebook-log.txt"))
        .ok();
    let logger = Box::leak(Box::new(FileLogger(Mutex::new(file))));
    if log::set_logger(logger).is_ok() {
        log::set_max_level(log::LevelFilter::Info);
    }
}
