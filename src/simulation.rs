use std::fmt;
use std::path::Path;

use crate::config::SimulationConfig;
use crate::currency::{CurrencyError, ExchangeTable, Money};
use crate::faction::Faction;
use crate::history::{History, YearHistoryRecord};
use crate::technology::Product;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SimulationError {
    InvalidInput(String),
    Currency(CurrencyError),
    Io(String),
}

impl fmt::Display for SimulationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SimulationError::InvalidInput(message) => write!(formatter, "{message}"),
            SimulationError::Currency(error) => write!(formatter, "{error}"),
            SimulationError::Io(message) => write!(formatter, "{message}"),
        }
    }
}

impl std::error::Error for SimulationError {}

impl From<CurrencyError> for SimulationError {
    fn from(value: CurrencyError) -> Self {
        SimulationError::Currency(value)
    }
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct Simulation {
    current_year: i32,
    start_year: i32,
    seed: u64,
    events: Vec<EconomicEvent>,
    rng: SimpleRng,
    config: SimulationConfig,
    exchange_table: ExchangeTable,
    companies: Vec<Faction>,
    markets: Vec<crate::market::MarketState>,
    economy: crate::economy::EconomicState,
    history: History,
}

impl Simulation {
    pub fn new(
        start_year: i32,
        seed: u64,
        config: SimulationConfig,
    ) -> Result<Self, SimulationError> {
        validate_config(&config)?;
        let mut companies = Vec::new();
        for (f, products) in std::iter::once((&config.pilton, &config.products))
            .chain(config.competitors.iter().map(|c| (&c.faction, &c.products)))
        {
            let mut company = Faction::new(
                f.name.clone(),
                f.primary_currency,
                f.starting_cash,
                f.base_operating_expenses,
                products
                    .iter()
                    .cloned()
                    .map(Product::from_config)
                    .collect::<Result<Vec<_>, _>>()?,
            )?;
            company.debt = f.starting_debt.unwrap_or(Money::zero(f.primary_currency));
            company.credit_limit = f.credit_limit.unwrap_or(Money::zero(f.primary_currency));
            company.productive_assets = f
                .starting_productive_assets
                .unwrap_or(Money::zero(f.primary_currency));
            company.financial_totals()?;
            companies.push(company);
        }
        let mut markets: Vec<crate::market::MarketState> =
            config.markets.iter().map(Into::into).collect();
        if markets.is_empty() {
            for company in &companies {
                for p in &company.products {
                    if let Some(m) = markets.iter_mut().find(|m| m.category == p.category) {
                        m.customers = m
                            .customers
                            .checked_add(p.base_demand)
                            .ok_or(CurrencyError::Overflow)?;
                        m.maximum_customers =
                            m.customers.checked_mul(4).ok_or(CurrencyError::Overflow)?;
                    } else {
                        markets.push(crate::market::MarketState::from(
                            &crate::config::MarketConfig {
                                category: p.category.clone(),
                                annual_customers: p.base_demand,
                                maximum_customers: p
                                    .base_demand
                                    .checked_mul(4)
                                    .ok_or(CurrencyError::Overflow)?,
                                growth_bps: 0,
                            },
                        ));
                    }
                }
            }
        }
        let economy = crate::economy::EconomicState::new(&config.economics);
        Ok(Self {
            current_year: start_year,
            start_year,
            seed,
            events: Vec::new(),
            rng: SimpleRng::new(seed),
            config,
            exchange_table: ExchangeTable::initial(),
            companies,
            markets,
            economy,
            history: History::default(),
        })
    }

    pub fn run_until(&mut self, end_year: i32) -> Result<(), SimulationError> {
        if end_year == i32::MAX {
            return Err(SimulationError::InvalidInput(
                "end year must be below i32::MAX".into(),
            ));
        }
        if end_year < self.current_year {
            return Err(SimulationError::InvalidInput(format!(
                "end year {end_year} is before start year {}",
                self.current_year
            )));
        }

        while self.current_year <= end_year {
            self.advance_one_year()?;
        }
        Ok(())
    }

