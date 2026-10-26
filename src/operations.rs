//! Company phases shared by Pilton and every competitor.
use crate::{
    accounting::Accounts,
    config::SimulationConfig,
    currency::Money,
    economy::EconomicState,
    faction::Faction,
    finance,
    history::{ProductHistoryRecord, YearHistoryRecord},
    investment,
    market::{MarketState, Offer},
    pricing,
    simulation::{EconomicEvent, EventType, Severity, SimpleRng, SimulationError},
};

pub fn event(
    year: i32,
    kind: EventType,
    company: &str,
    product: Option<&str>,
    value: Money,
) -> EconomicEvent {
    let severity = match kind {
        EventType::Bankruptcy | EventType::FinancialDistress | EventType::CashNegative => {
            Severity::Warning
        }
        _ => Severity::UnusualButValid,
    };
    let mut entities = vec![company.to_string()];
    if let Some(p) = product {
        entities.push(p.to_string());
    }
    EconomicEvent {
        year,
        description: match kind {
            EventType::ProductionSuspended => "Selling price no longer exceeds unit cost; new production is suspended.",
            EventType::ProductionEligible => "Selling price exceeds unit cost; production is eligible subject to funding and capacity.",
            EventType::CashNegative => "Cash became negative after obligations exceeded available funding.",
            EventType::Stockout => "Allocated customer demand exceeded available supply.",
            EventType::ExcessInventory => "Closing inventory crossed the configured excess-stock threshold.",
            EventType::MarketShareGain => "Sales share rose by the configured threshold since the last share event.",
            EventType::MarketShareLoss => "Sales share fell by the configured threshold since the last share event.",
            EventType::CapacityExpanded => "Cash-funded manufacturing capacity expansion completed for next year.",
            EventType::DebtTaken => "Company entered a borrowing period within its credit limit.",
            EventType::DebtRepaid => "Company entered a principal repayment period using surplus cash.",
            EventType::FinancialDistress => "Company entered financial distress through negative cash or negative net worth.",
            EventType::Bankruptcy => "Financial distress exceeded the grace period; company operations are frozen.",
            EventType::DemandShock => "A negative demand shock began in the global economy.",
            EventType::CostShock => "A temporary material-cost shock began in the global economy.",
        }.into(),
        event_type: kind,
        severity,
        entities,
        value,
    }
}

pub fn prepare(
    company: &mut Faction,
    economy: &EconomicState,
    config: &SimulationConfig,
) -> Result<Accounts, SimulationError> {
    let mut a = Accounts::new(company.primary_currency);
    if company.bankrupt {
        return Ok(a);
    }
    company.base_operating_expenses = company
        .base_operating_expenses
        .ratio(10000 + i128::from(economy.inflation_bps), 10000)?;
    a.operating_expenses = company.base_operating_expenses;
    a.depreciation = company
        .productive_assets
        .ratio(i128::from(config.economics.depreciation_percent), 100)?;
    company.productive_assets = company.productive_assets.checked_sub(a.depreciation)?;
    for p in &mut company.products {
        p.nominal_production_cost = p
            .nominal_production_cost
            .ratio(10000 + i128::from(economy.inflation_bps), 10000)?;
        p.base_market_value = p
            .base_market_value
            .ratio(10000 + i128::from(economy.inflation_bps), 10000)?;
        let material_shock =
            i128::from(economy.cost_shock_bps) * i128::from(p.input_share_percent) / 100;
        p.production_cost = p
            .nominal_production_cost
            .ratio(10000 + material_shock, 10000)?
            .ratio(10000 - i128::from(p.research_progress_bps), 10000)?;
        a.maintenance = a.maintenance.checked_add(
            p.production_cost
                .checked_mul_quantity(p.production_capacity)?
                .ratio(i128::from(config.economics.maintenance_bps), 10000)?,
        )?;
    }
    Ok(a)
}

pub fn produce(
    company: &mut Faction,
    offers: &[&Offer],
    a: &mut Accounts,
    config: &SimulationConfig,
) -> Result<Vec<(u64, Money, u64)>, SimulationError> {
    let mut results = Vec::new();
    for offer in offers {
        let p = &company.products[offer.product];
        let target = u64::try_from(
            u128::from(offer.demand) * u128::from(config.production.inventory_buffer_percent) / 100,
        )
        .map_err(|_| crate::currency::CurrencyError::Overflow)?;
        let units = if !company.bankrupt && p.current_market_value > p.production_cost {
            target
                .saturating_sub(p.inventory)
                .min(p.production_capacity)
        } else {
            0
        };
        let needed = p.production_cost.checked_mul_quantity(units)?;
        finance::fund(company, needed, a)?;
        let result = company.products[offer.product].produce(units, company.cash)?;
        company.pay(result.1)?;
        a.production_spending = a.production_spending.checked_add(result.1)?;
        // Record the decision already made above; no recalculation or RNG use.
        results.push((result.0, result.1, units));
    }
    Ok(results)
}

