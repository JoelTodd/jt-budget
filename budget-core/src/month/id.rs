use std::fmt;

use serde::{Deserialize, Serialize};

use crate::error::BudgetError;

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
