use reminisu_economy::{
    accounting::Accounts,
    config::{self, CompanyConfig, MarketConfig, SimulationConfig},
    currency::{Currency, Money},
    economy::EconomicState,
    export, finance, investment, market, pricing,
    simulation::{EventType, SimpleRng, Simulation},
    technology::Product,
};

fn money(v: i128) -> Money {
    Money::from_minor(Currency::Ufc, v)
}
fn controlled() -> SimulationConfig {
    let mut c = SimulationConfig::default();
    c.products.truncate(1);
    c.products[0].category = "suits".into();
    c.products[0].production_cost = money(100);
    c.products[0].starting_market_value = money(200);
    c.products[0].starting_inventory = 0;
    c.products[0].base_demand = 10;
    c.products[0].production_capacity = 20;
    c.products[0].maximum_capacity = Some(40);
    c.products[0].popularity_percent = 100;
    c.pilton.starting_cash = money(100000);
    c.pilton.base_operating_expenses = money(0);
    c.pilton.credit_limit = None;
    c.pilton.starting_debt = None;
    c.markets = vec![MarketConfig {
        category: "suits".into(),
        annual_customers: 10,
        maximum_customers: 40,
        growth_bps: 0,
    }];
    c.demand.yearly_variation_percent = 0;
    c.demand.max_price_change_percent = 0;
    c.economics.growth_bps = 0;
    c.economics.inflation_bps = 0;
    c.economics.macro_volatility_bps = 0;
    c.economics.shock_probability_percent = 0;
    c.economics.maintenance_bps = 0;
    c.economics.expansion_percent = 0;
    c.economics.research_cash_percent = 0;
    c
}
fn competitor(c: &mut SimulationConfig) {
    let mut rival = CompanyConfig {
        faction: c.pilton.clone(),
        products: c.products.clone(),
    };
    rival.faction.name = "Rival".into();
    c.competitors.push(rival);
}

#[test]
fn weighted_average_cogs_preserves_every_minor_unit() {
    let mut c = controlled();
    c.products[0].starting_inventory = 2;
    let mut p = Product::from_config(c.products[0].clone()).unwrap();
    p.production_cost = money(201);
    p.produce(2, money(1000)).unwrap();
    assert_eq!(p.inventory_cost_basis, money(602));
    p.sell(1).unwrap();
    assert_eq!(p.last_cogs, money(150));
    assert_eq!(p.inventory_cost_basis, money(452));
    p.sell(3).unwrap();
    assert_eq!(p.last_cogs, money(452));
    assert_eq!(p.inventory_cost_basis, money(0));
}

#[test]
fn unsold_production_is_inventory_not_cogs_and_cash_reconciles() {
    let c = controlled();
    let mut s = Simulation::new(1, 42, c).unwrap();
    let r = s.advance_one_year().unwrap();
    assert_eq!(r.products[0].units_produced, 12);
    assert_eq!(r.products[0].units_sold, 10);
    assert_eq!(r.accounts.production_spending, money(1200));
    assert_eq!(r.accounts.cost_of_goods_sold, money(1000));
    assert_eq!(r.inventory_cost_basis, money(200));
    assert_eq!(r.accounts.net_profit, money(1000));
    assert_eq!(r.accounts.cash_flow, money(800));
    assert_eq!(r.cash, money(100800));
}

