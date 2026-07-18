use std::{
    fs,
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

use crate::{
    crypto, db,
    model::{Account, Accounts, CurrentSnapshot},
    paths,
};

pub fn read_accounts() -> Accounts {
    let Ok(contents) = fs::read_to_string(paths::accounts_path()) else {
        return Accounts::new();
    };
    serde_json::from_str(&contents).unwrap_or_default()
}

pub fn write_accounts(accounts: &Accounts) -> Result<()> {
    fs::create_dir_all(paths::store_dir()).context("could not create account storage folder")?;
    let json = serde_json::to_string_pretty(accounts)?;
    fs::write(paths::accounts_path(), format!("{json}\n")).context("could not write accounts.json")
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct Settings {
    #[serde(default = "enabled_by_default")]
    periodic_usage_refresh: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            periodic_usage_refresh: true,
        }
    }
}

const fn enabled_by_default() -> bool {
    true
}

fn settings_path() -> std::path::PathBuf {
    paths::store_dir().join("settings.json")
}

pub fn periodic_usage_refresh_enabled() -> bool {
    fs::read_to_string(settings_path())
        .ok()
        .and_then(|contents| serde_json::from_str::<Settings>(&contents).ok())
        .unwrap_or_default()
        .periodic_usage_refresh
}

pub fn set_periodic_usage_refresh(enabled: bool) -> Result<()> {
    fs::create_dir_all(paths::store_dir()).context("could not create settings folder")?;
    let settings = Settings {
        periodic_usage_refresh: enabled,
    };
    let json = serde_json::to_string_pretty(&settings)?;
    fs::write(settings_path(), format!("{json}\n")).context("could not write settings.json")
}

pub fn account_token(account: &Account) -> Result<String> {
    if let Some(token) = account.token.as_ref().filter(|token| !token.is_empty()) {
        return Ok(token.clone());
    }
    crypto::decrypt_token(&account.stored, &account.app_data_path)
}

pub fn find_current_account_name(
    accounts: &Accounts,
    snapshot: Option<&CurrentSnapshot>,
) -> Option<String> {
    let snapshot = snapshot?;
    for (name, account) in accounts {
        if snapshot.account.user_id.is_some() && snapshot.account.user_id == account.user_id {
            return Some(name.clone());
        }
        if snapshot.account.email.is_some() && snapshot.account.email == account.email {
            return Some(name.clone());
        }
        if account_token(account).is_ok_and(|token| token == snapshot.token) {
            return Some(name.clone());
        }
    }
    None
}

fn unique_name(accounts: &Accounts, snapshot: &CurrentSnapshot) -> String {
    let base = snapshot
        .account
        .email
        .as_ref()
        .or(snapshot.account.display_name.as_ref())
        .or(snapshot.account.user_id.as_ref())
        .cloned()
        .unwrap_or_else(|| format!("account-{}", snapshot.account.saved_at));
    if !accounts.contains_key(&base) {
        return base;
    }
    let mut suffix = 2;
    loop {
        let candidate = format!("{base}-{suffix}");
        if !accounts.contains_key(&candidate) {
            return candidate;
        }
        suffix += 1;
    }
}

pub fn save_current_account() -> Result<(String, bool)> {
    let snapshot = db::read_current_snapshot()?
        .ok_or_else(|| anyhow::anyhow!("No account is currently signed in to Nani."))?;
    let mut accounts = read_accounts();
    let existing = find_current_account_name(&accounts, Some(&snapshot));
    let updated = existing.is_some();
    let name = existing.unwrap_or_else(|| unique_name(&accounts, &snapshot));
    accounts.insert(name.clone(), snapshot.account);
    write_accounts(&accounts)?;
    Ok((name, updated))
}

pub fn delete_account(name: &str) -> Result<bool> {
    let mut accounts = read_accounts();
    if accounts.shift_remove(name).is_none() {
        return Ok(false);
    }
    write_accounts(&accounts)?;
    Ok(true)
}

pub fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

pub fn require_account_token(name: &str) -> Result<String> {
    let accounts = read_accounts();
    let account = accounts
        .get(name)
        .ok_or_else(|| anyhow::anyhow!("Saved account not found: {name}"))?;
    let token = account_token(account)?;
    if token.is_empty() {
        bail!("Saved account has an empty access token: {name}");
    }
    Ok(token)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn periodic_refresh_defaults_to_enabled() {
        let settings: Settings = serde_json::from_str("{}").unwrap();
        assert!(settings.periodic_usage_refresh);
    }

    #[test]
    fn periodic_refresh_disabled_value_round_trips() {
        let settings = Settings {
            periodic_usage_refresh: false,
        };
        let json = serde_json::to_string(&settings).unwrap();
        let restored: Settings = serde_json::from_str(&json).unwrap();
        assert!(!restored.periodic_usage_refresh);
    }
}
