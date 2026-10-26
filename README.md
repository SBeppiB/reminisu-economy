# Reminisu Economy Simulator

A deterministic Rust corporate economy with finite category markets, competing companies, inventory cost accounting, production investment, debt, distress, inflation, and optional research. Pilton Dynamics remains the primary default company. Engine version: **0.2.0**.

The numerical scenarios and new economic defaults are **PLACEHOLDER / NON-CANON**. They are testing assumptions, not verified Reminisu lore or a calibrated real-world economic forecast.

## Run

### Desktop control center

On this Windows machine, double-click **Reminisu Economy** on the desktop. The
shortcut runs `launch.ps1`, which builds the latest local source before opening
the GUI. Cargo reuses unchanged build output. A small progress window appears
during the check; build failures show an error instead of opening stale code.
Close the app and click the icon again after code changes. This updates from
the files in this project, not from an online repository.

To install or repair the shortcut after moving the project:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\install-desktop.ps1
```

Keep the project folder and Rust installation available. The launcher uses
offline, locked dependencies; after dependency changes, run `cargo fetch` once
if needed. Launcher logs are in `target/launcher/`. The desktop build is kept in
`target/debug/`. Repeated clicks while the app is open do not start another
build. The launcher preserves the project working directory for configuration
and export paths.

```powershell
cargo run --offline --features gui --bin reminisu_gui
```

The optional desktop uses **egui/eframe 0.31.1**, OpenGL (`glow`), and bundled fonts. Fetch dependencies once with `cargo fetch` before using offline builds on a fresh machine. GUI dependencies are optional; CLI and library builds do not compile the renderer. The existing Windows linker configuration is retained. Native graphics drivers / a desktop session are required to launch the window.

The desktop has Dashboard, Companies, Products, Markets, Economy, Events, Seed Comparison, and **Scenario Builder** screens. Company selectors include every configured competitor; the default testing scenario has only Pilton. To explore competition, open Scenario Builder → Legacy TOML loader and provenance → Competition scenario → Validate configuration, then **Start this scenario**.

The dashboard leads with cash, company value, debt, revenue and profit cards, a financial-health explanation, and **What happened this year?**. Product summaries show sales, unmet demand, production and share. Hover economic values for plain-language definitions. Choose one historical measure at a time; expand cash-flow, balance and accounting sections for detail. Product pipelines show category demand, allocated demand, the recorded production request, actual production, inventory available for sales, sales and unmet demand.

Advance 1, 5, or 10 years, or run through an inclusive selected year. **Next year** means the next year to simulate. Stop cancels work after the last committed year. Restart controls are expandable and require confirmation before replacing history. Random seed selection is a frontend action; no clock enters the engine.

**Scenario Builder** edits a separate starting-value draft. Create, duplicate and delete companies/products, edit markets, economy settings, year and seed, and enter exact currency amounts without writing TOML. Validation explains invalid input before a run can start. Deletions, draft replacements, live-run replacements and file overwrites require confirmation. All engine settings remain available under Advanced. Export a draft before closing to retain edits.

Use **Import/Export Scenario**, **Import/Export Company**, or **Import/Export Product** with JSON or TOML paths. Company import can replace the selected company in place or append a new one. Invalid imports never partially apply. Full scenarios retain seed, year, economic rules and ordering for reproduction. See [portable format version 1 and examples](PORTABLE_FORMAT.md). The existing two-file TOML loader remains available in an expandable section.

Events are grouped by year and company, with human-readable explanations and filters for company, product, event type, severity and year. Click an explanation to inspect the recorded company/product in that year. Technical event names remain in hover/details. Health labels are presentation summaries with explicit thresholds, not new simulation mechanics.

Charts use existing history with hover readings and remain empty until a year completes. Coordinates and hover values are approximate display floats; cards, accounts, engine calculations and exports retain exact integer money. Shares are per category, not combined across unrelated markets.

Seed comparisons retain explicit ordered seeds (including duplicates), or consecutive seeds. Runs advance one year per frame, report every company, and can be cancelled between years. Complete comparisons retain the existing export schema.

Expand **Export history / current snapshot** to write history JSON, CSV plus metadata, a completed comparison, or a read-only current company snapshot. These analysis files cannot be re-imported as starting configurations. Existing history/comparison formats are unchanged; snapshots use their own version-1 envelope. File errors appear in the status bar; an I/O failure can leave a partial export or missing CSV metadata sidecar.

`src/gui/controller.rs` still owns the sole live simulation. `editor.rs` holds a detached editable representation of the existing configuration types; `portable.rs` validates transfers; `presentation.rs` provides read-only explanations; `views.rs` and `charts.rs` render real state. Engine additions are optional starting assets/basis and recording the already-computed production request. Economic formulas, RNG calls and transaction ordering are unchanged.

Current limitations: file selection uses path fields rather than OS dialogs; CSV import and save/resume are not provided; charts do not support zoom/pan; large histories or expensive years can slow rendering. File operations remain synchronous. Drafts are not autosaved.

GUI-inclusive checks:

```powershell
cargo test --offline --features gui
cargo clippy --offline --features gui --all-targets -- -D warnings
cargo fmt --check
```

### Existing CLI

The default `cargo run` still launches the CLI; an explicit equivalent is `cargo run --offline --bin reminisu_economy -- --preset quick --no-export`.

Run from the project directory, or supply explicit configuration paths.

```powershell
cargo run --offline -- validate
cargo run --offline -- --preset quick --quiet --no-export
cargo run --offline -- --preset 3421-4000 --summary-every 10 --seed 42
cargo run --offline -- --start-year 3421 --end-year 3500 --verbose --no-export
cargo run --offline -- --random-seed --export-history interesting.json --export-csv interesting.csv
cargo run --offline -- --config config/default.toml --scenario scenarios/testing.toml
cargo run --offline -- --scenario scenarios/competition.toml --preset quick --verbose --no-export
cargo run --offline -- --batch 100 --seed 42 --export-history comparison.json
cargo run --offline -- --seeds 1,42,99 --end-year 4000 --no-export --timing
```

Existing flags remain supported. Defaults remain years 3421 through 3455 inclusive, seed 42, normal output every 10 completed years, and `history.json` export. `--no-export` disables JSON; CSV is independently opt-in. Output files replace existing files at their supplied paths. The pre-existing `history.json` is an older format-2 artifact and is not silently migrated.

Presets: `quick` = 3421-3425, `3421-3455`, and `3421-4000`. Later year, preset, seed, export and verbosity flags override earlier ones. The final simulated year must be below `i32::MAX`. Intervals and batch counts must be positive. CSV supports single runs only.

Quiet mode prints run information, warnings, and each company's final summary. Normal mode adds interval summaries and events; verbose mode prints every company/product/year. Output is presented after execution, not streamed live. Time-derived random seeds are selected only by the CLI; the seed is recorded for reproduction.

## Architecture

| Module | Responsibility |
|---|---|
| `simulation.rs` | Validated world initialization, queries, ordered phases, atomic year commits, RNG and event types |
| `operations.rs` | Company preparation, production, product settlement, financial settlement, annual records |
| `accounting.rs` | Accrual income statement and separate cash-flow reconciliation |
| `market.rs` | Finite category pools, demand forecasts, competitive allocation and supply-constrained clearing |
| `economy.rs` | Deterministic growth, inflation, interest, demand conditions and shocks |
| `finance.rs` | Credit ceilings, borrowing, interest, repayments and insolvency policy |
| `pricing.rs` | Bounded cost-, inventory-, demand- and competitor-sensitive price decisions |
| `investment.rs` | Cash-funded capacity expansion and uncertain bounded research |
| `technology.rs` | Products, weighted-average inventory cost, manufacturing and selling |
| `faction.rs`, `asset.rs` | Company balances and transparent valuation |
| `config.rs` | Strict TOML schemas and resolved configuration |
| `history.rs` | Company/year records and company-specific summaries |
| `control.rs` | Frontend-independent requests and sequential seed comparisons |
| `export.rs` | Versioned JSON and CSV analysis artifacts |
| `main.rs` | CLI arguments, orchestration, provenance and presentation |

The economy engine dependencies are unchanged; the desktop adds optional eframe. Money is `i128` minor units, 100 per displayed unit; economic percentages and basis points use integer arithmetic. See [ECONOMIC_MODEL.md](ECONOMIC_MODEL.md) for formulas, every new parameter, and model limitations.

## Configuration

`config/default.toml` contains existing demand/production controls plus `[economics]`. `scenarios/testing.toml` configures Pilton, both products, finite markets and a 500,000 UFC credit limit. `scenarios/competition.toml` adds a fictional test rival in the tactical-exosuit category.

Each document has `status = "CANON"`, `"ESTIMATED"`, or `"PLACEHOLDER"`; `[economics]` also has its own status. Omitted new economic settings use independently marked PLACEHOLDER defaults. All supplied numbers are PLACEHOLDER. Do not relabel inferred values as canon.

Unknown fields are rejected, including inside new tables. Money accepts integer minor units or quoted decimal integer strings when TOML's i64 range is insufficient; floats are rejected. Validation checks names, currencies, money signs, credit limits, percentage/rate bounds, market references, market ceilings, research limits, capacity ceilings and year ranges.

Older TOML files still load: omitted competitors become an empty list, credit defaults to zero, categories default to product names, maximum manufacturing capacity defaults to four times initial capacity, and absent market tables derive a finite pool from category baseline demand with a four-times ceiling. These compatibility assumptions are PLACEHOLDER. Old numerical trajectories do not persist under the new engine.

Research is disabled by default (`research_cash_percent = 0`). Increase it in an engine configuration to exercise uncertain improvements in production efficiency and attractiveness. Both capacity expansion and research spend retained cash after obligations and debt service.

## Library API and determinism

`Simulation::new`, `advance_one_year`, `advance_years`, and inclusive `run_until` remain available. `current_year()` remains the next year to simulate. A failed step leaves company state, RNG, markets, macro state, events and history unchanged; earlier successful years remain committed. The transactional candidate copies live state without cloning accumulated history.

Compatibility queries `current_snapshot()`, `product_state(name)`, `history_for_year(year)`, and `History::summary()` describe the primary company. `faction_state(name)` searches all companies. New queries are `companies()`, `markets()`, `economy()`, `company_product_state(company, product)`, `company_history_for_year(company, year)`, and `History::summary_for(company)`.

History now contains one record per company per year, ordered by year then configured company order. Companies have unique names; products need unique names within their company. All companies currently share one currency. Stable numeric entity IDs and cross-currency competitive trade are future extensions.

Determinism depends on seed, engine version, resolved configuration, starting state and entity ordering. No system time or randomness enters the engine. Ordered vectors and deterministic validation sets are used; integer allocation ties favor configured ordering. Changes to ordering can change funding priority, rounding and RNG consumption.

## Accounting and valuation

Production reduces cash immediately and adds inventory cost basis. Selling recognizes weighted-average COGS. Revenue, COGS, production spending, overhead, maintenance, research, depreciation, interest, gross/operating/net profit, operating cash flow, capital spending, borrowing, repayments and total cash flow are distinct fields.

`profit` is an alias for **net accounting profit**; `expenses` is total recognized income-statement expense. Cash is reconciled against cash flow, not profit. Unsold inventory is no longer immediately expensed. Starting inventory has a cost basis at the configured initial unit production cost.

Valuation is cash + lower-of-cost-or-market inventory + depreciated purchased productive assets - debt. Research is expensed, so it is not also capitalized. No overlapping earnings multiple or speculative IP premium is added. Inventory valuation changes do not rewrite historical cost basis or accounting earnings.

Negative cash represents uncovered obligations in this simplified model. It cannot fund positive-cost production. Negative cash or negative net worth triggers distress; consecutive distressed years can trigger bankruptcy after the configured grace period. Bankruptcy retains and freezes the company; liquidation, creditor recoveries, restructuring and post-bankruptcy interest are not modeled.

## Events

Typed events include production suspension/eligibility, negative cash, stockouts, excess inventory, market-share gains/losses, capacity expansion, borrowing, repayment, financial distress, bankruptcy, demand shocks and cost shocks.

Stock, shock, borrowing, repayment and distress events mark entry into a condition, not every repeated year. Share events require movement from the last reported share anchor by a configured threshold. Expansion events represent actual investments. Annual monetary flows remain visible in accounts even when no new transition event is emitted.

## Export formats

History JSON and CSV are **format 3**, engine **0.2.0**. Seed comparison JSON is **format 2**. Consumers must explicitly recognize the new versions because profit and valuation meanings changed.

JSON retains historical `*_minor` fields and adds per-record `accounts` (typed `{currency, minor_units}` money), debt, inventory basis, distress, bankruptcy, macro state and market state. Product records add category, COGS, production spending, cost basis, unit cost, capacity, utilization, share, unmet demand and research progress. Top-level company/market/economy snapshots describe closing state. Metadata includes resolved configuration, statuses, currencies, engine version and actual simulated years. CLI metadata adds source paths, complete source documents and timestamp. The core library document stays timestamp-free.

CSV retains the previous first 21 columns and appends accounting, financing, valuation, production, research and macro fields. Rows are per company/product/year. Company totals repeat across products: do not sum them across product rows. The CLI writes `<csv path>.metadata.json` even if JSON history is disabled. CSV escaping handles commas, quotes and newlines.

Comparison outcomes preserve primary-company totals and add a `companies` list containing each company's results, debt and bankruptcy state. The legacy `major_events` field counts all events, not only severe ones.

Monetary output remains exact integer minor units. Consumers need arbitrary-precision integer parsing. Product prices in history are closing prices; revenue uses the earlier sale price. Capacity and research progress are closing values; utilization describes production against capacity available that year.

## Future saves

The complete `Simulation` implements serialization, including RNG state, all companies, inventory basis, demand/price/share memory, research, event latches, debt, market fractional-growth remainders and macro state. Individual financial/history types round-trip exact integers.

This prepares a future save format; it does **not** ship a supported save/load API. Analysis exports omit RNG state and remain non-resumable (`save_format_version = null`). A future loader needs an independently versioned envelope, full invariant validation, migration policy, atomic file replacement, and uninterrupted-versus-restored continuation tests. Raw serialization is not a validated save contract.

## Compatibility and verification

CLI flags and primary-company convenience queries remain available. Rust struct literals need the new fields; using bundled defaults then overriding fields is convenient. `History::records()` now includes all companies. `profit`, `expenses`, and valuation semantics intentionally change. Old exports are not read or reinterpreted. `Money::checked_from_units` offers checked construction; the existing infallible `from_units` convenience method now fails explicitly on overflow instead of wrapping in release mode.

The existing cash/profit identity test now checks cash flow. Affordability and no-credit distress fixtures explicitly disable the default credit facility. The price-crossing fixture disables inflation/shocks to isolate price behavior. The rollback fixture uses an actual expense overflow because its old incidental price overflow was eliminated. Existing CLI coverage remains intact.

```powershell
cargo test --offline
cargo fmt --check
cargo clippy --offline --all-targets -- -D warnings
```

The expanded suite covers accounting reconciliation, weighted-average COGS, finite markets, competitor allocation, debt and interest, insolvency, inflation, market growth, pricing limits, research uncertainty and bounds, capacity investment, deterministic multi-company runs, failed-step rollback, exact serialization and export compatibility.
