use serde::{Deserialize, Serialize};

use crate::error::BudgetError;
use crate::money::Money;

/// The application configuration stored in the budget repository.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppConfig {
    pub validation_tolerance_minor: i64,
    pub accounts: Vec<AccountConfig>,
    pub savings_pots: Vec<SavingsPotConfig>,
    pub next_month_earmarks: Vec<EarmarkConfig>,
}

impl AppConfig {
    pub fn validate(&self) -> Result<(), BudgetError> {
        validate_unique_ids(self.accounts.iter().map(|account| account.id.as_str()))?;
        validate_unique_ids(self.savings_pots.iter().map(|pot| pot.id.as_str()))?;
        validate_unique_ids(self.next_month_earmarks.iter().map(|item| item.id.as_str()))?;

        if self.validation_tolerance_minor < 0 {
            return Err(BudgetError::NegativeValue {
                field: "validation_tolerance_minor".to_owned(),
            });
        }

        for account in &self.accounts {
            if account.label.trim().is_empty() {
                return Err(BudgetError::InvalidMoney(
                    "account label cannot be empty".to_owned(),
                ));
            }
        }

        Ok(())
    }

    pub fn default_mvp() -> Self {
        Self {
            validation_tolerance_minor: 100,
            accounts: vec![
                AccountConfig::asset("current", "Current"),
                AccountConfig::asset("cash_isa", "Cash ISA"),
                AccountConfig::liability("amex_credit", "Amex credit"),
                AccountConfig::liability("nationwide_credit", "Nationwide credit"),
            ],
            savings_pots: vec![
                SavingsPotConfig::new("fun_expensive_stuff", "Fun expensive stuff", 15_500),
                SavingsPotConfig::new("long_term_savings", "Long-term savings", 6_000),
                SavingsPotConfig::new("label", "Label", 2_500),
            ],
            next_month_earmarks: vec![
                EarmarkConfig::new("subscriptions", "Subscriptions", 13_000),
                EarmarkConfig::new("general_spending", "General spending", 37_500),
            ],
        }
    }

    pub fn account(&self, id: &str) -> Option<&AccountConfig> {
        self.accounts.iter().find(|account| account.id == id)
    }

    pub fn pot(&self, id: &str) -> Option<&SavingsPotConfig> {
        self.savings_pots.iter().find(|pot| pot.id == id)
    }

    pub fn earmark(&self, id: &str) -> Option<&EarmarkConfig> {
        self.next_month_earmarks.iter().find(|item| item.id == id)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccountConfig {
    pub id: String,
    pub label: String,
    pub kind: AccountKind,
}

impl AccountConfig {
    fn asset(id: &str, label: &str) -> Self {
        Self {
            id: id.to_owned(),
            label: label.to_owned(),
            kind: AccountKind::Asset,
        }
    }

    fn liability(id: &str, label: &str) -> Self {
        Self {
            id: id.to_owned(),
            label: label.to_owned(),
            kind: AccountKind::Liability,
        }
    }

    pub fn apply_sign(&self, raw_balance_minor: i64) -> Money {
        match self.kind {
            AccountKind::Asset => Money::from_minor(raw_balance_minor),
            AccountKind::Liability => Money::from_minor(-raw_balance_minor),
        }
    }

    pub fn sign_cue(&self) -> &'static str {
        match self.kind {
            AccountKind::Asset => "+",
            AccountKind::Liability => "-",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AccountKind {
    Asset,
    Liability,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SavingsPotConfig {
    pub id: String,
    pub label: String,
    pub default_monthly_change_minor: i64,
}

impl SavingsPotConfig {
    fn new(id: &str, label: &str, default_monthly_change_minor: i64) -> Self {
        Self {
            id: id.to_owned(),
            label: label.to_owned(),
            default_monthly_change_minor,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EarmarkConfig {
    pub id: String,
    pub label: String,
    pub default_amount_minor: i64,
}

impl EarmarkConfig {
    fn new(id: &str, label: &str, default_amount_minor: i64) -> Self {
        Self {
            id: id.to_owned(),
            label: label.to_owned(),
            default_amount_minor,
        }
    }
}

fn validate_unique_ids<'a>(ids: impl IntoIterator<Item = &'a str>) -> Result<(), BudgetError> {
    let mut seen = std::collections::BTreeSet::new();
    for id in ids {
        if !seen.insert(id.to_owned()) {
            return Err(BudgetError::DuplicateConfigId(id.to_owned()));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{AccountKind, AppConfig};

    #[test]
    fn default_config_is_valid() {
        AppConfig::default_mvp().validate().unwrap();
    }

    #[test]
    fn liability_accounts_apply_negative_signs() {
        let config = AppConfig::default_mvp();
        let liability = config.account("amex_credit").unwrap();
        assert_eq!(liability.kind, AccountKind::Liability);
        assert_eq!(liability.apply_sign(12_345).minor(), -12_345);
    }

    #[test]
    fn subscriptions_default_matches_mvp_budget() {
        let config = AppConfig::default_mvp();
        assert_eq!(
            config
                .earmark("subscriptions")
                .unwrap()
                .default_amount_minor,
            13_000
        );
    }
}
