//! Core budgeting domain logic and persistence models.

pub mod config;
pub mod error;
pub mod money;
pub mod month;

pub use config::{AccountConfig, AccountKind, AppConfig, EarmarkConfig, SavingsPotConfig};
pub use error::BudgetError;
pub use money::{Money, format_minor_units, parse_money_input};
pub use month::{
    AccountRow, CalculatedMonth, DerivedCache, EarmarkRow, MonthDocument, MonthId, MonthMeta,
    PotRow, SavingsPotState, TimingAdjustments, TimingCalculation, Totals, ValidationState,
    calculate_month,
};
