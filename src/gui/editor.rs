use crate::{
    config::SimulationConfig,
    currency::{Currency, Money},
    portable::{self, Document},
    simulation::SimulationError,
};
use eframe::egui::{self, Color32};
use serde_json::{Value, json};
use std::path::PathBuf;

pub struct Editor {
    draft: Value,
    pub start: i32,
    pub seed: String,
    company: usize,
    product: usize,
    path: String,
    message: String,
    pending: Option<Action>,
    replace_company: bool,
}
enum Action {
    DeleteCompany(usize),
    DeleteProduct(usize, usize),
    DeleteMarket(usize),
    Import(Box<Document>),
    Start(Box<SimulationConfig>, i32, u64),
    Write(PathBuf, String),
}
impl Default for Editor {
    fn default() -> Self {
        Self::new(SimulationConfig::default(), 3421, 42)
    }
}
impl Editor {
    pub fn new(config: SimulationConfig, start: i32, seed: u64) -> Self {
        Self {
            draft: portable::exact_value(&config).expect("configuration serializes"),
            start,
            seed: seed.to_string(),
            company: 0,
            product: 0,
            path: "scenario.json".into(),
            message: String::new(),
            pending: None,
            replace_company: true,
        }
    }
    fn candidate(&self) -> Result<(SimulationConfig, i32, u64), SimulationError> {
        let c: SimulationConfig = serde_json::from_value(self.draft.clone()).map_err(|e| {
            SimulationError::InvalidInput(format!("Check the highlighted whole-number fields: {e}"))
        })?;
        let seed = self.seed.parse().map_err(|_| {
            SimulationError::InvalidInput(
                "Seed must be a whole number from 0 to 18446744073709551615.".into(),
            )
        })?;
        portable::validate(&c, self.start, seed)?;
        Ok((c, self.start, seed))
    }
    pub fn ui(&mut self, ui: &mut egui::Ui) -> Option<(SimulationConfig, i32, u64)> {
        ui.label("STARTING SCENARIO — editable draft").on_hover_text("This draft is separate from the running simulation. Only Start this scenario replaces the live run, after confirmation.");
        ui.weak("Company and product order affects deterministic allocation and funding. Duplicates are appended in order.");
        ui.horizontal_wrapped(|ui| {
            ui.label("Starting year");
            ui.add(egui::DragValue::new(&mut self.start));
            ui.label("RNG seed")
                .on_hover_text(super::views::tip("seed"));
            ui.text_edit_singleline(&mut self.seed);
        });
        ui.horizontal_wrapped(|ui| {
            ui.label("File path (.json or .toml)"); ui.text_edit_singleline(&mut self.path).on_hover_text("Relative to the application folder, or an absolute path. Portable starting configurations use JSON or TOML.");
        });
        ui.checkbox(&mut self.replace_company, "Company imports replace the selected company (uncheck to append)").on_hover_text("Replacement preserves the selected company's position in simulation order and requires confirmation. Appending requires a unique company name.");
        ui.horizontal_wrapped(|ui| {
            for (label, kind) in [("Import Scenario", "scenario"), ("Import Company", "company"), ("Import Product", "product")] {
                if ui.button(label).on_hover_text("Read, validate, then stage the complete import. Invalid files never replace the draft or live run.").clicked() {
                    let result = (|| {
                        let path = PathBuf::from(&self.path); let is_toml = format(&path)?;
                        let doc = portable::decode(&std::fs::read_to_string(path).map_err(crate::export::io_error)?, is_toml)?;
                        if doc.kind != kind { return Err(SimulationError::InvalidInput(format!("Choose a {kind} file for this button."))); }
                        self.imported_candidate(doc.clone())?;
                        if kind == "scenario" || (kind == "company" && self.replace_company) { self.pending = Some(Action::Import(Box::new(doc))); }
                        else { self.apply_import(doc)?; }
                        Ok(())
                    })();
                    self.message = result.err().map_or("Import validated. Any replacement awaits confirmation; the live run is unchanged.".into(), |e| e.to_string());
                }
            }
        });
        ui.horizontal_wrapped(|ui| {
            for (label, kind) in [("Export Scenario", 0), ("Export Company", 1), ("Export Product", 2)] {
                if ui.button(label).on_hover_text("Exports this draft's starting values for re-import, including exact money. This does not export the running company's balances.").clicked() {
                    let result = (|| {
                        let (config, year, seed) = self.candidate()?;
                        let c = portable::company_at(&config, self.company);
                        let doc = match kind { 0 => Document::scenario(config, year, seed), 1 => Document::company(c), _ => Document::product(c.products[self.product].clone()) };
                        let path = PathBuf::from(&self.path); let content = portable::encode(&doc, format(&path)?)?;
                        self.queue_write(path, content)
                    })();
                    if let Err(e) = result { self.message = e.to_string(); }
                }
            }
        });
        ui.separator();
        let count = self.draft["competitors"].as_array().map_or(0, Vec::len) + 1;
        self.company = self.company.min(count - 1);
        ui.horizontal_wrapped(|ui| {
            egui::ComboBox::from_id_salt("draft_company")
                .selected_text(company_name(&self.draft, self.company))
                .show_ui(ui, |ui| {
                    for i in 0..count {
                        if ui
                            .selectable_value(&mut self.company, i, company_name(&self.draft, i))
                            .changed()
                        {
                            self.product = 0;
                        }
                    }
                });
            if ui.button("Create company").clicked() {
                let mut config = SimulationConfig::default();
                config.pilton.name = unique_name(
                    "New company",
                    (0..count).map(|i| company_name(&self.draft, i)),
                );
                let mut c = portable::exact_value(&portable::company_at(&config, 0)).unwrap();
                c["products"].as_array_mut().unwrap().truncate(1);
                c["products"][0]["name"] = json!("New product");
                // Match the shared scenario currency; amounts are starting labels, not conversions.
                let currency = self.draft["pilton"]["primary_currency"].clone();
                relabel_currency(&mut c, &currency);
                if let Some(m) = self.draft["markets"].as_array().and_then(|a| a.first()) {
                    c["products"][0]["category"] = m["category"].clone();
                }
                self.draft["competitors"].as_array_mut().unwrap().push(c);
                self.company = count;
                self.product = 0;
            }
            if ui.button("Duplicate company").clicked() {
                let mut c = company_value(&self.draft, self.company);
                c["faction"]["name"] = json!(unique_name(
                    &format!("{} copy", company_name(&self.draft, self.company)),
                    (0..count).map(|i| company_name(&self.draft, i))
                ));
                self.draft["competitors"].as_array_mut().unwrap().push(c);
                self.company = count;
                self.product = 0;
            }
            if ui
                .add_enabled(count > 1, egui::Button::new("Delete company"))
                .clicked()
            {
                self.pending = Some(Action::DeleteCompany(self.company));
            }
        });
        let mut c = company_value(&self.draft, self.company);
        let old_currency = c["faction"]["primary_currency"].clone();
        ui.group(|ui| {
            ui.strong("Company starting finances");
            let f = &mut c["faction"];
            fields(ui, f, &["name", "primary_currency", "starting_cash", "starting_debt", "credit_limit", "base_operating_expenses", "starting_productive_assets"]);
            ui.small("Enter money as currency amounts, for example 1250000.00. Up to two decimal places; no rounding.");
        });
        let new_currency = c["faction"]["primary_currency"].clone();
        if new_currency != old_currency {
            relabel_currency(&mut c, &new_currency);
        }
        let products = c["products"].as_array_mut().unwrap();
        self.product = self.product.min(products.len().saturating_sub(1));
        ui.horizontal_wrapped(|ui| {
            egui::ComboBox::from_id_salt("draft_product")
                .selected_text(
                    products
                        .get(self.product)
                        .and_then(|p| p["name"].as_str())
                        .unwrap_or("No product"),
                )
                .show_ui(ui, |ui| {
                    for (i, p) in products.iter().enumerate() {
                        ui.selectable_value(
                            &mut self.product,
                            i,
                            p["name"].as_str().unwrap_or("Unnamed"),
                        );
                    }
                });
            if ui.button("Add product").clicked() {
                let mut p =
                    portable::exact_value(&SimulationConfig::default().products[0]).unwrap();
                p["name"] = json!(unique_name(
                    "New product",
                    products
                        .iter()
                        .map(|p| p["name"].as_str().unwrap_or("").to_owned())
                ));
                if let Some(m) = self.draft["markets"].as_array().and_then(|a| a.first()) {
                    p["category"] = m["category"].clone();
                }
                relabel_currency(&mut p, &self.draft["pilton"]["primary_currency"]);
                products.push(p);
                self.product = products.len() - 1;
            }
            if !products.is_empty() && ui.button("Duplicate product").clicked() {
                let mut p = products[self.product].clone();
                p["name"] = json!(unique_name(
                    &format!("{} copy", p["name"].as_str().unwrap_or("Product")),
                    products
                        .iter()
                        .map(|p| p["name"].as_str().unwrap_or("").to_owned())
                ));
                products.push(p);
                self.product = products.len() - 1;
            }
            if ui
                .add_enabled(products.len() > 1, egui::Button::new("Delete product"))
                .clicked()
            {
                self.pending = Some(Action::DeleteProduct(self.company, self.product));
            }
        });
        if let Some(p) = products.get_mut(self.product) {
            ui.group(|ui| {
                ui.strong("Product starting values");
                fields(
                    ui,
                    p,
                    &[
                        "name",
                        "category",
                        "starting_market_value",
                        "production_cost",
                        "starting_inventory",
                        "starting_inventory_basis",
                        "base_demand",
                        "production_capacity",
                        "maximum_capacity",
                        "popularity_percent",
                        "input_share_percent",
                    ],
                );
            });
        }
        set_company_value(&mut self.draft, self.company, c);
        ui.collapsing("Markets — shared customer pools", |ui| {
            ui.weak("An empty list derives markets from product categories. Once markets are specified, every product category must exist here.");
            let markets = self.draft["markets"].as_array_mut().unwrap();
            for (i, m) in markets.iter_mut().enumerate() {
                ui.push_id(i, |ui| { ui.group(|ui| { fields(ui, m, &["category", "annual_customers", "maximum_customers", "growth_bps"]); if ui.button("Delete market").clicked() { self.pending = Some(Action::DeleteMarket(i)); } }); });
            }
            if ui.button("Add market").clicked() { markets.push(json!({"category":"New category", "annual_customers":100, "maximum_customers":1000, "growth_bps":0})); }
        });
        ui.collapsing("Starting economy", |ui| {
            fields(
                ui,
                &mut self.draft["economics"],
                &[
                    "growth_bps",
                    "inflation_bps",
                    "interest_bps",
                    "demand_conditions_bps",
                ],
            );
        });
        ui.collapsing("Advanced engine settings", |ui| {
            ui.weak("Basis points: 100 = 1%. These settings apply only when starting a new run.");
            for key in ["economics", "demand", "production"] {
                ui.collapsing(label(key), |ui| {
                    let keys: Vec<_> = self.draft[key]
                        .as_object()
                        .unwrap()
                        .keys()
                        .cloned()
                        .collect();
                    for k in keys {
                        if [
                            "growth_bps",
                            "inflation_bps",
                            "interest_bps",
                            "demand_conditions_bps",
                        ]
                        .contains(&k.as_str())
                            && key == "economics"
                        {
                            continue;
                        }
                        ui.push_id((&key, &k), |ui| field(ui, &k, &mut self.draft[key][&k]));
                    }
                });
            }
            fields(ui, &mut self.draft, &["engine_status", "scenario_status"]);
        });
        let candidate = self.candidate();
        if let Err(e) = &candidate {
            ui.colored_label(Color32::from_rgb(255, 180, 120), e.to_string());
        } else {
            ui.colored_label(
                Color32::LIGHT_GREEN,
                "Draft is valid. The active simulation has not changed.",
            );
        }
        if ui.add_enabled(candidate.is_ok(), egui::Button::new("Start this scenario")).on_hover_text("Validate and start a new run from this draft; confirmation is required before replacing current history.").clicked() {
            let (c, y, s) = candidate.unwrap(); self.pending = Some(Action::Start(Box::new(c), y, s));
        }
        ui.label(&self.message);
        let mut result = None;
        if self.pending.is_some() {
            let mut accept = false;
            let mut cancel = false;
            egui::Modal::new(egui::Id::new("confirm_draft_change")).show(ui.ctx(), |ui| {
                ui.heading("Confirm change");
                ui.label(match self.pending.as_ref().unwrap() { Action::Start(..) => "Replace the running simulation and its history with this validated draft? Export history first to keep it.", Action::Import(doc) if doc.kind == "company" => "Replace the selected starting company and all of its products? The running simulation stays unchanged.", Action::Import(_) => "Replace the entire editor draft with the imported scenario? The running simulation stays unchanged.", Action::Write(..) => "Overwrite the existing file at the chosen path?", _ => "Delete this item from the draft? This cannot be undone. The running simulation stays unchanged." });
                ui.horizontal(|ui| { accept = ui.button("Confirm").clicked(); cancel = ui.button("Cancel").clicked(); });
            });
            if cancel {
                self.pending = None;
            }
            if accept {
                match self.pending.take().unwrap() {
                    Action::Start(c, y, s) => result = Some((*c, y, s)),
                    Action::Import(doc) => {
                        if let Err(e) = self.apply_import(*doc) {
                            self.message = e.to_string();
                        }
                    }
                    Action::Write(path, content) => {
                        self.message = std::fs::write(path, content)
                            .err()
                            .map_or("Exported.".into(), |e| e.to_string());
                    }
                    Action::DeleteCompany(i) => {
                        if i == 0 {
                            let c = self.draft["competitors"].as_array_mut().unwrap().remove(0);
                            set_company_value(&mut self.draft, 0, c);
                        } else {
                            self.draft["competitors"]
                                .as_array_mut()
                                .unwrap()
                                .remove(i - 1);
                        }
                        self.company = 0;
                        self.product = 0;
                    }
                    Action::DeleteProduct(i, p) => {
                        let mut c = company_value(&self.draft, i);
                        c["products"].as_array_mut().unwrap().remove(p);
                        set_company_value(&mut self.draft, i, c);
                        self.product = 0;
                    }
                    Action::DeleteMarket(i) => {
                        self.draft["markets"].as_array_mut().unwrap().remove(i);
                    }
                }
            }
        }
        result
    }
    fn imported_candidate(
        &self,
        doc: Document,
    ) -> Result<(SimulationConfig, i32, u64), SimulationError> {
        doc.validate_shape()?;
        Ok(if doc.kind == "scenario" {
            portable::import_candidate(&SimulationConfig::default(), doc, 0, self.start, 0)?
        } else if doc.kind == "company" && self.replace_company {
            let (c, y, s) = self.candidate()?;
            (
                portable::replace_company_candidate(&c, doc.company.unwrap(), self.company, y, s)?,
                y,
                s,
            )
        } else {
            let (c, y, s) = self.candidate()?;
            portable::import_candidate(&c, doc, self.company, y, s)?
        })
    }
    fn apply_import(&mut self, doc: Document) -> Result<(), SimulationError> {
        let (c, y, s) = self.imported_candidate(doc)?;
        self.draft = portable::exact_value(&c)?;
        self.start = y;
        self.seed = s.to_string();
        self.company = 0;
        self.product = 0;
        Ok(())
    }
    fn queue_write(&mut self, path: PathBuf, content: String) -> Result<(), SimulationError> {
        use std::io::Write;
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
        {
            Ok(mut file) => {
                file.write_all(content.as_bytes())
                    .map_err(crate::export::io_error)?;
                self.message = "Exported starting configuration.".into();
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                self.pending = Some(Action::Write(path, content))
            }
            Err(e) => return Err(crate::export::io_error(e)),
        }
        Ok(())
    }
}
fn format(path: &std::path::Path) -> Result<bool, SimulationError> {
    match path
        .extension()
        .and_then(|s| s.to_str())
        .map(str::to_lowercase)
        .as_deref()
    {
        Some("toml") => Ok(true),
        Some("json") => Ok(false),
        _ => Err(SimulationError::InvalidInput(
            "Choose a file ending in .json or .toml.".into(),
        )),
    }
}
fn company_value(d: &Value, i: usize) -> Value {
    if i == 0 {
        json!({"faction":d["pilton"], "products":d["products"]})
    } else {
        d["competitors"][i - 1].clone()
    }
}
fn company_name(d: &Value, i: usize) -> String {
    if i == 0 {
        &d["pilton"]["name"]
    } else {
        &d["competitors"][i - 1]["faction"]["name"]
    }
    .as_str()
    .unwrap_or("Unnamed company")
    .into()
}
fn set_company_value(d: &mut Value, i: usize, c: Value) {
    if i == 0 {
        d["pilton"] = c["faction"].clone();
        d["products"] = c["products"].clone();
    } else {
        d["competitors"][i - 1] = c;
    }
}
fn unique_name(base: &str, names: impl Iterator<Item = String>) -> String {
    let names: Vec<_> = names.collect();
    if !names.iter().any(|n| n == base) {
        return base.into();
    }
    for n in 2.. {
        let name = format!("{base} {n}");
        if !names.contains(&name) {
            return name;
        }
    }
    unreachable!()
}
fn relabel_currency(v: &mut Value, c: &Value) {
    if let Value::Object(m) = v {
        for (k, v) in m {
            if k == "currency" || k == "primary_currency" {
                *v = c.clone();
            } else {
                relabel_currency(v, c);
            }
        }
    } else if let Value::Array(a) = v {
        for v in a {
            relabel_currency(v, c);
        }
    }
}
fn label(key: &str) -> String {
    match key {
        "base_demand" => "Annual demand".into(),
        "input_share_percent" => "Material-sensitive cost %".into(),
        "base_operating_expenses" => "Annual operating expenses".into(),
        "starting_market_value" => "Starting selling price".into(),
        "starting_inventory_basis" => "Starting inventory cost basis".into(),
        "starting_productive_assets" => "Starting productive asset value".into(),
        _ => {
            let s = key.replace('_', " ");
            let mut c = s.chars();
            c.next().map_or(String::new(), |f| {
                f.to_uppercase().collect::<String>() + c.as_str()
            })
        }
    }
}
fn fields(ui: &mut egui::Ui, v: &mut Value, keys: &[&str]) {
    for key in keys {
        ui.push_id(*key, |ui| {
            if v.get(*key).is_none() || v[*key].is_null() {
                ui.horizontal(|ui| {
                    ui.label(format!("{}: engine default", label(key)))
                        .on_hover_text(if *key == "starting_inventory_basis" {
                            "Defaults to unit production cost multiplied by starting inventory."
                        } else if *key == "maximum_capacity" {
                            "Defaults to four times starting production capacity."
                        } else {
                            "Defaults to zero. Enable a custom value to edit it."
                        });
                    if ui.button("Set custom value").clicked() {
                        v[*key] = if *key == "maximum_capacity" {
                            json!(
                                v["production_capacity"]
                                    .as_u64()
                                    .unwrap_or(0)
                                    .saturating_mul(4)
                            )
                        } else {
                            let currency = v
                                .get("primary_currency")
                                .cloned()
                                .or_else(|| v.get("production_cost").map(|p| p["currency"].clone()))
                                .unwrap_or(json!("Ufc"));
                            json!({"currency":currency, "minor_units":"0"})
                        };
                    }
                });
            } else {
                field(ui, key, &mut v[*key]);
            }
        });
    }
}
fn field(ui: &mut egui::Ui, key: &str, v: &mut Value) {
    let name = label(key);
    let tip = crate::presentation::tooltip(&name);
    ui.horizontal(|ui| {
        ui.add_sized([230.0,20.0], egui::Label::new(&name)).on_hover_text(tip);
        if v.is_object() && v.get("minor_units").is_some() {
            let current = v["minor_units"].as_str().map(str::to_owned).unwrap_or_else(|| v["minor_units"].to_string());
            let id = ui.make_persistent_id("money_input");
            let stored = ui.data_mut(|d| d.get_temp::<(String,String)>(id));
            let mut text = stored.filter(|(old,_)| old == &current).map(|(_,text)|text).unwrap_or_else(|| current.parse::<i128>().map(portable::format_amount).unwrap_or_else(|_| current.strip_prefix("invalid:").unwrap_or(&current).into()));
            if ui.add(egui::TextEdit::singleline(&mut text).desired_width(200.0)).on_hover_text("Exact currency amount, for example 1200.50. At most two decimal places; no commas. Stored as integer minor units.").changed() {
                v["minor_units"] = json!(portable::parse_amount(&text).map(|n|n.to_string()).unwrap_or_else(|_|format!("invalid:{text}")));
            }
            ui.data_mut(|d| d.insert_temp(id,(v["minor_units"].as_str().unwrap_or("").to_owned(),text.clone())));
            match portable::parse_amount(&text) {
                Ok(n) => { if let Ok(c) = serde_json::from_value::<Currency>(v["currency"].clone()) { ui.label(Money::from_minor(c,n).to_string()).on_hover_text(tip); } }
                Err(e) => { ui.colored_label(Color32::LIGHT_RED,e.to_string()); }
            }
        } else if let Some(b) = v.as_bool() { let mut b=b; if ui.checkbox(&mut b, "Enabled").on_hover_text(tip).changed() { *v=json!(b); } }
        else if key == "primary_currency" {
            let mut selected = v.as_str().unwrap_or("Ufc").to_owned();
            egui::ComboBox::from_id_salt(key).selected_text(&selected).show_ui(ui, |ui| { for c in Currency::all() { let name = format!("{c:?}"); ui.selectable_value(&mut selected,name,c.code()); } }); *v=json!(selected);
            ui.small("Match every money field and company currency.");
        } else if key.ends_with("status") || key == "status" {
            let mut text=v.as_str().unwrap_or("PLACEHOLDER").to_owned(); egui::ComboBox::from_id_salt(key).selected_text(&text).show_ui(ui, |ui| { for s in ["CANON","ESTIMATED","PLACEHOLDER"] { ui.selectable_value(&mut text,s.to_owned(),s); } }); *v=json!(text);
        } else {
            let numeric = !["name", "category"].contains(&key);
            let mut text = v.as_str().map(str::to_owned).unwrap_or_else(|| v.to_string());
            if ui.add(egui::TextEdit::singleline(&mut text).desired_width(230.0)).on_hover_text(tip).changed() {
                *v = if numeric { serde_json::from_str::<Value>(&text).ok().filter(|v| v.is_i64() || v.is_u64()).unwrap_or(json!(text)) } else { json!(text) };
            }
            if numeric && !(v.is_i64() || v.is_u64()) { ui.colored_label(Color32::LIGHT_RED,"Enter a whole number."); }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn editor_renders_and_invalid_text_blocks_start_without_changing_live_state() {
        let ctx = egui::Context::default();
        let mut editor = Editor::default();
        let original = editor.candidate().unwrap();
        let _ = ctx.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                assert!(editor.ui(ui).is_none());
            });
        });
        assert_eq!(editor.candidate().unwrap(), original);
        editor.draft["pilton"]["starting_cash"]["minor_units"] = json!("invalid:1.234");
        assert!(editor.candidate().is_err());
        let _ = ctx.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                assert!(editor.ui(ui).is_none());
            });
        });
        assert!(editor.candidate().is_err());
    }
    #[test]
    fn failed_company_import_preserves_entire_draft() {
        let mut editor = Editor::default();
        let before = editor.draft.clone();
        let mut company = portable::company_at(&SimulationConfig::default(), 0);
        company.products[0].maximum_capacity = Some(0);
        assert!(editor.apply_import(Document::company(company)).is_err());
        assert_eq!(editor.draft, before);
        editor
            .apply_import(Document::company(portable::company_at(
                &SimulationConfig::default(),
                0,
            )))
            .unwrap();
        assert_eq!(editor.draft, before);
    }
}
