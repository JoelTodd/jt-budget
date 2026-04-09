mod calculation;
mod document;
mod id;
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
        document.accounts.insert("current".to_owned(), 200_000);
        document.accounts.insert("cash_isa".to_owned(), 50_000);
        document.accounts.insert("amex_credit".to_owned(), 20_000);
        document
            .accounts
            .insert("nationwide_credit".to_owned(), 10_000);
        document
            .timing_adjustments
            .previous_month_spending_correction_raw = -2_500;
        document.timing_adjustments.investment_not_yet_sent_raw = 24_500;
        document
            .next_month_earmarks
            .insert("subscriptions".to_owned(), 15_000);
        document
            .next_month_earmarks
            .insert("general_spending".to_owned(), 37_500);
        document.savings_pots.insert(
            "fun_expensive_stuff".to_owned(),
            SavingsPotState {
                carried_over: 10_000,
                monthly_change: 15_500,
            },
        );
        document.savings_pots.insert(
            "long_term_savings".to_owned(),
            SavingsPotState {
                carried_over: 5_000,
                monthly_change: 6_000,
            },
        );
        document.savings_pots.insert(
            "label".to_owned(),
            SavingsPotState {
                carried_over: 1_000,
                monthly_change: 2_500,
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
            Money::from_minor(-27_000)
        );
        assert_eq!(calculated.totals.net_available, Money::from_minor(193_000));
        assert_eq!(calculated.totals.total_allocated, Money::from_minor(92_500));
        assert_eq!(
            calculated.validation.overall_difference,
            Money::from_minor(100_500)
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
            current.savings_pots["fun_expensive_stuff"].carried_over,
            previous
                .pot_rows
                .iter()
                .find(|row| row.id == "fun_expensive_stuff")
                .unwrap()
                .final_balance
                .minor()
        );
    }

    #[test]
    fn new_draft_prefills_earmarks_from_config_defaults() {
        let config = AppConfig::default_mvp();
        let draft = MonthDocument::new_draft(MonthId::parse("2026-04").unwrap(), &config, None);

        assert_eq!(draft.next_month_earmarks["subscriptions"], 13_000);
        assert_eq!(draft.next_month_earmarks["general_spending"], 37_500);
    }

    #[test]
    fn validation_is_inclusive_at_tolerance_edges() {
        let config = AppConfig::default_mvp();
        let mut document =
            MonthDocument::new_draft(MonthId::parse("2026-03").unwrap(), &config, None);
        document.accounts.insert("current".to_owned(), 100);
        document.accounts.insert("cash_isa".to_owned(), 0);
        document.accounts.insert("amex_credit".to_owned(), 0);
        document.accounts.insert("nationwide_credit".to_owned(), 0);
        document
            .next_month_earmarks
            .insert("subscriptions".to_owned(), 0);
        document
            .next_month_earmarks
            .insert("general_spending".to_owned(), 0);
        document.savings_pots.insert(
            "fun_expensive_stuff".to_owned(),
            SavingsPotState {
                carried_over: 0,
                monthly_change: 0,
            },
        );
        document.savings_pots.insert(
            "long_term_savings".to_owned(),
            SavingsPotState {
                carried_over: 0,
                monthly_change: 0,
            },
        );
        document.savings_pots.insert(
            "label".to_owned(),
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
            current in 0_i64..500_000,
            cash_isa in 0_i64..500_000,
            amex in 0_i64..500_000,
            nationwide in 0_i64..500_000,
            previous_correction in -50_000_i64..50_000,
            investment in 0_i64..50_000,
            subscriptions in 0_i64..50_000,
            general_spending in 0_i64..100_000,
            carried_fun in 0_i64..200_000,
            carried_long in 0_i64..200_000,
            carried_label in 0_i64..200_000,
            change_fun in -50_000_i64..50_000,
            change_long in -50_000_i64..50_000,
            change_label in -50_000_i64..50_000,
        ) {
            prop_assume!(carried_fun + change_fun >= 0);
            prop_assume!(carried_long + change_long >= 0);
            prop_assume!(carried_label + change_label >= 0);

            let config = AppConfig::default_mvp();
            let mut document = MonthDocument::new_draft(MonthId::parse("2026-03").unwrap(), &config, None);
            document.accounts.insert("current".to_owned(), current);
            document.accounts.insert("cash_isa".to_owned(), cash_isa);
            document.accounts.insert("amex_credit".to_owned(), amex);
            document.accounts.insert("nationwide_credit".to_owned(), nationwide);
            document.timing_adjustments.previous_month_spending_correction_raw = previous_correction;
            document.timing_adjustments.investment_not_yet_sent_raw = investment;
            document.next_month_earmarks.insert("subscriptions".to_owned(), subscriptions);
            document.next_month_earmarks.insert("general_spending".to_owned(), general_spending);
            document.savings_pots.insert("fun_expensive_stuff".to_owned(), SavingsPotState { carried_over: carried_fun, monthly_change: change_fun });
            document.savings_pots.insert("long_term_savings".to_owned(), SavingsPotState { carried_over: carried_long, monthly_change: change_long });
            document.savings_pots.insert("label".to_owned(), SavingsPotState { carried_over: carried_label, monthly_change: change_label });

            let calculated = calculate_month(&config, &document).unwrap();

            prop_assert_eq!(
                calculated.totals.accounts_subtotal,
                Money::from_minor(current + cash_isa - amex - nationwide)
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
            current in 0_i64..200_000,
            cash_isa in 0_i64..200_000,
            amex in 0_i64..200_000,
            nationwide in 0_i64..200_000,
            subscriptions in 0_i64..50_000,
            general_spending in 0_i64..50_000,
            carried in 0_i64..100_000,
            change in -10_000_i64..10_000,
        ) {
            prop_assume!(carried + change >= 0);

            let config = AppConfig::default_mvp();
            let mut document = MonthDocument::new_draft(MonthId::parse("2026-03").unwrap(), &config, None);
            document.accounts.insert("current".to_owned(), current);
            document.accounts.insert("cash_isa".to_owned(), cash_isa);
            document.accounts.insert("amex_credit".to_owned(), amex);
            document.accounts.insert("nationwide_credit".to_owned(), nationwide);
            document.next_month_earmarks.insert("subscriptions".to_owned(), subscriptions);
            document.next_month_earmarks.insert("general_spending".to_owned(), general_spending);
            document.savings_pots.insert("fun_expensive_stuff".to_owned(), SavingsPotState { carried_over: carried, monthly_change: change });
            document.savings_pots.insert("long_term_savings".to_owned(), SavingsPotState { carried_over: carried, monthly_change: 0 });
            document.savings_pots.insert("label".to_owned(), SavingsPotState { carried_over: carried, monthly_change: 0 });

            let serialised = document.to_pretty_toml(&config).unwrap();
            let reparsed: MonthDocument = toml::from_str(&serialised).unwrap();
            prop_assert_eq!(document.month, reparsed.month);
            prop_assert_eq!(document.accounts, reparsed.accounts);
            prop_assert_eq!(document.next_month_earmarks, reparsed.next_month_earmarks);
            prop_assert_eq!(document.savings_pots, reparsed.savings_pots);
        }
    }
}
