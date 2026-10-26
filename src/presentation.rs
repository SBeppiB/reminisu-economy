//! Read-only explanations; no RNG calls or economic decisions.
use crate::{
    currency::Money,
    history::YearHistoryRecord,
    simulation::{EconomicEvent, EventType, Simulation},
};

pub fn tooltip(label: &str) -> &'static str {
    let s = label.to_lowercase();
    match s.as_str() {
        "status" | "engine status" | "scenario status" => {
            return "Provenance label: CANON is established setting information, ESTIMATED is an estimate, and PLACEHOLDER is a testing assumption. This label does not change formulas.";
        }
        "name" => {
            return "A company name must be unique across the scenario. Product names must be unique within their company; names identify history and events.";
        }
        "healthy" => {
            return "No current negative cash/value, low-cash, high-debt or latest annual loss flag. This is a presentation summary, not a guarantee about future years.";
        }
        "low cash" => {
            return "Presentation flag: current cash is below one year's current base operating expenses. No new engine consequences are attached to this label.";
        }
        "high debt" => {
            return "Presentation flag: outstanding debt exceeds 80% of the credit limit. The threshold is a UI summary, not a new borrowing rule.";
        }
        "unprofitable" => {
            return "The latest completed year's recognized expenses exceeded its revenue. A loss and a cash shortage are different conditions.";
        }
        "annual customers" => {
            return "Opening size of this category's customer pool before annual growth and economic demand conditions.";
        }
        "maximum customers" => {
            return "Upper bound on category population; cannot be less than the opening annual customer count.";
        }
        "yearly variation percent" => {
            return "Maximum size of the seeded annual variation applied to product demand. Use 0 to 100.";
        }
        "momentum percent" => {
            return "How strongly last year's demand relative to baseline carries into this year's demand. Use 0 to 100.";
        }
        "price sensitivity percent" => {
            return "How strongly price differences from the reference price influence demand. Use 0 to 100.";
        }
        "max price change percent" => {
            return "Maximum annual percentage adjustment to a product's selling price. Use 0 to 100.";
        }
        "inventory buffer percent" => {
            return "Production targets this percentage of forecast demand, then subtracts existing stock and applies capacity, viability and funding limits. Must be greater than zero; 100 means no extra buffer.";
        }
        "macro volatility bps" => {
            return "Maximum seeded variation in annual macro conditions, in basis points. 100 basis points = 1 percentage point; allowed range 0 to 5000.";
        }
        "demand conditions bps" | "demand conditions (100% = baseline)" => {
            return "Economy-wide customer-demand multiplier. 10000 basis points is the baseline (100%); allowed starting range 0 to 20000.";
        }
        "demand shock" => {
            return "Whether a temporary negative global demand shock is active in this year.";
        }
        "cost shock" | "cost shock magnitude" => {
            return "Temporary material-cost pressure this year. Only the configured material-sensitive portion of unit costs is affected.";
        }
        "shock probability percent" => {
            return "Seeded probability of a macroeconomic shock, from 0 to 100 percent.";
        }
        "shock size bps" => {
            return "Configured strength of macroeconomic shocks, from 0 to 10000 basis points.";
        }
        "backlog percent" => {
            return "Weight given to unmet customer demand carried from the previous year, from 0 to 100 percent.";
        }
        "loyalty percent" => {
            return "Weight of established market share in product demand allocation, from 0 to 100 percent.";
        }
        "availability percent" => {
            return "Weight of product availability in attracting finite market demand, from 0 to 100 percent.";
        }
        "recent price sensitivity percent" => {
            return "Weight given to recent product price changes in customer allocation, from 0 to 100 percent.";
        }
        "desired margin percent" => {
            return "Target gross margin used to suggest a cost-based price. Must be from 0 through 99; it does not guarantee realized profits.";
        }
        "cost price weight percent" => {
            return "Weight of the cost-and-margin target in the next price adjustment, from 0 to 100 percent.";
        }
        "competitor price weight percent" => {
            return "Weight of rival prices in the next price adjustment, from 0 to 100 percent.";
        }
        "inventory price weight percent" => {
            return "Weight of excess unsold inventory in the next price adjustment, from 0 to 100 percent.";
        }
        "stockout price weight percent" => {
            return "Weight of supply shortages in the next price adjustment, from 0 to 100 percent.";
        }
        "credit spread bps" => {
            return "Additional borrowing interest above the economy's base interest rate, from 0 to 10000 basis points.";
        }
        "repayment percent" => {
            return "Share of surplus cash used to repay outstanding principal, from 0 to 100 percent. Repayment is not a profit expense.";
        }
        "bankruptcy after years" => {
            return "Consecutive distressed years required before operations freeze when bankruptcy is enabled. Must be at least one.";
        }
        "expansion percent" => {
            return "Configured capacity-growth percentage used when factory expansion is triggered, from 0 to 100.";
        }
        "expansion cost multiple" => {
            return "Integer multiplier used with unit production cost to price added capacity. Allowed range 1 to 1000.";
        }
        "depreciation percent" => {
            return "Annual percentage reduction in productive-asset book value, from 0 to 100. It reduces profit, not cash.";
        }
        "research cash percent" => {
            return "Share of available research funding spent on research, from 0 to 100 percent. Spending is a cash and profit expense.";
        }
        "research success percent" => {
            return "Seeded probability of a successful funded research attempt, from 0 to 100 percent.";
        }
        "research step bps" => {
            return "Base efficiency improvement from a successful research step before diminishing returns, from 0 to 10000 basis points.";
        }
        "research limit bps" => {
            return "Upper limit on accumulated cost-reduction efficiency, from 0 to 9000 basis points.";
        }
        "share event threshold bps" => {
            return "Market-share change required to log another gain/loss event since the last such event, from 1 to 10000 basis points.";
        }
        "excess inventory percent" => {
            return "Closing stock above this percentage of demand triggers an excess-inventory flag. Allowed range 1 to 10000.";
        }
        _ => {}
    }
    if s.contains("total assets") {
        "Cash plus inventory valuation plus productive equipment. Debt is deducted afterwards to obtain company value. Assets are not all spendable money."
    } else if s.contains("production spending") {
        "Cash paid to manufacture this year's new products, including units not yet sold. Unsold units retain a cost basis in inventory."
    } else if s.contains("requested production") {
        "The engine's production request after its inventory buffer, existing stock, capacity and price checks, but before available funding limits actual production."
    } else if s.contains("actual production") || s.contains("units produced") {
        "Units actually manufactured this year, after price, capacity and funding constraints."
    } else if s.contains("units sold") {
        "Completed unit sales. Sales can include customers redirected from competitors and may exceed initially allocated demand."
    } else if s.contains("cash reserve") {
        "Cash retained relative to annual operating expenses before optional repayment and investment. 100% targets one year's overhead."
    } else if s.contains("cash flow") {
        "Actual change in spendable money. Unlike profit, this includes manufacturing unsold stock, borrowing, repayment and factory investment."
    } else if s.contains("cash") {
        "Money the company can spend immediately. Assets and unsold stock are not spendable cash."
    } else if s.contains("net worth") || s.contains("company value") {
        "Simulator valuation: cash + inventory valued at the lower of cost basis and closing market value + productive assets - debt. This is not a stock-market valuation."
    } else if s.contains("basis") || s.contains("cost of goods") {
        "Inventory basis is the cost attached to unsold products. The portion attached to products sold becomes cost of goods sold; manufacturing unsold stock does not immediately reduce profit."
    } else if s.contains("revenue") {
        "Money earned from selling products before expenses. Annual sales use the trading price, which can differ from the closing price."
    } else if s.contains("gross profit") {
        "Sales revenue minus the recorded cost of products sold."
    } else if s.contains("operating profit") {
        "Gross profit minus operating expenses, maintenance, research and depreciation; before interest."
    } else if s.contains("profit") {
        "Revenue minus recognized expenses, including the cost of products sold, overhead, depreciation and interest. Profit is different from cash flow."
    } else if s.contains("credit") {
        "Maximum outstanding borrowing allowed. Unused credit may fund production and obligations, but is not cash or income."
    } else if s.contains("debt") {
        "Money owed by the company. Borrowing increases cash and debt; principal repayment reduces both and is not a profit expense."
    } else if s.contains("market share") {
        "Percentage of completed sales in this category belonging to this product. 100 basis points = 1%."
    } else if s.contains("maximum capacity") {
        "Upper limit on factory capacity after expansion; cannot be lower than starting capacity."
    } else if s.contains("utilization") {
        "The fraction of production capacity used during the year. Low use can reflect low demand, insufficient funding or an unprofitable price."
    } else if s.contains("capacity") {
        "Maximum units the factory can manufacture in one year. Actual production also depends on inventory, demand, price and funding."
    } else if s.contains("inventory") {
        "Products manufactured but not yet sold. Inventory value is capped at its recorded cost basis or current selling value, whichever is lower."
    } else if s.contains("unmet") || s.contains("stockout") {
        "Customers allocated to this product who could not be supplied. Rival products may receive redirected sales."
    } else if s.contains("demand") {
        "Customer interest measured in units. The engine allocates finite category demand between products; actual sales may include customers redirected from rivals."
    } else if s.contains("production cost") || s.contains("unit cost") {
        "Cost to manufacture one unit, including current inflation, material shocks and research improvements. Production needs selling price to exceed this cost."
    } else if s.contains("price") || s.contains("market value") {
        "Price per unit. Closing prices are updated after sales and may differ from the price paid during the year."
    } else if s.contains("asset") {
        "Book value of productive equipment after depreciation. It contributes to company value but cannot be spent like cash."
    } else if s.contains("distress") {
        "The company ended the year with negative cash or negative net worth. Consecutive distressed years count toward the configured bankruptcy threshold."
    } else if s.contains("bankrupt") {
        "Operations freeze after enough consecutive financially distressed years, when bankruptcy is enabled."
    } else if s.contains("research") || s.contains("r&d") {
        "Research spending can improve efficiency and reduce unit costs. Progress and success follow the configured engine rules."
    } else if s.contains("maintenance") {
        "Cash spent maintaining factory capacity during the year; also recognized as an expense."
    } else if s.contains("depreciation") {
        "Reduction in the book value of productive assets. This reduces profit but is not a cash payment."
    } else if s.contains("capital") || s.contains("investment") {
        "Cash spent expanding productive capacity. It becomes a productive asset rather than an immediate profit expense."
    } else if s.contains("interest") {
        "Borrowing cost. A rate in basis points uses 100 points per percentage point."
    } else if s.contains("inflation") {
        "Annual change in nominal costs and reference prices. 100 basis points = 1%."
    } else if s.contains("growth") {
        "Change in economic or category size. 100 basis points = 1%."
    } else if s.contains("material") || s.contains("input share") {
        "Percentage of production cost exposed to material-cost shocks, from 0 to 100."
    } else if s.contains("popularity") || s.contains("attractiveness") {
        "Relative product appeal used in allocating customer demand. 100% is the neutral popularity baseline."
    } else if s.contains("expense") {
        "Annual overhead paid by the company, separate from manufacturing, maintenance, research and interest."
    } else if s.contains("eligible") {
        "Selling price exceeds unit cost. This does not guarantee production: funding, demand, capacity and bankruptcy also matter."
    } else if s.contains("currency") {
        "All companies and monetary fields in a scenario must use the same currency. Changing a currency label does not perform an exchange."
    } else if s.contains("category") {
        "Products in the same category compete for a shared finite pool of customers."
    } else if s.contains("seed") {
        "The same starting configuration, starting year and seed reproduce the same simulation. Company and product order also matters."
    } else {
        "Hover related financial values for their meaning. This is a recorded engine value or starting configuration; editing a draft takes effect only in a new run."
    }
}

