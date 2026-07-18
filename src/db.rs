use std::{fs, path::Path, time::Duration};

use anyhow::{Context, Result, bail};
use base64::{
    Engine,
    engine::general_purpose::{URL_SAFE, URL_SAFE_NO_PAD},
};
use rusqlite::{Connection, OptionalExtension, params};
use serde_json::Value;

use crate::{
    crypto,
    model::{Account, CurrentSnapshot},
    paths, store,
};

const ACT_KEY: &str = "act";
const GLOBAL_SETTINGS_CACHE_KEY: &str = "global-app-settings-cache";
const LAST_DAILY_JOB_KEY: &str = "last-daily-job-run";

fn first_value(connection: &Connection, key: &str) -> Result<Option<String>> {
    connection
        .query_row(
            "SELECT value FROM localkv WHERE key = ?1 LIMIT 1",
            [key],
            |row| row.get(0),
        )
        .optional()
        .map_err(Into::into)
}

fn jwt_user_id(token: &str) -> Option<String> {
    let payload = token.split('.').nth(1)?;
    let bytes = URL_SAFE_NO_PAD
        .decode(payload)
        .or_else(|_| URL_SAFE.decode(payload))
        .ok()?;
    let value: Value = serde_json::from_slice(&bytes).ok()?;
    ["userId", "sub", "id"]
        .iter()
        .find_map(|key| value.get(key).and_then(Value::as_str).map(str::to_owned))
}

pub fn read_current_snapshot() -> Result<Option<CurrentSnapshot>> {
    let path = paths::db_path();
    if !path.exists() {
        bail!("Nani database not found: {}", path.display());
    }
    let connection = Connection::open(&path)
        .with_context(|| format!("could not open Nani database: {}", path.display()))?;
    connection.busy_timeout(Duration::from_secs(2))?;
    let Some(stored) = first_value(&connection, ACT_KEY)? else {
        return Ok(None);
    };
    let app_data_path = paths::logical_app_data_path()
        .to_string_lossy()
        .into_owned();
    let token = crypto::decrypt_token(&stored, &app_data_path)?;
    let profile = first_value(&connection, GLOBAL_SETTINGS_CACHE_KEY)?
        .and_then(|cached| serde_json::from_str::<Value>(&cached).ok())
        .unwrap_or(Value::Null);
    let string_field = |key: &str| profile.get(key).and_then(Value::as_str).map(str::to_owned);
    let user_id = string_field("globalUserId").or_else(|| jwt_user_id(&token));
    Ok(Some(CurrentSnapshot {
        account: Account {
            stored,
            token: Some(token.clone()),
            user_id,
            email: string_field("email"),
            display_name: string_field("displayName"),
            saved_at: store::now_unix(),
            app_data_path,
        },
        token,
    }))
}

fn verify_database(path: &Path) -> Result<()> {
    let connection = Connection::open(path)?;
    let result: String = connection.query_row("PRAGMA integrity_check", [], |row| row.get(0))?;
    if result != "ok" {
        bail!("SQLite integrity check failed: {result}");
    }
    Ok(())
}

pub fn write_account_token(token: &str) -> Result<()> {
    let path = paths::db_path();
    if !path.exists() {
        bail!("Nani database not found: {}", path.display());
    }
    let backup = path.with_file_name(format!(
        "{}.nani-switch.bak",
        path.file_name().unwrap_or_default().to_string_lossy()
    ));
    fs::copy(&path, &backup).context("could not back up the Nani database")?;

    let update_result = (|| -> Result<()> {
        let app_data_path = paths::logical_app_data_path()
            .to_string_lossy()
            .into_owned();
        let stored = crypto::encrypt_token(token, &app_data_path)?;
        let mut connection = Connection::open(&path)?;
        connection.busy_timeout(Duration::from_secs(5))?;
        let transaction = connection.transaction()?;
        transaction.execute(
            "INSERT INTO localkv (key, value, updated_at) VALUES (?1, ?2, ?3)
             ON CONFLICT(key) DO UPDATE SET
               value = excluded.value,
               updated_at = excluded.updated_at",
            params![ACT_KEY, stored, store::now_unix()],
        )?;
        transaction.execute(
            "DELETE FROM localkv WHERE key IN (?1, ?2)",
            params![GLOBAL_SETTINGS_CACHE_KEY, LAST_DAILY_JOB_KEY],
        )?;
        transaction.commit()?;
        drop(connection);
        verify_database(&path)
    })();

    match update_result {
        Ok(()) => {
            let _ = fs::remove_file(&backup);
            Ok(())
        }
        Err(error) => {
            fs::copy(&backup, &path).context("database update failed and backup restore failed")?;
            let _ = fs::remove_file(&backup);
            Err(error)
        }
    }
}

#[cfg(test)]
mod tests {
    use std::env;

    use super::*;

    #[test]
    fn database_switch_rewrites_act_and_clears_caches() {
        let temporary = env::temp_dir().join(format!("nani-switch-db-{}", store::now_unix()));
        fs::create_dir_all(&temporary).unwrap();
        let db_path = temporary.join("db");
        let connection = Connection::open(&db_path).unwrap();
        connection
            .execute_batch(
                "CREATE TABLE localkv (
               key text PRIMARY KEY NOT NULL,
               value text NOT NULL,
               updated_at integer DEFAULT (unixepoch()) NOT NULL
             );
             INSERT INTO localkv VALUES ('act', 'old', 1);
             INSERT INTO localkv VALUES ('global-app-settings-cache', '{}', 1);
             INSERT INTO localkv VALUES ('last-daily-job-run', '1', 1);",
            )
            .unwrap();
        drop(connection);
        unsafe { env::set_var("NANI_DB_PATH", &db_path) };
        let token = "header.payload.signature";
        write_account_token(token).unwrap();
        let connection = Connection::open(&db_path).unwrap();
        let count: i64 = connection
            .query_row("SELECT count(*) FROM localkv", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 1);
        let stored = first_value(&connection, ACT_KEY).unwrap().unwrap();
        let app_data_path = paths::logical_app_data_path()
            .to_string_lossy()
            .into_owned();
        assert_eq!(
            crypto::decrypt_token(&stored, &app_data_path).unwrap(),
            token
        );
        drop(connection);
        unsafe { env::remove_var("NANI_DB_PATH") };
        fs::remove_dir_all(temporary).unwrap();
    }
}