    pub fn history(&self) -> &History {
        &self.history
    }

    pub fn exchange_table(&self) -> &ExchangeTable {
        &self.exchange_table
    }

    pub fn export_history_json(&self, path: &Path) -> Result<(), SimulationError> {
        crate::export::write_json(self, path)
    }
    /// Next year to simulate. History contains completed years only.
    pub fn current_year(&self) -> i32 {
        self.current_year
    }
    pub fn start_year(&self) -> i32 {
        self.start_year
    }
    pub fn seed(&self) -> u64 {
        self.seed
    }
    pub fn config(&self) -> &SimulationConfig {
        &self.config
    }
    pub fn current_snapshot(&self) -> &Faction {
        &self.companies[0]
    }
    pub fn faction_state(&self, name: &str) -> Option<&Faction> {
        self.companies.iter().find(|f| f.name == name)
    }
    pub fn product_state(&self, name: &str) -> Option<&Product> {
        self.companies[0].products.iter().find(|p| p.name == name)
    }
    pub fn history_for_year(&self, year: i32) -> Option<&YearHistoryRecord> {
        self.history.records().iter().find(|r| r.year == year)
    }
    pub fn events(&self) -> &[EconomicEvent] {
        &self.events
    }
    pub fn events_for_year(&self, year: i32) -> impl Iterator<Item = &EconomicEvent> {
        self.events.iter().filter(move |e| e.year == year)
    }
    /// Failed steps leave live state and RNG unchanged; history is not cloned.
    pub fn advance_one_year(&mut self) -> Result<&YearHistoryRecord, SimulationError> {
        let next = self
            .current_year
            .checked_add(1)
            .ok_or_else(|| SimulationError::InvalidInput("year overflow".into()))?;
        let mut candidate = Self {
            current_year: self.current_year,
            start_year: self.start_year,
            seed: self.seed,
            config: self.config.clone(),
            exchange_table: self.exchange_table.clone(),
            companies: self.companies.clone(),
            markets: self.markets.clone(),
            economy: self.economy.clone(),
            rng: self.rng,
            history: History::default(),
            events: Vec::new(),
        };
        candidate.tick_year()?;
        self.companies = candidate.companies;
        self.markets = candidate.markets;
        self.economy = candidate.economy;
        self.rng = candidate.rng;
        let first_record = self.history.records().len();
        for record in candidate.history.records() {
            self.history.push(record.clone());
        }
        self.events.extend(candidate.events);
        self.current_year = next;
        Ok(&self.history.records()[first_record])
    }
    pub fn advance_years(&mut self, count: u32) -> Result<(), SimulationError> {
        if i64::from(self.current_year) + i64::from(count) > i64::from(i32::MAX) {
            return Err(SimulationError::InvalidInput("year overflow".into()));
        }
        for _ in 0..count {
            self.advance_one_year()?;
        }
        Ok(())
    }

