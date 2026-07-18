use std::{thread, time::Duration};

use anyhow::{Context, Result};
use reqwest::blocking::Client;

use crate::{
    model::{Accounts, Usage},
    store,
};

const USAGE_URL: &str = "https://nani.now/api/common/usage";

pub fn fetch(token: &str, client: &Client) -> Result<Usage> {
    let response = client
        .get(USAGE_URL)
        .header("Accept", "application/json")
        .bearer_auth(token)
        .send()
        .context("usage request failed")?;
    let status = response.status();
    if !status.is_success() {
        anyhow::bail!("HTTP {}", status.as_u16());
    }
    response
        .json::<Usage>()
        .context("invalid usage response")?
        .validate()
}

pub fn fetch_all(accounts: Accounts) -> Vec<(String, Result<Usage, String>)> {
    let client = match Client::builder().timeout(Duration::from_secs(15)).build() {
        Ok(client) => client,
        Err(error) => {
            return accounts
                .into_keys()
                .map(|name| (name, Err(error.to_string())))
                .collect();
        }
    };
    let handles = accounts.into_iter().map(|(name, account)| {
        let client = client.clone();
        thread::spawn(move || {
            let result = store::account_token(&account)
                .and_then(|token| fetch(&token, &client))
                .map_err(|error| error.to_string());
            (name, result)
        })
    });
    handles
        .map(|handle| {
            handle
                .join()
                .unwrap_or_else(|_| ("unknown".into(), Err("usage worker panicked".into())))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use crate::model::{MonthlyUsage, TimeUntilReset};

    use super::*;

    #[test]
    fn calculates_remaining_usage() {
        let usage = Usage {
            monthly_usage: MonthlyUsage {
                tokens: 60_750.0,
                calls: 74.0,
            },
            max_tokens: 60_000.0,
            max_calls: 100.0,
            additional_free_tokens: 0.0,
            time_until_reset: Some(TimeUntilReset {
                days: 14.0,
                hours: 5.0,
            }),
        };
        assert_eq!(usage.tokens_left(), 0.0);
        assert_eq!(usage.calls_left(), 26.0);
    }
}
