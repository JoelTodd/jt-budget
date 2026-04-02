use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::config::{AccountConfig, AccountKind, AppConfig};
use crate::error::BudgetError;
use crate::money::Money;

/// Year-month identifier used for month files and UI labels.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MonthId {
    year: i32,
    month: u8,
}

impl MonthId {
    /// Parses a `YYYY-MM` month identifier.
    ///
    /// # Errors
    ///
    /// Returns [`BudgetError::InvalidMonthId`] when the input does not match
    /// the expected shape or refers to an out-of-range month.
    pub fn parse(input: &str) -> Result<Self, BudgetError> {
        let (year, month) = input
            .split_once('-')
            .ok_or_else(|| BudgetError::InvalidMonthId(input.to_owned()))?;
        let year: i32 = year
            .parse()
            .map_err(|_| BudgetError::InvalidMonthId(input.to_owned()))?;
        let month: u8 = month
            .parse()
            .map_err(|_| BudgetError::InvalidMonthId(input.to_owned()))?;
        Self::new(year, month).ok_or_else(|| BudgetError::InvalidMonthId(input.to_owned()))
    }

    /// Builds a validated month identifier from numeric components.
    pub fn new(year: i32, month: u8) -> Option<Self> {
        if (1..=12).contains(&month) {
            Some(Self { year, month })
        } else {
            None
        }
    }

    /// Returns the calendar year component.
    pub const fn year(self) -> i32 {
        self.year
    }

    /// Returns the calendar month component in the range `1..=12`.
    pub const fn month(self) -> u8 {
        self.month
    }

    /// Returns the repository filename for this month document.
    pub fn file_name(self) -> String {
        format!("{:04}-{:02}.toml", self.year(), self.month())
    }

    /// Returns the stable `YYYY-MM` key used throughout the app.
    pub fn key(self) -> String {
        format!("{:04}-{:02}", self.year(), self.month())
    }

    /// Returns the human-readable label shown in the UI.
    pub fn display_label(self) -> String {
        const MONTH_NAMES: [&str; 12] = [
            "January",
            "February",
            "March",
            "April",
            "May",
            "June",
            "July",
            "August",
            "September",
            "October",
            "November",
            "December",
        ];

        let month_index = usize::from(self.month() - 1);
        format!("{} {}", MONTH_NAMES[month_index], self.year())
    }
}

impl fmt::Display for MonthId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.key())
    }
}

impl Serialize for MonthId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.key())
    }
}

impl<'de> Deserialize<'de> for MonthId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::parse(&value).map_err(serde::de::Error::custom)
    }
}

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

/// Fully recomputed view of a persisted month document.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CalculatedMonth {
    pub month: MonthId,
    pub account_rows: Vec<AccountRow>,
    pub timing: TimingCalculation,
    pub earmark_rows: Vec<EarmarkRow>,
    pub pot_rows: Vec<PotRow>,
    pub totals: Totals,
    pub validation: ValidationState,
}

impl CalculatedMonth {
    /// Builds the inspectable derived cache written back into month files.
    pub fn cache(&self) -> DerivedCache {
        DerivedCache {
            accounts_subtotal_minor: self.totals.accounts_subtotal.minor(),
            timing_adjustments_subtotal_minor: self.totals.timing_adjustments_subtotal.minor(),
            net_available_minor: self.totals.net_available.minor(),
            pots_carried_total_minor: self.totals.pots_carried_total.minor(),
            pots_monthly_change_total_minor: self.totals.pots_monthly_change_total.minor(),
            pots_final_total_minor: self.totals.pots_final_total.minor(),
            next_month_earmarks_subtotal_minor: self.totals.next_month_earmarks_subtotal.minor(),
            total_allocated_minor: self.totals.total_allocated.minor(),
            overall_difference_minor: self.validation.overall_difference.minor(),
            is_valid: self.validation.is_valid,
        }
    }

