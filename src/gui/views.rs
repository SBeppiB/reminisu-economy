use super::charts::chart;
use crate::{
    currency::Money,
    presentation,
    simulation::{Severity, Simulation},
};
use eframe::egui::{self, Color32, RichText};
pub fn tip(label: &str) -> &'static str {
    presentation::tooltip(label)
}
pub fn money(ui: &mut egui::Ui, label: &str, value: Money) {
    ui.label(label).on_hover_text(tip(label));
    ui.label(
        RichText::new(value.to_string())
            .strong()
            .color(if value.minor_units() < 0 {
                Color32::LIGHT_RED
            } else {
                Color32::LIGHT_GREEN
            }),
    )
    .on_hover_text(tip(label));
    ui.end_row();
}
pub fn row(ui: &mut egui::Ui, label: &str, value: impl std::fmt::Display) {
    ui.label(label).on_hover_text(tip(label));
    ui.label(value.to_string()).on_hover_text(tip(label));
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
fn card(ui: &mut egui::Ui, label: &str, value: Money) {
    ui.group(|ui| {
        ui.set_min_width(165.0);
        ui.label(label).on_hover_text(tip(label));
        ui.label(
            RichText::new(value.to_string())
                .size(21.0)
                .color(if value.minor_units() < 0 {
                    Color32::LIGHT_RED
                } else {
                    Color32::WHITE
                }),
        )
        .on_hover_text(tip(label));
    });
}
fn selector(ui: &mut egui::Ui, id: &str, labels: &[&str]) -> usize {
    let id = ui.make_persistent_id(id);
    let mut selected = ui.data_mut(|d| d.get_temp::<usize>(id).unwrap_or(0));
    ui.horizontal_wrapped(|ui| {
        for (i, label) in labels.iter().enumerate() {
            ui.selectable_value(&mut selected, i, *label)
                .on_hover_text(tip(label));
        }
    });
    ui.data_mut(|d| d.insert_temp(id, selected));
    selected
}
fn bar(ui: &mut egui::Ui, label: &str, value: u128, max: u128) {
    ui.add(
        egui::ProgressBar::new(if max == 0 {
            0.0
        } else {
            (value as f64 / max as f64).clamp(0.0, 1.0) as f32
        })
        .text(format!("{label}: {value}")),
    )
    .on_hover_text(tip(label));
}
fn arrow(ui: &mut egui::Ui) {
    ui.weak("↓");
}
pub fn company(ui: &mut egui::Ui, sim: &Simulation, index: usize) {
    let f = &sim.companies()[index];
    ui.heading(&f.name);
    let records: Vec<_> = sim
        .history()
        .records()
        .iter()
        .filter(|r| r.faction_name == f.name)
        .collect();
    let latest = records.last().copied();
    ui.weak(latest.map_or(
        "Opening state — advance a year to see sales and profit.".into(),
        |r| format!("Year {} · closing balances and annual activity", r.year),
    ));
    ui.horizontal_wrapped(|ui| {
        card(ui, "Cash", f.cash);
        if let Ok(t) = f.financial_totals() {
            card(ui, "Company Value", t.net_worth);
        }
        card(ui, "Debt", f.debt);
        if let Some(r) = latest {
            card(ui, "Revenue", r.revenue);
            card(ui, "Profit", r.profit);
        }
    });
    let health = presentation::health(sim, index);
    ui.group(|ui| {
        ui.colored_label(if health.warning {Color32::LIGHT_RED} else {Color32::LIGHT_BLUE}, RichText::new(format!("Financial Health: {}",health.label)).strong()).on_hover_text(tip(health.label));
        for reason in health.reasons { ui.label(reason); }
        if f.distress_years > 0 {
            if sim.config().economics.bankruptcy_enabled { ui.label(format!("Distress duration: {} / {} consecutive years to bankruptcy", f.distress_years,sim.config().economics.bankruptcy_after_years)).on_hover_text(tip("distress")); }
            else { ui.label(format!("{} consecutive distressed years; bankruptcy is disabled.",f.distress_years)); }
        }
        ui.small("Presentation summary only. Low Cash = less than annual base overhead; High Debt = over 80% of credit limit. These labels add no engine rules.");
    });
    ui.add_space(8.0);
    ui.strong("What happened this year?");
    if let Some(r) = latest {
        if r.accounts.capital_spending.minor_units() > 0 {
            ui.label(format!(
                "{} went into expanding factories; that money is now tied up in productive assets.",
                r.accounts.capital_spending
            ))
            .on_hover_text(tip("investment"));
        }
        ui.label(format!(
            "Cash changed by {}. {}",
            r.accounts.cash_flow,
            if r.profit.minor_units() < 0 {
                "The company recorded a loss."
            } else {
                "Revenue covered recognized expenses."
            }
        ))
        .on_hover_text(tip("cash flow"));
        let events: Vec<_> = sim
            .events()
            .iter()
            .filter(|e| e.year == r.year && e.entities.first() == Some(&f.name))
            .collect();
        for e in events.iter().take(4) {
            ui.label(presentation::explain(sim, e))
                .on_hover_text(format!(
                    "Technical event: {:?}\n{}",
                    e.event_type, e.description
                ));
        }
        if events.len() > 4 {
            ui.collapsing(format!("{} more events", events.len() - 4), |ui| {
                for e in &events[4..] {
                    ui.label(presentation::explain(sim, e))
                        .on_hover_text(format!("{:?}", e.event_type));
                }
            });
        }
    }
    ui.strong("Are products selling?");
    for p in &f.products {
        ui.group(|ui| {
            ui.label(format!("{} · {}",p.name,p.category));
            if let Some(r)=latest.and_then(|r| r.products.iter().find(|r| r.name==p.name)) {
                bar(ui,"Units sold / allocated demand",u128::from(r.units_sold),u128::from(r.demand).max(u128::from(r.units_sold)));
                ui.label(format!("Allocated demand {} · Sold {} · Unmet {} · Produced {} · Remaining inventory {}",r.demand,r.units_sold,r.unmet_demand,r.units_produced,r.remaining_inventory)).on_hover_text(tip("demand"));
            }
            ui.add(egui::ProgressBar::new(p.market_share_bps as f32/10000.0).text(format!("Market share: {}",percent(p.market_share_bps)))).on_hover_text(tip("market share"));
            if f.bankrupt { ui.label("Production frozen by bankruptcy."); }
            else if !p.production_viable { ui.colored_label(Color32::LIGHT_RED,format!("New production suspended: closing price {} does not exceed unit cost {}.",p.current_market_value,p.production_cost)).on_hover_text(tip("production eligible")); }
            else { ui.label(format!("Unit cost {} · Cash {} · Unused credit {}",p.production_cost,f.cash, f.credit_limit.checked_sub(f.debt).unwrap_or(Money::zero(f.primary_currency)))).on_hover_text("Funding is shared by products in order. These balances are current; next year's costs, obligations and borrowing may change."); }
        });
    }
    ui.separator();
    ui.strong("History — choose a measure");
    let labels = ["Cash", "Company value", "Revenue", "Profit", "Debt"];
    let metric = selector(ui, "company_chart", &labels);
    let data = records
        .iter()
        .map(|r| {
            (
                r.year,
                match metric {
                    0 => r.cash,
                    1 => r.net_worth,
                    2 => r.revenue,
                    3 => r.profit,
                    _ => r.debt,
                }
                .minor_units() as f64
                    / 100.0,
            )
        })
        .collect();
    chart(
        ui,
        labels[metric],
        f.primary_currency.code(),
        &[(&f.name, data)],
    );
    ui.collapsing("Where did the cash go?", |ui| {
        ui.label("Annual cash reconciliation. Arrows show additions and deductions, not transaction timing.");
        if let Some(r)=latest { match presentation::cash_stages(r) {
            Ok(stages)=>for (i,(name,value)) in stages.iter().enumerate() { if i>0 {arrow(ui);} ui.group(|ui| { ui.label(format!("{name}: {value}")).on_hover_text(tip(name)); }); },
            Err(e)=>{ui.label(e.to_string());}
        }} else { ui.label("Advance a year to see actual cash movements."); }
        ui.small("Depreciation and cost of goods sold are profit entries, not additional cash deductions here.");
    });
    ui.collapsing("How is company value calculated?", |ui| {
        if let Ok(t)=f.financial_totals() {
            ui.horizontal_wrapped(|ui| {card(ui,"Cash",t.cash);ui.label("+");card(ui,"Inventory value",t.inventory_value);ui.label("+");card(ui,"Productive assets",t.technology_asset_value);});arrow(ui);
            if let Ok(assets)=t.cash.checked_add(t.inventory_value).and_then(|v|v.checked_add(t.technology_asset_value)) {card(ui,"Total assets",assets);}
            arrow(ui); ui.label(format!("Subtract debt: {}",t.debt)).on_hover_text(tip("debt"));arrow(ui);card(ui,"Net worth",t.net_worth);
            ui.label("Inventory uses the lower of cost basis and current market value. Owning valuable inventory and factories does not guarantee enough usable cash.");
        }
    });
    ui.collapsing("Accounting details — all annual accounts", |ui| {
        egui::Grid::new("all_accounts")
            .striped(true)
            .show(ui, |ui| {
                money(ui, "Credit limit", f.credit_limit);
                money(ui, "Productive assets", f.productive_assets);
                if let Ok(t) = f.financial_totals() {
                    money(ui, "Inventory value", t.inventory_value);
                    money(ui, "Inventory cost basis", t.inventory_cost_basis);
                }
                if let Some(r) = latest {
                    let a = &r.accounts;
                    for (n, v) in [
                        ("Revenue", a.revenue),
                        ("Cost of goods sold", a.cost_of_goods_sold),
                        ("Production spending", a.production_spending),
                        ("Operating expenses", a.operating_expenses),
                        ("Maintenance", a.maintenance),
                        ("Research", a.research),
                        ("Depreciation", a.depreciation),
                        ("Interest expense", a.interest_expense),
                        ("Capital spending", a.capital_spending),
                        ("Debt taken", a.debt_taken),
                        ("Debt repaid", a.debt_repaid),
                        ("Gross profit", a.gross_profit),
                        ("Operating profit", a.operating_profit),
                        ("Net profit", a.net_profit),
                        ("Operating cash flow", a.operating_cash_flow),
                        ("Cash flow", a.cash_flow),
                    ] {
                        money(ui, n, v);
                    }
                }
            });
    });
}
pub fn product(ui: &mut egui::Ui, sim: &Simulation, company: usize, product: usize) {
    let f = &sim.companies()[company];
    let Some(p) = f.products.get(product) else {
        ui.label("No products.");
        return;
    };
    ui.heading(&p.name);
    ui.label(&p.category).on_hover_text(tip("category"));
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
    ui.horizontal_wrapped(|ui| {
        card(ui, "Closing price", p.current_market_value);
        card(ui, "Unit production cost", p.production_cost);
    });
    if let Some((year, r)) = history.last() {
        ui.strong(format!("Production and sales in {year}"));
        let available = u128::from(r.remaining_inventory) + u128::from(r.units_sold);
        let market = sim
            .history()
            .records()
            .iter()
            .find(|record| record.year == *year && record.faction_name == f.name)
            .and_then(|record| record.markets.iter().find(|m| m.category == r.category));
        let customer_demand =
            market.map_or(u128::from(r.demand), |m| u128::from(m.available_demand));
        let max = available
            .max(u128::from(r.demand))
            .max(u128::from(r.units_produced))
            .max(customer_demand)
            .max(u128::from(r.requested_production.unwrap_or(0)));
        bar(ui, "Category customer demand", customer_demand, max);
        arrow(ui);
        bar(ui, "Allocated customer demand", u128::from(r.demand), max);
        arrow(ui);
        if let Some(requested) = r.requested_production {
            bar(ui, "Requested production", u128::from(requested), max);
            arrow(ui);
        }
        for (label, value) in [
            ("Actual production", u128::from(r.units_produced)),
            ("Available inventory before sales", available),
            ("Units sold", u128::from(r.units_sold)),
            ("Unmet demand", u128::from(r.unmet_demand)),
        ] {
            bar(ui, label, value, max);
            arrow(ui);
        }
        ui.small("Requests include the engine's stock buffer, existing inventory, capacity and price checks, before funding. Sold units may include customers redirected from competitors.");
        if let Some(requested) = r.requested_production
            && r.units_produced < requested
        {
            ui.colored_label(Color32::from_rgb(255,180,120), format!("Funding limited production: the factory requested {requested} units, but available cash after borrowing covered only {}.", r.units_produced)).on_hover_text("Products receive funding in their configured order. This statement uses the engine's recorded request before affordability and its actual output.");
        }
    } else {
        ui.label("Advance a year to see the production pipeline.");
    }
    let labels = [
        "Price",
        "Production cost",
        "Units sold",
        "Demand",
        "Market share",
    ];
    let metric = selector(ui, "product_chart", &labels);
    let data = history
        .iter()
        .map(|(y, r)| {
            (
                *y,
                match metric {
                    0 => r.market_value.minor_units() as f64 / 100.0,
                    1 => r.production_cost.minor_units() as f64 / 100.0,
                    2 => r.units_sold as f64,
                    3 => r.demand as f64,
                    _ => f64::from(r.market_share_bps) / 100.0,
                },
            )
        })
        .collect();
    chart(
        ui,
        labels[metric],
        if metric < 2 {
            f.primary_currency.code()
        } else if metric == 4 {
            "%"
        } else {
            "units"
        },
        &[(&p.name, data)],
    );
    ui.collapsing("More product details", |ui| {
        egui::Grid::new("product_details")
            .striped(true)
            .show(ui, |ui| {
                money(ui, "Inventory cost basis", p.inventory_cost_basis);
                row(ui, "Inventory", p.inventory);
                row(ui, "Demand (latest / initial forecast)", p.previous_demand);
                row(ui, "Unmet demand", p.unmet_demand);
                row(ui, "Production capacity", p.production_capacity);
                row(ui, "Maximum capacity", p.maximum_capacity);
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
    });
    ui.collapsing("Exact product history", |ui| {
        for (year, r) in history.iter().rev() {
            ui.label(format!(
                "{year}: Price {} · Unit cost {} · Demand {} · Produced {} · Sold {} · Share {}",
                r.market_value,
                r.production_cost,
                r.demand,
                r.units_produced,
                r.units_sold,
                percent(r.market_share_bps)
            ))
            .on_hover_text(tip("price"));
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
            ))
            .on_hover_text(tip("demand"));
            bar(
                ui,
                "Customer demand satisfied",
                u128::from(m.units_sold),
                u128::from(m.available_demand),
            );
            ui.label(format!(
                "Category growth: {}  |  Global growth: {}",
                percent(m.growth_bps),
                percent(sim.economy().growth_bps)
            ))
            .on_hover_text(tip("growth"));
            for f in sim.companies() {
                for p in &f.products {
                    if p.category == m.category {
                        ui.label(format!("{} / {}", f.name, p.name));
                        ui.add(
                            egui::ProgressBar::new(p.market_share_bps as f32 / 10000.0)
                                .text(percent(p.market_share_bps)),
                        )
                        .on_hover_text(tip("market share"));
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
    pub severity: String,
}
pub fn events(ui: &mut egui::Ui, sim: &Simulation, filter: &mut EventFilter) {
    ui.label("Find events — leave filters blank for all results");
    egui::Grid::new("filters").show(ui,|ui| {
        for (label,value) in [("Year",&mut filter.year),("Company",&mut filter.company),("Product",&mut filter.product),("Event type",&mut filter.kind)] { ui.label(label);ui.text_edit_singleline(value).on_hover_text("Names and technical event types match case-insensitive text; year is an exact whole number.");ui.end_row(); }
        ui.label("Severity");egui::ComboBox::from_id_salt("severity").selected_text(if filter.severity.is_empty(){"All"}else{&filter.severity}).show_ui(ui,|ui|{for s in ["","Warning","Error","UnusualButValid"]{ui.selectable_value(&mut filter.severity,s.into(),if s.is_empty(){"All"}else{s});}});ui.end_row();
    });
    if !filter.year.is_empty() && filter.year.parse::<i32>().is_err() {
        ui.colored_label(Color32::YELLOW, "Enter a whole-number year.");
        return;
    }
    let mut groups = std::collections::BTreeMap::<(i32, String), Vec<_>>::new();
    for e in sim.events().iter().filter(|e| {
        (filter.year.is_empty() || Some(e.year) == filter.year.parse().ok())
            && (filter.company.is_empty()
                || e.entities
                    .first()
                    .is_some_and(|v| v.to_lowercase().contains(&filter.company.to_lowercase())))
            && (filter.product.is_empty()
                || e.entities
                    .get(1)
                    .is_some_and(|v| v.to_lowercase().contains(&filter.product.to_lowercase())))
            && format!("{:?}", e.event_type)
                .to_lowercase()
                .contains(&filter.kind.to_lowercase())
            && (filter.severity.is_empty() || format!("{:?}", e.severity) == filter.severity)
    }) {
        groups
            .entry((
                e.year,
                e.entities.first().cloned().unwrap_or("Economy".into()),
            ))
            .or_default()
            .push(e);
    }
    ui.label(format!(
        "{} matching events",
        groups.values().map(Vec::len).sum::<usize>()
    ));
    for ((year, company), events) in groups.iter().rev() {
        ui.heading(format!("{year} · {company}"));
        for (i, e) in events.iter().enumerate() {
            let warning = matches!(e.severity, Severity::Warning | Severity::Error);
            let text = presentation::explain(sim, e);
            egui::CollapsingHeader::new(RichText::new(format!("{}{}",if warning{"⚠ "}else{""},text)).color(if warning{Color32::from_rgb(255,180,120)}else{Color32::LIGHT_GRAY})).id_salt((year,company,i)).show(ui,|ui|{
                ui.small(format!("Technical event: {:?} · Severity: {:?} · Recorded value: {}",e.event_type,e.severity,e.value)).on_hover_text("Recorded value has event-specific meaning; it is not always the expense or unit cost.");
                if let Some(r)=sim.history().records().iter().find(|r|r.year==*year && r.faction_name==*company){
                    ui.strong(format!("Inspect {company} — year {year}"));
                    egui::Grid::new(("event_record",year,company,i)).show(ui,|ui|{money(ui,"Cash",r.cash);money(ui,"Net worth",r.net_worth);money(ui,"Debt",r.debt);money(ui,"Revenue",r.revenue);money(ui,"Profit",r.profit);});
                    if let Some(p)=e.entities.get(1).and_then(|name|r.products.iter().find(|p|&p.name==name)){ui.label(format!("{}: closing price {} · production cost {} · produced {} · sold {} · unmet {}",p.name,p.market_value,p.production_cost,p.units_produced,p.units_sold,p.unmet_demand)).on_hover_text(tip("price"));}
                } else {ui.label(&e.description);}
            }).header_response.on_hover_text(format!("{:?} — click to inspect the recorded company/product/year",e.event_type));
        }
    }
}
