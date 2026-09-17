use crate::{
    config::{DemandConfig, EconomicsConfig},
    currency::{CurrencyError, Money},
    technology::Product,
};

pub fn update(
    p: &mut Product,
    demand: u64,
    competitor_price: Option<Money>,
    c: &EconomicsConfig,
    d: DemandConfig,
) -> Result<(), CurrencyError> {
    let old = p.current_market_value;
    let target = p
        .production_cost
        .ratio(100, i128::from(100 - c.desired_margin_percent))?;
    let cost_signal = if old.minor_units() == 0 {
        100
    } else {
        target
            .checked_sub(old)?
            .ratio(100, old.minor_units())?
            .minor_units()
            .clamp(-100, 100)
    };
    let competition = match competitor_price {
        Some(price) if old.minor_units() > 0 => price
            .checked_sub(old)?
            .ratio(100, old.minor_units())?
            .minor_units()
            .clamp(-100, 100),
        _ => 0,
    };
    let demand_signal = ((i128::from(demand) - i128::from(p.base_demand)) * 100
        / i128::from(p.base_demand.max(1)))
    .clamp(-100, 100);
    let inventory_signal = (i128::from(p.inventory) * 100 / i128::from(demand.max(1))).min(300);
    let change = (demand_signal
        + cost_signal * i128::from(c.cost_price_weight_percent) / 100
        + competition * i128::from(c.competitor_price_weight_percent) / 100
        - inventory_signal * i128::from(c.inventory_price_weight_percent) / 100
        + if p.stockout {
            i128::from(c.stockout_price_weight_percent)
        } else {
            0
        })
    .clamp(
        -i128::from(d.max_price_change_percent),
        i128::from(d.max_price_change_percent),
    );
    p.previous_price = old;
    p.current_market_value = old.ratio(100 + change, 100)?;
    Ok(())
}
