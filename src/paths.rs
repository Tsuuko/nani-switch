use std::{env, path::PathBuf};

const PACKAGE_FAMILY: &str = "KiokuLLC.NaniTranslate_mpzwtaxj5jyfc";

fn home_dir() -> PathBuf {
    env::var_os("USERPROFILE")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}

pub fn store_dir() -> PathBuf {
    env::var_os("NANI_SWITCH_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| home_dir().join(".nani-switch"))
}

pub fn accounts_path() -> PathBuf {
    store_dir().join("accounts.json")
}

pub fn log_path() -> PathBuf {
    store_dir().join("nani-switch.log")
}

pub fn logical_app_data_path() -> PathBuf {
    env::var_os("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| home_dir().join("AppData").join("Roaming"))
        .join("Nani")
}

pub fn db_candidates() -> [PathBuf; 2] {
    let home = home_dir();
    [
        home.join("AppData")
            .join("Local")
            .join("Packages")
            .join(PACKAGE_FAMILY)
            .join("LocalCache")
            .join("Roaming")
            .join("Nani")
            .join("app.db")
            .join("db"),
        home.join("AppData")
            .join("Roaming")
            .join("Nani")
            .join("app.db")
            .join("db"),
    ]
}

pub fn db_path() -> PathBuf {
    if let Some(path) = env::var_os("NANI_DB_PATH") {
        return PathBuf::from(path);
    }
    let candidates = db_candidates();
    candidates
        .iter()
        .find(|candidate| candidate.exists())
        .cloned()
        .unwrap_or_else(|| candidates[0].clone())
}

pub fn user_data_dir() -> PathBuf {
    let home = home_dir();
    let candidates = [
        home.join("AppData")
            .join("Local")
            .join("Packages")
            .join(PACKAGE_FAMILY)
            .join("LocalCache")
            .join("Roaming")
            .join("Nani"),
        home.join("AppData").join("Roaming").join("Nani"),
    ];
    candidates
        .iter()
        .find(|candidate| candidate.exists())
        .cloned()
        .unwrap_or_else(|| candidates[0].clone())
}

pub fn nani_alias() -> PathBuf {
    env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| home_dir().join("AppData").join("Local"))
        .join("Microsoft")
        .join("WindowsApps")
        .join("naniapp.exe")
}

pub const NANI_AUMID: &str = "KiokuLLC.NaniTranslate_mpzwtaxj5jyfc!KiokuLLC.NaniTranslate";
