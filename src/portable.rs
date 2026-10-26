//! Portable starting configurations. History/snapshots deliberately use other formats.
use crate::{
    config::{CompanyConfig, SimulationConfig},
    export::io_error,
    simulation::{Simulation, SimulationError},
    technology::ProductConfig,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Document {
    pub format_version: u32,
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_year: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seed: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scenario: Option<SimulationConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub company: Option<CompanyConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub product: Option<ProductConfig>,
}
fn invalid(s: impl Into<String>) -> SimulationError {
    SimulationError::InvalidInput(s.into())
}
impl Document {
    fn empty(kind: &str) -> Self {
        Self {
            format_version: 1,
            kind: kind.into(),
            start_year: None,
            seed: None,
            scenario: None,
            company: None,
            product: None,
        }
    }
    pub fn scenario(config: SimulationConfig, year: i32, seed: u64) -> Self {
        Self {
            scenario: Some(config),
            start_year: Some(year),
            seed: Some(seed.to_string()),
            ..Self::empty("scenario")
        }
    }
    pub fn company(company: CompanyConfig) -> Self {
        Self {
            company: Some(company),
            ..Self::empty("company")
        }
    }
    pub fn product(product: ProductConfig) -> Self {
        Self {
            product: Some(product),
            ..Self::empty("product")
        }
    }
    pub fn validate_shape(&self) -> Result<(), SimulationError> {
        if self.format_version != 1 {
            return Err(invalid("Unsupported portable format version; expected 1."));
        }
        let count = usize::from(self.scenario.is_some())
            + usize::from(self.company.is_some())
            + usize::from(self.product.is_some());
        if count != 1 {
            return Err(invalid(
                "A file must contain exactly one scenario, company, or product.",
            ));
        }
        match self.kind.as_str() {
            "scenario" if self.scenario.is_some() => {
                let year = self
                    .start_year
                    .ok_or_else(|| invalid("Scenario needs a starting year."))?;
                let seed = self
                    .seed
                    .as_ref()
                    .ok_or_else(|| invalid("Scenario needs a seed."))?
                    .parse::<u64>()
                    .map_err(|_| {
                        invalid("Seed must be a whole number from 0 to 18446744073709551615.")
                    })?;
                validate(self.scenario.as_ref().unwrap(), year, seed)?;
            }
            "company"
                if self.company.is_some() && self.start_year.is_none() && self.seed.is_none() => {}
            "product"
                if self.product.is_some() && self.start_year.is_none() && self.seed.is_none() => {}
            _ => return Err(invalid("File kind and contents do not match.")),
        }
        Ok(())
    }
}
/// Strings preserve every i128 minor unit in both TOML and JSON consumers.
pub fn exact_value<T: Serialize>(value: &T) -> Result<Value, SimulationError> {
    fn walk(v: &mut Value) {
        match v {
            Value::Object(map) => {
                for (k, v) in map {
                    if k == "minor_units" && v.is_number() {
                        *v = Value::String(v.to_string());
                    } else {
                        walk(v);
                    }
                }
            }
            Value::Array(items) => {
                for v in items {
                    walk(v);
                }
            }
            _ => {}
        }
    }
    let mut v = serde_json::to_value(value).map_err(io_error)?;
    walk(&mut v);
    Ok(v)
}
pub fn encode(doc: &Document, toml_format: bool) -> Result<String, SimulationError> {
    doc.validate_shape()?;
    let mut v = exact_value(doc)?;
    // TOML has no null; omitted optional fields deserialize to the same None.
    fn omit_nulls(v: &mut Value) {
        match v {
            Value::Object(map) => {
                map.retain(|_, v| !v.is_null());
                for v in map.values_mut() {
                    omit_nulls(v);
                }
            }
            Value::Array(items) => {
                for v in items {
                    omit_nulls(v);
                }
            }
            _ => {}
        }
    }
    if toml_format {
        omit_nulls(&mut v);
    }
    if toml_format {
        toml::to_string_pretty(&to_toml(&v)?).map_err(io_error)
    } else {
        serde_json::to_string_pretty(&v).map_err(io_error)
    }
}
fn to_toml(v: &Value) -> Result<toml::Value, SimulationError> {
    Ok(match v {
        Value::Object(m) => toml::Value::Table(
            m.iter()
                .map(|(k, v)| Ok((k.clone(), to_toml(v)?)))
                .collect::<Result<_, SimulationError>>()?,
        ),
        Value::Array(a) => toml::Value::Array(a.iter().map(to_toml).collect::<Result<_, _>>()?),
        Value::String(s) => toml::Value::String(s.clone()),
        Value::Bool(b) => toml::Value::Boolean(*b),
        Value::Number(n) => match n.as_i64() {
            Some(n) => toml::Value::Integer(n),
            None => toml::Value::String(n.to_string()),
        },
        Value::Null => return Err(invalid("TOML cannot contain a null value.")),
    })
}
pub fn decode(text: &str, toml_format: bool) -> Result<Document, SimulationError> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut value: Value = if toml_format {
        serde_json::to_value(toml::from_str::<toml::Value>(text).map_err(io_error)?)
            .map_err(io_error)?
    } else {
        serde_json::from_str(text).map_err(io_error)?
    };
    fn unsigned_strings(v: &mut Value) {
        match v {
            Value::Object(m) => {
                for (k, v) in m {
                    if [
                        "starting_inventory",
                        "base_demand",
                        "production_capacity",
                        "maximum_capacity",
                        "annual_customers",
                        "maximum_customers",
                    ]
                    .contains(&k.as_str())
                    {
                        if let Some(n) = v.as_str().and_then(|s| s.parse::<u64>().ok()) {
                            *v = Value::from(n);
                        }
                    } else {
                        unsigned_strings(v);
                    }
                }
            }
            Value::Array(a) => {
                for v in a {
                    unsigned_strings(v);
                }
            }
            _ => {}
        }
    }
    unsigned_strings(&mut value);
    let doc: Document = serde_json::from_value(value).map_err(io_error)?;
    doc.validate_shape()?;
    Ok(doc)
}
pub fn company_at(config: &SimulationConfig, index: usize) -> CompanyConfig {
    if index == 0 {
        CompanyConfig {
            faction: config.pilton.clone(),
            products: config.products.clone(),
        }
    } else {
        config.competitors[index - 1].clone()
    }
}
pub fn set_company(config: &mut SimulationConfig, index: usize, company: CompanyConfig) {
    if index == 0 {
        config.pilton = company.faction;
        config.products = company.products;
    } else {
        config.competitors[index - 1] = company;
    }
}
pub fn replace_company_candidate(
    base: &SimulationConfig,
    company: CompanyConfig,
    selected: usize,
    year: i32,
    seed: u64,
) -> Result<SimulationConfig, SimulationError> {
    if selected > base.competitors.len() {
        return Err(invalid("Choose a company to replace."));
    }
    let mut candidate = base.clone();
    set_company(&mut candidate, selected, company);
    validate(&candidate, year, seed)?;
    Ok(candidate)
}
/// Build and validate a full candidate first. Callers swap only on success.
pub fn import_candidate(
    base: &SimulationConfig,
    doc: Document,
    selected: usize,
    year: i32,
    seed: u64,
) -> Result<(SimulationConfig, i32, u64), SimulationError> {
    doc.validate_shape()?;
    if let Some(config) = doc.scenario {
        return Ok((
            config,
            doc.start_year.unwrap(),
            doc.seed.unwrap().parse().unwrap(),
        ));
    }
    let mut candidate = base.clone();
    if let Some(company) = doc.company {
        candidate.competitors.push(company);
    }
    if let Some(product) = doc.product {
        if selected > candidate.competitors.len() {
            return Err(invalid("Choose a company before importing a product."));
        }
        let mut company = company_at(&candidate, selected);
        company.products.push(product);
        set_company(&mut candidate, selected, company);
    }
    validate(&candidate, year, seed)?;
    Ok((candidate, year, seed))
}
pub fn validate(config: &SimulationConfig, year: i32, seed: u64) -> Result<(), SimulationError> {
    let mut names = std::collections::BTreeSet::new();
    for company in std::iter::once(CompanyConfig {
        faction: config.pilton.clone(),
        products: config.products.clone(),
    })
    .chain(config.competitors.clone())
    {
        let f = &company.faction;
        if f.name.trim().is_empty() {
            return Err(invalid("Give every company a name."));
        }
        if !names.insert(f.name.clone()) {
            return Err(invalid(format!(
                "A company named '{}' already exists. Rename the company before importing it, or import the entire scenario to replace the draft.",
                f.name
            )));
        }
        if f.primary_currency != config.pilton.primary_currency {
            return Err(invalid(format!(
                "{}: All companies in a scenario must use the same currency ({}).",
                f.name,
                config.pilton.primary_currency.code()
            )));
        }
        for (label, money) in [
            ("Starting cash", Some(f.starting_cash)),
            ("Annual operating expenses", Some(f.base_operating_expenses)),
            ("Starting debt", f.starting_debt),
            ("Credit limit", f.credit_limit),
            ("Starting productive assets", f.starting_productive_assets),
        ] {
            if let Some(m) = money {
                if m.currency() != f.primary_currency {
                    return Err(invalid(format!(
                        "{}: {label} must use {}.",
                        f.name,
                        f.primary_currency.code()
                    )));
                }
                if m.minor_units() < 0 {
                    return Err(invalid(format!("{}: {label} cannot be negative.", f.name)));
                }
            }
        }
        if company.products.is_empty() {
            return Err(invalid(format!("{} needs at least one product.", f.name)));
        }
        let mut product_names = std::collections::BTreeSet::new();
        for p in &company.products {
            if p.name.trim().is_empty() || !product_names.insert(p.name.clone()) {
                return Err(invalid(format!(
                    "{}: Give each product a nonempty, unique name.",
                    f.name
                )));
            }
            for (label, money) in [
                ("Production cost", Some(p.production_cost)),
                ("Starting price", Some(p.starting_market_value)),
                ("Inventory basis", p.starting_inventory_basis),
            ] {
                if let Some(m) = money
                    && (m.currency() != f.primary_currency || m.minor_units() < 0)
                {
                    return Err(invalid(format!(
                        "{}: {label} must be nonnegative and use {}.",
                        p.name,
                        f.primary_currency.code()
                    )));
                }
            }
            if p.maximum_capacity.is_some_and(|c| c > 4_000_000_000_000) {
                return Err(invalid(format!(
                    "{}: Maximum capacity cannot exceed 4 trillion units.",
                    p.name
                )));
            }
            if p.popularity_percent > 10000 {
                return Err(invalid(format!(
                    "{}: Popularity must be between 0 and 10000 percent.",
                    p.name
                )));
            }
            if p.starting_inventory == 0
                && p.starting_inventory_basis
                    .is_some_and(|m| m.minor_units() != 0)
            {
                return Err(invalid(format!(
                    "{}: Inventory basis must be zero when there are no starting products in inventory.",
                    p.name
                )));
            }
            let category = if p.category.is_empty() {
                &p.name
            } else {
                &p.category
            };
            if category.trim().is_empty()
                || (!config.markets.is_empty()
                    && !config.markets.iter().any(|m| &m.category == category))
            {
                return Err(invalid(format!(
                    "{}: Add a market named '{}' or choose an existing category.",
                    p.name, category
                )));
            }
            if p.maximum_capacity
                .is_some_and(|m| m < p.production_capacity)
            {
                return Err(invalid(format!(
                    "{}: Maximum production capacity cannot be lower than starting production capacity.",
                    p.name
                )));
            }
            if p.input_share_percent > 100 {
                return Err(invalid(format!(
                    "{}: Material-sensitive cost percentage must be between 0 and 100.",
                    p.name
                )));
            }
            if p.base_demand > 1_000_000_000_000 || p.production_capacity > 1_000_000_000_000 {
                return Err(invalid(format!(
                    "{}: Annual demand and starting capacity cannot exceed 1 trillion units.",
                    p.name
                )));
            }
        }
        if company
            .faction
            .starting_debt
            .unwrap_or(crate::currency::Money::zero(
                company.faction.primary_currency,
            ))
            > company
                .faction
                .credit_limit
                .unwrap_or(crate::currency::Money::zero(
                    company.faction.primary_currency,
                ))
        {
            return Err(invalid(format!(
                "{}: Starting debt cannot exceed the credit limit.",
                company.faction.name
            )));
        }
    }
    let mut categories = std::collections::BTreeSet::new();
    for m in &config.markets {
        if m.category.trim().is_empty() || !categories.insert(&m.category) {
            return Err(invalid(
                "Give each market a nonempty, unique category name.",
            ));
        }
        if m.maximum_customers < m.annual_customers {
            return Err(invalid(format!(
                "{}: Maximum customers cannot be lower than annual customers.",
                m.category
            )));
        }
        if m.maximum_customers > 1_000_000_000_000 {
            return Err(invalid(format!(
                "{}: Maximum customers cannot exceed 1 trillion.",
                m.category
            )));
        }
        if !(-5000..=5000).contains(&m.growth_bps) {
            return Err(invalid(format!(
                "{}: Market growth must be between -5000 and 5000 basis points (-50% to 50%).",
                m.category
            )));
        }
    }
    crate::control::RunRequest {
        start_year: year,
        end_year: year,
        seed,
    }
    .validate()?;
    Simulation::new(year, seed, config.clone())?;
    Ok(())
}

