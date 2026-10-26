# Portable starting configurations, version 1

The Scenario Builder imports and exports **JSON and TOML**. These are starting
configurations, not saved running simulations. Both formats use the same existing
Rust configuration types and preserve company/product array order.

## GUI workflow

1. Open **Scenario Builder**. Edit the starting company, its products, markets,
   starting economy, year and seed. Advanced settings are expandable.
2. Enter money as ordinary currency amounts (`1250000.00`). The editor converts
   text directly to integer minor units; extra decimal places are rejected.
3. Choose a `.json` or `.toml` path and click **Export Scenario**, **Export Company**,
   or **Export Product**. Existing files require overwrite confirmation.
4. Use the matching Import button. Company imports replace the selected starting
   company by default (with confirmation), preserving its position. Uncheck the
   replacement option to append a new company. Product imports append to the
   selected company. Names must be unique within their scope.
5. Click **Start this scenario**, then confirm replacing the live run/history.
   Editing or importing a draft never mutates the active simulation.

Imports parse and validate a complete candidate, including engine initialization
and overflow checks, before replacing anything. Invalid imports leave the draft
and active run unchanged. A company/product import requires a valid current draft;
importing a whole scenario can replace an invalid draft. Company imports retain the
current scenario's markets and rules: add any required categories before importing.

A company file alone does not fix the competitive environment, seed or economic
rules. Export the **whole scenario** to reproduce a run exactly.

## Envelope

Unknown fields and unsupported versions are rejected. Exactly one payload is
required, matching `kind`.

| Field | Type | Meaning |
| --- | --- | --- |
| `format_version` | integer | Must be `1` |
| `kind` | string | `scenario`, `company`, or `product` |
| `scenario` | SimulationConfig | Required only for scenario files |
| `company` | CompanyConfig | Required only for company files |
| `product` | ProductConfig | Required only for product files |
| `start_year` | signed 32-bit integer | Required only for scenarios; less than 2147483647 |
| `seed` | decimal integer string | Required only for scenarios; 0 through 18446744073709551615 |

Money is `{ "currency": "Ufc", "minor_units": "125000000" }`, representing
1,250,000.00 UFC. **100 minor units = 1 unit for every currency.** Exports always
write monetary minor units as decimal integer strings, preserving the full signed
128-bit range across TOML and JSON tools. Import also accepts exact integer minor
units, but never floating-point money. Starting monetary values must be nonnegative.

Currency enum spellings are `Ufc`, `Rimund`, `Yeria`, `Akenako`, `Komunry`,
`Cuneragen`, `FrontierDollar`, and `MarilciaStert`. All companies and money fields
must share the scenario currency. The editor relabels a company's money when its
currency is selected; this is **not an exchange conversion**. Set every company to
the desired shared currency before starting the scenario.

TOML has signed 64-bit integers. Unsigned quantity fields larger than that range
are exported as decimal strings; the portable loader accepts integer strings for
all quantity fields listed below. JSON normally writes quantities as integers.
Optional fields are omitted or JSON `null`; TOML omits them.

## CompanyConfig

| Field | Type | Meaning |
| --- | --- | --- |
| `faction` | FactionConfig | Starting finances below |
| `products` | ordered ProductConfig array | At least one product |

FactionConfig fields:

| Field | Type | Meaning / default |
| --- | --- | --- |
| `name` | string | Nonempty, unique company name |
| `primary_currency` | currency enum | Currency shared by the scenario |
| `starting_cash` | Money | Spendable opening cash |
| `base_operating_expenses` | Money | Annual base overhead |
| `starting_debt` | optional Money | Zero if omitted; cannot exceed credit limit |
| `credit_limit` | optional Money | Zero if omitted |
| `starting_productive_assets` | optional Money | Zero if omitted; depreciable opening book value |

The productive-asset field does not add factory capacity. Capacity remains a
per-product quantity, as in the existing model.

## ProductConfig

| Field | Type | Meaning / default |
| --- | --- | --- |
| `name` | string | Nonempty, unique within the company |
| `category` | string | Shared market category; omitted/empty uses product name |
| `production_cost` | Money | Opening unit manufacturing cost |
| `starting_market_value` | Money | Opening selling price per unit |
| `starting_inventory` | u64 quantity | Units already manufactured |
| `starting_inventory_basis` | optional Money | Total inventory cost basis; defaults to unit cost × opening inventory |
| `base_demand` | u64 quantity | Annual baseline demand; at most 1 trillion |
| `production_capacity` | u64 quantity | Starting annual capacity; at most 1 trillion |
| `maximum_capacity` | optional u64 quantity | Defaults to four times starting capacity; at least starting capacity, at most 4 trillion |
| `popularity_percent` | u32 integer | 100 = neutral; 0 through 10000 |
| `input_share_percent` | u32 integer | Material-sensitive cost share; 0 through 100, default 60 |

