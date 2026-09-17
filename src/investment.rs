use crate::{
    accounting::Accounts,
    config::EconomicsConfig,
    currency::Money,
    faction::Faction,
    simulation::{SimpleRng, SimulationError},
};

/// Investment uses retained cash, after bills and debt service; no guaranteed research payoff.
pub fn invest(
    company: &mut Faction,
    a: &mut Accounts,
    c: &EconomicsConfig,
    rng: &mut SimpleRng,
) -> Result<Vec<String>, SimulationError> {
    let mut expanded = Vec::new();
    let reserve = company
        .base_operating_expenses
        .ratio(i128::from(c.cash_reserve_percent), 100)?;
    for p in &mut company.products {
        if p.stockout && p.production_capacity < p.maximum_capacity && c.expansion_percent > 0 {
            let extra =
                ((u128::from(p.production_capacity) * u128::from(c.expansion_percent) / 100).max(1)
                    as u64)
                    .min(p.maximum_capacity - p.production_capacity);
            let cost = p
                .production_cost
                .checked_mul_quantity(extra)?
                .ratio(i128::from(c.expansion_cost_multiple), 1)?;
            if company.cash.checked_sub(reserve)?.minor_units() >= cost.minor_units()
                && cost.minor_units() > 0
            {
                company.cash = company.cash.checked_sub(cost)?;
                company.productive_assets = company.productive_assets.checked_add(cost)?;
                a.capital_spending = a.capital_spending.checked_add(cost)?;
                p.production_capacity += extra;
                expanded.push(p.name.clone());
            }
        }
    }
    let surplus = Money::from_minor(
        company.primary_currency,
        company.cash.checked_sub(reserve)?.minor_units().max(0),
    );
    let budget = surplus.ratio(i128::from(c.research_cash_percent), 100)?;
    let count = company.products.len() as i128;
    if count == 0 {
        return Ok(expanded);
    }
    let per_product = budget.ratio(1, count)?;
    for p in &mut company.products {
        if p.research_progress_bps >= c.research_limit_bps || per_product.minor_units() == 0 {
            continue;
        }
        company.cash = company.cash.checked_sub(per_product)?;
        a.research = a.research.checked_add(per_product)?;
        if rng.range_inclusive(0, 99) < i64::from(c.research_success_percent) {
            let remaining = c.research_limit_bps - p.research_progress_bps;
            let diminishing = u64::from(c.research_step_bps) * u64::from(remaining)
                / u64::from(c.research_limit_bps.max(1));
            // Spending relative to one unit's input cost limits progress from tiny budgets.
            let funding = per_product
                .ratio(10000, p.production_cost.minor_units().max(1))?
                .minor_units()
                .clamp(0, 10000) as u64;
            let gain = (diminishing * funding / 10000).min(u64::from(remaining)) as u32;
            p.research_progress_bps += gain;
            p.attractiveness_bps = 10000 + p.research_progress_bps;
        }
    }
    Ok(expanded)
}