/// Parse user-entered major units without floating point or rounding.
pub fn parse_amount(text: &str) -> Result<i128, SimulationError> {
    let text = text.trim();
    let (sign, unsigned) = if let Some(s) = text.strip_prefix('-') {
        ("-", s)
    } else {
        ("", text.strip_prefix('+').unwrap_or(text))
    };
    let mut parts = unsigned.split('.');
    let whole = parts.next().unwrap_or("");
    let fraction = parts.next().unwrap_or("");
    if parts.next().is_some()
        || fraction.len() > 2
        || (whole.is_empty() && fraction.is_empty())
        || !whole
            .bytes()
            .chain(fraction.bytes())
            .all(|b| b.is_ascii_digit())
    {
        return Err(invalid(
            "Enter an amount such as 1250000.00, with no commas and at most two decimal places.",
        ));
    }
    format!(
        "{sign}{}{:0<2}",
        if whole.is_empty() { "0" } else { whole },
        fraction
    )
    .parse::<i128>()
    .map_err(|_| invalid("This amount exceeds the exact money range."))
}
pub fn format_amount(minor: i128) -> String {
    format!(
        "{}{}.{:02}",
        if minor < 0 { "-" } else { "" },
        minor.unsigned_abs() / 100,
        minor.unsigned_abs() % 100
    )
}
