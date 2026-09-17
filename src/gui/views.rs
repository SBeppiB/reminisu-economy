use super::charts::chart;
use crate::{
    currency::Money,
    simulation::{Severity, Simulation},
};
use eframe::egui::{self, Color32, RichText};

pub fn money(ui: &mut egui::Ui, label: &str, value: Money) {
    ui.label(label);
    let negative = value.minor_units() < 0;
    ui.label(
        RichText::new(format!(
            "{}{value}",
            if value.minor_units() > 0 { "+" } else { "" }
        ))
        .strong()
        .color(if negative {
            Color32::from_rgb(255, 146, 146)
        } else {
            Color32::from_rgb(112, 222, 192)
        }),
    );
    ui.end_row();
}
pub fn row(ui: &mut egui::Ui, label: &str, value: impl std::fmt::Display) {
    ui.label(label);
    ui.label(value.to_string());
    ui.end_row();
}
pub fn percent(bps: impl Into<i64>) -> String {
    let b = bps.into();
    format!(
        "{}{}.{:02}%",
        if b < 0 { "-" } else { "" },
        b.unsigned_abs() / 100,
        b.unsigned_abs() % 100
    )
}
pub fn company(ui: &mut egui::Ui, sim: &Simulation, index: usize) {
    let f = &sim.companies()[index];
    ui.heading(&f.name);
    ui.label(if f.bankrupt {
        "WARNING • BANKRUPT — operations frozen"
    } else if f.distress_years > 0 {
        "WARNING • FINANCIAL DISTRESS"
    } else {
        "ACTIVE • No recorded distress"
    });
    let latest = sim
        .history()
        .records()
        .iter()
        .rev()
        .find(|r| r.faction_name == f.name);
    ui.weak(
        latest.map_or("Opening state • annual flows unavailable".into(), |r| {
            format!("Closing balances and annual flows for {}", r.year)
        }),
    );
    egui::Grid::new("company_financials")
        .num_columns(2)
        .striped(true)
        .spacing([40.0, 9.0])
        .show(ui, |ui| {
            money(ui, "Cash", f.cash);
            money(ui, "Debt", f.debt);
            match f.financial_totals() {
                Ok(t) => {
                    money(ui, "Net worth", t.net_worth);
                    money(ui, "Inventory value", t.inventory_value);
                }
                Err(e) => {
                    row(ui, "Valuation error", e);
                }
            }
            money(ui, "Productive assets", f.productive_assets);
            if let Some(r) = latest {
                for (name, value) in [
                    ("Revenue", r.revenue),
                    ("Gross profit", r.accounts.gross_profit),
                    ("Operating profit", r.accounts.operating_profit),
                    ("Net profit", r.profit),
                    ("Operating cash flow", r.accounts.operating_cash_flow),
                ] {
                    money(ui, name, value);
                }
            }
            row(ui, "Consecutive distressed years", f.distress_years);
        });
    ui.add_space(12.0);
    ui.strong("Products / category sales shares");
    for p in &f.products {
        ui.label(format!("{} • {}", p.name, p.category));
        ui.add(
            egui::ProgressBar::new(p.market_share_bps as f32 / 10000.0)
                .text(percent(p.market_share_bps)),
        );
    }
    ui.weak("Shares are per category; unrelated markets are not added together.");
    ui.separator();
    for (title, metric) in [
        ("Net worth", 0),
        ("Cash", 1),
        ("Revenue", 2),
        ("Net profit", 3),
        ("Debt", 4),
    ] {
        let data = sim
            .history()
            .records()
            .iter()
            .filter(|r| r.faction_name == f.name)
            .map(|r| {
                (
                    r.year,
                    match metric {
                        0 => r.net_worth,
                        1 => r.cash,
                        2 => r.revenue,
                        3 => r.profit,
                        _ => r.debt,
                    }
                    .minor_units() as f64
                        / 100.0,
                )
            })
            .collect();
        chart(ui, title, f.primary_currency.code(), &[(&f.name, data)]);
    }
}
pub fn product(ui: &mut egui::Ui, sim: &Simulation, company: usize, product: usize) {
    let f = &sim.companies()[company];
    let Some(p) = f.products.get(product) else {
        ui.label("No products in this company.");
        return;
    };
    ui.heading(&p.name);
    ui.label(&p.category);
    let history: Vec<_> = sim
        .history()
        .records()
        .iter()
        .filter(|r| r.faction_name == f.name)
        .filter_map(|r| {
            r.products
                .iter()
                .find(|v| v.name == p.name)
                .map(|v| (r.year, v))
        })
        .collect();
    egui::Grid::new("product_details")
        .striped(true)
        .show(ui, |ui| {
            money(ui, "Current closing price", p.current_market_value);
            money(ui, "Unit production cost", p.production_cost);
            money(ui, "Inventory cost basis", p.inventory_cost_basis);
            row(ui, "Inventory", p.inventory);
            row(ui, "Demand (latest / initial forecast)", p.previous_demand);
            if let Some((_, r)) = history.last() {
                row(ui, "Units produced (latest year)", r.units_produced);
                row(ui, "Units sold (latest year)", r.units_sold);
            }
            row(ui, "Unmet demand", p.unmet_demand);
            row(ui, "Capacity", p.production_capacity);
            row(
                ui,
                "Capacity utilization",
                percent(p.capacity_utilization_bps),
            );
            row(ui, "Market share", percent(p.market_share_bps));
            row(ui, "R&D progress", percent(p.research_progress_bps));
            row(ui, "Attractiveness", percent(p.attractiveness_bps));
            row(ui, "Stockout", p.stockout);
            row(ui, "Excess inventory", p.excess_inventory);
            row(ui, "Production eligible", p.production_viable);
        });
    chart(
        ui,
        "Product price",
        f.primary_currency.code(),
        &[(
            "Closing price",
            history
                .iter()
                .map(|(y, r)| (*y, r.market_value.minor_units() as f64 / 100.0))
                .collect(),
        )],
    );
    chart(
        ui,
        "Demand vs sales",
        "units",
        &[
            (
                "Demand",
                history.iter().map(|(y, r)| (*y, r.demand as f64)).collect(),
            ),
            (
                "Sales",
                history
                    .iter()
                    .map(|(y, r)| (*y, r.units_sold as f64))
                    .collect(),
            ),
        ],
    );
    chart(
        ui,
        "Market share",
        "%",
        &[(
            "Category sales share",
            history
                .iter()
                .map(|(y, r)| (*y, f64::from(r.market_share_bps) / 100.0))
                .collect(),
        )],
    );
    ui.collapsing("Exact product history", |ui| {
        for (year, r) in history.iter().rev() {
            ui.label(format!(
                "{year} • price {} • demand {} • sold {} • share {}",
                r.market_value,
                r.demand,
                r.units_sold,
                percent(r.market_share_bps)
            ));
        }
    });
}
pub fn markets(ui: &mut egui::Ui, sim: &Simulation) {
    for m in sim.markets() {
        ui.group(|ui| {
            ui.heading(&m.category);
            ui.label(format!(
                "Potential customers: {}  |  Available demand: {}  |  Sold: {}",
                m.customers, m.available_demand, m.units_sold
            ));
            ui.label(format!(
                "Category growth: {}  |  Global growth: {}",
                percent(m.growth_bps),
                percent(sim.economy().growth_bps)
            ));
            for f in sim.companies() {
                for p in &f.products {
                    if p.category == m.category {
                        ui.label(format!("{} / {}", f.name, p.name));
                        ui.add(
                            egui::ProgressBar::new(p.market_share_bps as f32 / 10000.0)
                                .text(percent(p.market_share_bps)),
                        );
                    }
                }
            }
        });
        ui.add_space(12.0);
    }
}
pub fn economy(ui: &mut egui::Ui, sim: &Simulation) {
    let e = sim.economy();
    if e.demand_shock || e.cost_shock || e.growth_bps < 0 || e.inflation_bps < 0 {
        ui.colored_label(Color32::YELLOW, "! Unusual economic conditions");
    }
    egui::Grid::new("macro").striped(true).show(ui, |ui| {
        row(ui, "Growth", percent(e.growth_bps));
        row(ui, "Inflation", percent(e.inflation_bps));
        row(ui, "Interest rate", percent(e.interest_bps));
        row(
            ui,
            "Demand conditions (100% = baseline)",
            percent(e.demand_conditions_bps),
        );
        row(ui, "Demand shock", e.demand_shock);
        row(ui, "Cost shock", e.cost_shock);
        row(ui, "Cost shock magnitude", percent(e.cost_shock_bps));
    });
}
#[derive(Default)]
pub struct EventFilter {
    pub year: String,
    pub company: String,
    pub product: String,
    pub kind: String,
}
pub fn events(ui: &mut egui::Ui, sim: &Simulation, filter: &mut EventFilter) {
    ui.label("Filters (blank = all; names/types match text)");
    egui::Grid::new("filters").show(ui, |ui| {
        for (label, value) in [
            ("Year", &mut filter.year),
            ("Company", &mut filter.company),
            ("Product", &mut filter.product),
            ("Event type", &mut filter.kind),
        ] {
            ui.label(label);
            ui.text_edit_singleline(value);
            ui.end_row();
        }
    });
    if !filter.year.is_empty() && filter.year.parse::<i32>().is_err() {
        ui.colored_label(Color32::YELLOW, "Enter an integer year.");
        return;
    }
    let matches: Vec<_> = sim
        .events()
        .iter()
        .rev()
        .filter(|e| {
            (filter.year.is_empty() || Some(e.year) == filter.year.parse().ok())
                && e.entities
                    .first()
                    .is_some_and(|v| v.to_lowercase().contains(&filter.company.to_lowercase()))
                && (filter.product.is_empty()
                    || e.entities
                        .get(1)
                        .is_some_and(|v| v.to_lowercase().contains(&filter.product.to_lowercase())))
                && format!("{:?}", e.event_type)
                    .to_lowercase()
                    .contains(&filter.kind.to_lowercase())
        })
        .collect();
    ui.label(format!("{} matching events", matches.len()));
    for e in matches {
        ui.group(|ui| {
            let warning = e.severity == Severity::Warning || e.severity == Severity::Error;
            ui.colored_label(
                if warning {
                    Color32::from_rgb(255, 170, 110)
                } else {
                    Color32::LIGHT_BLUE
                },
                format!(
                    "{}{} • {:?} • {}",
                    if warning { "! WARNING • " } else { "" },
                    e.year,
                    e.event_type,
                    e.entities.join(" / ")
                ),
            );
            ui.label(&e.description);
            ui.small(format!("Recorded value: {}", e.value));
        });
    }
}
