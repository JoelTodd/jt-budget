use serde::{Deserialize, Serialize};

use crate::error::BudgetError;
use crate::money::Money;

/// The application configuration stored in the budget repository.
///
/// The list order is significant because drafts, calculations, and the TUI all
/// preserve it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppConfig {
    pub validation_tolerance_minor: i64,
    pub accounts: Vec<AccountConfig>,
    pub savings_pots: Vec<SavingsPotConfig>,
    pub next_month_earmarks: Vec<EarmarkConfig>,
}

impl AppConfig {
    /// Validates that the persisted repository configuration matches the MVP's
    /// structural invariants.
    ///
    /// # Errors
    ///
    /// Returns [`BudgetError`] when identifiers collide, required labels are
    /// blank, or values that must remain non-negative violate that contract.
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

    /// Returns the default configuration baked into the MVP brief.
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

    /// Looks up an account definition by its stable identifier.
    pub fn account(&self, id: &str) -> Option<&AccountConfig> {
        self.accounts.iter().find(|account| account.id == id)
    }

    /// Looks up a savings pot definition by its stable identifier.
    pub fn pot(&self, id: &str) -> Option<&SavingsPotConfig> {
        self.savings_pots.iter().find(|pot| pot.id == id)
    }

    /// Looks up a next-month earmark definition by its stable identifier.
    pub fn earmark(&self, id: &str) -> Option<&EarmarkConfig> {
        self.next_month_earmarks.iter().find(|item| item.id == id)
    }
}

/// Stable account definition used in config files and month documents.
///
/// The `kind` decides how the user's always-positive entry is normalised for
/// calculations.
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

    /// Converts the user's always-positive entry into the signed value used by
    /// the calculation engine.
    pub fn apply_sign(&self, raw_balance_minor: i64) -> Money {
        match self.kind {
            AccountKind::Asset => Money::from_minor(raw_balance_minor),
            AccountKind::Liability => Money::from_minor(-raw_balance_minor),
        }
    }

    /// Returns the sign hint shown in the UI next to this account.
    pub fn sign_cue(&self) -> &'static str {
        match self.kind {
            AccountKind::Asset => "+",
            AccountKind::Liability => "-",
        }
    }
}

/// Determines whether an account increases or reduces net position.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AccountKind {
    /// A positive balance that increases net position.
    Asset,
    /// A positive entered balance that is treated as a negative contribution.
    Liability,
}

/// Static savings-pot definition used for draft defaults and validation.
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

/// Static next-month earmark definition used for draft defaults and validation.
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

    #[test]
    fn config_ignores_legacy_ui_theme_section() {
        let config: AppConfig = toml::from_str(
            r##"
validation_tolerance_minor = 100

[[accounts]]
id = "current"
label = "Current"
kind = "asset"

[[savings_pots]]
id = "rainy_day"
label = "Rainy day"
default_monthly_change_minor = 5000

[[next_month_earmarks]]
id = "groceries"
label = "Groceries"
default_amount_minor = 15000

[ui.base24]
base00 = "#262626"
"##,
        )
        .unwrap();

        assert_eq!(config.validation_tolerance_minor, 100);
        assert_eq!(config.accounts.len(), 1);
    }
}