    pub fn companies(&self) -> &[Faction] {
        &self.companies
    }
    pub fn markets(&self) -> &[crate::market::MarketState] {
        &self.markets
    }
    pub fn economy(&self) -> &crate::economy::EconomicState {
        &self.economy
    }
    pub fn company_product_state(&self, company: &str, product: &str) -> Option<&Product> {
        self.faction_state(company)?
            .products
            .iter()
            .find(|p| p.name == product)
    }
    pub fn company_history_for_year(&self, company: &str, year: i32) -> Option<&YearHistoryRecord> {
        self.history
            .records()
            .iter()
            .find(|r| r.year == year && r.faction_name == company)
    }
    fn tick_year(&mut self) -> Result<(), SimulationError> {
        use crate::{market, operations};
        let previous = self.economy.clone();
        self.economy.advance(&self.config.economics, &mut self.rng);
        for (entered, kind) in [
            (
                self.economy.demand_shock && !previous.demand_shock,
                EventType::DemandShock,
            ),
            (
                self.economy.cost_shock && !previous.cost_shock,
                EventType::CostShock,
            ),
        ] {
            if entered {
                self.events.push(operations::event(
                    self.current_year,
                    kind,
                    "Economy",
                    None,
                    Money::zero(self.companies[0].primary_currency),
                ));
            }
        }
        for m in &mut self.markets {
            m.advance(&self.economy);
        }
        let opening: Vec<_> = self.companies.iter().map(|f| f.cash).collect();
        let mut accounts = Vec::new();
        for company in &mut self.companies {
            accounts.push(operations::prepare(company, &self.economy, &self.config)?);
        }
        let mut offers = market::forecast(
            &self.companies,
            &self.markets,
            self.config.demand,
            &self.config.economics,
            &mut self.rng,
        )?;
        // Snapshot competitor prices before any company updates its prices.
        let competitor_prices: Vec<_> = offers
            .iter()
            .map(|offer| {
                let p = &self.companies[offer.company].products[offer.product];
                self.companies
                    .iter()
                    .enumerate()
                    .filter(|(i, f)| *i != offer.company && !f.bankrupt)
                    .flat_map(|(_, f)| &f.products)
                    .filter(|other| other.category == p.category)
                    .map(|other| other.current_market_value)
                    .min()
            })
            .collect();
        let mut production = Vec::new();
        for (ci, company) in self.companies.iter_mut().enumerate() {
            let selected: Vec<_> = offers.iter().filter(|o| o.company == ci).collect();
            production.push(operations::produce(
                company,
                &selected,
                &mut accounts[ci],
                &self.config,
            )?);
        }
        market::clear(&mut offers, &self.companies, &mut self.markets);
        for (ci, (company, a)) in self.companies.iter_mut().zip(accounts).enumerate() {
            let selected: Vec<_> = offers.iter().filter(|o| o.company == ci).collect();
            let prices: Vec<_> = offers
                .iter()
                .zip(&competitor_prices)
                .filter(|(o, _)| o.company == ci)
                .map(|(_, p)| *p)
                .collect();
            let record = operations::finish(
                company,
                &selected,
                &production[ci],
                &prices,
                a,
                opening[ci],
                &mut operations::YearContext {
                    economy: &self.economy,
                    markets: &self.markets,
                    config: &self.config,
                    rng: &mut self.rng,
                    year: self.current_year,
                    events: &mut self.events,
                },
            )?;
            self.history.push(record);
        }
        Ok(())
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SimpleRng {
    state: u64,
}

impl SimpleRng {
    pub fn new(seed: u64) -> Self {
        Self {
            state: seed ^ 0x9E37_79B9_7F4A_7C15,
        }
    }

    pub fn next_u64(&mut self) -> u64 {
        self.state = self
            .state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.state
    }

    pub fn range_inclusive(&mut self, min: i64, max: i64) -> i64 {
        debug_assert!(min <= max);
        let span = (max - min + 1) as u64;
        min + (self.next_u64() % span) as i64
    }
}

pub fn validate_config(config: &SimulationConfig) -> Result<(), SimulationError> {
    let invalid = |message: &str| SimulationError::InvalidInput(message.into());
    let c = &config.economics;
    if !(-5000..=5000).contains(&c.growth_bps)
        || !(-5000..=5000).contains(&c.inflation_bps)
        || c.macro_volatility_bps > 5000
        || c.interest_bps > 10000
        || c.credit_spread_bps > 10000
        || c.demand_conditions_bps > 20000
        || c.shock_size_bps > 10000
        || c.maintenance_bps > 10000
        || c.research_limit_bps > 9000
        || c.research_step_bps > 10000
        || c.share_event_threshold_bps == 0
        || c.share_event_threshold_bps > 10000
        || c.bankruptcy_after_years == 0
        || c.expansion_cost_multiple == 0
        || c.expansion_cost_multiple > 1000
        || c.excess_inventory_percent == 0
        || c.excess_inventory_percent > 10000
        || c.cash_reserve_percent > 10000
        || c.desired_margin_percent >= 100
    {
        return Err(invalid(
            "invalid economic rate, limit, or bankruptcy grace period",
        ));
    }
    for percent in [
        c.shock_probability_percent,
        c.backlog_percent,
        c.loyalty_percent,
        c.availability_percent,
        c.recent_price_sensitivity_percent,
        c.cost_price_weight_percent,
        c.competitor_price_weight_percent,
        c.inventory_price_weight_percent,
        c.stockout_price_weight_percent,
        c.repayment_percent,
        c.expansion_percent,
        c.depreciation_percent,
        c.research_cash_percent,
        c.research_success_percent,
    ] {
        if percent > 100 {
            return Err(invalid("economic percentages must be between 0 and 100"));
        }
    }
    let mut company_names = std::collections::BTreeSet::new();
    for (f, products) in std::iter::once((&config.pilton, &config.products))
        .chain(config.competitors.iter().map(|c| (&c.faction, &c.products)))
    {
        if f.name.trim().is_empty() || !company_names.insert(&f.name) || products.is_empty() {
            return Err(invalid(
                "company names must be unique and each company needs products",
            ));
        }
        // Competing prices are compared directly; mixed-currency trade is deferred.
        if f.primary_currency != config.pilton.primary_currency {
            return Err(invalid("companies must share the scenario currency"));
        }
        let mut names = std::collections::BTreeSet::new();
        for p in products {
            if p.name.trim().is_empty()
                || !names.insert(&p.name)
                || p.input_share_percent > 100
                || p.maximum_capacity
                    .is_some_and(|cap| cap < p.production_capacity)
                || p.production_cost.currency() != f.primary_currency
                || p.starting_market_value.currency() != f.primary_currency
                || p.base_demand > 1_000_000_000_000
                || p.production_capacity > 1_000_000_000_000
                || p.maximum_capacity
                    .is_some_and(|cap| cap > 4_000_000_000_000)
                || p.popularity_percent > 10000
            {
                return Err(invalid(
                    "invalid product name, currency, capacity, input share, or demand",
                ));
            }
            Product::from_config(p.clone())?;
            let category = if p.category.is_empty() {
                &p.name
            } else {
                &p.category
            };
            if category.trim().is_empty()
                || (!config.markets.is_empty()
                    && !config.markets.iter().any(|m| &m.category == category))
            {
                return Err(invalid(
                    "every product must reference a configured category",
                ));
            }
        }
        let zero = Money::zero(f.primary_currency);
        let debt = f.starting_debt.unwrap_or(zero);
        let limit = f.credit_limit.unwrap_or(zero);
        for money in [
            f.starting_cash,
            f.base_operating_expenses,
            debt,
            limit,
            f.starting_productive_assets.unwrap_or(zero),
        ] {
            if money.currency() != f.primary_currency || money.minor_units() < 0 {
                return Err(invalid(
                    "company money must be nonnegative and use its primary currency",
                ));
            }
        }
        if debt > limit {
            return Err(invalid("starting debt exceeds credit limit"));
        }
    }
    let mut categories = std::collections::BTreeSet::new();
    for m in &config.markets {
        if m.category.trim().is_empty()
            || !categories.insert(&m.category)
            || m.annual_customers > m.maximum_customers
            || m.maximum_customers > 1_000_000_000_000
            || !(-5000..=5000).contains(&m.growth_bps)
        {
            return Err(invalid(
                "invalid or duplicate market category, size, or growth",
            ));
        }
    }
    for percent in [
        config.demand.yearly_variation_percent,
        config.demand.momentum_percent,
        config.demand.price_sensitivity_percent,
        config.demand.max_price_change_percent,
    ] {
        if !(0..=100).contains(&percent) {
            return Err(invalid("demand percentages must be between 0 and 100"));
        }
    }
    if config.production.inventory_buffer_percent == 0 {
        return Err(SimulationError::InvalidInput(
            "inventory buffer must be greater than zero".to_string(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::currency::Currency;

    #[test]
    fn rejects_end_year_before_start_year() {
        let mut simulation = Simulation::new(3421, 1, SimulationConfig::default()).unwrap();
        assert!(simulation.run_until(3420).is_err());
    }

    #[test]
    fn production_is_limited_by_affordability() {
        let mut config = SimulationConfig::default();
        config.pilton.credit_limit = None;
        config.pilton.starting_cash = Money::from_units(Currency::Ufc, 100_000);
        config.pilton.base_operating_expenses = Money::zero(Currency::Ufc);
        config.products.truncate(1);
        config.products[0].production_cost = Money::from_units(Currency::Ufc, 60_000);
        config.products[0].starting_inventory = 0;
        config.products[0].base_demand = 10;
        config.products[0].production_capacity = 10;

        let mut simulation = Simulation::new(3421, 7, config).unwrap();
        simulation.run_until(3421).unwrap();
        let record = &simulation.history().records()[0].products[0];

        assert_eq!(record.units_produced, 1);
    }

    #[test]
    fn inventory_changes_and_sales_do_not_exceed_inventory() {
        let mut config = SimulationConfig::default();
        config.products.truncate(1);
        config.products[0].starting_inventory = 1;
        config.products[0].production_capacity = 0;
        config.products[0].base_demand = 100;

        let mut simulation = Simulation::new(3421, 4, config).unwrap();
        simulation.run_until(3421).unwrap();
        let record = &simulation.history().records()[0].products[0];

        assert_eq!(record.units_produced, 0);
        assert_eq!(record.units_sold, 1);
        assert_eq!(record.remaining_inventory, 0);
    }

    #[test]
    fn cash_profit_and_net_worth_are_consistent() {
        let config = SimulationConfig::default();
        let starting_cash = config.pilton.starting_cash;
        let mut simulation = Simulation::new(3421, 42, config).unwrap();
        simulation.run_until(3421).unwrap();
        let record = &simulation.history().records()[0];

        assert_eq!(record.cash, starting_cash + record.accounts.cash_flow);
        assert_eq!(
            record.net_worth,
            record.cash + record.inventory_value + record.technology_asset_value - record.debt
        );
        assert_eq!(record.profit, record.revenue - record.expenses);
    }

    #[test]
    fn deterministic_with_same_seed() {
        let config = SimulationConfig::default();
        let mut first = Simulation::new(3421, 99, config.clone()).unwrap();
        let mut second = Simulation::new(3421, 99, config).unwrap();

        first.run_until(3455).unwrap();
        second.run_until(3455).unwrap();

        assert_eq!(first.history(), second.history());
    }

    #[test]
    fn different_seeds_can_change_history() {
        let config = SimulationConfig::default();
        let mut first = Simulation::new(3421, 1, config.clone()).unwrap();
        let mut second = Simulation::new(3421, 2, config).unwrap();

        first.run_until(3455).unwrap();
        second.run_until(3455).unwrap();

        assert_ne!(first.history(), second.history());
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub enum Severity {
    Error,
    Warning,
    UnusualButValid,
}
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub enum EventType {
    ProductionSuspended,
    ProductionEligible,
    CashNegative,
    Stockout,
    ExcessInventory,
    MarketShareGain,
    MarketShareLoss,
    CapacityExpanded,
    DebtTaken,
    DebtRepaid,
    FinancialDistress,
    Bankruptcy,
    DemandShock,
    CostShock,
}
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct EconomicEvent {
    pub year: i32,
    pub event_type: EventType,
    pub entities: Vec<String>,
    pub description: String,
    pub value: Money,
    pub severity: Severity,
}
