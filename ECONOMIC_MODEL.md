# Economic model, version 0.2.0

All new coefficients and the supplied world data are PLACEHOLDER. The model is intentionally an annual corporate simulation, not a complete financial-accounting standard or a closed national economy. Integer division truncates toward zero; nonnegative quantity division floors. Monetary quantities use i128 minor units. Percentages use 100 as the denominator; basis points (bps) use 10000.

## Year order and decisions

1. Advance seeded macro state and bounded category customer pools.
2. Inflate nominal costs and the reference price used for demand; depreciate purchased productive assets.
3. Forecast all offers against the same starting market state, then allocate finite customer demand.
4. Each company manufactures toward its assigned demand and inventory target, in configured product order. Cash is spent immediately; credit can fund shortages up to a fixed ceiling.
5. Clear all category markets against actual inventories. Customers can switch to available substitutes, subject to each offer's demand ceiling.
6. Recognize sales and COGS. Set next year's prices using a snapshot of competitor prices taken before any company reprices.
7. Pay overhead, maintenance and interest; borrow within remaining credit if needed; repay principal from cash above reserves.
8. Invest retained cash in capacity, then optional research; assess distress and bankruptcy; close accounts and record state.
9. Commit the entire year only if every company's operations and all checked calculations succeed.

No company's sale receipts can fund another company's production. Within a company, current-year receipts arrive after all products have manufactured. Configuration order controls production cash priority. Finite-market clearing uses stable integer tie breaking. Failed steps also roll back macro shocks and RNG consumption.

## Accounting

For a product with inventory quantity Q and total historic cost basis B:

```text
production spending = units produced * current unit production cost
cash immediately decreases by production spending
B increases by production spending
Q increases by units produced

COGS = floor(B * units sold / Q), using quantity and basis before sale
B decreases by COGS
Q decreases by units sold
revenue = units sold * price at sale
```

Selling the last units releases all remaining basis, including integer rounding residue. Starting inventory is assigned its configured initial unit production cost. Inflation, research and year-end market-price changes never rewrite the remaining historic cost basis.

Company annual identities:

```text
gross profit = revenue - COGS
operating profit = gross profit - operating expenses - maintenance - research - depreciation
net profit = operating profit - interest expense
operating cash flow = revenue - production spending - operating expenses - maintenance - research - interest
cash flow = operating cash flow - capital spending + debt taken - debt repaid
closing cash = opening cash + cash flow
closing debt = opening debt + debt taken - debt repaid
closing inventory basis = opening basis + production spending - COGS
```

`YearHistoryRecord.profit` aliases net profit. `expenses` includes recognized COGS, cash overhead, depreciation and interest, not unsold manufacturing spending or principal repayments. All these flows are individually present in `Accounts`. Debt principal changes do not count as earnings. Expansion is capitalized; research is expensed.

## Macroeconomy

Every year draws an integer noise value in `[-macro_volatility_bps, +macro_volatility_bps]` and two shock draws. Each shock occurs when a 0-99 draw is below `shock_probability_percent`.

```text
growth = clamp(configured growth + noise, -5000, 5000)
inflation = clamp(configured inflation + noise/2, -5000, 5000)
base interest = clamp(configured interest + noise/2, 0, 10000)
demand conditions = clamp(configured conditions + noise - demand shock size, 0, 20000)
material shock = configured shock size if cost shock, otherwise zero
```

These are bounded deviations around configured means, not unbounded random walks. Cost shocks are temporary additions to the current year's unit cost; they do not permanently compound into nominal cost. Inflation does compound nominal unit costs, recurring overhead, and the reference price used to measure real price sensitivity. Year-end selling prices are still business decisions with adjustment limits; they are not automatically indexed to inflation.

## Finite category pools

Each explicit market defines a category, current annual customer pool, maximum pool, and category growth rate.

