//! UI-facing state types for the TUI state machine.
//!
//! These structures are intentionally plain data so the runtime and renderer
//! can exchange route state without hidden side effects.

use budget_core::{
    AppConfig, BudgetError, CalculatedMonth, Money, MonthDocument, MonthId, format_minor_units,
    parse_money_input,
};

/// Top-level application route.
#[derive(Clone, Debug)]
pub enum Route {
    Navigation(NavigationState),
    GuidedCreation(GuidedCreationState),
    MonthEditing(EditorState),
    BlockingFailure(FailureState),
    Shutdown,
}

/// State for the month list and its modal dialogues.
#[derive(Clone, Debug)]
pub struct NavigationState {
    pub months: Vec<MonthEntry>,
    pub selected: usize,
    pub dialogue: Option<NavigationDialogue>,
}

impl NavigationState {
    pub fn new(months: Vec<MonthEntry>) -> Self {
        Self {
            months,
            selected: 0,
            dialogue: None,
        }
    }

    pub fn selected_month(&self) -> Option<&MonthEntry> {
        self.months.get(self.selected)
    }
}

/// Navigation entry containing both editable and derived month data.
#[derive(Clone, Debug)]
pub struct MonthEntry {
    pub document: MonthDocument,
    pub calculated: CalculatedMonth,
}

/// Dialogue state for creating a new month.
#[derive(Clone, Debug)]
pub struct CreateDialogue {
    pub input: String,
    pub error: Option<String>,
}

/// Dialogue state for renaming an existing month.
#[derive(Clone, Debug)]
pub struct RenameDialogue {
    pub source: MonthId,
    pub input: String,
    pub error: Option<String>,
}

/// Dialogue state for deleting a month after explicit confirmation.
#[derive(Clone, Debug)]
pub struct DeleteDialogue {
    pub month: MonthId,
    pub confirmation: String,
    pub error: Option<String>,
}

/// Any modal dialogue that can appear from the navigation route.
#[derive(Clone, Debug)]
pub enum NavigationDialogue {
    Create(CreateDialogue),
    Rename(RenameDialogue),
    Delete(DeleteDialogue),
}

/// State for the guided month-creation workflow.
#[derive(Clone, Debug)]
pub struct GuidedCreationState {
    pub document: MonthDocument,
    pub calculated: CalculatedMonth,
    pub steps: Vec<FieldId>,
    pub step_index: usize,
    pub input: MoneyInput,
    pub message: Option<String>,
    pub persistence: PersistenceState,
    pub sync: SyncState,
}

/// State for the full monthly editor.
#[derive(Clone, Debug)]
pub struct EditorState {
    pub document: MonthDocument,
    pub calculated: CalculatedMonth,
    pub fields: Vec<FieldId>,
    pub focus_index: usize,
    pub edit_buffer: Option<MoneyInput>,
    pub message: Option<String>,
    pub interaction: InteractionState,
    pub persistence: PersistenceState,
    pub sync: SyncState,
}

/// Route state for failures that must block user progress until retried or quit.
#[derive(Clone, Debug)]
pub struct FailureState {
    pub title: String,
    pub message: String,
    pub retry: RetryTarget,
}

/// Operation that should be retried from the blocking failure screen.
#[derive(Clone, Debug)]
pub enum RetryTarget {
    RepositoryGate,
    CreateMonth(MonthId),
    CreateDraft(GuidedCreationState),
    GuidedSave(GuidedCreationState),
    EditorSave(EditorState),
    OpenMonth(MonthId),
    RenameMonth { source: MonthId, target: MonthId },
    DeleteMonth(MonthId),
    PushNavigation(Option<MonthId>),
}

/// Whether the monthly sheet is navigating fields or editing one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InteractionState {
    SheetIdle,
    FieldEditing,
}

/// Local persistence state for the current editor or guided draft.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PersistenceState {
    Clean,
    Dirty,
    Autosaving,
    SaveFailed,
}

