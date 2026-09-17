use crate::{config::EconomicsConfig, simulation::SimpleRng};

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct EconomicState {
    pub growth_bps: i32,
    pub inflation_bps: i32,
    pub interest_bps: u32,
    pub demand_conditions_bps: u32,
    pub cost_shock_bps: u32,
    pub demand_shock: bool,
    pub cost_shock: bool,
}
impl EconomicState {
    pub fn new(c: &EconomicsConfig) -> Self {
        Self {
            growth_bps: c.growth_bps,
            inflation_bps: c.inflation_bps,
            interest_bps: c.interest_bps,
            demand_conditions_bps: c.demand_conditions_bps,
            cost_shock_bps: 0,
            demand_shock: false,
            cost_shock: false,
        }
    }
    pub fn advance(&mut self, c: &EconomicsConfig, rng: &mut SimpleRng) {
        let noise = rng.range_inclusive(
            -i64::from(c.macro_volatility_bps),
            i64::from(c.macro_volatility_bps),
        ) as i32;
        self.growth_bps = (c.growth_bps + noise).clamp(-5000, 5000);
        self.inflation_bps = (c.inflation_bps + noise / 2).clamp(-5000, 5000);
        self.interest_bps =
            (i64::from(c.interest_bps) + i64::from(noise / 2)).clamp(0, 10000) as u32;
        self.demand_shock = rng.range_inclusive(0, 99) < i64::from(c.shock_probability_percent);
        self.cost_shock = rng.range_inclusive(0, 99) < i64::from(c.shock_probability_percent);
        self.cost_shock_bps = if self.cost_shock { c.shock_size_bps } else { 0 };
        self.demand_conditions_bps = (i64::from(c.demand_conditions_bps) + i64::from(noise)
            - if self.demand_shock {
                i64::from(c.shock_size_bps)
            } else {
                0
            })
        .clamp(0, 20000) as u32;
    }
}
