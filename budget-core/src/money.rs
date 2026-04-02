use std::fmt;
use std::iter::Sum;
use std::ops::{Add, AddAssign, Neg, Sub, SubAssign};

use serde::{Deserialize, Serialize};

use crate::error::BudgetError;

/// Signed monetary amount stored as integer minor units.
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[serde(transparent)]
pub struct Money(i64);

impl Money {
    pub const ZERO: Self = Self(0);

    /// Creates a monetary amount from integer minor units.
    pub const fn from_minor(minor: i64) -> Self {
        Self(minor)
    }

    /// Returns the raw integer minor units backing this amount.
    pub const fn minor(self) -> i64 {
        self.0
    }

    /// Reports whether the amount is greater than or equal to zero.
    pub const fn is_non_negative(self) -> bool {
        self.0 >= 0
    }

    /// Returns the absolute value while preserving the minor-unit invariant.
    pub const fn abs(self) -> Self {
        Self(self.0.abs())
    }

    /// Formats the amount for terminal display.
    pub fn format(self) -> String {
        format_minor_units(self.0)
    }
}

impl Add for Money {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self(self.0 + rhs.0)
    }
}

impl AddAssign for Money {
    fn add_assign(&mut self, rhs: Self) {
        self.0 += rhs.0;
    }
}

impl Sub for Money {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        Self(self.0 - rhs.0)
    }
}

impl SubAssign for Money {
    fn sub_assign(&mut self, rhs: Self) {
        self.0 -= rhs.0;
    }
}

impl Neg for Money {
    type Output = Self;

    fn neg(self) -> Self::Output {
        Self(-self.0)
    }
}

impl Sum for Money {
    fn sum<I: Iterator<Item = Self>>(iter: I) -> Self {
        iter.fold(Self::ZERO, |acc, item| acc + item)
    }
}

impl fmt::Display for Money {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.format())
    }
}

/// Parse a terminal-edited money field into integer minor units.
///
/// The parser accepts the editable forms produced by the TUI, including a
/// leading `£`, thousands separators, and an optional decimal point.
///
/// # Errors
///
/// Returns [`BudgetError::InvalidMoney`] when the input is empty, malformed, or
/// negative when the field does not allow negative values.
pub fn parse_money_input(input: &str, allow_negative: bool) -> Result<Money, BudgetError> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(BudgetError::InvalidMoney(input.to_owned()));
    }

    // Ignore purely visual formatting so the parser can accept both pasted and
    // interactively edited values.
    let sanitised: String = trimmed
        .chars()
        .filter(|character| !matches!(character, '£' | ',' | ' ' | '_'))
        .collect();

    if sanitised.is_empty() {
        return Err(BudgetError::InvalidMoney(input.to_owned()));
    }

    let (negative, digits) = if let Some(rest) = sanitised.strip_prefix('-') {
        if !allow_negative {
            return Err(BudgetError::InvalidMoney(input.to_owned()));
        }
        (true, rest)
    } else if let Some(rest) = sanitised.strip_prefix('+') {
        (false, rest)
    } else {
        (false, sanitised.as_str())
    };

    let mut parts = digits.split('.');
    let pounds = parts.next().unwrap_or_default();
    let pence = parts.next();

    if parts.next().is_some() || pounds.is_empty() && pence.is_none() {
        return Err(BudgetError::InvalidMoney(input.to_owned()));
    }

    if !pounds.chars().all(|ch| ch.is_ascii_digit()) {
        return Err(BudgetError::InvalidMoney(input.to_owned()));
    }

    let pounds_value: i64 = pounds
        .parse()
        .map_err(|_| BudgetError::InvalidMoney(input.to_owned()))?;

    let pence_value = match pence {
        None => 0,
        Some(value) if value.chars().all(|ch| ch.is_ascii_digit()) && value.len() <= 2 => {
            match value.len() {
                0 => 0,
                1 => {
                    value
                        .parse::<i64>()
                        .map_err(|_| BudgetError::InvalidMoney(input.to_owned()))?
                        * 10
                }
                2 => value
                    .parse::<i64>()
                    .map_err(|_| BudgetError::InvalidMoney(input.to_owned()))?,
                _ => unreachable!("validated pence length"),
            }
        }
        _ => return Err(BudgetError::InvalidMoney(input.to_owned())),
    };

    let minor = pounds_value
        .checked_mul(100)
        .and_then(|value| value.checked_add(pence_value))
        .ok_or_else(|| BudgetError::InvalidMoney(input.to_owned()))?;

    Ok(if negative {
        Money::from_minor(-minor)
    } else {
        Money::from_minor(minor)
    })
}

/// Format minor units as a GBP-style display string.
pub fn format_minor_units(minor: i64) -> String {
    let sign = if minor < 0 { "-" } else { "" };
    let absolute = minor.abs();
    let pounds = absolute / 100;
    let pence = absolute % 100;
    format!("{sign}£{pounds}.{pence:02}")
}

#[cfg(test)]
mod tests {
    use super::{Money, parse_money_input};

    #[test]
    fn parses_unsigned_money() {
        let parsed = parse_money_input("£245.00", false).unwrap();
        assert_eq!(parsed, Money::from_minor(24_500));
    }

    #[test]
    fn parses_signed_money() {
        let parsed = parse_money_input("-1.25", true).unwrap();
        assert_eq!(parsed, Money::from_minor(-125));
    }

    #[test]
    fn rejects_negative_input_when_not_allowed() {
        assert!(parse_money_input("-1.00", false).is_err());
    }
}