pub fn explain(sim: &Simulation, event: &EconomicEvent) -> String {
    let company = event
        .entities
        .first()
        .map(String::as_str)
        .unwrap_or("The economy");
    let product = event
        .entities
        .get(1)
        .map(String::as_str)
        .unwrap_or("this product");
    let record = sim
        .history()
        .records()
        .iter()
        .find(|r| r.year == event.year && r.faction_name == company);
    let p = record.and_then(|r| r.products.iter().find(|p| p.name == product));
    match event.event_type {
        EventType::ProductionSuspended => p.map(|p| format!("{product}: new production is suspended at year end because the closing selling price ({}) no longer exceeds the unit production cost ({}). This year produced {} units before the closing price update.", p.market_value, p.production_cost, p.units_produced)).unwrap_or_else(|| event.description.clone()),
        EventType::CashNegative => format!("{company} ended the year with cash of {} after its available funding could not cover its obligations.", event.value),
        EventType::Stockout => p.map(|p| format!("{product}: customers wanted more than could be supplied. {} units sold; {} units of allocated demand remained unmet.", p.units_sold, p.unmet_demand)).unwrap_or_else(|| event.description.clone()),
        EventType::CapacityExpanded => record.map(|r| format!("{company} expanded {product}'s factories for future production. Total company factory investment this year: {}.", r.accounts.capital_spending)).unwrap_or_else(|| event.description.clone()),
        EventType::FinancialDistress => record.map(|r| format!("{company} is financially distressed: cash {}, company value {}. Either being below zero triggers distress ({} consecutive years).", r.cash, r.net_worth, r.distress_years)).unwrap_or_else(|| event.description.clone()),
        EventType::Bankruptcy => format!("{company}'s operations were frozen after consecutive years of financial distress reached the bankruptcy threshold."),
        EventType::DebtTaken => format!("{company} started borrowing to help fund its spending. New borrowing this year: {}. Borrowing adds cash and debt, not profit.", event.value),
        EventType::DebtRepaid => format!("{company} used surplus cash to repay {} of debt principal. This reduces cash and debt, not profit.", event.value),
        EventType::ProductionEligible => p.map(|p|format!("{product} can produce again: its closing price ({}) exceeds its unit cost ({}). Actual output still depends on demand, capacity and funding.",p.market_value,p.production_cost)).unwrap_or_else(||event.description.clone()),
        EventType::ExcessInventory => p.map(|p|format!("{product} has excess unsold stock: {} units remained after sales, against {} units of allocated demand. Stock crossed the configured excess-inventory threshold.",p.remaining_inventory,p.demand)).unwrap_or_else(||event.description.clone()),
        EventType::MarketShareGain | EventType::MarketShareLoss => p.map(|p|format!("{product}'s share of completed category sales {} enough to cross the event threshold. This year's share: {}.{:02}%.",if event.event_type==EventType::MarketShareGain{"rose"}else{"fell"},p.market_share_bps/100,p.market_share_bps%100)).unwrap_or_else(||event.description.clone()),
        EventType::DemandShock => "Customer demand weakened as a temporary economy-wide demand shock began.".into(),
        EventType::CostShock => "Material costs rose as a temporary economy-wide cost shock began. Products with more material-sensitive costs are more exposed.".into(),
    }
}

