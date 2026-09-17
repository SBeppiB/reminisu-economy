//! Finite category markets. Ordered allocation gives stable integer tie breaking.
use crate::{
    config::DemandConfig,
    config::{EconomicsConfig, MarketConfig},
    currency::CurrencyError,
    economy::EconomicState,
    faction::Faction,
    simulation::{SimpleRng, SimulationError},
};

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct MarketState {
    pub category: String,
    pub customers: u64,
    pub maximum_customers: u64,
    pub growth_bps: i32,
    pub growth_remainder: i64,
    pub available_demand: u64,
    pub units_sold: u64,
}
impl From<&MarketConfig> for MarketState {
    fn from(c: &MarketConfig) -> Self {
        Self {
            category: c.category.clone(),
            customers: c.annual_customers,
            maximum_customers: c.maximum_customers,
            growth_bps: c.growth_bps,
            growth_remainder: 0,
            available_demand: 0,
            units_sold: 0,
        }
    }
}
impl MarketState {
    pub fn advance(&mut self, economy: &EconomicState) {
        let change = i128::from(self.customers)
            * i128::from((self.growth_bps + economy.growth_bps).clamp(-9000, 10000))
            + i128::from(self.growth_remainder);
        let next = (i128::from(self.customers) + change / 10000)
            .clamp(0, i128::from(self.maximum_customers));
        self.growth_remainder = if next == 0 || next == i128::from(self.maximum_customers) {
            0
        } else {
            (change % 10000) as i64
        };
        self.customers = next as u64;
        self.available_demand =
            ((u128::from(self.customers) * u128::from(economy.demand_conditions_bps) / 10000)
                .min(u128::from(self.customers))) as u64;
        self.units_sold = 0;
    }
}

/// Proportional integer allocation with capacity constraints and stable remainder ties.
pub fn allocate(pool: u64, weights: &[u64], caps: &[u64]) -> Vec<u64> {
    assert_eq!(weights.len(), caps.len());
    let mut result = vec![0; weights.len()];
    let mut remaining = pool;
    while remaining > 0 {
        let total: u128 = weights
            .iter()
            .zip(caps)
            .zip(&result)
            .filter(|((w, c), r)| **w > 0 && c > r)
            .map(|((w, _), _)| u128::from(*w))
            .sum();
        if total == 0 {
            break;
        }
        let before = remaining;
        for i in 0..weights.len() {
            if result[i] < caps[i] && weights[i] > 0 {
                let share = (u128::from(before) * u128::from(weights[i]) / total) as u64;
                let granted = share.min(caps[i] - result[i]).min(remaining);
                result[i] += granted;
                remaining -= granted;
            }
        }
        if remaining == before {
            for i in 0..weights.len() {
                if remaining > 0 && weights[i] > 0 && result[i] < caps[i] {
                    result[i] += 1;
                    remaining -= 1;
                }
            }
        }
    }
    result
}

#[derive(Debug, Clone)]
pub struct Offer {
    pub company: usize,
    pub product: usize,
    pub market: usize,
    pub desired: u64,
    pub weight: u64,
    pub demand: u64,
    pub sales: u64,
}
pub fn forecast(
    companies: &[Faction],
    markets: &[MarketState],
    demand: DemandConfig,
    c: &EconomicsConfig,
    rng: &mut SimpleRng,
) -> Result<Vec<Offer>, SimulationError> {
    let mut offers = Vec::new();
    for (ci, company) in companies.iter().enumerate() {
        for (pi, p) in company.products.iter().enumerate() {
            let mi = markets
                .iter()
                .position(|m| m.category == p.category)
                .expect("validated category");
            let estimated = if company.bankrupt {
                0
            } else {
                p.estimate_demand(demand, rng)?
            };
            let backlog = u128::from(p.unmet_demand) * u128::from(c.backlog_percent) / 100;
            let recent = if p.previous_price.minor_units() == 0 {
                0
            } else {
                p.current_market_value
                    .checked_sub(p.previous_price)?
                    .ratio(100, p.previous_price.minor_units())?
                    .minor_units()
                    .clamp(-100, 100)
            };
            let recent_factor = (100
                - recent * i128::from(c.recent_price_sensitivity_percent) / 100)
                .max(0) as u128;
            let desired = if company.bankrupt {
                0
            } else {
                ((u128::from(estimated) + backlog) * recent_factor / 100)
                    .min(u128::from(markets[mi].available_demand)) as u64
            };
            let available = u128::from(p.inventory) + u128::from(p.production_capacity);
            let availability = (available * 100 / u128::from(p.previous_demand.max(1))).min(100);
            let availability_factor = 100 - u128::from(c.availability_percent)
                + availability * u128::from(c.availability_percent) / 100;
            let loyalty =
                10000 + u128::from(p.market_share_bps) * u128::from(c.loyalty_percent) / 100;
            let cheapest = companies
                .iter()
                .filter(|f| !f.bankrupt)
                .flat_map(|f| &f.products)
                .filter(|other| other.category == p.category)
                .map(|other| other.current_market_value)
                .min()
                .unwrap_or(p.current_market_value);
            let relative_price = if p.current_market_value.minor_units() == 0 {
                100
            } else {
                cheapest
                    .ratio(100, p.current_market_value.minor_units())?
                    .minor_units()
                    .clamp(0, 100) as u128
            };
            let price_factor =
                100 - (100 - relative_price) * demand.price_sensitivity_percent as u128 / 100;
            let weight = u128::from(desired) * u128::from(p.attractiveness_bps) / 10000
                * availability_factor
                * loyalty
                * price_factor
                / 100
                / 10000;
            offers.push(Offer {
                company: ci,
                product: pi,
                market: mi,
                desired,
                weight: u64::try_from(weight).map_err(|_| CurrencyError::Overflow)?,
                demand: 0,
                sales: 0,
            });
        }
    }
    for (mi, market) in markets.iter().enumerate() {
        let indices: Vec<_> = offers
            .iter()
            .enumerate()
            .filter(|(_, o)| o.market == mi)
            .map(|(i, _)| i)
            .collect();
        let weights: Vec<_> = indices.iter().map(|&i| offers[i].weight).collect();
        let caps: Vec<_> = indices.iter().map(|&i| offers[i].desired).collect();
        for (&i, count) in indices
            .iter()
            .zip(allocate(market.available_demand, &weights, &caps))
        {
            offers[i].demand = count;
        }
    }
    Ok(offers)
}

pub fn clear(offers: &mut [Offer], companies: &[Faction], markets: &mut [MarketState]) {
    for (mi, market) in markets.iter_mut().enumerate() {
        let indices: Vec<_> = offers
            .iter()
            .enumerate()
            .filter(|(_, o)| o.market == mi)
            .map(|(i, _)| i)
            .collect();
        let weights: Vec<_> = indices.iter().map(|&i| offers[i].weight).collect();
        let caps: Vec<_> = indices
            .iter()
            .map(|&i| {
                let o = &offers[i];
                companies[o.company].products[o.product]
                    .inventory
                    .min(o.desired)
            })
            .collect();
        for (&i, count) in indices
            .iter()
            .zip(allocate(market.available_demand, &weights, &caps))
        {
            offers[i].sales = count;
            market.units_sold += count;
        }
    }
}