Inventory basis must be zero if opening inventory is zero. Inventory valuation
still uses the lower of recorded cost basis and current market value. Setting a
custom basis does not change the cost of manufacturing new units.

See executable examples [portable-company.json](scenarios/portable-company.json)
and [portable-company.toml](scenarios/portable-company.toml).

## Scenario payload

`scenario` is the existing `SimulationConfig`, with required fields `economics`,
`competitors`, `markets`, `engine_status`, `scenario_status`, `pilton`, `products`,
`demand`, and `production`. The first company is represented by `pilton` (a
FactionConfig) and `products`; the name need not be Pilton. Remaining companies
are the ordered `competitors` array of CompanyConfig. This layout deliberately
retains the engine's existing ordering semantics rather than creating another
configuration model.

`engine_status`, `scenario_status`, and `economics.status` are `CANON`, `ESTIMATED`,
or `PLACEHOLDER`. The editor exposes them under advanced settings.

A market has `category`, `annual_customers`, `maximum_customers`, and `growth_bps`.
Its category must be unique and nonempty. Annual customers cannot exceed maximum
customers, which cannot exceed 1 trillion; growth is -5000 through 5000 basis
points. Empty markets derive categories from products. Otherwise every product
must reference a configured category.

`demand` has `yearly_variation_percent`, `momentum_percent`,
`price_sensitivity_percent`, and `max_price_change_percent`, each 0 through 100.
`production.inventory_buffer_percent` is a positive integer.

`economics` is the existing EconomicsConfig. Its complete fields, defaults and
formulas are documented in [ECONOMIC_MODEL.md](ECONOMIC_MODEL.md) and defined in
[src/config.rs](src/config.rs). All are exposed in the editor; imported settings
pass the same engine validation as legacy scenarios. No economic rate uses
floating-point arithmetic. A basis point is 0.01 percentage points.

## Compatibility and analysis exports

- Existing two-file engine/scenario TOML loading remains in **Legacy TOML loader
  and provenance**. Legacy files are not portable envelopes.
- The optional starting assets and inventory-basis fields are backward compatible:
  omitted values reproduce the previous zero-assets and cost-times-inventory rules.
- Existing history JSON/CSV remain **format 3** and comparison JSON remains
  **format 2**, with their existing layouts and numeric money representation.
- **Current company snapshot JSON** is separately versioned as
  `kind = "company_snapshot"`, `format_version = 1`, `reimportable = false`.
  It includes current company state, financial totals and the latest annual record.
  Snapshot money uses exact minor-unit strings; its seed is a string.
- A new optional `requested_production` read-only field in internal product history
  records captures the engine's already-computed request before funding. It is
  available in snapshots and the GUI. It is intentionally not added to the legacy
  format-3 history/CSV layouts. Old serialized records may omit it.
- Analysis/history/snapshot files are rejected by the portable importer. They are
  not resumable saves. Export starting scenarios for reproduction.
- CSV remains an **analysis export only**. CSV company/scenario import is not
  implemented because nested settings and exact round trips use JSON/TOML.
- File selection currently uses an editable path, rather than native file dialogs.
  Drafts are kept in memory; export before closing to retain your edits.
- File writes report I/O errors but are not atomic filesystem replacements. An I/O
  failure can leave a partial file; CSV and its metadata are separate writes.

## Presentation semantics

Health labels summarize existing state without adding economic consequences.
Priority is Bankrupt, Financial Distress, Low Cash, High Debt, Unprofitable,
Healthy. Distress means negative cash or net worth; Low Cash means cash below
current annual base overhead; High Debt means over 80% of the credit limit;
Unprofitable means negative latest annual profit. Reasons remain visible even
when a higher-priority label is selected. Bankruptcy-disabled scenarios explicitly
say that there is no bankruptcy countdown.

Cash diagrams reconcile all recorded cash movements, including maintenance,
research, interest, borrowing, repayment and investment. Their arrow order is an
accounting reconciliation, not transaction timing. Profit is not treated as cash.
Production diagrams use recorded requests and actual production; closing prices
are identified as closing prices. A suspension event follows the closing price
update, so its explanation does not claim that earlier production in that year
was zero. Charts use approximate floating-point display coordinates only; exact
money in cards, accounts and exports never flows through those coordinates.

