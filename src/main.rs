use reminisu_economy::{config, control, export, simulation};
use std::env;
use std::path::PathBuf;
use std::process::ExitCode;

use simulation::{Simulation, SimulationError};

#[derive(Debug, Clone, PartialEq, Eq)]
struct CliArgs {
    start_year: i32,
    end_year: i32,
    seed: u64,
    export_history: Option<PathBuf>,
    config_path: PathBuf,
    scenario_path: PathBuf,
    csv_path: Option<PathBuf>,
    verbosity: u8,
    summary_every: u32,
    validate: bool,
    batch: Option<u32>,
    seeds: Option<Vec<u64>>,
    timing: bool,
}

impl Default for CliArgs {
    fn default() -> Self {
        Self {
            config_path: "config/default.toml".into(),
            scenario_path: "scenarios/testing.toml".into(),
            csv_path: None,
            verbosity: 1,
            summary_every: 10,
            validate: false,
            batch: None,
            seeds: None,
            timing: false,
            start_year: 3421,
            end_year: 3455,
            seed: 42,
            export_history: Some(PathBuf::from("history.json")),
        }
    }
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("error: {err}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), SimulationError> {
    let args = parse_args(env::args().skip(1))?;
    let engine_text = std::fs::read_to_string(&args.config_path).map_err(export::io_error)?;
    let scenario_text = std::fs::read_to_string(&args.scenario_path).map_err(export::io_error)?;
    let config = config::load_documents(&engine_text, &scenario_text)?;
    let request = control::RunRequest {
        start_year: args.start_year,
        end_year: args.end_year,
        seed: args.seed,
    };
    let mut simulation = request.initialize(config.clone())?;
    if args.validate {
        println!(
            "Configuration, scenario, currencies and year range are valid. No years simulated."
        );
        return Ok(());
    }
    let started = std::time::Instant::now();
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(export::io_error)?
        .as_secs();
    if args.batch.is_some() || args.seeds.is_some() {
        let seeds = if let Some(seeds) = &args.seeds {
            seeds.clone()
        } else {
            (0..args.batch.unwrap())
                .map(|offset| {
                    args.seed.checked_add(u64::from(offset)).ok_or_else(|| {
                        SimulationError::InvalidInput("batch seed range overflow".into())
                    })
                })
                .collect::<Result<Vec<_>, _>>()?
        };
        let outcomes = control::compare_seeds(&request, &config, seeds)?;
        for outcome in &outcomes {
            println!(
                "Seed {}: ending net worth {}, total profit {}, events {}",
                outcome.seed, outcome.ending_net_worth, outcome.total_profit, outcome.major_events
            );
            for company in outcome.companies.iter().skip(1) {
                println!(
                    "  {}: ending net worth {}, total profit {}, debt {}, bankrupt {}",
                    company.name,
                    company.ending_net_worth,
                    company.total_profit,
                    company.debt,
                    company.bankrupt
                );
            }
        }
        if let Some(path) = &args.export_history {
            let mut document = export::comparison_document(&request, &config, &outcomes);
            document["metadata"]["engine_source"] = serde_json::json!(engine_text);
            document["metadata"]["scenario_source"] = serde_json::json!(scenario_text);
            document["metadata"]["timestamp_unix_seconds"] = serde_json::json!(timestamp);
            export::write_document(&document, path)?;
        }
    } else {
        simulation.run_until(args.end_year)?;
        print_history(&simulation, &args)?;
        let mut document = export::document(&simulation);
        document["metadata"]["engine_path"] = serde_json::json!(args.config_path);
        document["metadata"]["scenario_path"] = serde_json::json!(args.scenario_path);
        document["metadata"]["engine_source"] = serde_json::json!(engine_text);
        document["metadata"]["scenario_source"] = serde_json::json!(scenario_text);
        document["metadata"]["timestamp_unix_seconds"] = serde_json::json!(timestamp);
        if let Some(path) = &args.export_history {
            export::write_document(&document, path)?;
        }
        if let Some(path) = &args.csv_path {
            export::write_csv(&simulation, path)?;
            // Sidecar carries full provenance without repeating documents in every CSV row.
            let metadata_path = PathBuf::from(format!("{}.metadata.json", path.display()));
            export::write_document(&document["metadata"], &metadata_path)?;
        }
    }
    if args.timing {
        eprintln!("Elapsed: {:.3}s", started.elapsed().as_secs_f64());
    }

    Ok(())
}

fn parse_args<I>(args: I) -> Result<CliArgs, SimulationError>
where
    I: IntoIterator<Item = String>,
{
    let mut parsed = CliArgs::default();
    let mut iter = args.into_iter();

    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "validate" => parsed.validate = true,
            "--config" => parsed.config_path = next_arg(&mut iter, "--config")?.into(),
            "--scenario" => parsed.scenario_path = next_arg(&mut iter, "--scenario")?.into(),
            "--export-csv" => parsed.csv_path = Some(next_arg(&mut iter, "--export-csv")?.into()),
            "--quiet" => parsed.verbosity = 0,
            "--normal" => parsed.verbosity = 1,
            "--verbose" => parsed.verbosity = 2,
            "--summary-every" => parsed.summary_every = parse_next(&mut iter, "--summary-every")?,
            "--batch" => parsed.batch = Some(parse_next(&mut iter, "--batch")?),
            "--seeds" => {
                parsed.seeds = Some(
                    next_arg(&mut iter, "--seeds")?
                        .split(',')
                        .map(|s| {
                            s.parse().map_err(|_| {
                                SimulationError::InvalidInput("invalid seed list".into())
                            })
                        })
                        .collect::<Result<Vec<_>, _>>()?,
                )
            }
            "--timing" => parsed.timing = true,
            "--random-seed" => {
                parsed.seed = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_err(export::io_error)?
                    .as_nanos() as u64
            }
            "--preset" => {
                parsed.start_year = 3421;
                parsed.end_year = match next_arg(&mut iter, "--preset")?.as_str() {
                    "quick" => 3425,
                    "3421-3455" => 3455,
                    "3421-4000" => 4000,
                    _ => return Err(SimulationError::InvalidInput("unknown preset".into())),
                };
            }
            "--start-year" => {
                parsed.start_year = parse_next(&mut iter, "--start-year")?;
            }
            "--end-year" => {
                parsed.end_year = parse_next(&mut iter, "--end-year")?;
            }
            "--seed" => {
                parsed.seed = parse_next(&mut iter, "--seed")?;
            }
            "--export-history" => {
                parsed.export_history =
                    Some(PathBuf::from(next_arg(&mut iter, "--export-history")?));
            }
            "--no-export" => {
                parsed.export_history = None;
            }
            "--help" | "-h" => {
                print_help();
                std::process::exit(0);
            }
            unknown => {
                return Err(SimulationError::InvalidInput(format!(
                    "unknown argument '{unknown}'"
                )));
            }
        }
    }

    if parsed.csv_path.as_ref() == parsed.export_history.as_ref() && parsed.csv_path.is_some() {
        return Err(SimulationError::InvalidInput(
            "JSON and CSV output paths must differ".into(),
        ));
    }
    if parsed.summary_every == 0 || parsed.batch == Some(0) {
        return Err(SimulationError::InvalidInput(
            "interval and batch count must be positive".into(),
        ));
    }
    if parsed.batch.is_some() && parsed.seeds.is_some() {
        return Err(SimulationError::InvalidInput(
            "use either --batch or --seeds".into(),
        ));
    }
    if parsed.csv_path.is_some() && (parsed.batch.is_some() || parsed.seeds.is_some()) {
        return Err(SimulationError::InvalidInput(
            "CSV currently supports single runs only".into(),
        ));
    }
    control::RunRequest {
        start_year: parsed.start_year,
        end_year: parsed.end_year,
        seed: parsed.seed,
    }
    .validate()?;
    Ok(parsed)
}

