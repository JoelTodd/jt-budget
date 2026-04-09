use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::config::AppConfig;
use crate::error::BudgetError;

use super::calculation::{CalculatedMonth, calculate_month};
use super::id::MonthId;

/// Persisted editable month document.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MonthDocument {
    pub month: MonthId,
    #[serde(default)]
    pub accounts: BTreeMap<String, i64>,
    #[serde(default)]
    pub timing_adjustments: TimingAdjustments,
    #[serde(default)]
    pub next_month_earmarks: BTreeMap<String, i64>,
    #[serde(default)]
    pub savings_pots: BTreeMap<String, SavingsPotState>,
    #[serde(default)]
    pub meta: MonthMeta,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub derived: Option<DerivedCache>,
}

impl MonthDocument {
    /// Creates a new editable draft for the requested month.
    ///
    /// Accounts start at zero, next-month earmarks start from configuration
    /// defaults, and savings pots carry forward the prior month's final
    /// balances when available.
    pub fn new_draft(
        month: MonthId,
        config: &AppConfig,
        previous: Option<&CalculatedMonth>,
    ) -> Self {
        let accounts = config
            .accounts
            .iter()
            .map(|account| (account.id.clone(), 0))
            .collect();
        let next_month_earmarks = config
            .next_month_earmarks
            .iter()
            .map(|item| (item.id.clone(), item.default_amount_minor))
            .collect();
        let savings_pots = config
            .savings_pots
            .iter()
            .map(|pot| {
                // Carry forward the last known final balance so the user
                // confirms real-world pot changes instead of retyping
                // everything from scratch.
                let carried_over = previous
                    .and_then(|month| month.pot_rows.iter().find(|row| row.id == pot.id))
                    .map(|row| row.final_balance.minor())
                    .unwrap_or(0);
                (
                    pot.id.clone(),
                    SavingsPotState {
                        carried_over,
                        monthly_change: pot.default_monthly_change_minor,
                    },
                )
            })
            .collect();

        Self {
            month,
            accounts,
            timing_adjustments: TimingAdjustments::default(),
            next_month_earmarks,
            savings_pots,
            meta: MonthMeta::default(),
            derived: None,
        }
    }

    /// Updates the document timestamps to the current UTC instant.
    pub fn stamp_updated_now(&mut self) {
        let now = OffsetDateTime::now_utc();
        let text = now
            .format(&time::format_description::well_known::Rfc3339)
            .expect("rfc3339 formatting should succeed");
        if self.meta.created_at.is_none() {
            self.meta.created_at = Some(text.clone());
        }
        self.meta.updated_at = Some(text);
    }

    /// Serialises the document with a freshly recomputed derived cache.
    ///
    /// The cache exists only as an inspectable convenience in the repository;
    /// callers must still recompute derived values from editable state.
    ///
    /// # Errors
    ///
    /// Returns [`BudgetError`] if recalculation fails or the document cannot be
    /// serialised to TOML.
    pub fn to_pretty_toml(&self, config: &AppConfig) -> Result<String, BudgetError> {
        let calculated = calculate_month(config, self)?;
        let mut persisted = self.clone();
        persisted.derived = Some(calculated.cache());
        Ok(toml::to_string_pretty(&persisted)?)
    }
}

/// Raw user-entered timing adjustments.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TimingAdjustments {
    pub investment_not_yet_sent_raw: i64,
    pub previous_month_spending_correction_raw: i64,
}

/// Persisted state for a single savings pot row.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SavingsPotState {
    pub carried_over: i64,
    pub monthly_change: i64,
}

/// Metadata stored alongside the editable month data.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MonthMeta {
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

/// Convenience cache persisted for humans inspecting the repository.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DerivedCache {
    pub accounts_subtotal_minor: i64,
    pub timing_adjustments_subtotal_minor: i64,
    pub net_available_minor: i64,
    pub pots_carried_total_minor: i64,
    pub pots_monthly_change_total_minor: i64,
    pub pots_final_total_minor: i64,
    pub next_month_earmarks_subtotal_minor: i64,
    pub total_allocated_minor: i64,
    pub overall_difference_minor: i64,
    pub is_valid: bool,
}
