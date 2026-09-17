use crate::{
    accounting::Accounts, config::EconomicsConfig, currency::Money, faction::Faction,
    simulation::SimulationError,
};

/// Borrow only the shortage, never beyond the explicit contractual credit ceiling.
pub fn fund(company: &mut Faction, needed: Money, a: &mut Accounts) -> Result<(), SimulationError> {
    if company.bankrupt {
        return Ok(());
    }
    let shortage = needed.checked_sub(company.cash)?.minor_units().max(0);
    let headroom = company
        .credit_limit
        .checked_sub(company.debt)?
        .minor_units()
        .max(0);
    let loan = Money::from_minor(company.primary_currency, shortage.min(headroom));
    company.debt = company.debt.checked_add(loan)?;
    company.receive(loan)?;
    a.debt_taken = a.debt_taken.checked_add(loan)?;
    Ok(())
}

pub fn settle(
    company: &mut Faction,
    a: &mut Accounts,
    c: &EconomicsConfig,
    interest_bps: u32,
) -> Result<(), SimulationError> {
    // Full-year interest on debt outstanding after production borrowing; rescue borrowing
    // to pay this interest starts accruing next year.
    a.interest_expense = company
        .debt
        .ratio(i128::from(interest_bps + c.credit_spread_bps), 10000)?;
    let bills = a
        .operating_expenses
        .checked_add(a.maintenance)?
        .checked_add(a.interest_expense)?;
    fund(company, bills, a)?;
    company.pay(bills)?;
    let reserve = company
        .base_operating_expenses
        .ratio(i128::from(c.cash_reserve_percent), 100)?;
    let surplus = Money::from_minor(
        company.primary_currency,
        company.cash.checked_sub(reserve)?.minor_units().max(0),
    );
    a.debt_repaid = company
        .debt
        .min(surplus.ratio(i128::from(c.repayment_percent), 100)?);
    company.pay(a.debt_repaid)?;
    company.debt = company.debt.checked_sub(a.debt_repaid)?;
    Ok(())
}

pub fn assess(company: &mut Faction, c: &EconomicsConfig) -> Result<(), SimulationError> {
    let distressed =
        company.cash.minor_units() < 0 || company.financial_totals()?.net_worth.minor_units() < 0;
    company.distress_years = if distressed {
        company.distress_years.saturating_add(1)
    } else {
        0
    };
    if c.bankruptcy_enabled && company.distress_years >= c.bankruptcy_after_years {
        company.bankrupt = true;
    }
    Ok(())
}
