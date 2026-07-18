use std::{env, path::PathBuf};

use anyhow::{Context, Result, ensure};
use winreg::{
    RegKey,
    enums::{HKEY_CURRENT_USER, KEY_READ, KEY_WRITE},
};

const APP_NAME: &str = "Nani Switch";
const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
const APPROVED_KEY: &str =
    r"Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run";

fn executable() -> Result<PathBuf> {
    env::current_exe().context("could not determine the Nani Switch executable path")
}

pub fn is_enabled() -> bool {
    let Ok(executable) = executable() else {
        return false;
    };
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let Ok(run) = hkcu.open_subkey_with_flags(RUN_KEY, KEY_READ) else {
        return false;
    };
    let Ok(command) = run.get_value::<String, _>(APP_NAME) else {
        return false;
    };
    if !command
        .to_ascii_lowercase()
        .contains(&executable.to_string_lossy().to_ascii_lowercase())
    {
        return false;
    }
    let Ok(approved) = hkcu.open_subkey_with_flags(APPROVED_KEY, KEY_READ) else {
        return true;
    };
    let Ok(value) = approved.get_raw_value(APP_NAME) else {
        return true;
    };
    value.bytes.first().is_none_or(|state| *state == 0x02)
}

pub fn set_enabled(enabled: bool) -> Result<()> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let (run, _) = hkcu
        .create_subkey(RUN_KEY)
        .context("could not open the Windows startup registry key")?;
    if enabled {
        let command = format!("\"{}\"", executable()?.display());
        run.set_value(APP_NAME, &command)
            .context("could not register Nani Switch for startup")?;
        if let Ok(approved) = hkcu.open_subkey_with_flags(APPROVED_KEY, KEY_WRITE) {
            let _ = approved.delete_value(APP_NAME);
        }
    } else {
        let _ = run.delete_value(APP_NAME);
    }
    ensure!(
        is_enabled() == enabled,
        "Windows startup setting was not applied."
    );
    Ok(())
}