#[test]
fn all_company_accounting_identities_hold_over_time() {
    let mut c = controlled();
    competitor(&mut c);
    c.economics.inflation_bps = 200;
    c.economics.maintenance_bps = 100;
    c.economics.research_cash_percent = 1;
    c.economics.expansion_percent = 10;
    let mut s = Simulation::new(1, 77, c.clone()).unwrap();
    s.advance_years(30).unwrap();
    for name in [&c.pilton.name, &c.competitors[0].faction.name] {
        let mut cash = c.pilton.starting_cash;
        let mut basis = money(0);
        let mut debt = money(0);
        for r in s
            .history()
            .records()
            .iter()
            .filter(|r| &r.faction_name == name)
        {
            let a = &r.accounts;
            assert_eq!(r.cash, cash + a.cash_flow);
            assert_eq!(
                r.inventory_cost_basis,
                basis + a.production_spending - a.cost_of_goods_sold
            );
            assert_eq!(r.debt, debt + a.debt_taken - a.debt_repaid);
            assert_eq!(a.gross_profit, a.revenue - a.cost_of_goods_sold);
            assert_eq!(
                a.operating_profit,
                a.gross_profit - a.operating_expenses - a.maintenance - a.research - a.depreciation
            );
            assert_eq!(a.net_profit, a.operating_profit - a.interest_expense);
            assert_eq!(
                r.net_worth,
                r.cash + r.inventory_value + r.technology_asset_value - r.debt
            );
            cash = r.cash;
            basis = r.inventory_cost_basis;
            debt = r.debt;
        }
    }
}

#[test]
fn identical_competitors_share_finite_market_equally() {
    let mut c = controlled();
    competitor(&mut c);
    let mut s = Simulation::new(1, 42, c).unwrap();
    s.advance_one_year().unwrap();
    assert_eq!(s.history().records().len(), 2);
    assert_eq!(s.history().records()[0].products[0].units_sold, 5);
    assert_eq!(s.history().records()[1].products[0].units_sold, 5);
    assert_eq!(s.markets()[0].units_sold, 10);
    assert_eq!(s.markets()[0].available_demand, 10);
    assert!(s.company_product_state("Rival", "E.N.D Suits").is_some());
    assert_eq!(
        s.history().summary().unwrap(),
        s.history().summary_for("Pilton Dynamics").unwrap()
    );
}

#[test]
fn cheaper_competitor_wins_more_customers() {
    let mut c = controlled();
    competitor(&mut c);
    c.markets[0].annual_customers = 40;
    c.products[0].base_demand = 40;
    c.competitors[0].products[0].base_demand = 40;
    c.products[0].starting_inventory = 40;
    c.competitors[0].products[0].starting_inventory = 40;
    c.competitors[0].products[0].starting_market_value = money(120);
    let mut s = Simulation::new(1, 42, c).unwrap();
    s.advance_one_year().unwrap();
    assert!(
        s.history().records()[1].products[0].units_sold
            > s.history().records()[0].products[0].units_sold
    );
    assert_eq!(s.markets()[0].units_sold, 40);
}

#[test]
fn stockout_demand_can_switch_to_available_rival() {
    let mut c = controlled();
    competitor(&mut c);
    c.products[0].production_capacity = 0;
    let mut s = Simulation::new(1, 42, c).unwrap();
    s.advance_one_year().unwrap();
    assert_eq!(s.history().records()[0].products[0].units_sold, 0);
    assert!(s.history().records()[1].products[0].units_sold > 5);
    assert!(s.markets()[0].units_sold <= 10);
}

#[test]
fn allocation_conserves_pool_and_supply_for_many_integer_cases() {
    for pool in 0..50 {
        for a in 0..20 {
            let caps = [a, 7, 13];
            let weights = [5, 3, 2];
            let result = market::allocate(pool, &weights, &caps);
            assert_eq!(result.iter().sum::<u64>(), pool.min(caps.iter().sum()));
            assert!(result.iter().zip(caps).all(|(r, c)| *r <= c));
        }
    }
    assert_eq!(market::allocate(10, &[0, 1], &[100, 3]), vec![0, 3]);
}

#[test]
fn debt_is_bounded_interest_is_expensed_and_principal_is_not_profit() {
    let c = controlled();
    let s = Simulation::new(1, 1, c.clone()).unwrap();
    let mut f = s.current_snapshot().clone();
    f.cash = money(0);
    f.credit_limit = money(10000);
    let mut a = Accounts::new(Currency::Ufc);
    finance::fund(&mut f, money(20000), &mut a).unwrap();
    assert_eq!(f.cash, money(10000));
    assert_eq!(f.debt, money(10000));
    let mut e = c.economics;
    e.repayment_percent = 0;
    e.credit_spread_bps = 0;
    finance::settle(&mut f, &mut a, &e, 1000).unwrap();
    a.close().unwrap();
    assert_eq!(a.interest_expense, money(1000));
    assert_eq!(a.net_profit, money(-1000));
    assert_eq!(a.cash_flow, money(9000));
    assert_eq!(f.debt, money(10000));
}