    /// Returns grouped summary data for compact UI surfaces.
    pub fn summary_groups(&self) -> Vec<SummaryGroup> {
        vec![
            SummaryGroup {
                title: "Accounts".to_owned(),
                items: self
                    .account_rows
                    .iter()
                    .map(|row| SummaryItem {
                        label: row.label.clone(),
                        value: row.normalised_balance,
                    })
                    .chain(std::iter::once(SummaryItem {
                        label: "Net position".to_owned(),
                        value: self.totals.accounts_subtotal,
                    }))
                    .collect(),
            },
            SummaryGroup {
                title: "Timing Adjustments".to_owned(),
                items: vec![
                    SummaryItem {
                        label: "Investment not yet sent".to_owned(),
                        value: self.timing.investment_effect,
                    },
                    SummaryItem {
                        label: "General spending over/under".to_owned(),
                        value: self.timing.previous_month_spending_correction_effect,
                    },
                ],
            },
            SummaryGroup {
                title: "Next Month Earmarks".to_owned(),
                items: self
                    .earmark_rows
                    .iter()
                    .map(|row| SummaryItem {
                        label: row.label.clone(),
                        value: row.amount,
                    })
                    .collect(),
            },
            SummaryGroup {
                title: "Savings Pots".to_owned(),
                items: self
                    .pot_rows
                    .iter()
                    .map(|row| SummaryItem {
                        label: row.label.clone(),
                        value: row.final_balance,
                    })
                    .collect(),
            },
            SummaryGroup {
                title: "Final Check".to_owned(),
                items: vec![
                    SummaryItem {
                        label: "Total allocated".to_owned(),
                        value: self.totals.total_allocated,
                    },
                    SummaryItem {
                        label: "Overall difference".to_owned(),
                        value: self.validation.overall_difference,
                    },
                ],
            },
        ]
    }
}

/// Calculated account row with both raw and sign-normalised values.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AccountRow {
    pub id: String,
    pub label: String,
    pub kind: AccountKind,
    pub raw_balance: Money,
    pub normalised_balance: Money,
}

/// Derived timing-adjustment values used by the UI and validation logic.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TimingCalculation {
    pub investment_not_yet_sent_raw: Money,
    pub investment_effect: Money,
    pub previous_month_spending_correction_raw: Money,
    pub previous_month_spending_correction_effect: Money,
    pub subtotal: Money,
}

/// Calculated next-month earmark row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EarmarkRow {
    pub id: String,
    pub label: String,
    pub amount: Money,
}

/// Calculated savings-pot row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PotRow {
    pub id: String,
    pub label: String,
    pub carried_over: Money,
    pub monthly_change: Money,
    pub final_balance: Money,
}

/// Aggregated totals that drive summary panels and validation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Totals {
    pub accounts_subtotal: Money,
    pub timing_adjustments_subtotal: Money,
    pub net_available: Money,
    pub pots_carried_total: Money,
    pub pots_monthly_change_total: Money,
    pub pots_final_total: Money,
    pub next_month_earmarks_subtotal: Money,
    pub total_allocated: Money,
}

/// Final balancing result for a month.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ValidationState {
    pub tolerance: Money,
    pub overall_difference: Money,
    pub is_valid: bool,
}

/// Group of summary items shown in compact navigation surfaces.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SummaryGroup {
    pub title: String,
    pub items: Vec<SummaryItem>,
}

/// Single line item within a [`SummaryGroup`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SummaryItem {
    pub label: String,
    pub value: Money,
}

