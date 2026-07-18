use std::{
    fs,
    io::Write,
    time::{SystemTime, UNIX_EPOCH},
};

use crate::paths;

fn write(level: &str, message: &str) {
    let result = (|| -> std::io::Result<()> {
        let target = paths::log_path();
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(target)?;
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        writeln!(file, "{timestamp} [{level}] {message}")
    })();
    let _ = result;
}

pub fn info(message: impl AsRef<str>) {
    write("info", message.as_ref());
}

pub fn error(message: impl AsRef<str>) {
    write("error", message.as_ref());
}
