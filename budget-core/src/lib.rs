//! Core budgeting domain logic and persistence models.

/// Repository configuration types and validation rules.
pub mod config;
/// Domain error types for parsing, validation, and serialisation.
pub mod error;
/// Money parsing, formatting, and minor-unit utilities.
pub mod money;
/// Month identifiers, persisted documents, and derived calculations.
pub mod month;

pub use config::{AccountConfig, AccountKind, AppConfig, EarmarkConfig, SavingsPotConfig};
pub use error::BudgetError;
pub use money::{Money, format_minor_units, parse_money_input};
pub use month::{
    AccountRow, CalculatedMonth, DerivedCache, EarmarkRow, MonthDocument, MonthId, MonthMeta,
    PotRow, SavingsPotState, TimingAdjustments, TimingCalculation, Totals, ValidationState,
    calculate_month,
};
