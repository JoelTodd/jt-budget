# MVP Brief: Rust TUI Personal Budgeting App

## Product Goal

Build a single-user, terminal-based personal budgeting app in Rust for a monthly allocation workflow.

This is not a transaction tracker, not a bank sync tool, and not a daily spending app. The user does their budgeting once a month on payday. The job of the app is to make that monthly session fast, clear, safe, and easy to resume across machines.

The user’s budgeting model is zero-based allocation. They work out how much money they actually have, subtract liabilities, account for temporary timing issues, earmark money into known categories, and aim for the final difference to be effectively zero. If there is extra money, they manually decide where it goes. If there is a shortfall, they manually decide what to reduce. That reshuffling is intentionally manual.

## Core Budgeting Model

The app should model the user’s real process, which is:

1. Enter headline account balances.
2. Derive how much money is actually available.
3. Apply timing adjustments.
4. Confirm carried-over pot balances from last month.
5. Add this month’s contributions and next-month earmarks.
6. Manually reshuffle if over or under.
7. Validate that the month balances within tolerance.
8. Save automatically as they go.

### Headline Accounts

The MVP must support individual accounts, not vague grouped totals.

Initial accounts:

- Current account
- Savings account
- Credit card A
- Credit card B

Balances are entered as positive amounts by the user. The app applies the sign rules internally.

Expected sign behaviour:

- Asset accounts like Current account and Savings account are positive
- Liability accounts like Credit card A and Credit card B are negative

The app should show a subtle baked-in sign cue beside each field so the user never has to type negative numbers.

### Net Position

The app should derive a top-line net position from accounts:

`net position = current account + savings account - credit card A - credit card B`

This is the user’s headline “how much money do I actually have” number.

## Monthly Sections

The edit screen should be split into these sections:

1. Accounts
2. Timing Adjustments
3. Next Month Earmarks
4. Savings Pots
5. Validation

The app should show both section subtotals and one live overall difference number at all times. The section subtotals help the user orient themselves. The live difference number is the main balancing signal.

## Timing Adjustments

This section exists for temporary real-world mismatches.

The MVP needs two timing adjustment concepts:

### 1. Investment Not Yet Sent

The user contributes **£180/month** to an investment account, but that investment account is normally **outside** the zero-sum budget model.

That means the investment contribution is normally off-book.

The only time it appears in the budget is when the £180 has not yet actually left the current account. In that case, a timing adjustment is used so the budget reflects reality.

### 2. General Spending Over/Under

The user tracks day-to-day general spending outside the app. If they over- or under-spent that earmark last month, the correction comes into this month through the timing adjustment section.

This needs to be an editable signed value.

Timing adjustments are not automated. They are user-entered fields.

## Next Month Earmarks

This section is for money reserved for the upcoming month’s routine spending.

For MVP it needs:

- Subscriptions
- General spending

These amounts should prefill from config every month and remain editable.

### General Spending Context

The user currently earmarks **£320** for general spending and tracks the day-by-day burn externally using a calendar trick. That daily tracking stays outside the app in V1.

The app only needs to treat general spending as one earmarked amount.

No daily allowance logic is needed in the MVP.

## Savings Pots

The app needs named savings pots with carry-forward balances and monthly contributions.

Initial pots:

- Travel fund
- Home upkeep
- Emergency buffer

Initial fixed monthly contributions:

- Travel fund: **£90**
- Home upkeep: **£55**
- Emergency buffer: **£35**

### How Pots Should Work

For each savings pot, the edit screen should show:

- carried-over balance
- monthly change / contribution
- resulting final balance

The monthly change should default to the configured contribution amount, but remain editable.

This split is important. The user may have changed pot balances mid-month outside the app, so showing only a final value is too opaque. The carried-over value and the monthly change must both be visible and editable.

### Carry-Forward Behaviour

When a new month is created, pot balances should be copied forward from the previous month, but the user must be prompted to confirm or edit them before continuing.

This is a deliberate safety feature because pots may have changed during the month.

## Validation and Balancing Rules

The budgeting target is zero-based, but the app should allow a small tolerance.

### Validation Rule

A month is considered valid if the final overall difference is between **-£1.00 and +£1.00** inclusive.

The user wants strict enforcement for validation, but also wants drafts.

### Drafts

Drafts should autosave continuously while editing.

The user may budget in interrupted sessions, often while at work, so resuming a partially completed month is part of the core product.

### Finalising

“Finalising” a month in MVP should mean “this month currently passes validation”, not “this month is locked forever”.

A finalised month should still be editable later.

## Monthly Workflow

### Creating a New Month

New months are created manually. The app should not auto-create them based on date.

The month should be named by the payday month, for example:

- March 2026
- April 2026

The specific payday date does not need operational meaning in the app. It does not need reminders, overdue logic, or date-based warnings.

### Initial Flow for a New Month

When creating a month, the app should start with a guided flow in this order:

1. Current account balance
2. Savings account balance
3. Credit card balances
4. General spending over/under
5. Investment timing adjustment
6. Pot carried-over balances
7. Fixed monthly pot contributions
8. Next month subscriptions
9. Next month general spending

That sequence should be the default guided creation order because it matches the user’s real mental workflow.

### After Guided Creation

Once the initial guided setup for the month is done, the user should land on a full-screen monthly sheet showing everything at once.

This full monthly sheet is the main editing surface.

It must support rapid manual reshuffling because when the budget is over or under, the user wants to adjust pots freely based on current priorities.

There is no fixed priority order for reshuffling. The app must not try to suggest or automate that. It is entirely manual.

