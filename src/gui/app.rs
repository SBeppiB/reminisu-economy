use super::{
    controller::{self, Controller},
    views,
};
use crate::{
    config::SimulationConfig,
    control::{self, RunRequest, SeedOutcome},
    export,
    simulation::{Simulation, SimulationError},
};
use eframe::egui::{self, Color32};

const SCREENS: [&str; 8] = [
    "Dashboard",
    "Companies",
    "Products",
    "Markets",
    "Economy",
    "Events",
    "Seed Comparison",
    "Configuration",
];
pub struct EconomyApp {
    controller: Option<Controller>,
    screen: usize,
    status: String,
    error: bool,
    seed: String,
    start: i32,
    until: i32,
    engine_path: String,
    scenario_path: String,
    validated: Option<SimulationConfig>,
    export_path: String,
    filter: views::EventFilter,
    seed_list: String,
    seed_count: u32,
    compare_start: i32,
    compare_end: i32,
    comparison: Option<Comparison>,
}
struct Comparison {
    request: RunRequest,
    config: SimulationConfig,
    seeds: Vec<u64>,
    active: Option<Simulation>,
    outcomes: Vec<SeedOutcome>,
    details: Vec<Vec<(usize, u32)>>,
    running: bool,
}
impl Comparison {
    fn tick(&mut self) -> Result<(), SimulationError> {
        if !self.running {
            return Ok(());
        }
        if self.active.is_none() {
            self.active = Some(Simulation::new(
                self.request.start_year,
                self.seeds[self.outcomes.len()],
                self.config.clone(),
            )?);
        }
        if let Some(sim) = &mut self.active {
            sim.advance_one_year()?;
            if sim.current_year() > self.request.end_year {
                self.outcomes.push(control::summarize(sim)?);
                self.details.push(
                    sim.companies()
                        .iter()
                        .map(|f| {
                            (
                                sim.events()
                                    .iter()
                                    .filter(|e| e.entities.first() == Some(&f.name))
                                    .count(),
                                f.distress_years,
                            )
                        })
                        .collect(),
                );
                self.active = None;
                if self.outcomes.len() == self.seeds.len() {
                    self.running = false;
                }
            }
        }
        Ok(())
    }
}
impl EconomyApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        cc.egui_ctx.set_visuals(egui::Visuals::dark());
        cc.egui_ctx.style_mut(|style| {
            style.spacing.item_spacing = egui::vec2(12.0, 8.0);
            style.visuals.panel_fill = Color32::from_rgb(23, 29, 40);
            style.visuals.selection.bg_fill = Color32::from_rgb(35, 92, 105);
        });
        let initial = Controller::new(3421, 42, SimulationConfig::default());
        let status = initial.as_ref().err().map_or(
            "Ready • bundled testing scenario • PLACEHOLDER assumptions".into(),
            ToString::to_string,
        );
        let error = initial.is_err();
        Self {
            controller: initial.ok(),
            screen: 0,
            status,
            error,
            seed: "42".into(),
            start: 3421,
            until: 3455,
            engine_path: "config/default.toml".into(),
            scenario_path: "scenarios/testing.toml".into(),
            validated: None,
            export_path: "gui-history.json".into(),
            filter: Default::default(),
            seed_list: "1,42,99".into(),
            seed_count: 10,
            compare_start: 3421,
            compare_end: 3455,
            comparison: None,
        }
    }
    fn report(&mut self, result: Result<(), SimulationError>, success: &str) {
        self.error = result.is_err();
        self.status = result
            .err()
            .map_or_else(|| success.into(), |e| format!("ERROR • {e}"));
    }
    fn parse_seed(&self) -> Result<u64, SimulationError> {
        self.seed.trim().parse().map_err(|_| {
            SimulationError::InvalidInput("Seed must be an unsigned 64-bit integer".into())
        })
    }
    fn new_simulation(&mut self) -> Result<(), SimulationError> {
        let config = self
            .validated
            .clone()
            .or_else(|| {
                self.controller
                    .as_ref()
                    .map(|c| c.simulation().config().clone())
            })
            .unwrap_or_default();
        let next = Controller::new(self.start, self.parse_seed()?, config)?;
        self.controller = Some(next);
        Ok(())
    }
    fn controls(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            let busy = self.controller.as_ref().is_some_and(Controller::busy);
            for years in [1, 5, 10] {
                if ui
                    .add_enabled(
                        !busy,
                        egui::Button::new(format!(
                            "Advance {years} year{}",
                            if years == 1 { "" } else { "s" }
                        )),
                    )
                    .clicked()
                    && let Some(c) = &mut self.controller
                {
                    let r = c.advance(years);
                    self.report(r, "Advancing simulation");
                }
            }
            ui.label("Through year");
            ui.add(egui::DragValue::new(&mut self.until));
            if ui
                .add_enabled(!busy, egui::Button::new("Run until"))
                .clicked()
                && let Some(c) = &mut self.controller
            {
                let r = c.run_until(self.until);
                self.report(r, "Run queued");
            }
            if busy && ui.button("Stop").clicked() {
                if let Some(c) = &mut self.controller {
                    c.cancel();
                }
                self.status = "Stopped after last committed year".into();
            }
        });
        ui.horizontal_wrapped(|ui| {
            ui.label("New run seed");
            ui.add(egui::TextEdit::singleline(&mut self.seed).desired_width(150.0));
            if ui.button("Random seed").clicked() {
                match std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
                    Ok(t) => {
                        self.seed = (t.as_nanos() as u64).to_string();
                    }
                    Err(e) => {
                        self.error = true;
                        self.status = e.to_string();
                    }
                }
            }
            ui.label("Starting year");
            ui.add(egui::DragValue::new(&mut self.start));
            if ui.button("Start new simulation").clicked() {
                let r = self.new_simulation();
                self.report(r, "New simulation started; previous live history replaced");
            }
            if ui.button("Reset simulation").clicked()
                && let Some(c) = &mut self.controller
            {
                let r = c.reset();
                self.report(
                    r,
                    "Reset to the active run's original seed, year and configuration",
                );
            }
        });
        ui.small("New / Reset replaces live history. Export first to keep a record. Validated configuration is applied only to New.");
    }
    fn configuration(&mut self, ui: &mut egui::Ui) {
        ui.label("Select TOML files by path (relative to the working directory, or absolute).");
        egui::Grid::new("config_paths").show(ui, |ui| {
            ui.label("Engine TOML");
            if ui.text_edit_singleline(&mut self.engine_path).changed() {
                self.validated = None;
            }
            ui.end_row();
            ui.label("Scenario TOML");
            if ui.text_edit_singleline(&mut self.scenario_path).changed() {
                self.validated = None;
            }
            ui.end_row();
        });
        ui.horizontal(|ui| {
            if ui.button("Testing scenario").clicked() {
                self.scenario_path = "scenarios/testing.toml".into();
                self.validated = None;
            }
            if ui.button("Competition scenario").clicked() {
                self.scenario_path = "scenarios/competition.toml".into();
                self.validated = None;
            }
        });
        if ui.button("Validate configuration").clicked() {
            match controller::load_configuration(&self.engine_path, &self.scenario_path) {
                Ok(c) => {
                    self.validated = Some(c);
                    self.report(Ok(()),"Configuration valid; Start new simulation to apply this validated snapshot");
                }
                Err(e) => {
                    self.validated = None;
                    self.report(Err(e), "");
                }
            }
        }
        ui.label("Files are read only. Validation never changes the running simulation or the source files.");
        if let Some(c) = &self.validated {
            ui.strong("Validated candidate for next simulation");
            provenance(ui, c);
        }
        if let Some(c) = &self.controller {
            ui.separator();
            ui.strong("Active simulation provenance");
            provenance(ui, c.simulation().config());
        }
    }
    fn comparison(&mut self, ui: &mut egui::Ui) {
        ui.label("Uses the active simulation's configuration. Explicit seeds preserve order and duplicates.");
        ui.label("Explicit seeds (comma separated; blank uses consecutive count)");
        ui.text_edit_singleline(&mut self.seed_list);
        ui.horizontal_wrapped(|ui| {
            ui.label("Consecutive count");
            ui.add(egui::DragValue::new(&mut self.seed_count).range(1..=1000));
            ui.label("First seed: New run seed above");
            ui.label("From");
            ui.add(egui::DragValue::new(&mut self.compare_start));
            ui.label("Through");
            ui.add(egui::DragValue::new(&mut self.compare_end));
        });
        if ui.button("Run seed comparison").clicked() {
            let result = (|| {
                let seed = if self.seed_list.trim().is_empty() {
                    self.parse_seed()?
                } else {
                    0
                };
                let seeds = controller::seeds(&self.seed_list, seed, self.seed_count)?;
                let request = RunRequest {
                    start_year: self.compare_start,
                    end_year: self.compare_end,
                    seed,
                };
                request.validate()?;
                let config = self
                    .controller
                    .as_ref()
                    .map(|c| c.simulation().config().clone())
                    .ok_or_else(|| {
                        SimulationError::InvalidInput("Start a simulation first".into())
                    })?;
                self.comparison = Some(Comparison {
                    request,
                    config,
                    seeds,
                    active: None,
                    outcomes: Vec::new(),
                    details: Vec::new(),
                    running: true,
                });
                Ok(())
            })();
            self.report(result, "Sequential comparison started");
        }
        if let Some(c) = &mut self.comparison {
            ui.label(format!(
                "{} / {} seeds complete • years {}–{}",
                c.outcomes.len(),
                c.seeds.len(),
                c.request.start_year,
                c.request.end_year
            ));
            if c.running && ui.button("Cancel comparison").clicked() {
                c.running = false;
                c.active = None;
            }
            if let Some(sim) = &c.active {
                ui.label(format!(
                    "Seed {} • next year {}",
                    sim.seed(),
                    sim.current_year()
                ));
            }
            egui::ScrollArea::horizontal().show(ui, |ui| {
                egui::Grid::new("comparison_table")
                    .striped(true)
                    .show(ui, |ui| {
                        for h in [
                            "Seed",
                            "Company",
                            "Ending net worth",
                            "Total revenue",
                            "Total profit",
                            "Company events",
                            "Outcome",
                        ] {
                            ui.strong(h);
                        }
                        ui.end_row();
                        for (i, outcome) in c.outcomes.iter().enumerate() {
                            for (j, f) in outcome.companies.iter().enumerate() {
                                ui.label(outcome.seed.to_string());
                                ui.label(&f.name);
                                ui.label(f.ending_net_worth.to_string());
                                ui.label(f.total_revenue.to_string());
                                ui.label(f.total_profit.to_string());
                                ui.label(c.details[i][j].0.to_string());
                                ui.label(if f.bankrupt {
                                    "BANKRUPT".into()
                                } else if c.details[i][j].1 > 0 {
                                    format!("DISTRESS ({} years)", c.details[i][j].1)
                                } else {
                                    "Active".into()
                                });
                                ui.end_row();
                            }
                        }
                    });
            });
            ui.small("Company event counts exclude global economy events. Exports retain the existing format's all-event count per seed.");
        }
    }
    fn exports(&mut self, ui: &mut egui::Ui) {
        ui.collapsing("Export analysis", |ui| {
            ui.label("Output path (export replaces an existing file at this path)");
            ui.text_edit_singleline(&mut self.export_path);
            let path = std::path::PathBuf::from(&self.export_path);
            ui.horizontal_wrapped(|ui| {
                for (label, csv) in [("History JSON", false), ("CSV + metadata sidecar", true)] {
                    if ui.button(label).clicked()
                        && let Some(c) = &self.controller
                    {
                        let r = c.export(&path, csv);
                        self.report(r, "Export completed");
                    }
                }
                if ui.button("Seed comparison JSON").clicked() {
                    let r = match &self.comparison {
                        Some(c) if !c.running && c.outcomes.len() == c.seeds.len() => {
                            controller::validate_export_path(&path, "json").and_then(|()| {
                                export::write_document(
                                    &export::comparison_document(
                                        &c.request,
                                        &c.config,
                                        &c.outcomes,
                                    ),
                                    &path,
                                )
                            })
                        }
                        _ => Err(SimulationError::InvalidInput(
                            "Complete a seed comparison before exporting it".into(),
                        )),
                    };
                    self.report(r, "Comparison exported");
                }
            });
        });
    }
}
fn provenance(ui: &mut egui::Ui, c: &SimulationConfig) {
    ui.label(
        format!(
            "Engine: {:?} • Scenario: {:?} • Economics: {:?}",
            c.engine_status, c.scenario_status, c.economics.status
        )
        .to_uppercase(),
    );
    ui.collapsing(
        "Resolved configuration",
        |ui| match toml::to_string_pretty(c) {
            Ok(text) => {
                ui.monospace(text);
            }
            Err(e) => {
                ui.label(format!("Cannot format configuration: {e}"));
            }
        },
    );
}
impl eframe::App for EconomyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if let Some(c) = &mut self.controller {
            let was_busy = c.busy();
            let r = c.tick();
            let done = was_busy && !c.busy();
            if r.is_err() {
                self.report(r, "");
            } else if done {
                self.report(Ok(()), "Run completed");
            }
        }
        if let Some(c) = &mut self.comparison
            && let Err(e) = c.tick()
        {
            c.running = false;
            c.active = None;
            self.report(Err(e), "");
        }
        if self.controller.as_ref().is_some_and(Controller::busy)
            || self.comparison.as_ref().is_some_and(|c| c.running)
        {
            ctx.request_repaint();
        }
        egui::TopBottomPanel::top("top").show(ctx, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.heading("REMINISU");
                ui.label("ECONOMY CONTROL CENTER");
                if let Some(c) = &self.controller {
                    ui.separator();
                    ui.label(format!(
                        "Next year {} • Seed {} • {}",
                        c.simulation().current_year(),
                        c.simulation().seed(),
                        if c.busy() { "RUNNING" } else { "PAUSED" }
                    ));
                }
            });
            self.controls(ui);
        });
        egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
            ui.colored_label(
                if self.error {
                    Color32::from_rgb(255, 150, 130)
                } else {
                    Color32::LIGHT_GRAY
                },
                &self.status,
            );
            if let Some(e) = self
                .controller
                .as_ref()
                .and_then(|c| c.simulation().events().last())
            {
                ui.small(format!(
                    "Recent event • {} • {} • {}",
                    e.year,
                    e.entities.join(" / "),
                    e.description
                ));
            }
        });
        egui::SidePanel::left("navigation")
            .resizable(false)
            .default_width(180.0)
            .show(ctx, |ui| {
                ui.add_space(18.0);
                for (index, label) in SCREENS.iter().enumerate() {
                    ui.selectable_value(&mut self.screen, index, *label);
                }
                ui.separator();
                ui.small("Deterministic engine\nExact integer accounts");
            });
        egui::CentralPanel::default().show(ctx, |ui| {
            egui::ScrollArea::both()
                .id_salt(self.screen)
                .show(ui, |ui| {
                    ui.heading(SCREENS[self.screen]);
                    ui.separator();
                    if self.screen == 7 {
                        self.configuration(ui);
                    } else if self.screen == 6 {
                        self.comparison(ui);
                    } else if let Some(c) = &mut self.controller {
                        if self.screen <= 2 {
                            let mut selected = c.company;
                            egui::ComboBox::from_id_salt("company_selection")
                                .selected_text(&c.simulation().companies()[selected].name)
                                .show_ui(ui, |ui| {
                                    for (i, f) in c.simulation().companies().iter().enumerate() {
                                        ui.selectable_value(&mut selected, i, &f.name);
                                    }
                                });
                            if selected != c.company {
                                c.select_company(selected);
                            }
                        }
                        if self.screen == 2 {
                            let mut selected = c.product;
                            let products = &c.simulation().companies()[c.company].products;
                            egui::ComboBox::from_id_salt("product_selection")
                                .selected_text(
                                    products
                                        .get(selected)
                                        .map_or("No product", |p| p.name.as_str()),
                                )
                                .show_ui(ui, |ui| {
                                    for (i, p) in products.iter().enumerate() {
                                        ui.selectable_value(&mut selected, i, &p.name);
                                    }
                                });
                            if selected != c.product {
                                c.select_product(selected);
                            }
                        }
                        match self.screen {
                            0 | 1 => views::company(ui, c.simulation(), c.company),
                            2 => views::product(ui, c.simulation(), c.company, c.product),
                            3 => views::markets(ui, c.simulation()),
                            4 => views::economy(ui, c.simulation()),
                            5 => views::events(ui, c.simulation(), &mut self.filter),
                            _ => {}
                        }
                    }
                    ui.separator();
                    self.exports(ui);
                });
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn incremental_comparison_matches_library_order_and_export() {
        let config = controller::validate_documents(
            include_str!("../../config/default.toml"),
            include_str!("../../scenarios/competition.toml"),
        )
        .unwrap();
        let request = RunRequest {
            start_year: 3421,
            end_year: 3425,
            seed: 0,
        };
        let mut comparison = Comparison {
            request: request.clone(),
            config: config.clone(),
            seeds: vec![42, 9, 42],
            active: None,
            outcomes: vec![],
            details: vec![],
            running: true,
        };
        while comparison.running {
            comparison.tick().unwrap();
        }
        let expected = control::compare_seeds(&request, &config, [42, 9, 42]).unwrap();
        assert_eq!(comparison.outcomes, expected);
        assert_eq!(comparison.details[0], comparison.details[2]);
        let document = export::comparison_document(&request, &config, &comparison.outcomes);
        assert_eq!(document["metadata"]["format_version"], 2);
        assert_eq!(
            document["outcomes"],
            serde_json::to_value(expected).unwrap()
        );
    }
    #[test]
    fn screens_render_real_opening_and_completed_states_without_mutation() {
        let context = egui::Context::default();
        let mut sim = Simulation::new(3421, 42, SimulationConfig::default()).unwrap();
        for completed in [false, true] {
            if completed {
                sim.advance_years(5).unwrap();
            }
            let before = sim.clone();
            let _ = context.run(Default::default(), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    views::company(ui, &sim, 0);
                    views::product(ui, &sim, 0, 0);
                    views::markets(ui, &sim);
                    views::economy(ui, &sim);
                    views::events(ui, &sim, &mut Default::default());
                });
            });
            assert_eq!(sim, before);
        }
    }
}
