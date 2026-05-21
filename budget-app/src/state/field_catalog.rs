use budget_core::{AppConfig, Money, MonthDocument, format_minor_units};

/// Stable identifier for an editable field in guided creation or the editor.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum FieldId {
    Account(String),
    PreviousMonthSpendingCorrection,
    InvestmentNotYetSent,
    Earmark(String),
    PotCarried(String),
    PotChange(String),
}

impl FieldId {
    /// Returns the guided-creation field order defined by the MVP workflow.
    pub fn guided_steps(config: &AppConfig) -> Vec<Self> {
        let fields = configured_fields(config);
        fields
            .iter()
            .filter(|descriptor| descriptor.is_account_or_timing())
            .map(|descriptor| descriptor.field.clone())
            .chain(
                fields
                    .iter()
                    .filter(|descriptor| matches!(descriptor.field, Self::PotCarried(_)))
                    .map(|descriptor| descriptor.field.clone()),
            )
            .chain(
                fields
                    .iter()
                    .filter(|descriptor| matches!(descriptor.field, Self::PotChange(_)))
                    .map(|descriptor| descriptor.field.clone()),
            )
            .chain(
                fields
                    .iter()
                    .filter(|descriptor| matches!(descriptor.field, Self::Earmark(_)))
                    .map(|descriptor| descriptor.field.clone()),
            )
            .collect()
    }

    /// Returns the editor traversal order used by the monthly sheet.
    pub fn editor_fields(config: &AppConfig) -> Vec<Self> {
        configured_fields(config)
            .into_iter()
            .map(|descriptor| descriptor.field)
            .collect()
    }

    /// Reports whether the field accepts negative monetary values.
    pub fn allows_negative(&self) -> bool {
        matches!(
            self,
            Self::PreviousMonthSpendingCorrection | Self::PotChange(_)
        )
    }

    /// Returns the user-facing label for the field.
    pub fn label(&self, config: &AppConfig) -> String {
        match self {
            Self::Account(id) => config
                .account(id)
                .map(|account| account.label.clone())
                .unwrap_or_else(|| id.clone()),
            Self::PreviousMonthSpendingCorrection => "General spending over/under".to_owned(),
            Self::InvestmentNotYetSent => "Investment not yet sent".to_owned(),
            Self::Earmark(id) => config
                .earmark(id)
                .map(|item| item.label.clone())
                .unwrap_or_else(|| id.clone()),
            Self::PotCarried(id) => format!(
                "{} carried-over balance",
                config.pot(id).map(|pot| pot.label.as_str()).unwrap_or(id)
            ),
            Self::PotChange(id) => format!(
                "{} monthly change",
                config.pot(id).map(|pot| pot.label.as_str()).unwrap_or(id)
            ),
        }
    }

    /// Returns the short field name used in validation errors.
    pub fn labelless_name(&self) -> &'static str {
        match self {
            Self::Account(_) => "account balance",
            Self::PreviousMonthSpendingCorrection => "general spending over/under",
            Self::InvestmentNotYetSent => "investment not yet sent",
            Self::Earmark(_) => "next-month earmark",
            Self::PotCarried(_) => "pot carried-over balance",
            Self::PotChange(_) => "pot monthly change",
        }
    }

    /// Reads the current field value from the editable month document.
    pub fn current_value(&self, document: &MonthDocument) -> Money {
        match self {
            Self::Account(id) => Money::from_minor(*document.accounts.get(id).unwrap_or(&0)),
            Self::PreviousMonthSpendingCorrection => Money::from_minor(
                document
                    .timing_adjustments
                    .previous_month_spending_correction_raw,
            ),
            Self::InvestmentNotYetSent => {
                Money::from_minor(document.timing_adjustments.investment_not_yet_sent_raw)
            }
            Self::Earmark(id) => {
                Money::from_minor(*document.next_month_earmarks.get(id).unwrap_or(&0))
            }
            Self::PotCarried(id) => Money::from_minor(
                document
                    .savings_pots
                    .get(id)
                    .map(|pot| pot.carried_over)
                    .unwrap_or(0),
            ),
            Self::PotChange(id) => Money::from_minor(
                document
                    .savings_pots
                    .get(id)
                    .map(|pot| pot.monthly_change)
                    .unwrap_or(0),
            ),
        }
    }

    /// Formats the current field value for UI display.
    pub fn current_value_text(&self, document: &MonthDocument) -> String {
        format_minor_units(self.current_value(document).minor())
    }

    /// Returns the editor section that owns this field.
    pub fn section(&self) -> SectionId {
        match self {
            Self::Account(_) => SectionId::Accounts,
            Self::PreviousMonthSpendingCorrection | Self::InvestmentNotYetSent => {
                SectionId::TimingAdjustments
            }
            Self::Earmark(_) => SectionId::NextMonthEarmarks,
            Self::PotCarried(_) | Self::PotChange(_) => SectionId::SavingsPots,
        }
    }
}