/// Remote synchronisation state for the current editor or guided draft.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SyncState {
    SyncPending,
    Syncing,
    Synced,
    SyncFailed,
}

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
        let mut steps = Vec::new();
        for account in &config.accounts {
            steps.push(Self::Account(account.id.clone()));
        }
        steps.push(Self::PreviousMonthSpendingCorrection);
        steps.push(Self::InvestmentNotYetSent);
        for pot in &config.savings_pots {
            steps.push(Self::PotCarried(pot.id.clone()));
        }
        for pot in &config.savings_pots {
            steps.push(Self::PotChange(pot.id.clone()));
        }
        for earmark in &config.next_month_earmarks {
            steps.push(Self::Earmark(earmark.id.clone()));
        }
        steps
    }

    /// Returns the editor traversal order used by the monthly sheet.
    pub fn editor_fields(config: &AppConfig) -> Vec<Self> {
        let mut fields = Vec::new();
        for account in &config.accounts {
            fields.push(Self::Account(account.id.clone()));
        }
        fields.push(Self::PreviousMonthSpendingCorrection);
        fields.push(Self::InvestmentNotYetSent);
        for earmark in &config.next_month_earmarks {
            fields.push(Self::Earmark(earmark.id.clone()));
        }
        for pot in &config.savings_pots {
            fields.push(Self::PotCarried(pot.id.clone()));
            fields.push(Self::PotChange(pot.id.clone()));
        }
        fields
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
            None => format_minor_units(self.base_minor),
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
    use std::collections::BTreeSet;

    use budget_core::{AppConfig, MonthDocument, MonthId};

    use super::{FieldId, MoneyInput, SectionId};

    #[test]
    fn editor_fields_follow_visible_layout_without_duplicates() {
        let config = AppConfig::default_mvp();
        let fields = FieldId::editor_fields(&config);
        assert_eq!(
            fields,
            vec![
                FieldId::Account("current".to_owned()),
                FieldId::Account("cash_isa".to_owned()),
                FieldId::Account("amex_credit".to_owned()),
                FieldId::Account("nationwide_credit".to_owned()),
                FieldId::PreviousMonthSpendingCorrection,
                FieldId::InvestmentNotYetSent,
                FieldId::Earmark("subscriptions".to_owned()),
                FieldId::Earmark("general_spending".to_owned()),
                FieldId::PotCarried("fun_expensive_stuff".to_owned()),
                FieldId::PotChange("fun_expensive_stuff".to_owned()),
                FieldId::PotCarried("long_term_savings".to_owned()),
                FieldId::PotChange("long_term_savings".to_owned()),
                FieldId::PotCarried("label".to_owned()),
                FieldId::PotChange("label".to_owned()),
            ]
        );
        let unique = fields.iter().collect::<BTreeSet<_>>();
        assert_eq!(unique.len(), fields.len());
    }

    #[test]
    fn money_input_uses_currency_as_non_editable_prefix() {
        let config = AppConfig::default_mvp();
        let document = MonthDocument::new_draft(MonthId::parse("2026-03").unwrap(), &config, None);
        let field = FieldId::Account("current".to_owned());
        let mut input = MoneyInput::from_field(&field, &document);

        for character in ['2', '4', '5'] {
            input.push(character);
        }
        assert_eq!(input.display_text(), "£245.00");

        input.push('£');
        assert_eq!(input.display_text(), "£245.00");

        input.backspace();
        assert_eq!(input.display_text(), "£24.00");
        input.push('.');
        input.push('5');
        assert_eq!(input.display_text(), "£24.50");
        assert_eq!(input.commit_value().unwrap().minor(), 2_450);
    }

    #[test]
    fn field_sections_follow_rendered_groups() {
        assert_eq!(
            FieldId::PotChange("fun_expensive_stuff".to_owned()).section(),
            SectionId::SavingsPots
        );
    }
}
