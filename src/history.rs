use crate::currency::{CurrencyError, Money};

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct ProductHistoryRecord {
    pub category: String,
    pub cost_of_goods_sold: Money,
    pub production_spending: Money,
    pub inventory_cost_basis: Money,
    pub production_cost: Money,
    pub production_capacity: u64,
    pub capacity_utilization_bps: u32,
    pub market_share_bps: u32,
    pub research_progress_bps: u32,
    pub unmet_demand: u64,
    pub name: String,
    pub units_produced: u64,
    pub units_sold: u64,
    pub remaining_inventory: u64,
    pub demand: u64,
    pub market_value: Money,
    pub revenue: Money,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct YearHistoryRecord {
    pub accounts: crate::accounting::Accounts,
    pub debt: Money,
    pub inventory_cost_basis: Money,
    pub bankrupt: bool,
    pub distress_years: u32,
    pub economy: crate::economy::EconomicState,
    pub markets: Vec<crate::market::MarketState>,
    pub year: i32,
    pub faction_name: String,
    pub cash: Money,
    pub revenue: Money,
    pub expenses: Money,
    pub profit: Money,
    pub net_worth: Money,
    pub inventory_value: Money,
    pub technology_asset_value: Money,
    pub products: Vec<ProductHistoryRecord>,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq, Eq, Default)]
pub struct History {
    records: Vec<YearHistoryRecord>,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct HistorySummary {
    pub starting_net_worth: Money,
    pub ending_net_worth: Money,
    pub total_revenue: Money,
    pub total_profit: Money,
    pub best_year: i32,
    pub best_profit: Money,
    pub worst_year: i32,
    pub worst_profit: Money,
    pub total_units_produced: u64,
    pub total_units_sold: u64,
}

impl History {
    pub fn push(&mut self, record: YearHistoryRecord) {
        self.records.push(record);
    }

    pub fn records(&self) -> &[YearHistoryRecord] {
        &self.records
    }

    pub fn summary(&self) -> Result<Option<HistorySummary>, CurrencyError> {
        let Some(first) = self.records.first() else {
            return Ok(None);
        };
        self.summary_for(&first.faction_name)
    }

    pub fn summary_for(&self, company: &str) -> Result<Option<HistorySummary>, CurrencyError> {
        let Some(first) = self.records.iter().find(|r| r.faction_name == company) else {
            return Ok(None);
        };
        let last = self
            .records
            .iter()
            .rev()
            .find(|r| r.faction_name == first.faction_name)
            .unwrap();
        let mut total_revenue = Money::zero(first.revenue.currency());
        let mut total_profit = Money::zero(first.profit.currency());
        let mut best = first;
        let mut worst = first;
        let mut total_units_produced = 0u64;
        let mut total_units_sold = 0u64;

        // Compatibility: summary describes the primary (first recorded) company.
        for record in self
            .records
            .iter()
            .filter(|r| r.faction_name == first.faction_name)
        {
            total_revenue = total_revenue.checked_add(record.revenue)?;
            total_profit = total_profit.checked_add(record.profit)?;
            if record.profit > best.profit {
                best = record;
            }
            if record.profit < worst.profit {
                worst = record;
            }
            for product in &record.products {
                total_units_produced = total_units_produced
                    .checked_add(product.units_produced)
                    .ok_or(CurrencyError::Overflow)?;
                total_units_sold = total_units_sold
                    .checked_add(product.units_sold)
                    .ok_or(CurrencyError::Overflow)?;
            }
        }

        Ok(Some(HistorySummary {
            starting_net_worth: first.net_worth,
            ending_net_worth: last.net_worth,
            total_revenue,
            total_profit,
            best_year: best.year,
            best_profit: best.profit,
            worst_year: worst.year,
            worst_profit: worst.profit,
            total_units_produced,
            total_units_sold,
        }))
    }
}
