use thiserror::Error;

/// Domain and serialisation failures for the budgeting model.
#[derive(Debug, Error)]
pub enum BudgetError {
    #[error("invalid month id `{0}`, expected YYYY-MM")]
    InvalidMonthId(String),
    #[error("invalid monetary value `{0}`")]
    InvalidMoney(String),
    #[error("duplicate config id `{0}`")]
    DuplicateConfigId(String),
    #[error("missing `{section}` entry `{id}`")]
    MissingEntry { section: &'static str, id: String },
    #[error("unexpected `{section}` entry `{id}`")]
    UnexpectedEntry { section: &'static str, id: String },
    #[error("`{field}` must be non-negative")]
    NegativeValue { field: String },
    #[error(
        "savings pot `{id}` would go negative: carried_over={carried_over}, monthly_change={monthly_change}"
    )]
    NegativePotFinal {
        id: String,
        carried_over: i64,
        monthly_change: i64,
    },
    #[error("toml deserialisation failed: {0}")]
    TomlDeserialise(#[from] toml::de::Error),
    #[error("toml serialisation failed: {0}")]
    TomlSerialise(#[from] toml::ser::Error),
}
