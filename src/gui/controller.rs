use crate::{
    config::{self, SimulationConfig},
    control::RunRequest,
    export,
    simulation::{Simulation, SimulationError},
};
use std::path::Path;

/// Owns the only live simulation. Selections and pending work are presentation state.
pub struct Controller {
    simulation: Simulation,
    pub company: usize,
    pub product: usize,
    target: Option<i32>,
}
impl Controller {
    pub fn new(start: i32, seed: u64, config: SimulationConfig) -> Result<Self, SimulationError> {
        let simulation = RunRequest {
            start_year: start,
            end_year: start,
            seed,
        }
        .initialize(config)?;
        Ok(Self {
            simulation,
            company: 0,
            product: 0,
            target: None,
        })
    }
    pub fn simulation(&self) -> &Simulation {
        &self.simulation
    }
    pub fn busy(&self) -> bool {
        self.target.is_some()
    }
    pub fn cancel(&mut self) {
        self.target = None;
    }
    pub fn select_company(&mut self, index: usize) {
        if index < self.simulation.companies().len() {
            self.company = index;
            self.product = 0;
        }
    }
    pub fn select_product(&mut self, index: usize) {
        if index < self.simulation.companies()[self.company].products.len() {
            self.product = index;
        }
    }
    pub fn replace(
        &mut self,
        start: i32,
        seed: u64,
        config: SimulationConfig,
    ) -> Result<(), SimulationError> {
        *self = Self::new(start, seed, config)?;
        Ok(())
    }
    pub fn reset(&mut self) -> Result<(), SimulationError> {
        self.replace(
            self.simulation.start_year(),
            self.simulation.seed(),
            self.simulation.config().clone(),
        )
    }
    pub fn run_until(&mut self, year: i32) -> Result<(), SimulationError> {
        RunRequest {
            start_year: self.simulation.current_year(),
            end_year: year,
            seed: self.simulation.seed(),
        }
        .validate()?;
        self.target = Some(year);
        Ok(())
    }
    pub fn advance(&mut self, count: u32) -> Result<(), SimulationError> {
        let target = i64::from(self.simulation.current_year()) + i64::from(count) - 1;
        let target = i32::try_from(target)
            .map_err(|_| SimulationError::InvalidInput("year overflow".into()))?;
        self.run_until(target)
    }
    /// One atomic engine step per UI frame; failures stop the queue.
    pub fn tick(&mut self) -> Result<(), SimulationError> {
        if let Some(target) = self.target {
            if let Err(error) = self.simulation.advance_one_year() {
                self.cancel();
                return Err(error);
            }
            if self.simulation.current_year() > target {
                self.cancel();
            }
        }
        Ok(())
    }
    pub fn export(&self, path: &Path, csv: bool) -> Result<(), SimulationError> {
        validate_export_path(path, if csv { "csv" } else { "json" })?;
        if csv {
            export::write_csv(&self.simulation, path)?;
            export::write_document(
                &export::document(&self.simulation)["metadata"],
                &std::path::PathBuf::from(format!("{}.metadata.json", path.display())),
            )
        } else {
            export::write_json(&self.simulation, path)
        }
    }
}
pub fn validate_export_path(path: &Path, extension: &str) -> Result<(), SimulationError> {
    if !path
        .extension()
        .is_some_and(|s| s.eq_ignore_ascii_case(extension))
    {
        return Err(SimulationError::InvalidInput(format!(
            "Choose a .{extension} output file; configuration TOML files cannot be export targets"
        )));
    }
    Ok(())
}
pub fn validate_documents(
    engine: &str,
    scenario: &str,
) -> Result<SimulationConfig, SimulationError> {
    let config = config::load_documents(engine, scenario)?;
    crate::simulation::validate_config(&config)?;
    // Initialization also checks valuation and derived-market overflows.
    Simulation::new(3421, 42, config.clone())?;
    Ok(config)
}
pub fn load_configuration(
    engine: &str,
    scenario: &str,
) -> Result<SimulationConfig, SimulationError> {
    validate_documents(
        &std::fs::read_to_string(engine).map_err(export::io_error)?,
        &std::fs::read_to_string(scenario).map_err(export::io_error)?,
    )
}
pub fn seeds(text: &str, first: u64, count: u32) -> Result<Vec<u64>, SimulationError> {
    let invalid = || {
        SimulationError::InvalidInput(
            "Use comma-separated u64 seeds, or 1–1000 consecutive seeds without overflow".into(),
        )
    };
    if !text.trim().is_empty() {
        let values = text
            .split(',')
            .map(|s| s.trim().parse::<u64>().map_err(|_| invalid()))
            .collect::<Result<Vec<_>, _>>()?;
        if values.len() > 1000 {
            return Err(invalid());
        }
        Ok(values)
    } else {
        if count == 0 || count > 1000 {
            return Err(invalid());
        }
        (0..count)
            .map(|i| first.checked_add(u64::from(i)).ok_or_else(invalid))
            .collect()
    }
}
