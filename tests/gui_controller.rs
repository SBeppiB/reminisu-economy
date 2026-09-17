use reminisu_economy::{
    config::SimulationConfig,
    currency::{Currency, Money},
    export,
    gui::controller::{self, Controller},
    simulation::Simulation,
};

fn drain(c: &mut Controller) {
    while c.busy() {
        c.tick().unwrap();
    }
}
#[test]
fn queued_steps_are_real_engine_steps_and_reset_is_exact() {
    let config = controller::validate_documents(
        include_str!("../config/default.toml"),
        include_str!("../scenarios/competition.toml"),
    )
    .unwrap();
    let mut gui = Controller::new(3421, 42, config.clone()).unwrap();
    let initial = gui.simulation().clone();
    let mut direct = initial.clone();
    direct.run_until(3455).unwrap();
    gui.advance(1).unwrap();
    drain(&mut gui);
    gui.advance(5).unwrap();
    drain(&mut gui);
    gui.advance(10).unwrap();
    drain(&mut gui);
    gui.run_until(3455).unwrap();
    drain(&mut gui);
    assert_eq!(gui.simulation(), &direct);
    let before = gui.simulation().clone();
    gui.select_company(1);
    gui.select_product(0);
    gui.select_company(usize::MAX);
    gui.select_product(usize::MAX);
    assert_eq!(gui.simulation(), &before);
    gui.reset().unwrap();
    assert_eq!(gui.simulation(), &initial);
    assert_eq!(gui.company, 0);
    gui.replace(3500, 99, config.clone()).unwrap();
    assert_eq!(
        gui.simulation(),
        &Simulation::new(3500, 99, config).unwrap()
    );
}
#[test]
fn failed_step_and_failed_replacement_preserve_live_state() {
    let mut config = SimulationConfig::default();
    config.pilton.base_operating_expenses = Money::from_minor(Currency::Ufc, i128::MAX);
    config.economics.inflation_bps = 200;
    config.economics.macro_volatility_bps = 0;
    let mut gui = Controller::new(3421, 42, config).unwrap();
    let before = gui.simulation().clone();
    gui.advance(5).unwrap();
    assert!(gui.tick().is_err());
    assert_eq!(gui.simulation(), &before);
    assert!(!gui.busy());
    let mut invalid = SimulationConfig::default();
    invalid.economics.bankruptcy_after_years = 0;
    assert!(gui.replace(3421, 7, invalid).is_err());
    assert_eq!(gui.simulation(), &before);
    assert!(gui.run_until(i32::MAX).is_err());
    assert!(gui.advance(0).is_err());
}
#[test]
fn validation_and_seed_inputs_are_checked() {
    let engine = include_str!("../config/default.toml");
    let scenario = include_str!("../scenarios/testing.toml");
    assert!(controller::validate_documents(engine, scenario).is_ok());
    assert!(
        controller::validate_documents(&engine.replace("momentum_percent", "typo"), scenario)
            .is_err()
    );
    assert!(
        controller::validate_documents(
            &engine.replace("bankruptcy_after_years = 3", "bankruptcy_after_years = 0"),
            scenario
        )
        .is_err()
    );
    assert!(
        controller::load_configuration("nonexistent-engine.toml", "nonexistent-scenario.toml")
            .is_err()
    );
    assert_eq!(
        controller::seeds("42, 1,42", 0, 0).unwrap(),
        vec![42, 1, 42]
    );
    assert_eq!(controller::seeds("", 7, 3).unwrap(), vec![7, 8, 9]);
    assert!(controller::seeds("", u64::MAX, 2).is_err());
    assert!(controller::seeds("-1", 0, 1).is_err());
    assert!(controller::seeds("", 0, 0).is_err());
}
#[test]
fn cancellation_keeps_committed_years() {
    let mut gui = Controller::new(3421, 42, SimulationConfig::default()).unwrap();
    gui.advance(10).unwrap();
    gui.tick().unwrap();
    gui.cancel();
    let before = gui.simulation().clone();
    gui.tick().unwrap();
    assert_eq!(gui.simulation(), &before);
    assert_eq!(gui.simulation().current_year(), 3422);
}
#[test]
fn gui_exports_use_existing_exact_formats_and_report_io_errors() {
    let mut config = SimulationConfig::default();
    config.pilton.starting_cash = Money::from_minor(Currency::Ufc, 9007199254740993123456789);
    let mut gui = Controller::new(3421, 42, config).unwrap();
    gui.advance(1).unwrap();
    drain(&mut gui);
    let root = std::env::temp_dir().join(format!("reminisu-gui-tests-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let json = root.join("history.json");
    let csv = root.join("history.csv");
    gui.export(&json, false).unwrap();
    gui.export(&csv, true).unwrap();
    let actual: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&json).unwrap()).unwrap();
    assert_eq!(actual, export::document(gui.simulation()));
    assert_eq!(
        actual["metadata"]["configuration"]["pilton"]["starting_cash"]["minor_units"].to_string(),
        "9007199254740993123456789"
    );
    let reference = root.join("reference.csv");
    export::write_csv(gui.simulation(), &reference).unwrap();
    assert_eq!(
        std::fs::read(&csv).unwrap(),
        std::fs::read(&reference).unwrap()
    );
    let sidecar = root.join("history.csv.metadata.json");
    let meta: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&sidecar).unwrap()).unwrap();
    assert_eq!(meta, actual["metadata"]);
    let before = gui.simulation().clone();
    assert!(gui.export(&root, false).is_err());
    assert!(
        gui.export(&root.join("missing-directory/history.json"), false)
            .is_err()
    );
    assert!(gui.export(&root.join("configuration.toml"), false).is_err());
    assert_eq!(gui.simulation(), &before);
    for path in [json, csv, reference, sidecar] {
        std::fs::remove_file(path).unwrap();
    }
    std::fs::remove_dir(root).unwrap();
}
