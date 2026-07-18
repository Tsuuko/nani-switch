use std::{
    fs,
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

use anyhow::{Context, Result, bail};

use crate::{db, paths, store};

pub fn is_running() -> bool {
    Command::new("tasklist.exe")
        .args(["/FI", "IMAGENAME eq Nani.exe", "/NH", "/FO", "CSV"])
        .creation_flags_no_window()
        .output()
        .ok()
        .is_some_and(|output| {
            String::from_utf8_lossy(&output.stdout)
                .to_ascii_lowercase()
                .contains("nani.exe")
        })
}

pub fn kill() {
    let _ = Command::new("taskkill.exe")
        .args(["/IM", "Nani.exe", "/T", "/F"])
        .creation_flags_no_window()
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}

fn wait_for_exit() -> Result<()> {
    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline {
        if !is_running() {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(250));
    }
    bail!("Nani did not exit within 10 seconds.")
}

pub fn clear_local_storage() -> Result<bool> {
    let target = paths::user_data_dir().join("Local Storage").join("leveldb");
    if !target.exists() {
        return Ok(false);
    }
    let mut removed = false;
    for entry in fs::read_dir(&target)? {
        let path = entry?.path();
        let result = if path.is_dir() {
            fs::remove_dir_all(&path)
        } else {
            fs::remove_file(&path)
        };
        if result.is_ok() {
            removed = true;
        }
    }
    Ok(removed)
}

pub fn launch() -> Result<()> {
    let alias = paths::nani_alias();
    if alias.exists() {
        Command::new(alias)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .context("could not launch Nani")?;
        return Ok(());
    }
    Command::new("explorer.exe")
        .arg(format!(r"shell:AppsFolder\{}", paths::NANI_AUMID))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .context("could not launch Nani through the Apps folder")?;
    Ok(())
}

pub fn switch_to_account(name: &str) -> Result<()> {
    let token = store::require_account_token(name)?;
    if is_running() {
        kill();
        wait_for_exit()?;
    }
    db::write_account_token(&token)?;
    let _ = clear_local_storage();
    launch()
}

trait CommandExt {
    fn creation_flags_no_window(&mut self) -> &mut Self;
}

impl CommandExt for Command {
    fn creation_flags_no_window(&mut self) -> &mut Self {
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt as _;
            self.creation_flags(0x08000000);
        }
        self
    }
}