```text
growth numerator = customers * clamp(category growth + macro growth, -9000, 10000) + previous remainder
new customers = clamp(customers + trunc(growth numerator/10000), 0, maximum customers)
new remainder = growth numerator % 10000, cleared at either bound
available demand = min(customers, floor(customers * demand conditions / 10000))
```

The saved fractional-growth remainder lets a small pool grow slowly instead of losing every fractional customer. Sales across all products and companies in a category cannot exceed available demand. A zero customer pool does not spontaneously recover without a future migration/intervention system.

## Demand and competition

An offer begins with the existing transparent demand structure:

```text
popularity-adjusted baseline = base demand * popularity percent / 100
varied demand = popularity-adjusted baseline * (100 + seeded annual variation percent) / 100
momentum = (previous demand - base demand) * momentum percent / 100
price penalty = price sensitivity percent * base demand * (current price - inflation-adjusted reference price) / reference price / 100
estimated demand = max(0, varied demand + momentum - price penalty)
```

A zero reference price disables that penalty. Forecast demand then includes retained unmet demand and recent-price sensitivity:

```text
backlog = previous unmet demand * backlog percent / 100
recent price change = clamp(100 * (current price - previous selling price) / previous selling price, -100, 100)
recent-price factor = max(0, 100 - recent price change * recent price sensitivity percent / 100)
desired demand = min(available category demand, (estimated demand + backlog) * recent-price factor / 100)
```

A zero previous price disables the recent-price term. Bankruptcy sets desired demand to zero. Backlog is only from allocated customers the company could not serve; it is not an ever-growing independent customer pool.

Offer weights use desired demand, research-enhanced attractiveness, anticipated availability, prior sales share, and price relative to the cheapest active offer in the category:

```text
availability = min(100, 100 * (stock + annual capacity) / max(previous demand, 1))
availability factor = 100 - availability weight + availability * availability weight / 100
loyalty factor = 10000 + previous sales share bps * loyalty percent / 100
relative price = clamp(100 * cheapest category price / own price, 0, 100)
price factor = 100 - (100 - relative price) * price sensitivity percent / 100
weight is proportional to desired demand * attractiveness * availability factor * loyalty factor * price factor
```

The implementation retains an extra factor of 100 in weights to preserve integer precision for small markets. Popularity already affects desired demand. Attractiveness starts at 10000 bps. Zero-price offers use a relative-price factor of 100.

Allocation repeatedly divides remaining customers in proportion to active weights, caps each grant, and redistributes the remainder. Whole-unit ties follow entity ordering. Initial allocation caps each product at desired demand; final clearing caps it at min(desired demand, physical stock). Switching can therefore make actual sales higher than a product's initial allocated demand, but never larger than its own desired demand or the shared pool. Demand memory retains max(initial allocated demand, actual sales). Unmet demand is max(initial allocation - sales, 0).

Sales share is product sales / total category sales, in bps; it is zero when the category records no sales. The model currently treats each product as an offer, not each company as a single aggregated brand. Competitors obey the exact same economic phases.

## Manufacturing and investment

```text
target stock = allocated demand * inventory buffer percent / 100
requested production = max(target stock - existing stock, 0)
actual production = min(requested production, annual capacity, affordable units)
```

No new manufacturing is requested if selling price is at or below unit cost. Existing inventory may still sell at a loss. Positive-cost production cannot use negative cash. Zero-cost products are permitted and do not require unit funding.

```text
nominal unit cost *= (10000 + inflation bps) / 10000
material shock contribution = cost shock bps * input share percent / 100
unit cost = nominal unit cost * (10000 + material shock contribution) / 10000
            * (10000 - research progress bps) / 10000
maintenance = unit cost * installed capacity * maintenance bps / 10000
utilization bps = units produced * 10000 / available annual capacity
```

`input_share_percent` separates the material-sensitive fraction of unit cost from the rest. There is no explicit material stockpile. Maintenance is a cash fixed-manufacturing expense paid even if the plant produces nothing. Ordinary company overhead is separate.

A stockout can trigger expansion when enabled:

