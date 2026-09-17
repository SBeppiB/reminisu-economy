use crate::asset::InventoryLine;
use crate::currency::{CurrencyError, Money};
use crate::simulation::SimulationError;
use crate::technology::Product;

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum FactionKind {
    TechnologyMegacorporation,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct Faction {
    pub debt: Money,
    pub credit_limit: Money,
    pub productive_assets: Money,
    pub distress_years: u32,
    pub bankrupt: bool,
    pub borrowing: bool,
    pub repaying: bool,
    pub name: String,
    pub kind: FactionKind,
    pub cash: Money,
    pub primary_currency: crate::currency::Currency,
    pub base_operating_expenses: Money,
    pub products: Vec<Product>,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct FinancialTotals {
    pub debt: Money,
    pub inventory_cost_basis: Money,
    pub cash: Money,
    pub inventory_value: Money,
    pub technology_asset_value: Money,
    pub net_worth: Money,
}

impl Faction {
    pub fn new(
        name: String,
        primary_currency: crate::currency::Currency,
        cash: Money,
        base_operating_expenses: Money,
        products: Vec<Product>,
    ) -> Result<Self, SimulationError> {
        if cash.currency() != primary_currency
            || base_operating_expenses.currency() != primary_currency
        {
            return Err(SimulationError::InvalidInput(
                "Pilton starting cash and operating expenses must use its primary currency"
                    .to_string(),
            ));
        }
        if cash.minor_units() < 0 || base_operating_expenses.minor_units() < 0 {
            return Err(SimulationError::InvalidInput(
                "starting cash and operating expenses cannot be negative".to_string(),
            ));
        }

        Ok(Self {
            debt: Money::zero(primary_currency),
            credit_limit: Money::zero(primary_currency),
            productive_assets: Money::zero(primary_currency),
            distress_years: 0,
            bankrupt: false,
            borrowing: false,
            repaying: false,
            name,
            kind: FactionKind::TechnologyMegacorporation,
            cash,
            primary_currency,
            base_operating_expenses,
            products,
        })
    }

    pub fn pay(&mut self, amount: Money) -> Result<(), SimulationError> {
        if amount.currency() != self.primary_currency {
            return Err(SimulationError::Currency(CurrencyError::CurrencyMismatch {
                left: amount.currency(),
                right: self.primary_currency,
            }));
        }
        if amount.minor_units() < 0 {
            return Err(SimulationError::InvalidInput(
                "cannot pay a negative amount".to_string(),
            ));
        }
        self.cash = self.cash.checked_sub(amount)?;
        Ok(())
    }

    pub fn receive(&mut self, amount: Money) -> Result<(), SimulationError> {
        if amount.currency() != self.primary_currency {
            return Err(SimulationError::Currency(CurrencyError::CurrencyMismatch {
                left: amount.currency(),
                right: self.primary_currency,
            }));
        }
        if amount.minor_units() < 0 {
            return Err(SimulationError::InvalidInput(
                "cannot receive a negative amount".to_string(),
            ));
        }
        self.cash = self.cash.checked_add(amount)?;
        Ok(())
    }

    pub fn inventory_lines(&self) -> Vec<InventoryLine> {
        self.products.iter().map(Product::inventory_line).collect()
    }

    pub fn financial_totals(&self) -> Result<FinancialTotals, SimulationError> {
        let mut inventory_value = Money::zero(self.primary_currency);
        let mut inventory_cost_basis = Money::zero(self.primary_currency);
        for product in &self.products {
            let market_value = product
                .current_market_value
                .checked_mul_quantity(product.inventory)?;
            inventory_value =
                inventory_value.checked_add(product.inventory_cost_basis.min(market_value))?;
            inventory_cost_basis =
                inventory_cost_basis.checked_add(product.inventory_cost_basis)?;
        }
        let technology_asset_value = self.productive_assets;
        Ok(FinancialTotals {
            cash: self.cash,
            debt: self.debt,
            inventory_cost_basis,
            inventory_value,
            technology_asset_value,
            net_worth: self
                .cash
                .checked_add(inventory_value)?
                .checked_add(technology_asset_value)?
                .checked_sub(self.debt)?,
        })
    }
}
