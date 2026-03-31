# Calculation Rules

## 1. Normalisation

Let all stored and calculated amounts be integer minor units.

### Accounts
For each account with raw user input `raw_balance`:

- `normalised_balance = raw_balance` for asset accounts
- `normalised_balance = -raw_balance` for liability accounts

### Timing adjustments
Use two distinct rules:

- `investment_not_yet_sent_effect = -investment_not_yet_sent_raw`
- `previous_month_spending_correction_effect = previous_month_spending_correction_raw`

`previous_month_spending_correction_raw` is genuinely signed:

- negative = reduce this month’s available money
- positive = increase this month’s available money

### Savings pots
For each pot:

- `carried_over` is non-negative
- `monthly_change` is signed
- `final_balance = carried_over + monthly_change`

Constraint:

- `final_balance >= 0`

### Next-month earmarks
Each earmark amount is non-negative.

## 2. Derived totals

### Accounts subtotal

    accounts_subtotal = Σ normalised_balance(account)

### Timing adjustments subtotal

    timing_adjustments_subtotal =
        investment_not_yet_sent_effect
      + previous_month_spending_correction_effect

Equivalent expanded form:

    timing_adjustments_subtotal =
        previous_month_spending_correction_raw
      - investment_not_yet_sent_raw

### Net available after timing

    net_available = accounts_subtotal + timing_adjustments_subtotal

### Savings pots totals

    pots_carried_total = Σ carried_over(pot)
    pots_monthly_change_total = Σ monthly_change(pot)
    pots_final_total = Σ final_balance(pot)

Invariant:

    pots_final_total = pots_carried_total + pots_monthly_change_total

### Next-month earmarks subtotal

    next_month_earmarks_subtotal = Σ earmark_amount(earmark)

### Total allocated

    total_allocated = pots_final_total + next_month_earmarks_subtotal

### Overall difference

    overall_difference = net_available - total_allocated

Interpretation:

- `overall_difference > 0` = money still unallocated
- `overall_difference < 0` = overallocated or shortfall
- `overall_difference = 0` = exactly balanced

## 3. Validation

Let:

    tolerance = validation_tolerance

Then:

    is_valid = (-tolerance <= overall_difference) && (overall_difference <= tolerance)

With the current product rule:

    tolerance = 100

So valid means:

    -100 <= overall_difference <= 100

## 4. Recalculation rules

Any change to any editable monetary field must trigger recomputation of, at minimum:

- affected `normalised_balance`
- `accounts_subtotal`
- `timing_adjustments_subtotal`
- `net_available`
- all affected `final_balance(pot)`
- `pots_carried_total`
- `pots_monthly_change_total`
- `pots_final_total`
- `next_month_earmarks_subtotal`
- `total_allocated`
- `overall_difference`
- `is_valid`

No derived total is authoritative if it disagrees with recomputation from editable fields.

## 5. Persistence rule for derived fields

If derived values are stored in month files for convenience, they are cache only.

On load:

1. parse editable fields
2. recompute all derived values
3. use recomputed values as truth

A stored derived value must never override current recomputation.

## 6. Sign and balance invariants

These must always hold:

    accounts_subtotal = Σ normalised_balance(account)
    timing_adjustments_subtotal = previous_month_spending_correction_raw - investment_not_yet_sent_raw
    pots_final_total = pots_carried_total + pots_monthly_change_total
    total_allocated = pots_final_total + next_month_earmarks_subtotal
    overall_difference = accounts_subtotal + timing_adjustments_subtotal - total_allocated

And per pot:

    final_balance = carried_over + monthly_change
    final_balance >= 0
