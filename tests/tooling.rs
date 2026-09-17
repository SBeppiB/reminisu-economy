use reminisu_economy::{
    config::{self, SimulationConfig},
    control::{self, RunRequest},
    currency::{Currency, Money},
    export,
    simulation::{EventType, Simulation},
};
#[test]
fn stepping_matches_continuous_run_and_queries() {
    let mut continuous = Simulation::new(3421, 42, SimulationConfig::default()).unwrap();
    let mut stepped = continuous.clone();
    continuous.run_until(3455).unwrap();
    stepped.advance_one_year().unwrap();
    stepped.advance_years(10).unwrap();
    stepped.run_until(3455).unwrap();
    assert_eq!(continuous, stepped);
    assert_eq!(stepped.current_year(), 3456);
    assert!(stepped.history_for_year(3440).is_some());
    assert!(stepped.history_for_year(3456).is_none());
    assert!(stepped.product_state("A.B.Y.S.S Suits").is_some());
}
#[test]
fn invalid_configuration_is_rejected_without_running() {
    let mut c = SimulationConfig::default();
    c.products[0].production_cost = Money::from_units(Currency::Rimund, 1);
    assert!(Simulation::new(3421, 42, c).is_err());
    let engine =
        include_str!("../config/default.toml").replace("momentum_percent", "momentumm_percent");
    assert!(config::load_documents(&engine, include_str!("../scenarios/testing.toml")).is_err());
    assert!(
        RunRequest {
            start_year: 10,
            end_year: 9,
            seed: 1
        }
        .validate()
        .is_err()
    );
}
#[test]
fn comparison_matches_individual_runs() {
    let request = RunRequest {
        start_year: 3421,
        end_year: 3430,
        seed: 0,
    };
    let config = SimulationConfig::default();
    let outcomes = control::compare_seeds(&request, &config, [42, 9, 42]).unwrap();
    assert_eq!(outcomes[0], outcomes[2]);
    let mut sim = Simulation::new(3421, 42, config).unwrap();
    sim.run_until(3430).unwrap();
    assert_eq!(
        outcomes[0].ending_net_worth,
        sim.history().summary().unwrap().unwrap().ending_net_worth
    );
}
#[test]
fn events_are_transitions_and_negative_cash_is_allowed() {
    let mut c = SimulationConfig::default();
    c.pilton.credit_limit = None;
    c.pilton.starting_cash = Money::zero(Currency::Ufc);
    for p in &mut c.products {
        p.starting_inventory = 0;
    }
    let mut sim = Simulation::new(3421, 42, c).unwrap();
    sim.advance_years(5).unwrap();
    assert_eq!(
        sim.events()
            .iter()
            .filter(|e| e.event_type == EventType::CashNegative)
            .count(),
        1
    );
    assert!(sim.current_snapshot().cash.minor_units() < 0);
}
#[test]
fn failed_step_is_atomic() {
    let mut c = SimulationConfig::default();
    // Price multiplication no longer overflows for the old fixture. An inflated
    // operating bill now supplies a real checked overflow after the RNG advances.
    c.pilton.base_operating_expenses = Money::from_minor(Currency::Ufc, i128::MAX);
    c.economics.inflation_bps = 200;
    c.economics.macro_volatility_bps = 0;
    let mut sim = Simulation::new(3421, 42, c).unwrap();
    let before = sim.clone();
    assert!(sim.advance_one_year().is_err());
    assert_eq!(before, sim);
}
#[test]
fn json_escapes_names_and_preserves_large_integers() {
    let mut c = SimulationConfig::default();
    c.pilton.name = "name\t\n\"".into();
    c.pilton.starting_cash = Money::from_minor(Currency::Ufc, 9007199254740993123456789);
    let mut sim = Simulation::new(3421, 42, c).unwrap();
    sim.advance_one_year().unwrap();
    let encoded = serde_json::to_string(&export::document(&sim)).unwrap();
    let decoded: serde_json::Value = serde_json::from_str(&encoded).unwrap();
    assert_eq!(
        decoded["metadata"]["configuration"]["pilton"]["starting_cash"]["minor_units"].to_string(),
        "9007199254740993123456789"
    );
    assert_eq!(decoded["records"][0]["faction_name"], "name\t\n\"");
    assert_eq!(decoded["metadata"]["kind"], "history_export");
}

#[test]
fn csv_round_trip_preserves_quoted_names_and_values() {
    let mut c = SimulationConfig::default();
    c.products[0].name = "Suit, \"special\"\nline".into();
    let mut sim = Simulation::new(3421, 42, c).unwrap();
    sim.advance_one_year().unwrap();
    let path = std::env::temp_dir().join(format!("reminisu-csv-test-{}.csv", std::process::id()));
    export::write_csv(&sim, &path).unwrap();
    let mut reader = csv::Reader::from_path(&path).unwrap();
    let rows = reader.records().collect::<Result<Vec<_>, _>>().unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(&rows[0][14], "Suit, \"special\"\nline");
    assert_eq!(
        &rows[0][9],
        sim.history().records()[0].cash.minor_units().to_string()
    );
    drop(reader);
    std::fs::remove_file(path).unwrap();
}
#[test]
fn production_event_occurs_only_when_threshold_is_crossed() {
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
        .filter(|e| e.event_type == EventType::ProductionSuspended)
        .collect();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].year, 3421);
    assert_eq!(
        sim.history_for_year(3422).unwrap().products[0].units_produced,
        0
    );
}

#[test]
fn cli_validates_options_and_controls_output() {
    let exe = env!("CARGO_BIN_EXE_reminisu_economy");
    for args in [
        vec!["--summary-every", "0"],
        vec!["--batch", "0"],
        vec!["--start-year", "4000", "--end-year", "3421"],
        vec!["--seeds", "42", "--batch", "2"],
    ] {
        assert!(
            !std::process::Command::new(exe)
                .args(args)
                .output()
                .unwrap()
                .status
                .success()
        );
    }
    let output = std::process::Command::new(exe)
        .args([
            "--preset",
            "quick",
            "--normal",
            "--summary-every",
            "2",
            "--no-export",
        ])
        .output()
        .unwrap();
    assert!(output.status.success());
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.contains("Year 3422:"));
    assert!(text.contains("Year 3424:"));
    assert!(text.contains("Year 3425:"));
    assert!(!text.contains("Year 3423:"));
    let output = std::process::Command::new(exe)
        .args(["validate"])
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(
        String::from_utf8(output.stdout)
            .unwrap()
            .contains("No years simulated")
    );
}
