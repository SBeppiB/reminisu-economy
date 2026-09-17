//! Versioned analysis exports, deliberately not resumable simulation saves.
use crate::simulation::{Simulation, SimulationError};
use serde_json::{Value, json};
use std::path::Path;
pub const HISTORY_FORMAT_VERSION: u32 = 3;
/// Shared format-2 seed comparison envelope; callers may add source provenance.
pub fn comparison_document(
    request: &crate::control::RunRequest,
    config: &crate::config::SimulationConfig,
    outcomes: &[crate::control::SeedOutcome],
) -> Value {
    json!({"metadata": {"kind": "seed_comparison", "format_version": 2,
        "simulation_version": env!("CARGO_PKG_VERSION"), "start_year": request.start_year,
        "end_year": request.end_year, "configuration": config}, "outcomes": outcomes})
}
pub fn document(sim: &Simulation) -> Value {
    let metadata = json!({
        "kind":"history_export", "format_version":HISTORY_FORMAT_VERSION,
        "simulation_version":env!("CARGO_PKG_VERSION"), "save_format_version":null,
        "seed":sim.seed(), "start_year":sim.start_year(),
        "end_year":sim.history().records().last().map(|r|r.year), "next_year":sim.current_year(),
        "configuration":sim.config(), "currency":sim.current_snapshot().primary_currency.code(),
        "reference_currency":sim.exchange_table().reference_currency().code(),
        "exchange_table":sim.exchange_table(),
        "money_encoding":"integer minor units; 100 per unit; parse as arbitrary precision integers"
    });
    json!({"metadata":metadata,"events":sim.events(),"companies":sim.companies(),
        "markets":sim.markets(),"economy":sim.economy(),
        "records":sim.history().records().iter().map(record_document).collect::<Vec<_>>()})
}
fn record_document(r: &crate::history::YearHistoryRecord) -> Value {
    let mut value = json!({
        "year":r.year,"faction_name":r.faction_name,"currency":r.cash.currency().code(),
        "cash_minor":r.cash.minor_units(),"revenue_minor":r.revenue.minor_units(),
        "expenses_minor":r.expenses.minor_units(),"profit_minor":r.profit.minor_units(),
        "net_worth_minor":r.net_worth.minor_units(),"inventory_value_minor":r.inventory_value.minor_units(),
        "technology_asset_value_minor":r.technology_asset_value.minor_units(),
        "debt_minor":r.debt.minor_units(),"inventory_cost_basis_minor":r.inventory_cost_basis.minor_units()
    });
    value["accounts"] = json!(r.accounts);
    value["bankrupt"] = json!(r.bankrupt);
    value["distress_years"] = json!(r.distress_years);
    value["economy"] = json!(r.economy);
    value["markets"] = json!(r.markets);
    value["products"] = json!(r.products.iter().map(product_document).collect::<Vec<_>>());
    value
}
fn product_document(p: &crate::history::ProductHistoryRecord) -> Value {
    let mut value = json!({
        "name":p.name,"category":p.category,"units_produced":p.units_produced,"units_sold":p.units_sold,
        "remaining_inventory":p.remaining_inventory,"demand":p.demand,"unmet_demand":p.unmet_demand,
        "market_value_minor":p.market_value.minor_units(),"revenue_minor":p.revenue.minor_units(),
        "cost_of_goods_sold_minor":p.cost_of_goods_sold.minor_units(),
        "production_spending_minor":p.production_spending.minor_units(),
        "inventory_cost_basis_minor":p.inventory_cost_basis.minor_units()
    });
    value["production_cost_minor"] = json!(p.production_cost.minor_units());
    value["production_capacity"] = json!(p.production_capacity);
    value["capacity_utilization_bps"] = json!(p.capacity_utilization_bps);
    value["market_share_bps"] = json!(p.market_share_bps);
    value["research_progress_bps"] = json!(p.research_progress_bps);
    value
}
pub fn write_json(sim: &Simulation, path: &Path) -> Result<(), SimulationError> {
    write_document(&document(sim), path)
}
pub fn write_document(value: &Value, path: &Path) -> Result<(), SimulationError> {
    let file = std::fs::File::create(path).map_err(io_error)?;
    serde_json::to_writer_pretty(file, value).map_err(io_error)
}
pub fn io_error(e: impl std::fmt::Display) -> SimulationError {
    SimulationError::Io(e.to_string())
}