## Editing Model

The user only wants to actively use the app once a month. Mid-month detailed maintenance is not acceptable.

So V1 must **not** require:

- transaction entry
- event logging
- transfer histories
- per-spend tracking
- detailed audit trails of manual reshuffles

If the user has moved money between pots during the month, they should simply correct the carried-over pot values when starting the next month, or edit the month manually if needed.

That is enough for MVP.

## History View

History matters, but lightly.

The user does not often review past months, but wants the option.

The MVP should include a month list with:

- compact month summaries
- ability to expand/open any month
- ability to edit past months
- explicit rename and delete actions

### Summary Design

The summary should stay compact enough to work in smaller terminals. It should show one total per major section, not a wall of detail.

Recommended summary groups:

- Accounts total
- Timing Adjustments total
- Next Month Earmarks total
- Savings Pots total
- Final Check
  - total allocated
  - overall difference
  - valid / invalid status if useful

That is enough to jog the user’s memory without turning history into a second copy of the edit screen.

## Config

There should be no settings screen in the MVP.

The app should use a reasonably comprehensive config file.

A config file is sufficient and preferable for a Rust TUI of this kind.

### Config Should Define

At minimum:

- account definitions
- account types or sign behaviour
- display order of accounts
- section order for guided flow
- section order for full edit screen
- savings pot names
- summary groupings
- default/fixed monthly contribution values
- default next-month earmark values
- validation tolerance
- which values are carried forward
- which values require monthly confirmation
- labels and display names

### Config Should Not Store

Actual monthly data should not live in config.

Config is for structure and defaults only.

## Data Storage and Sync

The user wants to start budgeting on one machine and continue on another with minimal friction.

The right MVP answer is plain files in a Git-backed data store, with Git handled by the app.

This should be treated as a sync mechanism, not as part of the user workflow.

### Requirements

- Single-user only
- One budget only
- No multi-profile support
- No multi-user collaboration
- Data stored in plain inspectable files
- Git used for syncing between machines
- App handles pull/rebase, commit, and push automatically
- User should not need to manually juggle Git in normal use

### Sync Failure Behaviour

If sync goes wrong, the app should **refuse to continue** and show a clear sync error.

It should not silently allow divergent local editing.  
It should not maintain local vs remote draft branches for the user to resolve later.  
Money data should be stubborn.

### Autosave and Sync Interaction

Autosave is required, but it must not create a noisy or fragile sync system.

A sensible implementation would autosave locally as the source of truth for the current session, then sync opportunistically at safe points such as:

- opening the app
- creating a month
- leaving a month
- explicit refresh/reload moments
- perhaps debounced after meaningful edit bursts

The exact implementation detail is up to the team, but the user experience should feel like one continuous budget state across machines.

## Currency and Precision

All amounts should support pence.

Store amounts internally as integer pence.  
Display them in pounds and pence.

Do not use floating point for money.

## UX Expectations for the TUI

The app should feel fast, plain, and opinionated.

The user is not asking for dashboards, charts, or clever coaching. The app’s value is speed and reliability.

### The TUI Should Support

- guided month creation
- one full-screen editable month sheet
- clear section boundaries
- inline editing of all key values
- live recalculation
- section subtotals
- live overall difference
- obvious validation status
- obvious autosave status
- clear sync status
- easy reopening of past months

### The TUI Should Not Try to Do

- advice
- financial nudging
- bank import
- daily spend tracking
- transaction logs
- category analytics
- forecasts
- mobile-style personal finance gimmicks

## Suggested Data Model

A month record should contain, at minimum:

- month identifier, for example `2026-03`
- display label, for example `March 2026`
- status, such as draft / valid
- account balances by account
- derived net position
- timing adjustments
- next-month earmarks
- savings pots with:
  - carried-over amount
  - monthly change
  - resulting final amount
- section subtotals
- overall difference
- timestamps for created/updated

The history view can derive its compact summary from the month record rather than storing a second bespoke summary blob.

## Domain Rules the App Must Respect

1. This is a monthly budgeting app, not an always-open ledger.
2. The user enters balances manually.
3. Headline accounts are the foundation.
4. Credit cards are liabilities.
5. Investment contribution is normally outside the budget.
6. Investment only appears through a timing adjustment when the money has not left the current account yet.
7. Previous month overspend or underspend in general spending also enters through timing adjustments.
8. Savings pots carry forward month to month, but must be confirmed or edited each new month.
9. Monthly savings contributions prefill from config and remain editable.
10. Next-month earmarks prefill from config and remain editable.
11. Reshuffling between pots is fully manual.
12. Validation tolerance is ±£1.
13. Drafts autosave.
14. “Finalised” does not mean locked.
15. Sync failure blocks further editing until resolved.

## Explicitly Out of Scope for MVP

- transaction import
- Open Banking
- daily general spending tracker
- calendar integration
- investment account tracking inside the budget model
- automated recommendations on how to balance a shortfall or surplus
- per-month locking or version approval flow
- multiple users
- multiple budgets
- cloud backend service
- GUI settings screen
- detailed change history within a month

## What Success Looks Like

A successful MVP lets the user:

- open the app on either machine
- have their budget sync cleanly
- create a new month manually
- be guided through the exact fields they actually think about
- land on one sheet with everything visible
- adjust pots manually until the difference is close enough to zero
- stop halfway and safely resume later
- review old months when needed
- trust the app not to let sync weirdness or hidden maths corrupt the budget

That is the right MVP. Anything fancier too early would be classic personal finance app disease: more machinery, less usefulness.