#[test]
fn debt_repayment_reduces_cash_and_principal_without_changing_earnings() {
    let c = controlled();
    let s = Simulation::new(1, 1, c.clone()).unwrap();
    let mut f = s.current_snapshot().clone();
    f.cash = money(1000);
    f.debt = money(400);
    f.credit_limit = money(400);
    let mut e = c.economics;
    e.credit_spread_bps = 0;
    e.repayment_percent = 50;
    let mut a = Accounts::new(Currency::Ufc);
    finance::settle(&mut f, &mut a, &e, 0).unwrap();
    a.close().unwrap();
    assert_eq!(f.debt, money(0));
    assert_eq!(f.cash, money(600));
    assert_eq!(a.net_profit, money(0));
    assert_eq!(a.cash_flow, money(-400));
}

#[test]
fn insolvency_has_grace_period_and_bankrupt_company_is_retained() {
    let mut c = controlled();
    c.pilton.starting_cash = money(0);
    c.pilton.base_operating_expenses = money(100);
    c.economics.bankruptcy_after_years = 3;
    let mut s = Simulation::new(1, 42, c).unwrap();
    s.advance_years(2).unwrap();
    assert!(!s.current_snapshot().bankrupt);
    s.advance_one_year().unwrap();
    assert!(s.current_snapshot().bankrupt);
    let cash = s.current_snapshot().cash;
    s.advance_years(5).unwrap();
    assert_eq!(s.current_snapshot().cash, cash);
    for kind in [
        EventType::FinancialDistress,
        EventType::Bankruptcy,
        EventType::CashNegative,
    ] {
        assert_eq!(
            s.events().iter().filter(|e| e.event_type == kind).count(),
            1
        );
    }
}

#[test]
fn inflation_changes_new_costs_but_not_old_inventory_basis() {
    let mut c = controlled();
    c.economics.inflation_bps = 1000;
    c.products[0].production_capacity = 0;
    c.products[0].starting_inventory = 20;
    let mut s = Simulation::new(1, 42, c).unwrap();
    s.advance_one_year().unwrap();
    let p = s.current_snapshot().products.first().unwrap();
    assert_eq!(p.production_cost, money(110));
    assert_eq!(p.inventory_cost_basis, money(i128::from(p.inventory) * 100));
}

#[test]
fn market_growth_is_bounded_and_fractional_growth_accumulates() {
    let mut m = market::MarketState::from(&MarketConfig {
        category: "x".into(),
        annual_customers: 10,
        maximum_customers: 12,
        growth_bps: 100,
    });
    let e = EconomicState::new(&controlled().economics);
    for _ in 0..30 {
        m.advance(&e);
    }
    assert_eq!(m.customers, 12);
    m.growth_bps = -1000;
    for _ in 0..10 {
        m.advance(&e);
    }
    assert!(m.customers < 12);
}

#[test]
fn prices_obey_limits_and_respond_to_stock_and_competitors() {
    let c = controlled();
    let p = Product::from_config(c.products[0].clone()).unwrap();
    let mut d = c.demand;
    d.max_price_change_percent = 6;
    let mut stocked = p.clone();
    stocked.inventory = 100;
    pricing::update(&mut stocked, 10, None, &c.economics, d).unwrap();
    assert!(stocked.current_market_value < p.current_market_value);
    assert!(stocked.current_market_value >= money(188));
    let mut expensive = p.clone();
    let mut cheap = p;
    pricing::update(&mut expensive, 10, Some(money(400)), &c.economics, d).unwrap();
    pricing::update(&mut cheap, 10, Some(money(100)), &c.economics, d).unwrap();
    assert!(expensive.current_market_value > cheap.current_market_value);
    assert!(expensive.current_market_value <= money(212));
}

