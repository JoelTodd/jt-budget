//! Shared document-edit helpers used by both guided creation and the editor.

use anyhow::Result;
use budget_core::{MonthDocument, MonthId};

use crate::state::{FieldId, MoneyInput};

/// Applies a committed field edit to the persisted month document.
pub(super) fn update_document_field(
    document: &mut MonthDocument,
    field: &FieldId,
    input: &MoneyInput,
) -> Result<bool> {
    let value = input.commit_value()?;
    if !field.allows_negative() && value.minor() < 0 {
        anyhow::bail!("{} cannot be negative", field.labelless_name());
    }
    let value = value.minor();

    let changed = match field {
        FieldId::Account(id) => document.accounts.insert(id.clone(), value) != Some(value),
        FieldId::PreviousMonthSpendingCorrection => {
            let changed = document
                .timing_adjustments
                .previous_month_spending_correction_raw
                != value;
            document
                .timing_adjustments
                .previous_month_spending_correction_raw = value;
            changed
        }
        FieldId::InvestmentNotYetSent => {
            let changed = document.timing_adjustments.investment_not_yet_sent_raw != value;
            document.timing_adjustments.investment_not_yet_sent_raw = value;
            changed
        }
        FieldId::Earmark(id) => {
            document.next_month_earmarks.insert(id.clone(), value) != Some(value)
        }
        FieldId::PotCarried(id) => {
            let entry = document.savings_pots.entry(id.clone()).or_default();
            let changed = entry.carried_over != value;
            entry.carried_over = value;
            changed
        }
        FieldId::PotChange(id) => {
            let entry = document.savings_pots.entry(id.clone()).or_default();
            let changed = entry.monthly_change != value;
            entry.monthly_change = value;
            changed
        }
    };

    Ok(changed)
}

pub(super) fn is_month_id_character(character: char) -> bool {
    character.is_ascii_digit() || character == '-'
}

/// Parses and validates a rename target entered in the navigation dialogue.
pub(super) fn validate_rename_target(
    source: MonthId,
    input: &str,
) -> std::result::Result<MonthId, String> {
    let target = MonthId::parse(input.trim()).map_err(|error| error.to_string())?;
    if target == source {
        return Err(format!("Month is already named {source}"));
    }
    Ok(target)
}

/// Returns whether a keypress is valid inside a money-editing field.
pub(super) fn is_money_input_character(character: char) -> bool {
    character.is_ascii_digit() || matches!(character, '.' | '-')
}
