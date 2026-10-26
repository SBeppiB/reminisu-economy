use crate::asset::InventoryLine;
use crate::config::DemandConfig;
use crate::currency::{CurrencyError, Money};
use crate::simulation::{SimpleRng, SimulationError};

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ProductConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub starting_inventory_basis: Option<Money>,
    #[serde(default)]
    pub category: String,
    #[serde(default = "default_input_share")]
    pub input_share_percent: u32,
    #[serde(default)]
    pub maximum_capacity: Option<u64>,
    pub name: String,
    pub production_cost: Money,
    pub starting_market_value: Money,
    pub starting_inventory: u64,
    pub base_demand: u64,
    pub production_capacity: u64,
    pub popularity_percent: u32,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct Product {
    pub category: String,
    pub input_share_percent: u32,
    pub maximum_capacity: u64,
    pub inventory_cost_basis: Money,
    pub last_cogs: Money,
    pub nominal_production_cost: Money,
    pub previous_price: Money,
    pub unmet_demand: u64,
    pub market_share_bps: u32,
    pub research_progress_bps: u32,
    pub attractiveness_bps: u32,
    pub capacity_utilization_bps: u32,
    pub stockout: bool,
    pub excess_inventory: bool,
    pub share_event_anchor_bps: u32,
    pub production_viable: bool,
    pub name: String,
    pub inventory: u64,
    pub production_cost: Money,
    pub base_market_value: Money,
    pub current_market_value: Money,
    pub base_demand: u64,
    pub production_capacity: u64,
    pub popularity_percent: u32,
    pub previous_demand: u64,
    pub lifetime_units_produced: u64,
    pub lifetime_units_sold: u64,
    pub lifetime_revenue: Money,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct ProductYearResult {
    pub name: String,
    pub demand: u64,
    pub units_produced: u64,
    pub units_sold: u64,
    pub remaining_inventory: u64,
    pub production_expense: Money,
    pub revenue: Money,
    pub market_value: Money,
}

impl Product {
    pub fn from_config(config: ProductConfig) -> Result<Self, SimulationError> {
        if let Some(basis) = config.starting_inventory_basis
            && (basis.currency() != config.production_cost.currency()
                || basis.minor_units() < 0
                || (config.starting_inventory == 0 && basis.minor_units() != 0))
        {
            return Err(SimulationError::InvalidInput(format!(
                "{}: Inventory basis must be nonnegative, use the product currency, and be zero for empty inventory.",
                config.name
            )));
        }
        if config.production_cost.minor_units() < 0 {
            return Err(SimulationError::InvalidInput(format!(
                "{} has negative production cost",
                config.name
            )));
        }
        if config.starting_market_value.minor_units() < 0 {
            return Err(SimulationError::InvalidInput(format!(
                "{} has negative market value",
                config.name
            )));
        }
        if config.production_cost.currency() != config.starting_market_value.currency() {
            return Err(SimulationError::InvalidInput(format!(
                "{} production cost and market value use different currencies",
                config.name
            )));
        }

        Ok(Self {
            category: if config.category.is_empty() {
                config.name.clone()
            } else {
                config.category
            },
            input_share_percent: config.input_share_percent,
            maximum_capacity: config
                .maximum_capacity
                .unwrap_or(config.production_capacity.saturating_mul(4)),
            inventory_cost_basis: match config.starting_inventory_basis {
                Some(basis) => basis,
                None => config
                    .production_cost
                    .checked_mul_quantity(config.starting_inventory)?,
            },
            last_cogs: Money::zero(config.production_cost.currency()),
            nominal_production_cost: config.production_cost,
            previous_price: config.starting_market_value,
            unmet_demand: 0,
            market_share_bps: 0,
            research_progress_bps: 0,
            attractiveness_bps: 10000,
            capacity_utilization_bps: 0,
            stockout: false,
            excess_inventory: false,
            share_event_anchor_bps: 0,
            production_viable: config.starting_market_value > config.production_cost,
            name: config.name,
            inventory: config.starting_inventory,
            production_cost: config.production_cost,
            base_market_value: config.starting_market_value,
            current_market_value: config.starting_market_value,
            base_demand: config.base_demand,
            production_capacity: config.production_capacity,
            popularity_percent: config.popularity_percent,
            previous_demand: config.base_demand,
            lifetime_units_produced: 0,
            lifetime_units_sold: 0,
            lifetime_revenue: Money::zero(config.starting_market_value.currency()),
        })
    }

    pub fn estimate_demand(
        &self,
        config: DemandConfig,
        rng: &mut SimpleRng,
    ) -> Result<u64, SimulationError> {
        let variation = rng.range_inclusive(
            -i64::from(config.yearly_variation_percent),
            i64::from(config.yearly_variation_percent),
        );
        let popularity_adjusted =
            (i128::from(self.base_demand) * i128::from(self.popularity_percent)) / 100;
        let momentum = (i128::from(self.previous_demand) - i128::from(self.base_demand))
            * i128::from(config.momentum_percent)
            / 100;

        // Higher prices soften demand, but the penalty is intentionally mild
        // in the MVP so random noise cannot dominate the product fundamentals.
        let price_delta = self
            .current_market_value
            .checked_sub(self.base_market_value)?
            .minor_units();
        let price_penalty = if self.base_market_value.minor_units() == 0 {
            0
        } else {
            i128::from(config.price_sensitivity_percent)
                .checked_mul(i128::from(self.base_demand))
                .and_then(|v| v.checked_mul(price_delta))
                .ok_or(CurrencyError::Overflow)?
                / self.base_market_value.minor_units()
                / 100
        };

        let varied = popularity_adjusted * (100 + i128::from(variation)) / 100;
        let demand = varied
            .checked_add(momentum)
            .and_then(|v| v.checked_sub(price_penalty))
            .ok_or(CurrencyError::Overflow)?;
        u64::try_from(demand.max(0)).map_err(|_| CurrencyError::Overflow.into())
    }

    pub fn produce(
        &mut self,
        requested_units: u64,
        available_cash: Money,
    ) -> Result<(u64, Money), SimulationError> {
        if available_cash.currency() != self.production_cost.currency() {
            return Err(SimulationError::Currency(CurrencyError::CurrencyMismatch {
                left: available_cash.currency(),
                right: self.production_cost.currency(),
            }));
        }

        let affordable_units = if self.production_cost.minor_units() == 0 {
            requested_units
        } else {
            (available_cash.minor_units() / self.production_cost.minor_units())
                .clamp(0, i128::from(u64::MAX)) as u64
        };
        let units = requested_units
            .min(self.production_capacity)
            .min(affordable_units);
        let expense = self.production_cost.checked_mul_quantity(units)?;
        self.inventory_cost_basis = self.inventory_cost_basis.checked_add(expense)?;
        self.capacity_utilization_bps = if self.production_capacity == 0 {
            0
        } else {
            (u128::from(units) * 10000 / u128::from(self.production_capacity)) as u32
        };
        self.inventory = self
            .inventory
            .checked_add(units)
            .ok_or(SimulationError::InvalidInput(
                "product inventory overflow".to_string(),
            ))?;
        self.lifetime_units_produced = self
            .lifetime_units_produced
            .checked_add(units)
            .ok_or(CurrencyError::Overflow)?;
        Ok((units, expense))
    }

    pub fn sell(&mut self, demand: u64) -> Result<(u64, Money), SimulationError> {
        let units = demand.min(self.inventory);
        let revenue = self.current_market_value.checked_mul_quantity(units)?;
        self.last_cogs = if self.inventory == 0 {
            Money::zero(self.production_cost.currency())
        } else {
            self.inventory_cost_basis
                .ratio(i128::from(units), i128::from(self.inventory))?
        };
        self.inventory_cost_basis = self.inventory_cost_basis.checked_sub(self.last_cogs)?;
        self.inventory -= units;
        self.previous_demand = demand;
        self.lifetime_units_sold = self
            .lifetime_units_sold
            .checked_add(units)
            .ok_or(CurrencyError::Overflow)?;
        self.lifetime_revenue = self.lifetime_revenue.checked_add(revenue)?;
        Ok((units, revenue))
    }

    pub fn update_market_value(
        &mut self,
        demand: u64,
        config: DemandConfig,
    ) -> Result<(), SimulationError> {
        if self.base_demand == 0 || config.max_price_change_percent <= 0 {
            return Ok(());
        }

        let demand_gap_percent = (i128::from(demand) - i128::from(self.base_demand)) * 100
            / i128::from(self.base_demand);
        let bounded_gap = demand_gap_percent.clamp(
            -i128::from(config.max_price_change_percent),
            i128::from(config.max_price_change_percent),
        );
        let new_minor = self
            .current_market_value
            .minor_units()
            .checked_mul(100 + bounded_gap)
            .ok_or(CurrencyError::Overflow)?
            / 100;
        self.current_market_value =
            Money::from_minor(self.current_market_value.currency(), new_minor.max(0));
        Ok(())
    }

    pub fn inventory_line(&self) -> InventoryLine {
        InventoryLine {
            product_name: self.name.clone(),
            quantity: self.inventory,
            unit_value: self.current_market_value,
        }
    }
}

fn default_input_share() -> u32 {
    60
}