#[test]
fn research_has_diminishing_bounded_and_uncertain_returns() {
    let mut c = controlled();
    c.economics.research_cash_percent = 10;
    c.economics.research_success_percent = 100;
    c.economics.research_limit_bps = 1000;
    c.economics.research_step_bps = 500;
    let s = Simulation::new(1, 1, c.clone()).unwrap();
    let mut f = s.current_snapshot().clone();
    let mut rng = SimpleRng::new(1);
    let mut previous_gain = u32::MAX;
    let mut progress = 0;
    for _ in 0..20 {
        f.cash = money(100000);
        let mut a = Accounts::new(Currency::Ufc);
        investment::invest(&mut f, &mut a, &c.economics, &mut rng).unwrap();
        let next = f.products[0].research_progress_bps;
        assert!(next <= 1000);
        assert!(next - progress <= previous_gain);
        previous_gain = next - progress;
        progress = next;
    }
    assert!(progress > 0);
    c.economics.research_success_percent = 0;
    let before = progress;
    let mut a = Accounts::new(Currency::Ufc);
    investment::invest(&mut f, &mut a, &c.economics, &mut rng).unwrap();
    assert_eq!(f.products[0].research_progress_bps, before);
    assert!(a.research.minor_units() > 0);
}

#[test]
fn expansion_spends_cash_and_adds_depreciable_assets() {
    let mut c = controlled();
    c.economics.expansion_percent = 50;
    let s = Simulation::new(1, 1, c.clone()).unwrap();
    let mut f = s.current_snapshot().clone();
    f.products[0].stockout = true;
    let mut a = Accounts::new(Currency::Ufc);
    investment::invest(&mut f, &mut a, &c.economics, &mut SimpleRng::new(1)).unwrap();
    assert_eq!(f.products[0].production_capacity, 30);
    assert_eq!(a.capital_spending, money(3000));
    assert_eq!(f.productive_assets, money(3000));
    assert_eq!(f.cash, money(97000));
    for _ in 0..10 {
        investment::invest(&mut f, &mut a, &c.economics, &mut SimpleRng::new(1)).unwrap();
    }
    assert_eq!(f.products[0].production_capacity, 40);
}

#[test]
fn multi_company_state_and_exports_are_deterministic_and_seed_sensitive() {
    let mut c = controlled();
    competitor(&mut c);
    c.economics.macro_volatility_bps = 1000;
    c.demand.yearly_variation_percent = 30;
    let mut a = Simulation::new(1, 42, c.clone()).unwrap();
    let mut b = a.clone();
    let mut other = Simulation::new(1, 43, c).unwrap();
    a.advance_years(25).unwrap();
    b.advance_years(5).unwrap();
    b.run_until(25).unwrap();
    other.advance_years(25).unwrap();
    assert_eq!(a, b);
    assert_eq!(export::document(&a), export::document(&b));
    assert_ne!(a.history(), other.history());
    let state = serde_json::to_value(&a).unwrap();
    for key in ["rng", "companies", "markets", "economy"] {
        assert!(!state[key].is_null());
    }
}

#[test]
fn rollback_includes_other_companies_markets_and_macro_state() {
    let mut c = controlled();
    competitor(&mut c);
    c.economics.inflation_bps = 100;
    c.competitors[0].faction.base_operating_expenses = money(i128::MAX);
    let mut s = Simulation::new(1, 42, c).unwrap();
    let before = s.clone();
    assert!(s.advance_one_year().is_err());
    assert_eq!(s, before);
    assert_eq!(
        serde_json::to_value(&s).unwrap(),
        serde_json::to_value(&before).unwrap()
    );
}

