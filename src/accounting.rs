//! Accrual earnings and cash movements are deliberately separate.
use crate::currency::{Currency, CurrencyError, Money};

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Accounts {
    pub revenue: Money,
    pub cost_of_goods_sold: Money,
    pub production_spending: Money,
    pub operating_expenses: Money,
    pub maintenance: Money,
    pub research: Money,
    pub depreciation: Money,
    pub interest_expense: Money,
    pub capital_spending: Money,
    pub debt_taken: Money,
    pub debt_repaid: Money,
    pub gross_profit: Money,
    pub operating_profit: Money,
    pub net_profit: Money,
    pub operating_cash_flow: Money,
    pub cash_flow: Money,
}
impl Accounts {
    pub fn new(currency: Currency) -> Self {
        let z = Money::zero(currency);
        Self {
            revenue: z,
            cost_of_goods_sold: z,
            production_spending: z,
            operating_expenses: z,
            maintenance: z,
            research: z,
            depreciation: z,
            interest_expense: z,
            capital_spending: z,
            debt_taken: z,
            debt_repaid: z,
            gross_profit: z,
            operating_profit: z,
            net_profit: z,
            operating_cash_flow: z,
            cash_flow: z,
        }
    }
    pub fn close(&mut self) -> Result<(), CurrencyError> {
        self.gross_profit = self.revenue.checked_sub(self.cost_of_goods_sold)?;
        let cash_overhead = self
            .operating_expenses
            .checked_add(self.maintenance)?
            .checked_add(self.research)?;
        self.operating_profit = self
            .gross_profit
            .checked_sub(cash_overhead)?
            .checked_sub(self.depreciation)?;
        self.net_profit = self.operating_profit.checked_sub(self.interest_expense)?;
        self.operating_cash_flow = self
            .revenue
            .checked_sub(self.production_spending)?
            .checked_sub(cash_overhead)?
            .checked_sub(self.interest_expense)?;
        self.cash_flow = self
            .operating_cash_flow
            .checked_sub(self.capital_spending)?
            .checked_add(self.debt_taken)?
            .checked_sub(self.debt_repaid)?;
        Ok(())
    }
}