pub struct YearContext<'a> {
    pub economy: &'a EconomicState,
    pub markets: &'a [MarketState],
    pub config: &'a SimulationConfig,
    pub rng: &'a mut SimpleRng,
    pub year: i32,
    pub events: &'a mut Vec<EconomicEvent>,
}
struct ProductTrade<'a> {
    offer: &'a Offer,
    produced: (u64, Money, u64),
    competitor_price: Option<Money>,
}
fn settle_product(
    company_name: &str,
    p: &mut crate::technology::Product,
    was_bankrupt: bool,
    trade: ProductTrade<'_>,
    a: &mut Accounts,
    ctx: &mut YearContext<'_>,
) -> Result<ProductHistoryRecord, SimulationError> {
    let offer = trade.offer;
    let config = ctx.config;
    let markets = ctx.markets;
    let year = ctx.year;
    let events = &mut ctx.events;

    let old_inventory = p.inventory;
    let old_basis = p.inventory_cost_basis;
    let old_price = p.current_market_value;
    let (sold, revenue) = p.sell(offer.sales)?;
    p.previous_demand = offer.demand.max(sold);
    p.unmet_demand = offer.demand.saturating_sub(sold);
    if old_inventory.checked_sub(sold) != Some(p.inventory)
        || old_basis.checked_sub(p.last_cogs)? != p.inventory_cost_basis
        || revenue != old_price.checked_mul_quantity(sold)?
    {
        return Err(SimulationError::InvalidInput(
            "inventory conservation failed".into(),
        ));
    }
    let stockout = p.unmet_demand > 0;
    let excess = u128::from(p.inventory) * 100
        > u128::from(offer.demand.max(1)) * u128::from(config.economics.excess_inventory_percent);
    for (entered, kind) in [
        (stockout && !p.stockout, EventType::Stockout),
        (excess && !p.excess_inventory, EventType::ExcessInventory),
    ] {
        if entered {
            events.push(event(
                year,
                kind,
                company_name,
                Some(&p.name),
                p.inventory_cost_basis,
            ));
        }
    }
    p.stockout = stockout;
    p.excess_inventory = excess;
    let total = markets[offer.market].units_sold;
    p.market_share_bps = if total == 0 {
        0
    } else {
        (u128::from(sold) * 10000 / u128::from(total)) as u32
    };
    let delta = i64::from(p.market_share_bps) - i64::from(p.share_event_anchor_bps);
    if !was_bankrupt
        && delta.unsigned_abs() >= u64::from(config.economics.share_event_threshold_bps)
    {
        events.push(event(
            year,
            if delta > 0 {
                EventType::MarketShareGain
            } else {
                EventType::MarketShareLoss
            },
            company_name,
            Some(&p.name),
            revenue,
        ));
        p.share_event_anchor_bps = p.market_share_bps;
    }
    if !was_bankrupt {
        pricing::update(
            p,
            offer.demand,
            trade.competitor_price,
            &config.economics,
            config.demand,
        )?;
        let viable = p.current_market_value > p.production_cost;
        if p.production_viable != viable {
            events.push(event(
                year,
                if viable {
                    EventType::ProductionEligible
                } else {
                    EventType::ProductionSuspended
                },
                company_name,
                Some(&p.name),
                p.current_market_value,
            ));
        }
        p.production_viable = viable;
    }
    a.revenue = a.revenue.checked_add(revenue)?;
    a.cost_of_goods_sold = a.cost_of_goods_sold.checked_add(p.last_cogs)?;
    Ok(ProductHistoryRecord {
        requested_production: Some(trade.produced.2),
        name: p.name.clone(),
        category: p.category.clone(),
        units_produced: trade.produced.0,
        units_sold: sold,
        remaining_inventory: p.inventory,
        demand: offer.demand,
        market_value: p.current_market_value,
        revenue,
        cost_of_goods_sold: p.last_cogs,
        production_spending: trade.produced.1,
        inventory_cost_basis: p.inventory_cost_basis,
        production_cost: p.production_cost,
        production_capacity: p.production_capacity,
        capacity_utilization_bps: p.capacity_utilization_bps,
        market_share_bps: p.market_share_bps,
        research_progress_bps: p.research_progress_bps,
        unmet_demand: p.unmet_demand,
    })
}
fn settle_finances(
    company: &mut Faction,
    a: &mut Accounts,
    opening_cash: Money,
    ctx: &mut YearContext<'_>,
) -> Result<(), SimulationError> {
    let config = ctx.config;
    let economy = ctx.economy;
    let year = ctx.year;
    let rng = &mut ctx.rng;
    let events = &mut ctx.events;
    let was_bankrupt = company.bankrupt;
    company.receive(a.revenue)?;
    let old_distress = company.distress_years;
    if !was_bankrupt {
        finance::settle(company, a, &config.economics, economy.interest_bps)?;
        if company.cash.minor_units() >= 0 {
            for product in investment::invest(company, a, &config.economics, rng)? {
                events.push(event(
                    year,
                    EventType::CapacityExpanded,
                    &company.name,
                    Some(&product),
                    a.capital_spending,
                ));
            }
        }
        finance::assess(company, &config.economics)?;
    }
    if opening_cash.minor_units() >= 0 && company.cash.minor_units() < 0 {
        events.push(event(
            year,
            EventType::CashNegative,
            &company.name,
            None,
            company.cash,
        ));
    }
    if old_distress == 0 && company.distress_years > 0 {
        events.push(event(
            year,
            EventType::FinancialDistress,
            &company.name,
            None,
            company.cash,
        ));
    }
    if !was_bankrupt && company.bankrupt {
        events.push(event(
            year,
            EventType::Bankruptcy,
            &company.name,
            None,
            company.debt,
        ));
    }
    let borrowing = a.debt_taken.minor_units() > 0;
    let repaying = a.debt_repaid.minor_units() > 0;
    if borrowing && !company.borrowing {
        events.push(event(
            year,
            EventType::DebtTaken,
            &company.name,
            None,
            a.debt_taken,
        ));
    }
    if repaying && !company.repaying {
        events.push(event(
            year,
            EventType::DebtRepaid,
            &company.name,
            None,
            a.debt_repaid,
        ));
    }
    company.borrowing = borrowing;
    company.repaying = repaying;
    Ok(())
}
pub fn finish(
    company: &mut Faction,
    offers: &[&Offer],
    produced: &[(u64, Money, u64)],
    prices: &[Option<Money>],
    mut a: Accounts,
    opening_cash: Money,
    ctx: &mut YearContext<'_>,
) -> Result<YearHistoryRecord, SimulationError> {
    let mut records = Vec::new();
    for (i, offer) in offers.iter().enumerate() {
        let trade = ProductTrade {
            offer,
            produced: produced[i],
            competitor_price: prices[i],
        };
        records.push(settle_product(
            &company.name,
            &mut company.products[offer.product],
            company.bankrupt,
            trade,
            &mut a,
            ctx,
        )?);
    }
    settle_finances(company, &mut a, opening_cash, ctx)?;
    let year = ctx.year;
    let economy = ctx.economy;
    let markets = ctx.markets;
    a.close()?;
    if opening_cash.checked_add(a.cash_flow)? != company.cash {
        return Err(SimulationError::InvalidInput(
            "cash flow reconciliation failed".into(),
        ));
    }
    let totals = company.financial_totals()?;
    // Record closing capacity/research after investment, with utilization for this year's capacity.
    for (record, p) in records.iter_mut().zip(&company.products) {
        record.production_capacity = p.production_capacity;
        record.research_progress_bps = p.research_progress_bps;
    }
    Ok(YearHistoryRecord {
        year,
        faction_name: company.name.clone(),
        cash: company.cash,
        revenue: a.revenue,
        expenses: a.revenue.checked_sub(a.net_profit)?,
        profit: a.net_profit,
        net_worth: totals.net_worth,
        inventory_value: totals.inventory_value,
        technology_asset_value: totals.technology_asset_value,
        debt: company.debt,
        inventory_cost_basis: totals.inventory_cost_basis,
        bankrupt: company.bankrupt,
        distress_years: company.distress_years,
        accounts: a,
        products: records,
        economy: economy.clone(),
        markets: markets.to_vec(),
    })
}