#[test]
fn validation_rejects_new_unknown_fields_and_invalid_finance() {
    let engine =
        include_str!("../config/default.toml").replace("credit_spread_bps", "credit_spred_bps");
    assert!(config::load_documents(&engine, include_str!("../scenarios/testing.toml")).is_err());
    let mut c = controlled();
    c.economics.desired_margin_percent = 100;
    assert!(Simulation::new(1, 1, c).is_err());
    let mut c = controlled();
    c.pilton.starting_debt = Some(money(1));
    assert!(Simulation::new(1, 1, c).is_err());
    let mut c = controlled();
    competitor(&mut c);
    c.competitors[0].faction.name = c.pilton.name.clone();
    assert!(Simulation::new(1, 1, c).is_err());
}

#[test]
fn extreme_money_paths_return_checked_errors() {
    assert!(Money::checked_from_units(Currency::Ufc, i128::MAX).is_err());
    assert_eq!(money(i128::MAX).ratio(100, 100).unwrap(), money(i128::MAX));
    assert!(money(i128::MAX).ratio(101, 100).is_err());
}

#[test]
fn bundled_competitive_scenario_runs_and_exports_every_company() {
    let c = config::load_documents(
        include_str!("../config/default.toml"),
        include_str!("../scenarios/competition.toml"),
    )
    .unwrap();
    let mut s = Simulation::new(3421, 42, c).unwrap();
    s.advance_years(20).unwrap();
    let d = export::document(&s);
    assert_eq!(d["metadata"]["format_version"], 3);
    assert_eq!(d["companies"].as_array().unwrap().len(), 2);
    assert_eq!(d["records"].as_array().unwrap().len(), 40);
    for year in 3421..3441 {
        let r = s.history_for_year(year).unwrap();
        for market in &r.markets {
            assert!(market.units_sold <= market.available_demand);
        }
    }
}

