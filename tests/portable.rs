use reminisu_economy::{
    config::SimulationConfig,
    currency::{Currency, Money},
    portable::{self, Document},
    presentation,
    simulation::Simulation,
};

#[test]
fn scenarios_round_trip_both_formats_and_reproduce_seeded_history() {
    let config = reminisu_economy::config::load_documents(
        include_str!("../config/default.toml"),
        include_str!("../scenarios/competition.toml"),
    )
    .unwrap();
    let original = Document::scenario(config.clone(), 3421, u64::MAX);
    for toml in [false, true] {
        let text = portable::encode(&original, toml).unwrap();
        let decoded = portable::decode(&text, toml).unwrap();
        assert_eq!(decoded, original);
        let (c, y, s) =
            portable::import_candidate(&SimulationConfig::default(), decoded, 0, 0, 0).unwrap();
        let mut a = Simulation::new(y, s, c).unwrap();
        let mut b = Simulation::new(3421, u64::MAX, config.clone()).unwrap();
        a.advance_years(10).unwrap();
        b.advance_years(10).unwrap();
        assert_eq!(a, b);
    }
}
#[test]
fn companies_and_products_round_trip_exact_extreme_money() {
    let mut company = portable::company_at(&SimulationConfig::default(), 0);
    company.faction.starting_cash = Money::from_minor(Currency::Ufc, i128::MAX / 4 + 17);
    company.faction.starting_productive_assets =
        Some(Money::from_minor(Currency::Ufc, 123456789012345678901));
    company.products[0].starting_inventory_basis =
        Some(Money::from_minor(Currency::Ufc, 999999999999999999));
    for toml in [false, true] {
        for doc in [
            Document::company(company.clone()),
            Document::product(company.products[0].clone()),
        ] {
            let encoded = portable::encode(&doc, toml).unwrap();
            assert!(encoded.contains("minor_units"));
            assert_eq!(portable::decode(&encoded, toml).unwrap(), doc);
        }
    }
}
#[test]
fn invalid_import_is_transactional_and_human_readable() {
    let config = SimulationConfig::default();
    let before = config.clone();
    let mut company = portable::company_at(&config, 0);
    company.faction.name = "Imported".into();
    company.products[0].maximum_capacity = Some(0);
    let error =
        portable::import_candidate(&config, Document::company(company), 0, 3421, 42).unwrap_err();
    assert!(
        error
            .to_string()
            .contains("Maximum production capacity cannot be lower")
    );
    assert_eq!(config, before);
    let duplicate = portable::company_at(&config, 0);
    assert!(
        portable::import_candidate(&config, Document::company(duplicate), 0, 3421, 42)
            .unwrap_err()
            .to_string()
            .contains("already exists")
    );
    assert_eq!(config, before);
}
#[test]
fn imports_append_in_order_without_changing_existing_values() {
    let config = SimulationConfig::default();
    let mut company = portable::company_at(&config, 0);
    company.faction.name = "Second".into();
    let (mut candidate, _, _) =
        portable::import_candidate(&config, Document::company(company.clone()), 0, 3421, 42)
            .unwrap();
    assert_eq!(candidate.competitors.last(), Some(&company));
    assert_eq!(candidate.products, config.products);
    let mut p = company.products[0].clone();
    p.name = "Extra product".into();
    let old = candidate.clone();
    candidate = portable::import_candidate(&candidate, Document::product(p.clone()), 1, 3421, 42)
        .unwrap()
        .0;
    assert_eq!(candidate.competitors[0].products.last(), Some(&p));
    assert_eq!(candidate.products, old.products);
}
#[test]
fn unsupported_mixed_unknown_and_analysis_documents_are_rejected() {
    let original = Document::scenario(SimulationConfig::default(), 3421, 42);
    let mut v = portable::exact_value(&original).unwrap();
    v["format_version"] = 99.into();
    assert!(portable::decode(&v.to_string(), false).is_err());
    v["format_version"] = 1.into();
    v["unknown_field"] = true.into();
    assert!(portable::decode(&v.to_string(), false).is_err());
    let mut mixed = original.clone();
    mixed.product = Some(SimulationConfig::default().products[0].clone());
    assert!(mixed.validate_shape().is_err());
    let sim = Simulation::new(3421, 42, SimulationConfig::default()).unwrap();
    assert!(
        portable::decode(&reminisu_economy::export::document(&sim).to_string(), false).is_err()
    );
    assert!(
        portable::decode(
            &reminisu_economy::export::snapshot_document(&sim, 0)
                .unwrap()
                .to_string(),
            false
        )
        .is_err()
    );
}
#[test]
fn added_starting_values_preserve_accounting_and_legacy_defaults() {
    let config = SimulationConfig::default();
    let mut explicit = config.clone();
    explicit.pilton.starting_productive_assets = Some(Money::zero(Currency::Ufc));
    for p in &mut explicit.products {
        p.starting_inventory_basis = Some(
            p.production_cost
                .checked_mul_quantity(p.starting_inventory)
                .unwrap(),
        );
    }
    let mut a = Simulation::new(3421, 42, config.clone()).unwrap();
    let mut b = Simulation::new(3421, 42, explicit).unwrap();
    a.advance_years(8).unwrap();
    b.advance_years(8).unwrap();
    assert_eq!(a.history(), b.history());
    assert_eq!(a.events(), b.events());
    let mut custom = config;
    custom.pilton.starting_productive_assets = Some(Money::from_units(Currency::Ufc, 10000));
    custom.products[0].starting_inventory_basis = Some(Money::from_minor(Currency::Ufc, 777));
    let mut sim = Simulation::new(3421, 42, custom).unwrap();
    assert_eq!(
        sim.companies()[0].products[0]
            .inventory_cost_basis
            .minor_units(),
        777
    );
    assert_eq!(sim.companies()[0].productive_assets.minor_units(), 1000000);
    sim.advance_years(5).unwrap();
    for r in sim.history().records() {
        assert_eq!(
            r.net_worth,
            r.cash + r.inventory_value + r.technology_asset_value - r.debt
        );
        assert_eq!(r.profit, r.revenue - r.expenses);
    }
}
#[test]
fn added_fields_reject_negative_mismatched_or_empty_stock_basis() {
    let mut c = SimulationConfig::default();
    c.pilton.starting_productive_assets = Some(Money::from_minor(Currency::Ufc, -1));
    assert!(Simulation::new(3421, 42, c.clone()).is_err());
    c.pilton.starting_productive_assets = Some(Money::from_minor(Currency::Rimund, 1));
    assert!(Simulation::new(3421, 42, c.clone()).is_err());
    c.pilton.starting_productive_assets = None;
    c.products[0].starting_inventory = 0;
    c.products[0].starting_inventory_basis = Some(Money::from_minor(Currency::Ufc, 1));
    assert!(Simulation::new(3421, 42, c).is_err());
}
#[test]
fn diagrams_and_explanations_are_read_only_and_cash_reconciles() {
    let mut sim = Simulation::new(3421, 42, SimulationConfig::default()).unwrap();
    sim.advance_years(20).unwrap();
    let before = sim.clone();
    for r in sim.history().records() {
        let stages = presentation::cash_stages(r).unwrap();
        let mut cash = stages[0].1;
        for (name, value) in stages.iter().skip(1).take(stages.len() - 2) {
            cash = if name.starts_with('+') {
                cash.checked_add(*value)
            } else {
                cash.checked_sub(*value)
            }
            .unwrap();
        }
        assert_eq!(cash, r.cash);
    }
    for e in sim.events() {
        let text = presentation::explain(&sim, e);
        assert!(!text.is_empty());
    }
    let _ = presentation::health(&sim, 0);
    assert_eq!(sim, before);
}
#[test]
fn suspension_explanation_uses_historical_cost_and_closing_price() {
    let mut c = SimulationConfig::default();
    c.economics.inflation_bps = 0;
    c.economics.macro_volatility_bps = 0;
    c.economics.shock_probability_percent = 0;
    c.products.truncate(1);
    c.products[0].production_cost = Money::from_units(Currency::Ufc, 99);
    c.products[0].starting_market_value = Money::from_units(Currency::Ufc, 100);
    c.products[0].popularity_percent = 0;
    c.demand.yearly_variation_percent = 0;
    c.demand.momentum_percent = 0;
    c.demand.price_sensitivity_percent = 0;
    let mut sim = Simulation::new(3421, 42, c).unwrap();
    sim.advance_years(3).unwrap();
    let events: Vec<_> = sim
        .events()
        .iter()
        .filter(|e| e.event_type == reminisu_economy::simulation::EventType::ProductionSuspended)
        .collect();
    assert_eq!(events.len(), 1);
    for e in events {
        let r = sim
            .history()
            .records()
            .iter()
            .find(|r| r.year == e.year && r.faction_name == e.entities[0])
            .unwrap();
        let p = r.products.iter().find(|p| p.name == e.entities[1]).unwrap();
        let text = presentation::explain(&sim, e);
        assert!(text.contains(&p.production_cost.to_string()));
        assert!(text.contains(&p.market_value.to_string()));
        assert!(text.contains("year end"));
    }
}