/// One row per product/year. Faction totals repeat; do not sum them across products.
pub fn write_csv(sim: &Simulation, path: &Path) -> Result<(), SimulationError> {
    let mut writer = csv::Writer::from_path(path).map_err(io_error)?;
    writer
        .write_record([
            "format_version",
            "simulation_version",
            "seed",
            "start_year",
            "end_year",
            "year",
            "currency",
            "reference_currency",
            "faction",
            "cash_minor",
            "revenue_minor",
            "expenses_minor",
            "profit_minor",
            "net_worth_minor",
            "product",
            "demand",
            "produced",
            "sold",
            "inventory",
            "price_minor",
            "product_revenue_minor",
            "cogs_minor",
            "operating_expenses_minor",
            "production_spending_minor",
            "inventory_cost_basis_minor",
            "gross_profit_minor",
            "operating_profit_minor",
            "net_profit_minor",
            "cash_flow_minor",
            "interest_expense_minor",
            "debt_minor",
            "debt_taken_minor",
            "debt_repaid_minor",
            "maintenance_minor",
            "research_minor",
            "depreciation_minor",
            "capital_spending_minor",
            "category",
            "market_share_bps",
            "capacity",
            "utilization_bps",
            "research_progress_bps",
            "bankrupt",
            "product_cogs_minor",
            "product_inventory_cost_basis_minor",
            "unmet_demand",
            "operating_cash_flow_minor",
            "inventory_value_minor",
            "productive_assets_minor",
            "product_production_spending_minor",
            "product_unit_cost_minor",
            "growth_bps",
            "inflation_bps",
            "base_interest_bps",
            "demand_conditions_bps",
            "distress_years",
        ])
        .map_err(io_error)?;
    for r in sim.history().records() {
        for p in &r.products {
            writer
                .write_record([
                    HISTORY_FORMAT_VERSION.to_string(),
                    env!("CARGO_PKG_VERSION").into(),
                    sim.seed().to_string(),
                    sim.start_year().to_string(),
                    sim.history().records().last().unwrap().year.to_string(),
                    r.year.to_string(),
                    r.cash.currency().code().into(),
                    sim.exchange_table().reference_currency().code().into(),
                    r.faction_name.clone(),
                    r.cash.minor_units().to_string(),
                    r.revenue.minor_units().to_string(),
                    r.expenses.minor_units().to_string(),
                    r.profit.minor_units().to_string(),
                    r.net_worth.minor_units().to_string(),
                    p.name.clone(),
                    p.demand.to_string(),
                    p.units_produced.to_string(),
                    p.units_sold.to_string(),
                    p.remaining_inventory.to_string(),
                    p.market_value.minor_units().to_string(),
                    p.revenue.minor_units().to_string(),
                    r.accounts.cost_of_goods_sold.minor_units().to_string(),
                    r.accounts.operating_expenses.minor_units().to_string(),
                    r.accounts.production_spending.minor_units().to_string(),
                    r.inventory_cost_basis.minor_units().to_string(),
                    r.accounts.gross_profit.minor_units().to_string(),
                    r.accounts.operating_profit.minor_units().to_string(),
                    r.accounts.net_profit.minor_units().to_string(),
                    r.accounts.cash_flow.minor_units().to_string(),
                    r.accounts.interest_expense.minor_units().to_string(),
                    r.debt.minor_units().to_string(),
                    r.accounts.debt_taken.minor_units().to_string(),
                    r.accounts.debt_repaid.minor_units().to_string(),
                    r.accounts.maintenance.minor_units().to_string(),
                    r.accounts.research.minor_units().to_string(),
                    r.accounts.depreciation.minor_units().to_string(),
                    r.accounts.capital_spending.minor_units().to_string(),
                    p.category.clone(),
                    p.market_share_bps.to_string(),
                    p.production_capacity.to_string(),
                    p.capacity_utilization_bps.to_string(),
                    p.research_progress_bps.to_string(),
                    r.bankrupt.to_string(),
                    p.cost_of_goods_sold.minor_units().to_string(),
                    p.inventory_cost_basis.minor_units().to_string(),
                    p.unmet_demand.to_string(),
                    r.accounts.operating_cash_flow.minor_units().to_string(),
                    r.inventory_value.minor_units().to_string(),
                    r.technology_asset_value.minor_units().to_string(),
                    p.production_spending.minor_units().to_string(),
                    p.production_cost.minor_units().to_string(),
                    r.economy.growth_bps.to_string(),
                    r.economy.inflation_bps.to_string(),
                    r.economy.interest_bps.to_string(),
                    r.economy.demand_conditions_bps.to_string(),
                    r.distress_years.to_string(),
                ])
                .map_err(io_error)?;
        }
    }
    writer.flush().map_err(io_error)
}
