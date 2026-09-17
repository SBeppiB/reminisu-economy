use crate::currency::{Currency, Money};
use crate::technology::ProductConfig;

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct SimulationConfig {
    pub economics: EconomicsConfig,
    pub competitors: Vec<CompanyConfig>,
    pub markets: Vec<MarketConfig>,
    pub engine_status: CanonStatus,
    pub scenario_status: CanonStatus,
    pub pilton: FactionConfig,
    pub products: Vec<ProductConfig>,
    pub demand: DemandConfig,
    pub production: ProductionStrategyConfig,
}

impl Default for SimulationConfig {
    fn default() -> Self {
        load_documents(
            include_str!("../config/default.toml"),
            include_str!("../scenarios/testing.toml"),
        )
        .expect("bundled configuration must be valid")
    }
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct FactionConfig {
    #[serde(default)]
    pub credit_limit: Option<Money>,
    #[serde(default)]
    pub starting_debt: Option<Money>,
    pub name: String,
    pub primary_currency: Currency,
    pub starting_cash: Money,
    pub base_operating_expenses: Money,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct DemandConfig {
    pub yearly_variation_percent: i32,
    pub momentum_percent: i32,
    pub price_sensitivity_percent: i32,
    pub max_price_change_percent: i32,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ProductionStrategyConfig {
    pub inventory_buffer_percent: u32,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EngineConfig {
    #[serde(default)]
    pub economics: EconomicsConfig,
    pub status: CanonStatus,
    pub demand: DemandConfig,
    pub production: ProductionStrategyConfig,
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Scenario {
    #[serde(default)]
    pub competitors: Vec<CompanyConfig>,
    #[serde(default)]
    pub markets: Vec<MarketConfig>,
    pub status: CanonStatus,
    pub pilton: FactionConfig,
    pub products: Vec<ProductConfig>,
}
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CanonStatus {
    Canon,
    Estimated,
    Placeholder,
}
pub fn load_documents(
    engine: &str,
    scenario: &str,
) -> Result<SimulationConfig, crate::simulation::SimulationError> {
    let engine: EngineConfig = toml::from_str(engine).map_err(|e| {
        crate::simulation::SimulationError::InvalidInput(format!("engine configuration: {e}"))
    })?;
    let scenario: Scenario = toml::from_str(scenario)
        .map_err(|e| crate::simulation::SimulationError::InvalidInput(format!("scenario: {e}")))?;
    Ok(SimulationConfig {
        economics: engine.economics,
        competitors: scenario.competitors,
        markets: scenario.markets,
        engine_status: engine.status,
        scenario_status: scenario.status,
        pilton: scenario.pilton,
        products: scenario.products,
        demand: engine.demand,
        production: engine.production,
    })
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CompanyConfig {
    pub faction: FactionConfig,
    pub products: Vec<ProductConfig>,
}
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct MarketConfig {
    pub category: String,
    pub annual_customers: u64,
    pub maximum_customers: u64,
    pub growth_bps: i32,
}
/// Implicit economic defaults have independent PLACEHOLDER provenance.
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub struct EconomicsConfig {
    pub status: CanonStatus,
    pub growth_bps: i32,
    pub inflation_bps: i32,
    pub interest_bps: u32,
    pub macro_volatility_bps: u32,
    pub demand_conditions_bps: u32,
    pub shock_probability_percent: u32,
    pub shock_size_bps: u32,
    pub backlog_percent: u32,
    pub loyalty_percent: u32,
    pub availability_percent: u32,
    pub recent_price_sensitivity_percent: u32,
    pub desired_margin_percent: u32,
    pub cost_price_weight_percent: u32,
    pub competitor_price_weight_percent: u32,
    pub inventory_price_weight_percent: u32,
    pub stockout_price_weight_percent: u32,
    pub credit_spread_bps: u32,
    pub repayment_percent: u32,
    pub cash_reserve_percent: u32,
    pub bankruptcy_after_years: u32,
    pub bankruptcy_enabled: bool,
    pub maintenance_bps: u32,
    pub expansion_percent: u32,
    pub expansion_cost_multiple: u32,
    pub depreciation_percent: u32,
    pub research_cash_percent: u32,
    pub research_success_percent: u32,
    pub research_step_bps: u32,
    pub research_limit_bps: u32,
    pub share_event_threshold_bps: u32,
    pub excess_inventory_percent: u32,
}
impl Default for EconomicsConfig {
    fn default() -> Self {
        Self {
            status: CanonStatus::Placeholder,
            growth_bps: 100,
            inflation_bps: 200,
            interest_bps: 400,
            macro_volatility_bps: 150,
            demand_conditions_bps: 10000,
            shock_probability_percent: 8,
            shock_size_bps: 1500,
            backlog_percent: 25,
            loyalty_percent: 20,
            availability_percent: 30,
            recent_price_sensitivity_percent: 30,
            desired_margin_percent: 30,
            cost_price_weight_percent: 20,
            competitor_price_weight_percent: 20,
            inventory_price_weight_percent: 8,
            stockout_price_weight_percent: 4,
            credit_spread_bps: 200,
            repayment_percent: 25,
            cash_reserve_percent: 100,
            bankruptcy_after_years: 3,
            bankruptcy_enabled: true,
            maintenance_bps: 100,
            expansion_percent: 10,
            expansion_cost_multiple: 3,
            depreciation_percent: 10,
            research_cash_percent: 0,
            research_success_percent: 40,
            research_step_bps: 250,
            research_limit_bps: 3000,
            share_event_threshold_bps: 1000,
            excess_inventory_percent: 200,
        }
    }
}