#[test]
fn company_reimport_can_replace_selected_without_reordering() {
    let base = SimulationConfig::default();
    let company = portable::company_at(&base, 0);
    let restored = portable::replace_company_candidate(&base, company, 0, 3421, 42).unwrap();
    assert_eq!(restored, base);
    let mut invalid = portable::company_at(&base, 0);
    invalid.faction.starting_debt = Some(Money::from_units(Currency::Ufc, 999999999));
    assert!(portable::replace_company_candidate(&base, invalid, 0, 3421, 42).is_err());
}
#[test]
fn decimal_money_input_is_exact_and_rejects_rounding() {
    for n in [0, 1, -1, 12345678901234567890123, i128::MAX, i128::MIN] {
        assert_eq!(
            portable::parse_amount(&portable::format_amount(n)).unwrap(),
            n
        );
    }
    assert_eq!(portable::parse_amount("1250000.01").unwrap(), 125000001);
    assert_eq!(portable::parse_amount(".5").unwrap(), 50);
    for s in ["", "1.001", "1e5", "1,000.00", "NaN", "--1", "1.2.3"] {
        assert!(portable::parse_amount(s).is_err(), "{s}");
    }
}
#[test]
fn production_trace_records_request_before_funding_without_changing_decision() {
    let mut c = SimulationConfig::default();
    c.products.truncate(1);
    c.products[0].starting_inventory = 0;
    c.pilton.starting_cash = Money::zero(Currency::Ufc);
    c.pilton.credit_limit = None;
    let mut sim = Simulation::new(3421, 42, c).unwrap();
    sim.advance_one_year().unwrap();
    let p = &sim.history().records()[0].products[0];
    assert!(p.requested_production.unwrap() > 0);
    assert_eq!(p.units_produced, 0);
}

