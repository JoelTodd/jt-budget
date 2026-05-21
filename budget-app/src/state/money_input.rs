use budget_core::{BudgetError, Money, MonthDocument, parse_money_input};

use super::field_catalog::FieldId;

/// Editable money buffer used by guided creation and the monthly sheet.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MoneyInput {
    base_minor: i64,
    edited_text: Option<String>,
    allow_negative: bool,
}

impl MoneyInput {
    pub fn from_value(value: Money, allow_negative: bool) -> Self {
        Self {
            base_minor: value.minor(),
            edited_text: None,
            allow_negative,
        }
    }

    /// Starts editing from a field's current persisted value.
    pub fn from_field(field: &FieldId, document: &MonthDocument) -> Self {
        Self::from_value(field.current_value(document), field.allows_negative())
    }

    /// Applies a single terminal keypress to the editable buffer.
    pub fn push(&mut self, character: char) {
        match character {
            '£' | '+' => {}
            '-' if self.allow_negative => {
                let text = self.ensure_text();
                if let Some(rest) = text.strip_prefix('-') {
                    *text = rest.to_owned();
                } else {
                    text.insert(0, '-');
                }
            }
            '.' => {
                let text = self.ensure_text();
                if text.contains('.') {
                    return;
                }
                if text.is_empty() || text == "-" {
                    text.push('0');
                }
                text.push('.');
            }
            digit if digit.is_ascii_digit() => {
                let text = self.ensure_text();
                if let Some((_, pence)) = text.split_once('.') {
                    if pence.len() >= 2 {
                        return;
                    }
                }
                match text.as_str() {
                    "0" => text.clear(),
                    "-0" => {
                        text.clear();
                        text.push('-');
                    }
                    _ => {}
                }
                text.push(digit);
            }
            _ => {}
        }
    }

    /// Deletes one character from the editable buffer.
    pub fn backspace(&mut self) {
        if self.edited_text.is_none() {
            let mut text = editable_text_from_minor(self.base_minor);
            text.pop();
            self.edited_text = Some(text);
            return;
        }

        if let Some(text) = &mut self.edited_text {
            text.pop();
        }
    }

    /// Returns the display form shown in the UI, including the currency prefix.
    pub fn display_text(&self) -> String {
        match &self.edited_text {
            None => budget_core::format_minor_units(self.base_minor),
            Some(text) => format_editable_money_text(text),
        }
    }

    /// Converts the current buffer into a committed money value.
    ///
    /// # Errors
    ///
    /// Returns [`BudgetError::InvalidMoney`] when the current edited text does
    /// not form a complete amount.
    pub fn commit_value(&self) -> Result<Money, BudgetError> {
        match self.edited_text.as_deref() {
            None => Ok(Money::from_minor(self.base_minor)),
            Some("") => Ok(Money::ZERO),
            Some("-") => Err(BudgetError::InvalidMoney("-".to_owned())),
            Some(text) => parse_money_input(text, self.allow_negative),
        }
    }

    /// Reports whether the user has diverged from the original field value.
    pub fn is_edited(&self) -> bool {
        self.edited_text.is_some()
    }

    fn ensure_text(&mut self) -> &mut String {
        self.edited_text.get_or_insert_with(String::new)
    }
}

fn editable_text_from_minor(minor: i64) -> String {
    let sign = if minor < 0 { "-" } else { "" };
    let absolute = minor.abs();
    format!("{sign}{}.{:02}", absolute / 100, absolute % 100)
}

fn format_editable_money_text(text: &str) -> String {
    let (negative, unsigned) = if let Some(rest) = text.strip_prefix('-') {
        (true, rest)
    } else {
        (false, text)
    };
    let sign = if negative { "-" } else { "" };

    if unsigned.is_empty() {
        return format!("{sign}£0.00");
    }

    let (pounds_text, pence_text) = unsigned.split_once('.').unwrap_or((unsigned, ""));
    let pounds = pounds_text.parse::<u64>().unwrap_or(0);
    let pence = match pence_text.len() {
        0 => "00".to_owned(),
        1 => format!("{}0", &pence_text[..1]),
        _ => pence_text[..2].to_owned(),
    };

    format!("{sign}£{pounds}.{pence}")
}

#[cfg(test)]
mod tests {
    use budget_core::{AppConfig, MonthDocument, MonthId};

    use super::MoneyInput;
    use crate::state::{FieldId, SectionId};

    #[test]
    fn money_input_uses_currency_as_non_editable_prefix() {
        let config = AppConfig::default_mvp();
        let document = MonthDocument::new_draft(MonthId::parse("2026-03").unwrap(), &config, None);
        let field = FieldId::Account("current_account".to_owned());
        let mut input = MoneyInput::from_field(&field, &document);

        for character in ['1', '8', '0'] {
            input.push(character);
        }
        assert_eq!(input.display_text(), "£180.00");

        input.push('£');
        assert_eq!(input.display_text(), "£180.00");

        input.backspace();
        assert_eq!(input.display_text(), "£18.00");
        input.push('.');
        input.push('5');
        assert_eq!(input.display_text(), "£18.50");
        assert_eq!(input.commit_value().unwrap().minor(), 1_850);
    }

    #[test]
    fn field_sections_follow_rendered_groups() {
        assert_eq!(
            FieldId::PotChange("travel_fund".to_owned()).section(),
            SectionId::SavingsPots
        );
    }
}