#[test]
fn money_and_complete_history_round_trip_extreme_integer_precision() {
    for amount in [i128::MIN, i128::MAX, 9007199254740993123456789] {
        let original = money(amount);
        let encoded = serde_json::to_string(&original).unwrap();
        assert_eq!(serde_json::from_str::<Money>(&encoded).unwrap(), original);
    }
    assert!(serde_json::from_str::<Money>(r#"{"currency":"Ufc","minor_units":1.5}"#).is_err());
    let mut c = controlled();
    c.pilton.starting_cash = money(9007199254740993123456789);
    let mut s = Simulation::new(1, 42, c).unwrap();
    s.advance_years(2).unwrap();
    let encoded = serde_json::to_string(s.history()).unwrap();
    let decoded: reminisu_economy::history::History = serde_json::from_str(&encoded).unwrap();
    assert_eq!(&decoded, s.history());
    let company_json = serde_json::to_string(s.current_snapshot()).unwrap();
    let company: reminisu_economy::faction::Faction = serde_json::from_str(&company_json).unwrap();
    assert_eq!(&company, s.current_snapshot());
    let mut rng = SimpleRng::new(42);
    rng.next_u64();
    let mut restored: SimpleRng =
        serde_json::from_str(&serde_json::to_string(&rng).unwrap()).unwrap();
    assert_eq!(rng.next_u64(), restored.next_u64());
}

#[test]
fn zero_market_never_sells_or_recognizes_cogs() {
    let mut c = controlled();
    c.markets[0].annual_customers = 0;
    c.products[0].starting_inventory = 20;
    let mut s = Simulation::new(1, 42, c).unwrap();
    s.advance_years(3).unwrap();
    for r in s.history().records() {
        assert_eq!(r.revenue, money(0));
        assert_eq!(r.accounts.cost_of_goods_sold, money(0));
        assert_eq!(r.inventory_cost_basis, money(2000));
    }
    assert_eq!(
        s.events()
            .iter()
            .filter(|e| e.event_type == EventType::ExcessInventory)
            .count(),
        1
    );
}

#[test]
fn persistent_macro_shocks_emit_once_and_raise_input_costs() {
    let mut c = controlled();
    c.economics.shock_probability_percent = 100;
    c.economics.shock_size_bps = 2000;
    c.products[0].input_share_percent = 50;
    let mut s = Simulation::new(1, 42, c).unwrap();
    s.advance_years(3).unwrap();
    assert_eq!(s.current_snapshot().products[0].production_cost, money(110));
    assert_eq!(s.markets()[0].available_demand, 8);
    for kind in [EventType::DemandShock, EventType::CostShock] {
        assert_eq!(
            s.events().iter().filter(|e| e.event_type == kind).count(),
            1
        );
    }
}

#[test]
fn bankruptcy_can_be_disabled_and_distress_can_recover() {
    let mut c = controlled();
    c.economics.bankruptcy_enabled = false;
    let s = Simulation::new(1, 42, c.clone()).unwrap();
    let mut f = s.current_snapshot().clone();
    f.cash = money(-100);
    for _ in 0..5 {
        finance::assess(&mut f, &c.economics).unwrap();
    }
    assert!(!f.bankrupt);
    assert_eq!(f.distress_years, 5);
    f.cash = money(100);
    finance::assess(&mut f, &c.economics).unwrap();
    assert_eq!(f.distress_years, 0);
}

#[test]
fn reputation_and_backlog_affect_demand_without_exceeding_market() {
    let mut c = controlled();
    competitor(&mut c);
    c.economics.loyalty_percent = 100;
    let s = Simulation::new(1, 42, c.clone()).unwrap();
    let mut companies = s.companies().to_vec();
    companies[1].products[0].market_share_bps = 10000;
    let mut markets = s.markets().to_vec();
    markets[0].advance(s.economy());
    let offers = market::forecast(
        &companies,
        &markets,
        c.demand,
        &c.economics,
        &mut SimpleRng::new(42),
    )
    .unwrap();
    assert!(offers[1].demand > offers[0].demand);
    companies[0].products[0].base_demand = 2;
    companies[0].products[0].previous_demand = 2;
    let before = market::forecast(
        &companies,
        &markets,
        c.demand,
        &c.economics,
        &mut SimpleRng::new(42),
    )
    .unwrap();
    companies[0].products[0].unmet_demand = 100;
    let after = market::forecast(
        &companies,
        &markets,
        c.demand,
        &c.economics,
        &mut SimpleRng::new(42),
    )
    .unwrap();
    assert!(after[0].desired > before[0].desired);
    assert!(after.iter().map(|o| o.demand).sum::<u64>() <= markets[0].available_demand);
}

#[test]
fn research_efficiency_reduces_future_unit_cost_without_revaluing_old_stock() {
    let mut c = controlled();
    c.economics.research_cash_percent = 10;
    c.economics.research_success_percent = 100;
    c.economics.research_step_bps = 1000;
    let mut s = Simulation::new(1, 42, c).unwrap();
    s.advance_years(2).unwrap();
    let first = &s.history().records()[0].products[0];
    let second = &s.history().records()[1].products[0];
    assert!(first.research_progress_bps > 0);
    assert!(second.production_cost < first.production_cost);
    assert_eq!(first.inventory_cost_basis, money(200));
}

#[test]
fn a_late_company_failure_rolls_back_already_processed_company_sales() {
    let mut c = controlled();
    competitor(&mut c);
    // Overhead itself fits, but adding full-year debt interest does not.
    c.competitors[0].faction.base_operating_expenses = money(i128::MAX);
    c.competitors[0].faction.credit_limit = Some(money(10000));
    c.competitors[0].faction.starting_debt = Some(money(10000));
    let mut s = Simulation::new(1, 42, c).unwrap();
    let before = s.clone();
    assert!(s.advance_one_year().is_err());
    assert_eq!(s, before);
}

#[test]
fn legacy_toml_loads_with_explicitly_placeholder_economic_defaults() {
    let engine = r#"
status = "ESTIMATED"
[demand]
yearly_variation_percent = 18
momentum_percent = 25
price_sensitivity_percent = 35
max_price_change_percent = 6
[production]
inventory_buffer_percent = 125
"#;
    let scenario = r#"
status = "ESTIMATED"
[pilton]
name = "Old company"
primary_currency = "Ufc"
starting_cash = { currency = "Ufc", minor_units = 10000 }
base_operating_expenses = { currency = "Ufc", minor_units = 10 }
[[products]]
name = "Old product"
production_cost = { currency = "Ufc", minor_units = 100 }
starting_market_value = { currency = "Ufc", minor_units = 200 }
starting_inventory = 1
base_demand = 10
production_capacity = 12
popularity_percent = 100
"#;
    let c = config::load_documents(engine, scenario).unwrap();
    assert_eq!(c.economics.status, config::CanonStatus::Placeholder);
    let mut s = Simulation::new(1, 42, c).unwrap();
    assert_eq!(s.markets()[0].customers, 10);
    assert_eq!(s.markets()[0].maximum_customers, 40);
    assert_eq!(s.current_snapshot().credit_limit, money(0));
    s.advance_one_year().unwrap();
}

#[test]
fn csv_and_cli_sidecar_preserve_new_schema_and_provenance() {
    let exe = env!("CARGO_BIN_EXE_reminisu_economy");
    let path = std::env::temp_dir().join(format!("economy-v3-{}.csv", std::process::id()));
    let output = std::process::Command::new(exe)
        .args([
            "--scenario",
            "scenarios/competition.toml",
            "--preset",
            "quick",
            "--no-export",
            "--quiet",
            "--export-csv",
        ])
        .arg(&path)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let mut reader = csv::Reader::from_path(&path).unwrap();
    let header = reader.headers().unwrap().clone();
    let rows = reader.records().collect::<Result<Vec<_>, _>>().unwrap();
    assert_eq!(rows.len(), 15);
    let index = |name: &str| header.iter().position(|h| h == name).unwrap();
    for row in &rows {
        assert_eq!(&row[index("format_version")], "3");
        for field in [
            "cash_flow_minor",
            "cogs_minor",
            "gross_profit_minor",
            "debt_minor",
            "product_inventory_cost_basis_minor",
        ] {
            assert!(row[index(field)].parse::<i128>().is_ok());
        }
        let revenue = row[index("revenue_minor")].parse::<i128>().unwrap();
        let cogs = row[index("cogs_minor")].parse::<i128>().unwrap();
        assert_eq!(
            revenue - cogs,
            row[index("gross_profit_minor")].parse::<i128>().unwrap()
        );
    }
    let metadata_path = std::path::PathBuf::from(format!("{}.metadata.json", path.display()));
    let metadata: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&metadata_path).unwrap()).unwrap();
    assert_eq!(metadata["format_version"], 3);
    assert!(
        metadata["scenario_source"]
            .as_str()
            .unwrap()
            .contains("Test Rival")
    );
    assert_eq!(
        metadata["configuration"]["economics"]["status"],
        "PLACEHOLDER"
    );
    drop(reader);
    std::fs::remove_file(path).unwrap();
    std::fs::remove_file(metadata_path).unwrap();
}

#[test]
fn recurring_stockouts_emit_only_entry_event() {
    let mut c = controlled();
    c.products[0].production_capacity = 1;
    let mut s = Simulation::new(1, 42, c).unwrap();
    s.advance_years(5).unwrap();
    assert_eq!(
        s.events()
            .iter()
            .filter(|e| e.event_type == EventType::Stockout)
            .count(),
        1
    );
    assert!(
        s.history()
            .records()
            .iter()
            .all(|r| r.products[0].unmet_demand > 0)
    );
}

#[test]
fn comparison_exports_include_competitor_results() {
    let mut c = controlled();
    competitor(&mut c);
    let request = reminisu_economy::control::RunRequest {
        start_year: 1,
        end_year: 3,
        seed: 42,
    };
    let outcomes = reminisu_economy::control::compare_seeds(&request, &c, [42, 42]).unwrap();
    assert_eq!(outcomes[0], outcomes[1]);
    assert_eq!(outcomes[0].companies.len(), 2);
    assert_eq!(
        outcomes[0].companies[0].ending_net_worth,
        outcomes[0].ending_net_worth
    );
    assert_eq!(outcomes[0].companies[1].name, "Rival");
}
