# lazytrade

Rust terminal watchlist for A-share market quotes.

`lazytrade` is a lightweight TUI that tracks configured stock groups, polls live quotes during A-share trading hours, and renders indices, watchlists, quote details, and level-5 order book data in the terminal.

## Current Status

- Runtime app: Rust + Ratatui + Crossterm.
- Live quote source: Tencent Finance `qt.gtimg.cn`.
- Mock market data: removed. The UI only fills from real quote responses.
- Polling: live cadence during regular A-share trading sessions; one snapshot refresh outside trading hours.
- Chart panel: recent daily K-line history is shown when available; intraday chart rendering is not implemented yet.

## Features

- Watchlist groups with persisted local configuration.
- Live A-share stock quote refresh during market hours.
- Three index slots for 上证指数, 深证成指, 创业板指.
- Stock table with price, change, and percentage change.
- Detail panel with open, previous close, high, low, volume, amount.
- Level-5 bid/ask order book when returned by Tencent.
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

Outside trading hours it fetches one snapshot so the UI can still show the latest available quote snapshot, including the most recent close/open/high/low/volume fields returned by Tencent. It also fetches recent daily K-line history for configured watchlist stocks. After that, it only checks the clock every `60s` until the market opens, unless the watchlist codes change.

Holiday calendars and ad-hoc market closures are not currently modeled.

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

The app does not show mock prices. During A-share trading hours it starts polling Tencent realtime quotes automatically. Outside trading hours it fetches once and shows the latest available quote snapshot and recent daily K-line history returned by Tencent.

Status messages at the bottom tell you whether the app is polling live quotes or has refreshed the closed-market snapshot.

### 3. Navigate watchlists

Use:

```text
j / Down    Select next stock
k / Up      Select previous stock
[          Previous group
]          Next group
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

### 6. Edit groups manually

The TUI currently supports adding/deleting stocks inside existing groups. To rename groups, reorder groups, or create new groups, edit the config file directly:

```text
~/Library/Application Support/lazytrade/config.toml
```

Example:

```toml
[[groups]]
name = "光通信"
stocks = ["sh600487", "sh601869"]
```

Restart the app after editing the config file.

### 7. Read quote fields

When live data is available:

- Top bar shows major indices.
- Left table shows code, name, last price, change, and percent change.
- Right detail panel shows open, previous close, high, low, volume, and amount.
- Right order-book panel shows five-level bid/ask prices and volumes.

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
t       Toggle chart mode label between intraday and daily K
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
```

The app creates a default config on first launch if no config file exists.

Daily K-line history is cached as per-stock JSON files under:

```text
{config_dir}/lazytrade/kline-cache/
```

On macOS this is usually:

```text
~/Library/Application Support/lazytrade/kline-cache/
```

The app loads this cache at startup and overwrites each stock's cache file when fresh daily K-line data is fetched.

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

- No real intraday chart yet; recent daily K-line history is displayed as a compact table.
- No holiday calendar; weekend/time-window rules only.
- The `r` key updates status text but does not yet trigger an immediate fetch.
- Quote parsing depends on Tencent's field order and availability.

## License

See `LICENSE`.
