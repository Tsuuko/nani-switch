use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

pub type Accounts = IndexMap<String, Account>;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    pub stored: String,
    pub token: Option<String>,
    pub user_id: Option<String>,
    pub email: Option<String>,
    pub display_name: Option<String>,
    pub saved_at: i64,
    pub app_data_path: String,
}

#[derive(Clone, Debug)]
pub struct CurrentSnapshot {
    pub account: Account,
    pub token: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Usage {
    pub monthly_usage: MonthlyUsage,
    pub max_tokens: f64,
    pub max_calls: f64,
    #[serde(default)]
    pub additional_free_tokens: f64,
    pub time_until_reset: Option<TimeUntilReset>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct MonthlyUsage {
    pub tokens: f64,
    pub calls: f64,
}

#[derive(Clone, Debug, Deserialize)]
pub struct TimeUntilReset {
    #[serde(default)]
    pub days: f64,
    #[serde(default)]
    pub hours: f64,
}

impl Usage {
    pub fn max_tokens_total(&self) -> f64 {
        self.max_tokens + self.additional_free_tokens
    }

    pub fn tokens_left(&self) -> f64 {
        (self.max_tokens_total() - self.monthly_usage.tokens).max(0.0)
    }

    pub fn calls_left(&self) -> f64 {
        (self.max_calls - self.monthly_usage.calls).max(0.0)
    }

    pub fn validate(self) -> anyhow::Result<Self> {
        let required = [
            self.monthly_usage.tokens,
            self.monthly_usage.calls,
            self.max_tokens,
            self.max_calls,
            self.additional_free_tokens,
        ];
        anyhow::ensure!(
            required.iter().all(|value| value.is_finite()),
            "invalid usage response"
        );
        Ok(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn account(id: &str) -> Account {
        Account {
            stored: format!("stored-{id}"),
            token: Some(format!("token-{id}")),
            user_id: Some(id.into()),
            email: None,
            display_name: None,
            saved_at: 1,
            app_data_path: r"C:\Nani".into(),
        }
    }

    #[test]
    fn account_json_order_is_preserved() {
        let mut accounts = Accounts::new();
        accounts.insert("second".into(), account("2"));
        accounts.insert("first".into(), account("1"));
        accounts.insert("third".into(), account("3"));

        let json = serde_json::to_string(&accounts).unwrap();
        let restored: Accounts = serde_json::from_str(&json).unwrap();

        assert_eq!(
            restored.keys().map(String::as_str).collect::<Vec<_>>(),
            ["second", "first", "third"]
        );
    }
}