#[test]
fn documented_examples_import_in_both_formats() {
    let json = portable::decode(include_str!("../scenarios/portable-company.json"), false).unwrap();
    let toml = portable::decode(include_str!("../scenarios/portable-company.toml"), true).unwrap();
    assert_eq!(json, toml);
    let mut base = SimulationConfig::default();
    base.markets.clear();
    let candidate =
        portable::replace_company_candidate(&base, json.company.unwrap(), 0, 3421, 42).unwrap();
    Simulation::new(3421, 42, candidate)
        .unwrap()
        .advance_one_year()
        .unwrap();
}

#[test]
fn health_labels_explain_recorded_finances_without_new_engine_rules() {
    let mut c = SimulationConfig::default();
    c.pilton.starting_cash = Money::from_units(Currency::Ufc, 1);
    c.pilton.base_operating_expenses = Money::from_units(Currency::Ufc, 2);
    assert_eq!(
        presentation::health(&Simulation::new(3421, 42, c).unwrap(), 0).label,
        "Low Cash"
    );
    let mut c = SimulationConfig::default();
    c.pilton.starting_cash = Money::from_units(Currency::Ufc, 1000000);
    c.pilton.starting_debt = Some(Money::from_units(Currency::Ufc, 90));
    c.pilton.credit_limit = Some(Money::from_units(Currency::Ufc, 100));
    assert_eq!(
        presentation::health(&Simulation::new(3421, 42, c).unwrap(), 0).label,
        "High Debt"
    );
    let mut c = SimulationConfig::default();
    c.pilton.starting_cash = Money::zero(Currency::Ufc);
    c.pilton.credit_limit = None;
    c.economics.bankruptcy_after_years = 1;
    for p in &mut c.products {
        p.starting_inventory = 0;
        p.production_capacity = 0;
    }
    let mut sim = Simulation::new(3421, 42, c).unwrap();
    sim.advance_one_year().unwrap();
    let health = presentation::health(&sim, 0);
    assert_eq!(health.label, "Bankrupt");
    assert!(
        health
            .reasons
            .iter()
            .any(|r| r.contains("Cash fell below zero"))
    );
}
#[test]
fn toml_unsigned_quantities_preserve_full_range() {
    let mut c = SimulationConfig::default();
    c.products[0].starting_inventory = u64::MAX;
    c.products[0].production_cost = Money::zero(Currency::Ufc);
    c.products[0].starting_market_value = Money::zero(Currency::Ufc);
    let doc = Document::scenario(c, 3421, 42);
    let text = portable::encode(&doc, true).unwrap();
    assert_eq!(portable::decode(&text, true).unwrap(), doc);
}
