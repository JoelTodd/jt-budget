mod calculation;
mod document;
mod id;
/// Compact summary projection types for UI-facing month overviews.
pub mod projection;

pub use calculation::{
    AccountRow, CalculatedMonth, EarmarkRow, PotRow, TimingCalculation, Totals, ValidationState,
    calculate_month,
};
pub use document::{DerivedCache, MonthDocument, MonthMeta, SavingsPotState, TimingAdjustments};
pub use id::MonthId;
pub use projection::{SummaryGroup, SummaryItem};

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::{DerivedCache, MonthDocument, MonthId, SavingsPotState, calculate_month};
    use crate::config::AppConfig;
    use crate::money::Money;

    fn sample_month() -> MonthDocument {
        let mut document = MonthDocument::new_draft(
            MonthId::parse("2026-03").unwrap(),
            &AppConfig::default_mvp(),
            None,
        );
        document
            .accounts
            .insert("current_account".to_owned(), 200_000);
        document
            .accounts
            .insert("savings_account".to_owned(), 40_000);
        document.accounts.insert("credit_card_a".to_owned(), 15_000);
        document.accounts.insert("credit_card_b".to_owned(), 5_000);
        document
            .timing_adjustments
            .previous_month_spending_correction_raw = -2_000;
        document.timing_adjustments.investment_not_yet_sent_raw = 18_000;
        document
            .next_month_earmarks
            .insert("subscriptions".to_owned(), 12_000);
        document
            .next_month_earmarks
            .insert("general_spending".to_owned(), 32_000);
        document.savings_pots.insert(
            "travel_fund".to_owned(),
            SavingsPotState {
                carried_over: 8_000,
                monthly_change: 9_000,
            },
        );
        document.savings_pots.insert(
            "home_upkeep".to_owned(),
            SavingsPotState {
                carried_over: 4_000,
                monthly_change: 5_500,
            },
        );
        document.savings_pots.insert(
            "emergency_buffer".to_owned(),
            SavingsPotState {
                carried_over: 2_000,
                monthly_change: 3_500,
            },
        );
        document
    }

    #[test]
    fn calculates_expected_totals() {
        let calculated = calculate_month(&AppConfig::default_mvp(), &sample_month()).unwrap();
        assert_eq!(
            calculated.totals.accounts_subtotal,
            Money::from_minor(220_000)
        );
        assert_eq!(
            calculated.totals.timing_adjustments_subtotal,
            Money::from_minor(-20_000)
        );
        assert_eq!(calculated.totals.net_available, Money::from_minor(200_000));
        assert_eq!(calculated.totals.total_allocated, Money::from_minor(76_000));
        assert_eq!(
            calculated.validation.overall_difference,
            Money::from_minor(124_000)
        );
        assert!(!calculated.validation.is_valid);
    }

    #[test]
    fn carries_forward_previous_final_balances() {
        let config = AppConfig::default_mvp();
        let previous = calculate_month(&config, &sample_month()).unwrap();
        let current =
            MonthDocument::new_draft(MonthId::parse("2026-04").unwrap(), &config, Some(&previous));
        assert_eq!(
            current.savings_pots["travel_fund"].carried_over,
            previous
                .pot_rows
                .iter()
                .find(|row| row.id == "travel_fund")
                .unwrap()
                .final_balance
                .minor()
        );
    }

    #[test]
    fn new_draft_prefills_earmarks_from_config_defaults() {
        let config = AppConfig::default_mvp();
        let draft = MonthDocument::new_draft(MonthId::parse("2026-04").unwrap(), &config, None);

        assert_eq!(draft.next_month_earmarks["subscriptions"], 12_000);
        assert_eq!(draft.next_month_earmarks["general_spending"], 32_000);
    }

    #[test]
    fn validation_is_inclusive_at_tolerance_edges() {
        let config = AppConfig::default_mvp();
        let mut document =
            MonthDocument::new_draft(MonthId::parse("2026-03").unwrap(), &config, None);
        document.accounts.insert("current_account".to_owned(), 100);
        document.accounts.insert("savings_account".to_owned(), 0);
        document.accounts.insert("credit_card_a".to_owned(), 0);
        document.accounts.insert("credit_card_b".to_owned(), 0);
        document
            .next_month_earmarks
            .insert("subscriptions".to_owned(), 0);
        document
            .next_month_earmarks
            .insert("general_spending".to_owned(), 0);
        document.savings_pots.insert(
            "travel_fund".to_owned(),
            SavingsPotState {
                carried_over: 0,
                monthly_change: 0,
            },
        );
        document.savings_pots.insert(
            "home_upkeep".to_owned(),
            SavingsPotState {
                carried_over: 0,
                monthly_change: 0,
            },
        );
        document.savings_pots.insert(
            "emergency_buffer".to_owned(),
            SavingsPotState {
                carried_over: 0,
                monthly_change: 0,
            },
        );

        let calculated = calculate_month(&config, &document).unwrap();
        assert!(calculated.validation.is_valid);
    }

    #[test]
    fn ignores_stored_derived_values_when_recomputing() {
        let config = AppConfig::default_mvp();
        let mut document = sample_month();
        document.derived = Some(DerivedCache {
            accounts_subtotal_minor: 0,
            timing_adjustments_subtotal_minor: 0,
            net_available_minor: 0,
            pots_carried_total_minor: 0,
            pots_monthly_change_total_minor: 0,
            pots_final_total_minor: 0,
            next_month_earmarks_subtotal_minor: 0,
            total_allocated_minor: 0,
            overall_difference_minor: 0,
            is_valid: true,
        });

        let toml = toml::to_string_pretty(&document).unwrap();
        let reparsed: MonthDocument = toml::from_str(&toml).unwrap();
        let calculated = calculate_month(&config, &reparsed).unwrap();
        assert_eq!(
            calculated.totals.accounts_subtotal,
            Money::from_minor(220_000)
        );
    }

    proptest! {
        #[test]
        fn arithmetic_invariants_hold(
            current_account in 0_i64..500_000,
            savings_account in 0_i64..500_000,
            credit_card_a in 0_i64..500_000,
            credit_card_b in 0_i64..500_000,
            previous_correction in -50_000_i64..50_000,
            investment in 0_i64..50_000,
            subscriptions in 0_i64..50_000,
            general_spending in 0_i64..100_000,
            carried_travel in 0_i64..200_000,
            carried_home in 0_i64..200_000,
            carried_emergency in 0_i64..200_000,
            change_travel in -50_000_i64..50_000,
            change_home in -50_000_i64..50_000,
            change_emergency in -50_000_i64..50_000,
        ) {
            prop_assume!(carried_travel + change_travel >= 0);
            prop_assume!(carried_home + change_home >= 0);
            prop_assume!(carried_emergency + change_emergency >= 0);

            let config = AppConfig::default_mvp();
            let mut document = MonthDocument::new_draft(MonthId::parse("2026-03").unwrap(), &config, None);
            document.accounts.insert("current_account".to_owned(), current_account);
            document.accounts.insert("savings_account".to_owned(), savings_account);
            document.accounts.insert("credit_card_a".to_owned(), credit_card_a);
            document.accounts.insert("credit_card_b".to_owned(), credit_card_b);
            document.timing_adjustments.previous_month_spending_correction_raw = previous_correction;
            document.timing_adjustments.investment_not_yet_sent_raw = investment;
            document.next_month_earmarks.insert("subscriptions".to_owned(), subscriptions);
            document.next_month_earmarks.insert("general_spending".to_owned(), general_spending);
            document.savings_pots.insert("travel_fund".to_owned(), SavingsPotState { carried_over: carried_travel, monthly_change: change_travel });
            document.savings_pots.insert("home_upkeep".to_owned(), SavingsPotState { carried_over: carried_home, monthly_change: change_home });
            document.savings_pots.insert("emergency_buffer".to_owned(), SavingsPotState { carried_over: carried_emergency, monthly_change: change_emergency });

            let calculated = calculate_month(&config, &document).unwrap();

            prop_assert_eq!(
                calculated.totals.accounts_subtotal,
                Money::from_minor(current_account + savings_account - credit_card_a - credit_card_b)
            );
            prop_assert_eq!(
                calculated.totals.timing_adjustments_subtotal,
                Money::from_minor(previous_correction - investment)
            );
            prop_assert_eq!(
                calculated.totals.pots_final_total,
                calculated.totals.pots_carried_total + calculated.totals.pots_monthly_change_total
            );
            prop_assert_eq!(
                calculated.validation.overall_difference,
                calculated.totals.accounts_subtotal
                    + calculated.totals.timing_adjustments_subtotal
                    - calculated.totals.total_allocated
            );
        }

        #[test]
        fn month_roundtrips_through_toml(
            current_account in 0_i64..200_000,
            savings_account in 0_i64..200_000,
            credit_card_a in 0_i64..200_000,
            credit_card_b in 0_i64..200_000,
            subscriptions in 0_i64..50_000,
            general_spending in 0_i64..50_000,
            carried in 0_i64..100_000,
            change in -10_000_i64..10_000,
        ) {
            prop_assume!(carried + change >= 0);

            let config = AppConfig::default_mvp();
            let mut document = MonthDocument::new_draft(MonthId::parse("2026-03").unwrap(), &config, None);
            document.accounts.insert("current_account".to_owned(), current_account);
            document.accounts.insert("savings_account".to_owned(), savings_account);
            document.accounts.insert("credit_card_a".to_owned(), credit_card_a);
            document.accounts.insert("credit_card_b".to_owned(), credit_card_b);
            document.next_month_earmarks.insert("subscriptions".to_owned(), subscriptions);
            document.next_month_earmarks.insert("general_spending".to_owned(), general_spending);
            document.savings_pots.insert("travel_fund".to_owned(), SavingsPotState { carried_over: carried, monthly_change: change });
            document.savings_pots.insert("home_upkeep".to_owned(), SavingsPotState { carried_over: carried, monthly_change: 0 });
            document.savings_pots.insert("emergency_buffer".to_owned(), SavingsPotState { carried_over: carried, monthly_change: 0 });

            let serialised = document.to_pretty_toml(&config).unwrap();
            let reparsed: MonthDocument = toml::from_str(&serialised).unwrap();
            prop_assert_eq!(document.month, reparsed.month);
            prop_assert_eq!(document.accounts, reparsed.accounts);
            prop_assert_eq!(document.next_month_earmarks, reparsed.next_month_earmarks);
            prop_assert_eq!(document.savings_pots, reparsed.savings_pots);
        }
    }
}
