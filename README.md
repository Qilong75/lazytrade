# lazytrade

Rust terminal watchlist for A-share market quotes.

`lazytrade` is a lightweight TUI that tracks configured stock groups, polls live quotes during A-share trading hours, and renders indices, watchlists, quote details, and level-5 order book data in the terminal.

## Current Status

- Runtime app: Rust + Ratatui + Crossterm.
- Live quote source: Tencent Finance `qt.gtimg.cn`.
- Mock market data: removed. The UI only fills from real quote responses.
- Polling: live cadence during regular A-share trading sessions; one snapshot refresh outside trading hours.
- Chart panel: intraday minute price/volume and minute/daily/weekly/monthly K-line views are available when Tencent returns the data.
- Scope: pure market-watch TUI. AI summaries, deep analysis pipelines, and local agent skills are intentionally excluded.

## Features

- Watchlist groups with persisted local configuration.
- Live A-share stock quote refresh during market hours.
- In-TUI group create/rename/delete/reorder plus stock move/copy between groups.
- Watchlist sorting by code, change, percent change, volume, or amount.
- Watchlist filters for rising, falling, unchanged, missing-data, and text/code matches.
- Quote freshness labels showing source, update age, live/snapshot state, and per-stock refresh errors.
- Purple watchlist highlighting for board-aware high-gain or high-loss rows.
- Group overview metrics for breadth, average change, leaders, laggards, and total amount.
- Configurable layouts for balanced, compact, large-chart, and order-book focused views.
- CSV watchlist import/export from the app config directory.
- Three index slots for 上证指数, 深证成指, 创业板指.
- Stock table with price, change, and percentage change.
- Detail panel with open, previous close, high, low, volume, amount, turnover, volume ratio, amplitude, limit prices, and market cap when source-backed.
- Level-5 bid/ask order book with totals, spread, and imbalance when returned by Tencent.
- Intraday price/volume chart with average-price line and multi-period K-line chart toggle.
- Keyboard-driven add/delete/navigation workflow.

## Market Data Behavior

The app uses Tencent's GBK-encoded quote endpoint:

```text
http://qt.gtimg.cn/q=...
```

The polling loop requests data every `3s` during regular A-share sessions:

```text
09:30-11:30
13:00-15:00
Monday-Friday
```

Outside trading hours it fetches one snapshot so the UI can still show the latest available quote snapshot, including the most recent close/open/high/low/volume fields returned by Tencent. It also fetches recent K-line history and intraday minute points for configured watchlist stocks when available. After that, it only checks the clock every `60s` until the market opens, unless the watchlist codes change.

The polling gate includes a maintained A-share holiday override table for known 2026 full-day closures, plus normal weekday/weekend rules.

## Install And Run

Prerequisites:

- Rust toolchain
- Network access to Tencent quote endpoints

Run:

```bash
cargo run
```

## How To Use

### 1. Start the app

Run the TUI from the repository root:

```bash
cargo run
```

The app opens in an alternate terminal screen. Press `q` to exit and return to your normal shell.

### 2. Wait for live data

The app does not show mock prices. During A-share trading hours it starts polling Tencent realtime quotes automatically. Outside trading hours it fetches once and shows the latest available quote snapshot, intraday minute points, and recent daily K-line history returned by Tencent.

Status messages at the bottom tell you whether the app is polling live quotes or has refreshed the closed-market snapshot.

### 3. Navigate watchlists

Use:

```text
j / Down    Select next stock
k / Up      Select previous stock
[          Previous group
]          Next group
s          Cycle sort mode
f          Cycle quick filter
/          Text/code filter
l          Cycle layout mode
```

The left panel shows the current group. The right panel shows details for the selected stock once live data has been fetched.

### 4. Add a stock

Press `a`, type a 6-digit A-share code, exchange-prefixed code, Chinese stock name, or pinyin keyword, then press `Enter`.

Examples:

```text
600487
贵州茅台
pingan
```

When the input is a fuzzy keyword, the dialog shows matching A-share candidates. Use `Up`/`Down` to select a candidate and `Enter` to add it. Plain numeric codes still infer the exchange automatically, so `600487` is stored as `sh600487`.

Press `Esc` to cancel the add-stock dialog.

