use crate::money::Money;

use super::calculation::CalculatedMonth;

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

impl CalculatedMonth {
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
