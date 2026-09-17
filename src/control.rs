//! Frontend-independent run requests and sequential seed comparison.
use crate::{
    config::SimulationConfig,
    simulation::{Simulation, SimulationError},
};
#[derive(Debug, Clone)]
pub struct RunRequest {
    pub start_year: i32,
    pub end_year: i32,
    pub seed: u64,
}
impl RunRequest {
    pub fn validate(&self) -> Result<(), SimulationError> {
        if self.end_year < self.start_year || self.end_year == i32::MAX {
            return Err(SimulationError::InvalidInput(
                "invalid year range (end must be below i32::MAX)".into(),
            ));
        }
        Ok(())
    }
    pub fn initialize(&self, config: SimulationConfig) -> Result<Simulation, SimulationError> {
        self.validate()?;
        Simulation::new(self.start_year, self.seed, config)
    }
}
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct SeedOutcome {
    pub companies: Vec<CompanyOutcome>,
    pub seed: u64,
    pub ending_net_worth: crate::currency::Money,
    pub total_revenue: crate::currency::Money,
    pub total_profit: crate::currency::Money,
    pub major_events: usize,
}
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct CompanyOutcome {
    pub name: String,
    pub ending_net_worth: crate::currency::Money,
    pub total_revenue: crate::currency::Money,
    pub total_profit: crate::currency::Money,
    pub debt: crate::currency::Money,
    pub bankrupt: bool,
}
/// Keeps only one seed's history in memory. Explicit seed order is preserved.
pub fn compare_seeds(
    request: &RunRequest,
    config: &SimulationConfig,
    seeds: impl IntoIterator<Item = u64>,
) -> Result<Vec<SeedOutcome>, SimulationError> {
    request.validate()?;
    seeds
        .into_iter()
        .map(|seed| {
            let mut sim = RunRequest {
                seed,
                ..request.clone()
            }
            .initialize(config.clone())?;
            sim.run_until(request.end_year)?;
            summarize(&sim)
        })
        .collect()
}

/// Summarize a completed run without rerunning the engine.
pub fn summarize(sim: &Simulation) -> Result<SeedOutcome, SimulationError> {
    let summary = sim
        .history()
        .summary()?
        .ok_or_else(|| SimulationError::InvalidInput("No completed years".into()))?;
    let companies = sim
        .companies()
        .iter()
        .map(|f| {
            let s = sim
                .history()
                .summary_for(&f.name)?
                .ok_or_else(|| SimulationError::InvalidInput("No company history".into()))?;
            Ok(CompanyOutcome {
                name: f.name.clone(),
                ending_net_worth: s.ending_net_worth,
                total_revenue: s.total_revenue,
                total_profit: s.total_profit,
                debt: f.debt,
                bankrupt: f.bankrupt,
            })
        })
        .collect::<Result<Vec<_>, SimulationError>>()?;
    Ok(SeedOutcome {
        companies,
        seed: sim.seed(),
        ending_net_worth: summary.ending_net_worth,
        total_revenue: summary.total_revenue,
        total_profit: summary.total_profit,
        major_events: sim.events().len(),
    })
}