```text
additional capacity = min(max(1, floor(current capacity * expansion percent / 100)), maximum capacity - current capacity)
expansion cost = additional capacity * current unit cost * expansion cost multiple
```

Investment proceeds only if retained cash after the configured reserve covers the entire cost and cost is positive. It is not debt-funded. Expansion becomes available next year; cash spending adds equal productive-asset book value. There is no free R&D capacity expansion. Purchased asset value depreciates each year by `depreciation_percent`; physical capacity itself does not decay.

## Pricing

Let P be the selling price, C unit cost, D allocated demand, B baseline demand, and I closing inventory:

```text
target price = C * 100 / (100 - desired margin percent)
cost signal = clamp(100 * (target price - P) / P, -100, 100)
competitor signal = clamp(100 * (cheapest rival-company category price - P) / P, -100, 100)
demand signal = clamp(100 * (D - B) / max(B, 1), -100, 100)
inventory signal = min(300, 100 * I / max(D, 1))
change percent = demand signal
               + cost signal * cost price weight / 100
               + competitor signal * competitor price weight / 100
               - inventory signal * inventory price weight / 100
               + stockout price weight when stockout
change percent = clamp(change percent, -max price change, +max price change)
new price = P * (100 + change percent) / 100
```

Absent rivals contribute zero competitor signal. Zero own prices use a cost signal of 100 and zero competitor signal, but percentage-only repricing cannot lift a zero price. Desired margin is a gross-margin target, not a hard guarantee. Demand/inventory signals can keep prices below cost and make a company fail. The policy is intentionally fallible. History records the closing price, while revenue uses the pre-adjustment price.

The old public `Product::update_market_value` helper remains available for source compatibility; the complete simulation uses `pricing::update` instead.

## Credit and insolvency

A company has explicit starting debt and a fixed credit limit, both in its primary currency. Starting debt is a starting balance-sheet liability; it does not create an additional cash receipt during initialization.

```text
loan = min(max(needed cash - available cash, 0), max(credit limit - current debt, 0))
interest = debt outstanding after production borrowing * (macro base rate + credit spread) / 10000
reserve = current annual operating expense * cash reserve percent / 100
repayment = min(outstanding debt, max(cash after bills - reserve, 0) * repayment percent / 100)
```

Interest is a full-year approximation. Borrowing to cover the current interest/overhead bill does not itself incur interest until next year. All principal movements appear in financing cash flow. Credit is a contractual limit, not unlimited recapitalization.

If obligations exceed cash plus remaining credit, cash becomes negative, representing uncovered obligations/overdraft in this simplified ledger. It cannot finance positive-cost production. Negative cash OR negative valuation starts a distress streak. Healthy years reset the streak. When enabled, bankruptcy begins at `bankruptcy_after_years` consecutive distressed year ends. A bankrupt company remains visible but freezes further production, sales, overhead, debt interest and investment. There is no modeled liquidation or debt forgiveness; balances remain on record.

## Research

Research is optional and uses a percentage of surplus cash after expansion, divided equally among products. A product at its research limit receives no further spending.

```text
remaining progress = research limit - current progress
potential progress = research step * remaining progress / max(research limit, 1)
funding factor = min(10000, research spending per product * 10000 / max(unit cost, 1))
actual progress = potential progress * funding factor / 10000, only on a successful seeded trial
attractiveness bps = 10000 + cumulative research progress
```

The success probability is configurable; spending can yield no improvement. Returns diminish with accumulated progress and insufficient funding. Integer quantization may stall progress slightly below the ceiling. The limit is at most 9000 bps, preventing complete cost elimination solely from research. New efficiency affects next year's unit costs. Research is expensed regardless of success and adds no separate valuation asset.

## Valuation

```text
inventory valuation = sum(min(product historical cost basis, stock * closing selling price))
productive asset value = purchased capacity investment less accumulated depreciation
net worth = cash + inventory valuation + productive asset value - debt principal
```

