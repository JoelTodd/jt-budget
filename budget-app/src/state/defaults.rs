use budget_core::{AppConfig, Money};

use super::{MoneyInput, NavigationState};

/// A monthly default, indexed in the configuration's display order.
#[derive(Clone, Copy, Debug)]
pub enum DefaultField {
    SavingsPot(usize),
    Earmark(usize),
}

impl DefaultField {
    /// Returns the configured display label for this field.
    pub fn label(self, config: &AppConfig) -> &str {
        match self {
            Self::SavingsPot(index) => &config.savings_pots[index].label,
            Self::Earmark(index) => &config.next_month_earmarks[index].label,
        }
    }

    /// Returns the group shown alongside the editable default.
    pub fn group(self) -> &'static str {
        match self {
            Self::SavingsPot(_) => "Savings contribution",
            Self::Earmark(_) => "Next-month earmark",
        }
    }

    /// Reads the default as integer minor units.
    pub fn value(self, config: &AppConfig) -> Money {
        Money::from_minor(match self {
            Self::SavingsPot(index) => config.savings_pots[index].default_monthly_change_minor,
            Self::Earmark(index) => config.next_month_earmarks[index].default_amount_minor,
        })
    }

    /// Updates only the selected monetary default.
    pub fn set_value(self, config: &mut AppConfig, value: Money) {
        match self {
            Self::SavingsPot(index) => {
                config.savings_pots[index].default_monthly_change_minor = value.minor();
            }
            Self::Earmark(index) => {
                config.next_month_earmarks[index].default_amount_minor = value.minor();
            }
        }
    }
}

/// Editable defaults for future months, separate from monthly documents.
#[derive(Clone, Debug)]
pub struct DefaultsState {
    pub config: AppConfig,
    pub fields: Vec<DefaultField>,
    pub focus_index: usize,
    pub edit_buffer: Option<MoneyInput>,
    pub message: Option<String>,
    pub return_navigation: Box<NavigationState>,
}

impl DefaultsState {
    /// Builds the defaults editor for all configured savings pots and earmarks.
    pub fn new(config: AppConfig, navigation: NavigationState) -> Self {
        let fields = (0..config.savings_pots.len())
            .map(DefaultField::SavingsPot)
            .chain((0..config.next_month_earmarks.len()).map(DefaultField::Earmark))
            .collect();
        Self {
            config,
            fields,
            focus_index: 0,
            edit_buffer: None,
            message: None,
            return_navigation: Box::new(navigation),
        }
    }
}
