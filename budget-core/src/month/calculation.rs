use std::collections::BTreeSet;

use crate::config::{AccountConfig, AccountKind, AppConfig};
use crate::error::BudgetError;
use crate::money::Money;

use super::document::MonthDocument;
use super::id::MonthId;

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
    pub fn cache(&self) -> super::document::DerivedCache {
        super::document::DerivedCache {
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
}

/// Calculated account row with both raw and sign-normalised values.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AccountRow {
    pub id: String,
    pub label: String,
    pub kind: AccountKind,
    /// User-entered balance before asset or liability sign rules are applied.
    pub raw_balance: Money,
    /// Signed balance used by totals, validation, and summaries.
    pub normalised_balance: Money,
}

/// Derived timing-adjustment values used by the UI and validation logic.
///
/// The `*_raw` fields mirror what the user typed, while the `*_effect` fields
/// are the signed values that flow into totals.
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