/// Top-level sections visible in the monthly sheet.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SectionId {
    Accounts,
    TimingAdjustments,
    NextMonthEarmarks,
    SavingsPots,
}

impl SectionId {
    pub const ALL: [Self; 4] = [
        Self::Accounts,
        Self::TimingAdjustments,
        Self::NextMonthEarmarks,
        Self::SavingsPots,
    ];

    /// Full section title for boxed layouts.
    pub fn title(self) -> &'static str {
        match self {
            Self::Accounts => "Accounts",
            Self::TimingAdjustments => "Timing Adjustments",
            Self::NextMonthEarmarks => "Next Month Earmarks",
            Self::SavingsPots => "Savings Pots",
        }
    }

    /// Shortened section title for compact tab layouts.
    pub fn compact_title(self) -> &'static str {
        match self {
            Self::Accounts => "Accounts",
            Self::TimingAdjustments => "Timing",
            Self::NextMonthEarmarks => "Earmarks",
            Self::SavingsPots => "Pots",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct FieldDescriptor {
    field: FieldId,
}

impl FieldDescriptor {
    fn is_account_or_timing(&self) -> bool {
        matches!(
            self.field,
            FieldId::Account(_)
                | FieldId::PreviousMonthSpendingCorrection
                | FieldId::InvestmentNotYetSent
        )
    }
}

fn configured_fields(config: &AppConfig) -> Vec<FieldDescriptor> {
    let mut fields = config
        .accounts
        .iter()
        .map(|account| FieldDescriptor {
            field: FieldId::Account(account.id.clone()),
        })
        .collect::<Vec<_>>();

    fields.extend([
        FieldDescriptor {
            field: FieldId::PreviousMonthSpendingCorrection,
        },
        FieldDescriptor {
            field: FieldId::InvestmentNotYetSent,
        },
    ]);

    fields.extend(
        config
            .next_month_earmarks
            .iter()
            .map(|earmark| FieldDescriptor {
                field: FieldId::Earmark(earmark.id.clone()),
            }),
    );

    fields.extend(config.savings_pots.iter().flat_map(|pot| {
        [
            FieldDescriptor {
                field: FieldId::PotCarried(pot.id.clone()),
            },
            FieldDescriptor {
                field: FieldId::PotChange(pot.id.clone()),
            },
        ]
    }));

    fields
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use budget_core::AppConfig;

    use super::FieldId;

    #[test]
    fn editor_fields_follow_visible_layout_without_duplicates() {
        let config = AppConfig::default_mvp();
        let fields = FieldId::editor_fields(&config);
        assert_eq!(
            fields,
            vec![
                FieldId::Account("current_account".to_owned()),
                FieldId::Account("savings_account".to_owned()),
                FieldId::Account("credit_card_a".to_owned()),
                FieldId::Account("credit_card_b".to_owned()),
                FieldId::PreviousMonthSpendingCorrection,
                FieldId::InvestmentNotYetSent,
                FieldId::Earmark("subscriptions".to_owned()),
                FieldId::Earmark("general_spending".to_owned()),
                FieldId::PotCarried("travel_fund".to_owned()),
                FieldId::PotChange("travel_fund".to_owned()),
                FieldId::PotCarried("home_upkeep".to_owned()),
                FieldId::PotChange("home_upkeep".to_owned()),
                FieldId::PotCarried("emergency_buffer".to_owned()),
                FieldId::PotChange("emergency_buffer".to_owned()),
            ]
        );
        let unique = fields.iter().collect::<BTreeSet<_>>();
        assert_eq!(unique.len(), fields.len());
    }
}