/// Recomputes all derived values for a month document.
///
/// # Errors
///
/// Returns [`BudgetError`] when the configuration is invalid, required entries
/// are missing, unexpected keys are present, or user-entered values break a
/// domain invariant.
pub fn calculate_month(
    config: &AppConfig,
    document: &MonthDocument,
) -> Result<CalculatedMonth, BudgetError> {
    config.validate()?;
    // Keep derived values trustworthy by validating that the persisted document
    // matches the configured set of editable fields exactly.
    verify_expected_keys(
        "accounts",
        document.accounts.keys().cloned().collect(),
        config
            .accounts
            .iter()
            .map(|account| account.id.clone())
            .collect(),
    )?;
    verify_expected_keys(
        "next_month_earmarks",
        document.next_month_earmarks.keys().cloned().collect(),
        config
            .next_month_earmarks
            .iter()
            .map(|item| item.id.clone())
            .collect(),
    )?;
    verify_expected_keys(
        "savings_pots",
        document.savings_pots.keys().cloned().collect(),
        config
            .savings_pots
            .iter()
            .map(|pot| pot.id.clone())
            .collect(),
    )?;

    let account_rows = config
        .accounts
        .iter()
        .map(|account| calculate_account_row(account, document))
        .collect::<Result<Vec<_>, _>>()?;
    let accounts_subtotal: Money = account_rows.iter().map(|row| row.normalised_balance).sum();

    if document.timing_adjustments.investment_not_yet_sent_raw < 0 {
        return Err(BudgetError::NegativeValue {
            field: "timing_adjustments.investment_not_yet_sent_raw".to_owned(),
        });
    }

    let timing = TimingCalculation {
        investment_not_yet_sent_raw: Money::from_minor(
            document.timing_adjustments.investment_not_yet_sent_raw,
        ),
        investment_effect: Money::from_minor(
            -document.timing_adjustments.investment_not_yet_sent_raw,
        ),
        previous_month_spending_correction_raw: Money::from_minor(
            document
                .timing_adjustments
                .previous_month_spending_correction_raw,
        ),
        previous_month_spending_correction_effect: Money::from_minor(
            document
                .timing_adjustments
                .previous_month_spending_correction_raw,
        ),
        subtotal: Money::from_minor(
            document
                .timing_adjustments
                .previous_month_spending_correction_raw
                - document.timing_adjustments.investment_not_yet_sent_raw,
        ),
    };

    let earmark_rows = config
        .next_month_earmarks
        .iter()
        .map(|item| {
            let amount = *document.next_month_earmarks.get(&item.id).ok_or_else(|| {
                BudgetError::MissingEntry {
                    section: "next_month_earmarks",
                    id: item.id.clone(),
                }
            })?;
            if amount < 0 {
                return Err(BudgetError::NegativeValue {
                    field: format!("next_month_earmarks.{}", item.id),
                });
            }
            Ok(EarmarkRow {
                id: item.id.clone(),
                label: item.label.clone(),
                amount: Money::from_minor(amount),
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let next_month_earmarks_subtotal: Money = earmark_rows.iter().map(|row| row.amount).sum();

    let pot_rows =
        config
            .savings_pots
            .iter()
            .map(|pot| {
                let state = document.savings_pots.get(&pot.id).ok_or_else(|| {
                    BudgetError::MissingEntry {
                        section: "savings_pots",
                        id: pot.id.clone(),
                    }
                })?;
                if state.carried_over < 0 {
                    return Err(BudgetError::NegativeValue {
                        field: format!("savings_pots.{}.carried_over", pot.id),
                    });
                }
                let final_balance = state.carried_over + state.monthly_change;
                if final_balance < 0 {
                    return Err(BudgetError::NegativePotFinal {
                        id: pot.id.clone(),
                        carried_over: state.carried_over,
                        monthly_change: state.monthly_change,
                    });
                }
                Ok(PotRow {
                    id: pot.id.clone(),
                    label: pot.label.clone(),
                    carried_over: Money::from_minor(state.carried_over),
                    monthly_change: Money::from_minor(state.monthly_change),
                    final_balance: Money::from_minor(final_balance),
                })
            })
            .collect::<Result<Vec<_>, _>>()?;

    let pots_carried_total: Money = pot_rows.iter().map(|row| row.carried_over).sum();
    let pots_monthly_change_total: Money = pot_rows.iter().map(|row| row.monthly_change).sum();
    let pots_final_total: Money = pot_rows.iter().map(|row| row.final_balance).sum();

    let timing_adjustments_subtotal = timing.subtotal;
    let net_available = accounts_subtotal + timing_adjustments_subtotal;
    let total_allocated = pots_final_total + next_month_earmarks_subtotal;
    let overall_difference = net_available - total_allocated;
    let tolerance = Money::from_minor(config.validation_tolerance_minor);
    let is_valid = (-tolerance).minor() <= overall_difference.minor()
        && overall_difference.minor() <= tolerance.minor();

    Ok(CalculatedMonth {
        month: document.month,
        account_rows,
        timing,
        earmark_rows,
        pot_rows,
        totals: Totals {
            accounts_subtotal,
            timing_adjustments_subtotal,
            net_available,
            pots_carried_total,
            pots_monthly_change_total,
            pots_final_total,
            next_month_earmarks_subtotal,
            total_allocated,
        },
        validation: ValidationState {
            tolerance,
            overall_difference,
            is_valid,
        },
    })
}

fn calculate_account_row(
    account: &AccountConfig,
    document: &MonthDocument,
) -> Result<AccountRow, BudgetError> {
    let raw = *document
        .accounts
        .get(&account.id)
        .ok_or_else(|| BudgetError::MissingEntry {
            section: "accounts",
            id: account.id.clone(),
        })?;
    if raw < 0 {
        return Err(BudgetError::NegativeValue {
            field: format!("accounts.{}", account.id),
        });
    }

    Ok(AccountRow {
        id: account.id.clone(),
        label: account.label.clone(),
        kind: account.kind,
        raw_balance: Money::from_minor(raw),
        normalised_balance: account.apply_sign(raw),
    })
}

fn verify_expected_keys(
    section: &'static str,
    actual: BTreeSet<String>,
    expected: BTreeSet<String>,
) -> Result<(), BudgetError> {
    if let Some(missing) = expected.difference(&actual).next() {
        return Err(BudgetError::MissingEntry {
            section,
            id: missing.clone(),
        });
    }
    if let Some(unexpected) = actual.difference(&expected).next() {
        return Err(BudgetError::UnexpectedEntry {
            section,
            id: unexpected.clone(),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::{MonthDocument, MonthId, SavingsPotState, calculate_month};
    use crate::config::AppConfig;
    use crate::money::Money;

    fn sample_month() -> MonthDocument {
        let mut document = MonthDocument::new_draft(
            MonthId::parse("2026-03").unwrap(),
            &AppConfig::default_mvp(),
            None,
        );
        document.accounts.insert("current".to_owned(), 200_000);
        document.accounts.insert("cash_isa".to_owned(), 50_000);
        document.accounts.insert("amex_credit".to_owned(), 20_000);
        document
            .accounts
            .insert("nationwide_credit".to_owned(), 10_000);
        document
            .timing_adjustments
            .previous_month_spending_correction_raw = -2_500;
        document.timing_adjustments.investment_not_yet_sent_raw = 24_500;
        document
            .next_month_earmarks
            .insert("subscriptions".to_owned(), 15_000);
        document
            .next_month_earmarks
            .insert("general_spending".to_owned(), 37_500);
        document.savings_pots.insert(
            "fun_expensive_stuff".to_owned(),
            SavingsPotState {
                carried_over: 10_000,
                monthly_change: 15_500,
            },
        );
        document.savings_pots.insert(
            "long_term_savings".to_owned(),
            SavingsPotState {
                carried_over: 5_000,
                monthly_change: 6_000,
            },
        );
        document.savings_pots.insert(
            "label".to_owned(),
            SavingsPotState {
                carried_over: 1_000,
                monthly_change: 2_500,
            },
        );
        document
    }

    #[test]
    fn calculates_expected_totals() {
        let calculated = calculate_month(&AppConfig::default_mvp(), &sample_month()).unwrap();
        assert_eq!(
            calculated.totals.accounts_subtotal,
            Money::from_minor(220_000)
        );
        assert_eq!(
            calculated.totals.timing_adjustments_subtotal,
            Money::from_minor(-27_000)
        );
        assert_eq!(calculated.totals.net_available, Money::from_minor(193_000));
        assert_eq!(calculated.totals.total_allocated, Money::from_minor(92_500));
        assert_eq!(
            calculated.validation.overall_difference,
            Money::from_minor(100_500)
        );
        assert!(!calculated.validation.is_valid);
    }

    #[test]
    fn carries_forward_previous_final_balances() {
        let config = AppConfig::default_mvp();
        let previous = calculate_month(&config, &sample_month()).unwrap();
        let current =
            MonthDocument::new_draft(MonthId::parse("2026-04").unwrap(), &config, Some(&previous));
        assert_eq!(
            current.savings_pots["fun_expensive_stuff"].carried_over,
            previous
                .pot_rows
                .iter()
                .find(|row| row.id == "fun_expensive_stuff")
                .unwrap()
                .final_balance
                .minor()
        );
    }

    #[test]
    fn new_draft_prefills_earmarks_from_config_defaults() {
        let config = AppConfig::default_mvp();
        let draft = MonthDocument::new_draft(MonthId::parse("2026-04").unwrap(), &config, None);

        assert_eq!(draft.next_month_earmarks["subscriptions"], 13_000);
        assert_eq!(draft.next_month_earmarks["general_spending"], 37_500);
    }

    #[test]
    fn validation_is_inclusive_at_tolerance_edges() {
        let config = AppConfig::default_mvp();
        let mut document =
            MonthDocument::new_draft(MonthId::parse("2026-03").unwrap(), &config, None);
        document.accounts.insert("current".to_owned(), 100);
        document.accounts.insert("cash_isa".to_owned(), 0);
        document.accounts.insert("amex_credit".to_owned(), 0);
        document.accounts.insert("nationwide_credit".to_owned(), 0);
        document
            .next_month_earmarks
            .insert("subscriptions".to_owned(), 0);
        document
            .next_month_earmarks
            .insert("general_spending".to_owned(), 0);
        document.savings_pots.insert(
            "fun_expensive_stuff".to_owned(),
            SavingsPotState {
                carried_over: 0,
                monthly_change: 0,
            },
        );
        document.savings_pots.insert(
            "long_term_savings".to_owned(),
            SavingsPotState {
                carried_over: 0,
                monthly_change: 0,
            },
        );
        document.savings_pots.insert(
            "label".to_owned(),
            SavingsPotState {
                carried_over: 0,
                monthly_change: 0,
            },
        );

        let calculated = calculate_month(&config, &document).unwrap();
        assert!(calculated.validation.is_valid);
    }

    #[test]
    fn ignores_stored_derived_values_when_recomputing() {
        let config = AppConfig::default_mvp();
        let mut document = sample_month();
        document.derived = Some(super::DerivedCache {
            accounts_subtotal_minor: 0,
            timing_adjustments_subtotal_minor: 0,
            net_available_minor: 0,
            pots_carried_total_minor: 0,
            pots_monthly_change_total_minor: 0,
            pots_final_total_minor: 0,
            next_month_earmarks_subtotal_minor: 0,
            total_allocated_minor: 0,
            overall_difference_minor: 0,
            is_valid: true,
        });

        let toml = toml::to_string_pretty(&document).unwrap();
        let reparsed: MonthDocument = toml::from_str(&toml).unwrap();
        let calculated = calculate_month(&config, &reparsed).unwrap();
        assert_eq!(
            calculated.totals.accounts_subtotal,
            Money::from_minor(220_000)
        );
    }

    proptest! {
        #[test]
        fn arithmetic_invariants_hold(
            current in 0_i64..500_000,
            cash_isa in 0_i64..500_000,
            amex in 0_i64..500_000,
            nationwide in 0_i64..500_000,
            previous_correction in -50_000_i64..50_000,
            investment in 0_i64..50_000,
            subscriptions in 0_i64..50_000,
            general_spending in 0_i64..100_000,
            carried_fun in 0_i64..200_000,
            carried_long in 0_i64..200_000,
            carried_label in 0_i64..200_000,
            change_fun in -50_000_i64..50_000,
            change_long in -50_000_i64..50_000,
            change_label in -50_000_i64..50_000,
        ) {
            prop_assume!(carried_fun + change_fun >= 0);
            prop_assume!(carried_long + change_long >= 0);
            prop_assume!(carried_label + change_label >= 0);

            let config = AppConfig::default_mvp();
            let mut document = MonthDocument::new_draft(MonthId::parse("2026-03").unwrap(), &config, None);
            document.accounts.insert("current".to_owned(), current);
            document.accounts.insert("cash_isa".to_owned(), cash_isa);
            document.accounts.insert("amex_credit".to_owned(), amex);
            document.accounts.insert("nationwide_credit".to_owned(), nationwide);
            document.timing_adjustments.previous_month_spending_correction_raw = previous_correction;
            document.timing_adjustments.investment_not_yet_sent_raw = investment;
            document.next_month_earmarks.insert("subscriptions".to_owned(), subscriptions);
            document.next_month_earmarks.insert("general_spending".to_owned(), general_spending);
            document.savings_pots.insert("fun_expensive_stuff".to_owned(), SavingsPotState { carried_over: carried_fun, monthly_change: change_fun });
            document.savings_pots.insert("long_term_savings".to_owned(), SavingsPotState { carried_over: carried_long, monthly_change: change_long });
            document.savings_pots.insert("label".to_owned(), SavingsPotState { carried_over: carried_label, monthly_change: change_label });

            let calculated = calculate_month(&config, &document).unwrap();

            prop_assert_eq!(
                calculated.totals.accounts_subtotal,
                Money::from_minor(current + cash_isa - amex - nationwide)
            );
            prop_assert_eq!(
                calculated.totals.timing_adjustments_subtotal,
                Money::from_minor(previous_correction - investment)
            );
            prop_assert_eq!(
                calculated.totals.pots_final_total,
                calculated.totals.pots_carried_total + calculated.totals.pots_monthly_change_total
            );
            prop_assert_eq!(
                calculated.validation.overall_difference,
                calculated.totals.accounts_subtotal
                    + calculated.totals.timing_adjustments_subtotal
                    - calculated.totals.total_allocated
            );
        }

        #[test]
        fn month_roundtrips_through_toml(
            current in 0_i64..200_000,
            cash_isa in 0_i64..200_000,
            amex in 0_i64..200_000,
            nationwide in 0_i64..200_000,
            subscriptions in 0_i64..50_000,
            general_spending in 0_i64..50_000,
            carried in 0_i64..100_000,
            change in -10_000_i64..10_000,
        ) {
            prop_assume!(carried + change >= 0);

            let config = AppConfig::default_mvp();
            let mut document = MonthDocument::new_draft(MonthId::parse("2026-03").unwrap(), &config, None);
            document.accounts.insert("current".to_owned(), current);
            document.accounts.insert("cash_isa".to_owned(), cash_isa);
            document.accounts.insert("amex_credit".to_owned(), amex);
            document.accounts.insert("nationwide_credit".to_owned(), nationwide);
            document.next_month_earmarks.insert("subscriptions".to_owned(), subscriptions);
            document.next_month_earmarks.insert("general_spending".to_owned(), general_spending);
            document.savings_pots.insert("fun_expensive_stuff".to_owned(), SavingsPotState { carried_over: carried, monthly_change: change });
            document.savings_pots.insert("long_term_savings".to_owned(), SavingsPotState { carried_over: carried, monthly_change: 0 });
            document.savings_pots.insert("label".to_owned(), SavingsPotState { carried_over: carried, monthly_change: 0 });

            let serialised = document.to_pretty_toml(&config).unwrap();
            let reparsed: MonthDocument = toml::from_str(&serialised).unwrap();
            prop_assert_eq!(document.month, reparsed.month);
            prop_assert_eq!(document.accounts, reparsed.accounts);
            prop_assert_eq!(document.next_month_earmarks, reparsed.next_month_earmarks);
            prop_assert_eq!(document.savings_pots, reparsed.savings_pots);
        }
    }
}