fn parse_next<T, I>(iter: &mut I, name: &str) -> Result<T, SimulationError>
where
    T: std::str::FromStr,
    I: Iterator<Item = String>,
{
    let value = next_arg(iter, name)?;
    value
        .parse()
        .map_err(|_| SimulationError::InvalidInput(format!("invalid value '{value}' for {name}")))
}

fn next_arg<I>(iter: &mut I, name: &str) -> Result<String, SimulationError>
where
    I: Iterator<Item = String>,
{
    iter.next()
        .ok_or_else(|| SimulationError::InvalidInput(format!("missing value for {name}")))
}

fn print_help() {
    println!("Reminisu Economy MVP");
    println!("Usage: cargo run -- [--start-year 3421] [--end-year 3455] [--seed 42]");
    println!("       [--export-history history.json] [--export-csv history.csv] [--no-export]");
    println!("       [--config config/default.toml] [--scenario scenarios/testing.toml]");
    println!("       [--quiet|--normal|--verbose] [--summary-every 10] [--timing]");
    println!("       [--preset quick|3421-3455|3421-4000] [--random-seed]");
    println!("       [--batch COUNT|--seeds 1,42,99] or: cargo run -- validate");
}

fn print_history(simulation: &Simulation, args: &CliArgs) -> Result<(), SimulationError> {
    println!("Simulation seed: {}", args.seed);
    println!("Starting year: {}", args.start_year);
    println!("Ending year: {}", args.end_year);
    println!(
        "Reference currency: {}",
        simulation.exchange_table().reference_currency().code()
    );

    for record in simulation.history().records() {
        if args.verbosity == 0 {
            continue;
        }
        if args.verbosity == 1 {
            if (i64::from(record.year) - i64::from(args.start_year) + 1)
                % i64::from(args.summary_every)
                == 0
                || record.year == args.end_year
            {
                println!(
                    "Year {}: {} cash {}, net profit {}, net worth {}",
                    record.year, record.faction_name, record.cash, record.profit, record.net_worth
                );
            }
            continue;
        }
        println!("\nYEAR {}", record.year);
        println!("\n{}", record.faction_name);
        println!("Revenue: {}", record.revenue);
        println!("Expenses: {}", record.expenses);
        println!("Profit: {}", record.profit);
        println!("COGS: {}", record.accounts.cost_of_goods_sold);
        println!(
            "Production spending: {}",
            record.accounts.production_spending
        );
        println!("Gross profit: {}", record.accounts.gross_profit);
        println!("Operating profit: {}", record.accounts.operating_profit);
        println!("Cash flow: {}", record.accounts.cash_flow);
        println!("Debt: {}", record.debt);
        println!("Interest: {}", record.accounts.interest_expense);
        println!("Cash: {}", record.cash);
        println!("Net Worth: {}", record.net_worth);

        for product in &record.products {
            println!("\n{}", product.name);
            println!("Produced: {}", product.units_produced);
            println!("Sold: {}", product.units_sold);
            println!("Inventory: {}", product.remaining_inventory);
            println!("Demand: {}", product.demand);
            println!("Market Value: {}", product.market_value);
            println!("Revenue: {}", product.revenue);
        }
    }

    for event in simulation.events() {
        if args.verbosity > 0 || event.severity == simulation::Severity::Warning {
            println!(
                "{} {:?}: {} ({})",
                event.year,
                event.severity,
                event.description,
                event.entities.join(", ")
            );
        }
    }
    for company in simulation.companies() {
        let Some(summary) = simulation.history().summary_for(&company.name)? else {
            continue;
        };
        println!("\nFINAL SUMMARY: {}", company.name);
        println!(
            "First completed year net worth: {}",
            summary.starting_net_worth
        );
        println!("Ending net worth: {}", summary.ending_net_worth);
        println!("Total revenue: {}", summary.total_revenue);
        println!("Total profit: {}", summary.total_profit);
        println!("Best year: {} ({})", summary.best_year, summary.best_profit);
        println!(
            "Worst year: {} ({})",
            summary.worst_year, summary.worst_profit
        );
        println!("Total units produced: {}", summary.total_units_produced);
        println!("Total units sold: {}", summary.total_units_sold);
        println!("Outstanding debt: {}", company.debt);
        println!("Bankrupt: {}", company.bankrupt);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_cli_arguments() {
        let args = parse_args([
            "--start-year".to_string(),
            "3421".to_string(),
            "--end-year".to_string(),
            "3455".to_string(),
            "--seed".to_string(),
            "12345".to_string(),
            "--no-export".to_string(),
        ])
        .unwrap();

        assert_eq!(args.start_year, 3421);
        assert_eq!(args.end_year, 3455);
        assert_eq!(args.seed, 12345);
        assert_eq!(args.export_history, None);
    }
}