### 5. Delete a stock

Select the stock in the left watchlist and press `d`.

The stock is removed from the current group and the config file is saved immediately.

### 6. Manage groups

Use:

```text
g          Create a group
r          Rename the active group
x          Delete the active group
< / >      Move the active group left/right
m          Move selected stock to the next group
c          Copy selected stock to the next group
e          Export watchlists to CSV
i          Import watchlists from CSV
```

Group and stock-list changes are saved immediately to:

```text
~/Library/Application Support/lazytrade/config.toml
```

If saving fails, the status bar reports the error.

### 7. Read quote fields

When live data is available:

- Top bar shows major indices.
- Left table shows code, name, last price, change, and percent change.
- Rows with high percent gain or high percent loss are marked with purple attention styling: 10cm stocks at `>= 6%` absolute move, and 20cm stocks at `>= 12%` absolute move.
- Right detail panel shows open, previous close, high, low, volume, amount, turnover, volume ratio, amplitude, limit prices, market cap, and group overview when source-backed.
- Detail status shows quote source, whether the data is live or a snapshot, update age, and the latest per-stock refresh error when present.
- Right order-book panel shows five-level bid/ask prices, volumes, total bid/ask volume, spread, and imbalance.
- Bottom chart panel toggles between intraday price/volume and `5m/15m/30m/60m/day/week/month` K-line views with `t`; intraday view includes a yellow average-price line.

Useful development commands:

```bash
cargo fmt
cargo check
cargo test
```

## Keyboard Controls

```text
q       Quit
j/k     Move selected stock down/up
[/]     Switch watchlist group
a       Add stock to current group
Up/Down Select add-stock search result
d       Delete selected stock from current group
s       Cycle watchlist sort mode
f       Cycle quick filter
/       Apply text/code filter
g       Create watchlist group
r       Rename current group
x       Delete current group
< / >   Reorder current group
m / c   Move/copy selected stock to next group
l       Cycle layout mode
e / i   Export/import watchlists CSV
t       Cycle chart period
Esc     Cancel add-stock input
Enter   Confirm add-stock input
```

Adding a stock accepts a 6-digit A-share code. The app infers the exchange prefix:

- `6`, `9`, `5` -> `sh`
- `0`, `1`, `2`, `3` -> `sz`
- `4`, `8` -> `bj`

## Configuration

Watchlists are stored as TOML under the platform config directory:

```text
{config_dir}/lazytrade/config.toml
```

On macOS this is usually:

```text
~/Library/Application Support/lazytrade/config.toml
```

Example:

```toml
[[groups]]
name = "默认自选"
stocks = ["sh600519", "sz000001", "sh601318"]

[[groups]]
name = "科技半导体"
stocks = ["sh688981", "sz300750", "sz002415"]

[settings]
layout_mode = "balanced"
chart_mode = "intraday"
holiday_overrides = []
workday_overrides = []
```

The app creates a default config on first launch if no config file exists.

K-line history is cached as per-stock, per-period JSON files under:

```text
{config_dir}/lazytrade/kline-cache/
```

On macOS this is usually:

```text
~/Library/Application Support/lazytrade/kline-cache/
```

The app loads this cache at startup and overwrites each stock-period cache file when fresh K-line data is fetched. Older daily cache files named `{code}.json` are still read as a fallback for the daily view.

Watchlist CSV import/export uses:

```text
{config_dir}/lazytrade/watchlist-import.csv
{config_dir}/lazytrade/watchlist-export.csv
```

CSV rows use `group,code`; single-column rows import into the active group.

## Project Layout

```text
src/
  main.rs      Terminal setup, event loop, API loop wiring
  app.rs       App state, navigation, stock add/delete behavior
  api.rs       Tencent quote client, parsing, trading-hours polling gate
  config.rs    TOML config load/save
  event.rs     Input/tick/API event channel
  ui.rs        Ratatui layout and rendering
```

## Data Source Notes

The Rust TUI currently uses Tencent realtime quote and K-line endpoints. No AI summary, deep-analysis pipeline, or Python research workflow is part of this repository.

## Limitations

- The built-in holiday table must be maintained as exchange calendars are announced.
- Quote parsing depends on Tencent's field order and availability.

## License

See `LICENSE`.