The legacy `technology_asset_value` field now contains conservative purchased productive-asset book value. Initial assets have zero assigned value because there is no supplied verified book-value dataset. There is no separate capitalized research asset, baseline-margin IP estimate, or earnings multiple. Inventory valuation discounts remain valuation adjustments, not changes to historic basis or income-statement write-downs. This is transparent conservative valuation, not full statutory accounting.

## New configuration reference

All engine keys below belong to `[economics]`; `status` defaults to `PLACEHOLDER`. Existing `[demand]` and `[production]` keys remain unchanged.

| Keys | Defaults | Meaning |
|---|---|---|
| `growth_bps`, `inflation_bps`, `interest_bps` | 100, 200, 400 | Annual macro means |
| `macro_volatility_bps`, `demand_conditions_bps` | 150, 10000 | Noise bound and demand baseline |
| `shock_probability_percent`, `shock_size_bps` | 8, 1500 | Separate demand/cost shock trials and magnitude |
| `backlog_percent`, `loyalty_percent`, `availability_percent` | 25, 20, 30 | Retained unmet demand, share persistence, availability weighting |
| `recent_price_sensitivity_percent` | 30 | Demand response to recent price movement |
| `desired_margin_percent` | 30 | Pricing target; must be below 100 |
| `cost_price_weight_percent`, `competitor_price_weight_percent` | 20, 20 | Pricing signals |
| `inventory_price_weight_percent`, `stockout_price_weight_percent` | 8, 4 | Inventory discount and shortage premium |
| `credit_spread_bps`, `repayment_percent` | 200, 25 | Loan spread and surplus-cash repayment fraction |
| `cash_reserve_percent` | 100 | Reserve as a percentage of annual overhead |
| `bankruptcy_after_years`, `bankruptcy_enabled` | 3, true | Consecutive distress threshold and enforcement |
| `maintenance_bps` | 100 | Fixed annual manufacturing cost relative to installed capacity at unit cost |
| `expansion_percent`, `expansion_cost_multiple` | 10, 3 | Capacity increment and investment price |
| `depreciation_percent` | 10 | Annual book-value depreciation |
| `research_cash_percent`, `research_success_percent` | 0, 40 | Optional budget and trial success probability |
| `research_step_bps`, `research_limit_bps` | 250, 3000 | Initial progress step and cumulative ceiling |
| `share_event_threshold_bps`, `excess_inventory_percent` | 1000, 200 | Event thresholds |

New scenario fields:

- `[pilton]` and `[competitors.faction]`: optional `credit_limit` and `starting_debt`, both `Money` objects, default zero.
- Product tables: optional `category` (default product name), `input_share_percent` (default 60), `maximum_capacity` (default four times initial capacity).
- `[[markets]]`: `category`, `annual_customers`, `maximum_customers`, `growth_bps`.
- `[[competitors]]`: nested `[competitors.faction]` and `[[competitors.products]]`, using the same schemas as Pilton.

See `scenarios/competition.toml` for a complete working example. Company and product orders are meaningful. Currency enum strings are case-sensitive. Numeric rates, quantities and money signs are validated; unknown fields are rejected. Quantity safety limits are 10^12 baseline demand/initial capacity/explicit market ceiling and 4*10^12 configured maximum capacity. All companies currently require the same currency.

## Remaining limitations

Annual rather than intra-year timing; implicit customers and lenders; no closed money supply, household budgets or explicit replacement cycles; fixed credit ceilings and spreads; one shared currency for competition; no tax, dividends, wages by worker, supply chain, input shortages, acquisitions or liquidation. Bankruptcy freezes balances. Capacity has no lead time beyond next-year availability, and depreciation changes book value without wearing out capacity. Prices use a heuristic policy, not profit optimization. New products with very low demand can receive zero integer allocation. Growth is capped and a depleted zero market does not regrow. Research affects cost and attractiveness, not an invention tree. Configuration provenance is document-level. Serialization is prepared but versioned validated resumable saves are not implemented.
