use thiserror::Error;

/// Domain and serialisation failures for the budgeting model.
#[derive(Debug, Error)]
pub enum BudgetError {
    /// The supplied month identifier was not in valid `YYYY-MM` form.
    #[error("invalid month id `{0}`, expected YYYY-MM")]
    InvalidMonthId(String),
    /// The supplied money text could not be parsed into minor units.
    #[error("invalid monetary value `{0}`")]
    InvalidMoney(String),
    /// Two config entries shared the same stable identifier.
    #[error("duplicate config id `{0}`")]
    DuplicateConfigId(String),
    /// A required entry was missing from the persisted document.
    #[error("missing `{section}` entry `{id}`")]
    MissingEntry { section: &'static str, id: String },
    /// The persisted document contained an entry not present in config.
    #[error("unexpected `{section}` entry `{id}`")]
    UnexpectedEntry { section: &'static str, id: String },
    /// A field that must stay non-negative was set below zero.
    #[error("`{field}` must be non-negative")]
    NegativeValue { field: String },
    /// A savings pot would end the month below zero.
    #[error(
        "savings pot `{id}` would go negative: carried_over={carried_over}, monthly_change={monthly_change}"
    )]
    NegativePotFinal {
        id: String,
        carried_over: i64,
        monthly_change: i64,
    },
    /// TOML deserialisation failed while reading a config or month file.
    #[error("toml deserialisation failed: {0}")]
    TomlDeserialise(#[from] toml::de::Error),
    /// TOML serialisation failed while writing a config or month file.
    #[error("toml serialisation failed: {0}")]
    TomlSerialise(#[from] toml::ser::Error),
}