pub struct Health {
    pub label: &'static str,
    pub reasons: Vec<String>,
    pub warning: bool,
}
pub fn health(sim: &Simulation, index: usize) -> Health {
    let f = &sim.companies()[index];
    let latest = sim
        .history()
        .records()
        .iter()
        .rev()
        .find(|r| r.faction_name == f.name);
    let mut reasons = vec![];
    if f.cash.minor_units() < 0 {
        reasons.push(format!("Cash fell below zero: {}.", f.cash));
    }
    if let Ok(t) = f.financial_totals()
        && t.net_worth.minor_units() < 0
    {
        reasons.push(format!("Company value fell below zero: {}.", t.net_worth));
    }
    let distressed = !reasons.is_empty();
    let low = f.cash.minor_units() >= 0 && f.cash < f.base_operating_expenses;
    let high = f.credit_limit.minor_units() > 0
        && f.debt > f.credit_limit.ratio(80, 100).unwrap_or(f.credit_limit);
    let loss = latest.is_some_and(|r| r.profit.minor_units() < 0);
    if low {
        reasons.push(format!(
            "Cash {} is below one year's current base overhead ({}).",
            f.cash, f.base_operating_expenses
        ));
    }
    if high {
        reasons.push("Outstanding debt uses more than 80% of the credit limit.".into());
    }
    if loss {
        reasons.push("Recognized expenses exceeded sales revenue this year.".into());
    }
    let label = if f.bankrupt {
        "Bankrupt"
    } else if distressed {
        "Financial Distress"
    } else if low {
        "Low Cash"
    } else if high {
        "High Debt"
    } else if loss {
        "Unprofitable"
    } else {
        "Healthy"
    };
    if reasons.is_empty() {
        reasons.push(
            "No negative cash/value, annual loss, low-cash or high-debt flag in the current data."
                .into(),
        );
    }
    Health {
        label,
        reasons,
        warning: f.bankrupt || distressed,
    }
}
/// Exact cash reconciliation from the engine's already-closed accounts.
pub fn cash_stages(
    r: &YearHistoryRecord,
) -> Result<Vec<(&'static str, Money)>, crate::currency::CurrencyError> {
    let a = &r.accounts;
    Ok(vec![
        ("Starting cash", r.cash.checked_sub(a.cash_flow)?),
        ("+ Sales revenue", a.revenue),
        ("- Production spending", a.production_spending),
        ("- Operating expenses", a.operating_expenses),
        ("- Maintenance", a.maintenance),
        ("- Research", a.research),
        ("- Interest", a.interest_expense),
        ("+ Borrowing", a.debt_taken),
        ("- Debt repayment", a.debt_repaid),
        ("- Factory investment", a.capital_spending),
        ("Closing cash", r.cash),
    ])
}
