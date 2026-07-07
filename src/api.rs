use crate::app::{
    ChartMode, KLine, MarketIndex, MinutePoint, QuoteSessionState, QuoteSource, Stock,
    StockSearchResult,
};
use crate::event::Event;
use chrono::{Datelike, Local, NaiveDate, Timelike, Weekday};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::mpsc;

const CLOSED_MARKET_POLL_INTERVAL: Duration = Duration::from_secs(60);

pub struct ApiClient {
    client: reqwest::Client,
}

#[derive(Debug, Clone, Copy)]
enum QuoteProvider {
    Tencent,
    Sina,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum ProviderCapability {
    Quote,
    KLine,
    Intraday,
    Index,
    Search,
}

impl QuoteProvider {
    /// Returns source capabilities that are implemented by this provider.
    fn capabilities(self) -> &'static [ProviderCapability] {
        match self {
            QuoteProvider::Tencent => &[
                ProviderCapability::Quote,
                ProviderCapability::KLine,
                ProviderCapability::Intraday,
                ProviderCapability::Index,
                ProviderCapability::Search,
            ],
            QuoteProvider::Sina => &[ProviderCapability::Quote],
        }
    }
}

impl ApiClient {
    /// Builds the HTTP client used for all live market data requests.
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(3))
                .no_proxy()
                .build()
                .unwrap_or_default(),
        }
    }

    /// Fetches full stock quotes for a list of A-share codes.
    pub async fn fetch_stocks(
        &self,
        codes: &[String],
    ) -> Result<Vec<Stock>, Box<dyn std::error::Error + Send + Sync>> {
        let _ = QuoteProvider::Tencent.capabilities();
        let _ = QuoteProvider::Sina.capabilities();
        if codes.is_empty() {
            return Ok(vec![]);
        }

        let mut stocks_by_code = HashMap::new();
        let mut last_error: Option<Box<dyn std::error::Error + Send + Sync>> = None;

        match self
            .fetch_stocks_from_provider(QuoteProvider::Tencent, codes)
            .await
        {
            Ok(stocks) => {
                for stock in stocks {
                    stocks_by_code.insert(stock.code.clone(), stock);
                }
            }
            Err(e) => {
                last_error = Some(e);
            }
        }

        let missing_codes: Vec<String> = codes
            .iter()
            .filter(|code| !stocks_by_code.contains_key(*code))
            .cloned()
            .collect();

        if !missing_codes.is_empty() {
            match self
                .fetch_stocks_from_provider(QuoteProvider::Sina, &missing_codes)
                .await
            {
                Ok(stocks) => {
                    for stock in stocks {
                        stocks_by_code.insert(stock.code.clone(), stock);
                    }
                }
                Err(e) => {
                    last_error = Some(e);
                }
            }
        }

        let stocks: Vec<Stock> = codes
            .iter()
            .filter_map(|code| stocks_by_code.remove(code))
            .collect();

        if stocks.is_empty() {
            return Err(last_error.unwrap_or_else(|| "all quote providers returned no data".into()));
        }

        Ok(stocks)
    }

    /// Fetches stock quotes from one concrete provider and normalizes them into Stock.
    async fn fetch_stocks_from_provider(
        &self,
        provider: QuoteProvider,
        codes: &[String],
    ) -> Result<Vec<Stock>, Box<dyn std::error::Error + Send + Sync>> {
        match provider {
            QuoteProvider::Tencent => self.fetch_tencent_stocks(codes).await,
            QuoteProvider::Sina => self.fetch_sina_stocks(codes).await,
        }
    }

    /// Fetches and parses stock quotes from Tencent Finance.
    async fn fetch_tencent_stocks(
        &self,
        codes: &[String],
    ) -> Result<Vec<Stock>, Box<dyn std::error::Error + Send + Sync>> {
        if codes.is_empty() {
            return Ok(vec![]);
        }

        let url = format!("http://qt.gtimg.cn/q={}", codes.join(","));
        let response = self.client.get(&url).send().await?;
        let bytes = response.bytes().await?;

        // Decode from GBK
        let (decoded, _, had_errors) = encoding_rs::GBK.decode(&bytes);
        if had_errors {
            return Err("GBK decoding failed".into());
        }

        let raw_str = decoded.into_owned();
        let mut stocks = Vec::new();

        for line in raw_str.lines() {
            if line.trim().is_empty() {
                continue;
            }
            if let Some(stock) = parse_tencent_stock_line(line) {
                stocks.push(stock);
            }
        }

        Ok(stocks)
    }

    /// Fetches and parses stock quotes from Sina Finance.
    async fn fetch_sina_stocks(
        &self,
        codes: &[String],
    ) -> Result<Vec<Stock>, Box<dyn std::error::Error + Send + Sync>> {
        if codes.is_empty() {
            return Ok(vec![]);
        }

        let url = format!("https://hq.sinajs.cn/list={}", codes.join(","));
        let response = self
            .client
            .get(&url)
            .header("Referer", "https://finance.sina.com.cn/")
            .send()
            .await?;
        let bytes = response.bytes().await?;

        let (decoded, _, had_errors) = encoding_rs::GBK.decode(&bytes);
        if had_errors {
            return Err("GBK decoding failed".into());
        }

        let raw_str = decoded.into_owned();
        let mut stocks = Vec::new();

        for line in raw_str.lines() {
            if line.trim().is_empty() {
                continue;
            }
            if let Some(stock) = parse_sina_stock_line(line) {
                stocks.push(stock);
            }
        }

        Ok(stocks)
    }

    /// Fetches market indices.
    pub async fn fetch_indices(
        &self,
    ) -> Result<Vec<MarketIndex>, Box<dyn std::error::Error + Send + Sync>> {
        // s_sh000001 (上证指数), s_sz399001 (深证成指), s_sz399006 (创业板指)
        let codes = vec![
            "s_sh000001".to_string(),
            "s_sz399001".to_string(),
            "s_sz399006".to_string(),
        ];
        let url = format!("http://qt.gtimg.cn/q={}", codes.join(","));
        let response = self.client.get(&url).send().await?;
        let bytes = response.bytes().await?;

        let (decoded, _, had_errors) = encoding_rs::GBK.decode(&bytes);
        if had_errors {
            return Err("GBK decoding failed".into());
        }

        let raw_str = decoded.into_owned();
        let mut indices = Vec::new();

        for line in raw_str.lines() {
            if line.trim().is_empty() {
                continue;
            }
            if let Some(index) = self.parse_index_line(line) {
                indices.push(index);
            }
        }

        Ok(indices)
    }

    /// Fetches recent K-line bars for one A-share code and period.
    pub async fn fetch_kline(
        &self,
        code: &str,
        period: ChartMode,
        count: usize,
    ) -> Result<Vec<KLine>, Box<dyn std::error::Error + Send + Sync>> {
        let Some(period_key) = period.tencent_period() else {
            return Ok(Vec::new());
        };
        let url = if period.is_minute_kline() {
            format!(
                "http://ifzq.gtimg.cn/appstock/app/kline/mkline?param={},{},,{}",
                code, period_key, count
            )
        } else {
            format!(
                "http://web.ifzq.gtimg.cn/appstock/app/fqkline/get?param={},{},,,{count},qfq",
                code, period_key
            )
        };
        let response = self.client.get(&url).send().await?;
        let body = response.text().await?;
        let value: Value = serde_json::from_str(&body)?;

        Ok(parse_kline_json(&value, code, period))
    }

    /// Fetches recent K-line bars for one A-share code and period.
    async fn fetch_and_send_kline(
        &self,
        tx: &mpsc::Sender<Event>,
        code: &str,
        period: ChartMode,
        count: usize,
    ) {
        match self.fetch_kline(code, period, count).await {
            Ok(kline) if !kline.is_empty() => {
                let _ = tx
                    .send(Event::KLineUpdate(code.to_string(), period, kline))
                    .await;
            }
            Ok(_) => {}
            Err(e) => {
                let _ = tx
                    .send(Event::ApiError(format!(
                        "{}刷新失败({}): {}",
                        period.label(),
                        code,
                        e
                    )))
                    .await;
            }
        }
    }

    /// Fetches all historical periods currently supported for a stock.
    async fn fetch_and_send_all_klines(&self, tx: &mpsc::Sender<Event>, code: &str) {
        for period in ChartMode::historical_periods() {
            self.fetch_and_send_kline(tx, code, period, 60).await;
        }
    }

    /// Fetches the selected period when it needs historical bars.
    pub async fn fetch_selected_kline(
        &self,
        tx: &mpsc::Sender<Event>,
        code: String,
        period: ChartMode,
    ) {
        if period.is_kline() {
            self.fetch_and_send_kline(tx, &code, period, 60).await;
        }
    }

    /// Fetches intraday minute points for one A-share code.
    pub async fn fetch_intraday_minutes(
        &self,
        code: &str,
    ) -> Result<Vec<MinutePoint>, Box<dyn std::error::Error + Send + Sync>> {
        let response = self
            .client
            .get("http://ifzq.gtimg.cn/appstock/app/minute/query")
            .query(&[("code", code)])
            .send()
            .await?;
        let body = response.text().await?;
        let value: Value = serde_json::from_str(&body)?;

        Ok(parse_tencent_intraday_json(&value, code))
    }

    /// Searches Tencent's smartbox endpoint for A-share stock candidates.
    pub async fn search_stocks(
        &self,
        keyword: &str,
    ) -> Result<Vec<StockSearchResult>, Box<dyn std::error::Error + Send + Sync>> {
        if keyword.trim().is_empty() {
            return Ok(Vec::new());
        }

        let response = self
            .client
            .get("https://smartbox.gtimg.cn/s3/")
            .query(&[("q", keyword), ("t", "all")])
            .send()
            .await?;
        let body = response.text().await?;

        Ok(parse_stock_search_response(&body))
    }

    /// Parses one Tencent index quote line into the app's index model.
    fn parse_index_line(&self, line: &str) -> Option<MarketIndex> {
        // Format: v_s_sh000001="2~上证指数~000001~...";
        let parts: Vec<&str> = line.split('=').collect();
        if parts.len() < 2 {
            return None;
        }

        let raw_code = parts[0].trim();
        let code = raw_code
            .strip_prefix("v_s_")
            .unwrap_or(raw_code)
            .to_string();

        let data_str = parts[1].trim().trim_matches('"').trim_end_matches(';');
        let data: Vec<&str> = data_str.split('~').collect();

        if data.len() < 6 {
            return None;
        }

        let name = data[1].to_string();
        let price = data[3].parse::<f64>().unwrap_or(0.0);
        let change = data[4].parse::<f64>().unwrap_or(0.0);
        let pct_change = data[5].parse::<f64>().unwrap_or(0.0);

        Some(MarketIndex {
            code,
            name,
            price,
            change,
            pct_change,
        })
    }
}

/// Parses one Tencent stock quote line into the internal stock model.
fn parse_tencent_stock_line(line: &str) -> Option<Stock> {
    // Format: v_sh600519="1~贵州茅台~600519~...";
    let parts: Vec<&str> = line.split('=').collect();
    if parts.len() < 2 {
        return None;
    }

    let raw_code = parts[0].trim();
    let code = raw_code.strip_prefix("v_").unwrap_or(raw_code).to_string();

    let data_str = parts[1].trim().trim_matches('"').trim_end_matches(';');
    let data: Vec<&str> = data_str.split('~').collect();

    if data.len() < 43 {
        return None;
    }

    let name = data[1].to_string();
    let price = data[3].parse::<f64>().unwrap_or(0.0);
    let close = data[4].parse::<f64>().unwrap_or(0.0);
    let open = data[5].parse::<f64>().unwrap_or(0.0);
    let volume = data[6].parse::<f64>().unwrap_or(0.0);

    let mut bid_prices = Vec::new();
    let mut bid_volumes = Vec::new();
    for i in 0..5 {
        let p_idx = 9 + i * 2;
        let v_idx = 10 + i * 2;
        if v_idx < data.len() {
            bid_prices.push(data[p_idx].parse::<f64>().unwrap_or(0.0));
            bid_volumes.push(data[v_idx].parse::<i64>().unwrap_or(0));
        }
    }

    let mut ask_prices = Vec::new();
    let mut ask_volumes = Vec::new();
    for i in 0..5 {
        let p_idx = 19 + i * 2;
        let v_idx = 20 + i * 2;
        if v_idx < data.len() {
            ask_prices.push(data[p_idx].parse::<f64>().unwrap_or(0.0));
            ask_volumes.push(data[v_idx].parse::<i64>().unwrap_or(0));
        }
    }

    let change = data[31].parse::<f64>().unwrap_or(0.0);
    let pct_change = data[32].parse::<f64>().unwrap_or(0.0);
    let high = data[33].parse::<f64>().unwrap_or(0.0);
    let low = data[34].parse::<f64>().unwrap_or(0.0);
    let amount = parse_tencent_amount_yuan(&data);
    let turnover_rate = parse_optional_f64(data.get(38).copied());
    let amplitude = parse_optional_f64(data.get(43).copied());
    let market_cap = parse_optional_f64(data.get(44).copied());
    let limit_up = parse_optional_f64(data.get(47).copied());
    let limit_down = parse_optional_f64(data.get(48).copied());
    let volume_ratio = parse_optional_f64(data.get(49).copied());

    Some(Stock {
        code,
        name,
        price,
        change,
        pct_change,
        open,
        high,
        low,
        close,
        volume,
        amount,
        bid_prices,
        bid_volumes,
        ask_prices,
        ask_volumes,
        quote_source: QuoteSource::Tencent,
        turnover_rate,
        volume_ratio,
        amplitude,
        market_cap,
        limit_up,
        limit_down,
    })
}

/// Parses one Sina stock quote line into the internal stock model.
fn parse_sina_stock_line(line: &str) -> Option<Stock> {
    // Format: var hq_str_sh600519="贵州茅台,1169.000,...";
    let (raw_name, raw_data) = line.split_once("=\"")?;
    let code = raw_name.trim().strip_prefix("var hq_str_")?.to_string();
    let data_str = raw_data.trim().trim_matches('"').trim_end_matches(';');
    let data: Vec<&str> = data_str.split(',').collect();

    if data.len() < 32 || data[0].is_empty() {
        return None;
    }

    let name = data[0].to_string();
    let open = data[1].parse::<f64>().unwrap_or(0.0);
    let close = data[2].parse::<f64>().unwrap_or(0.0);
    let price = data[3].parse::<f64>().unwrap_or(0.0);
    let high = data[4].parse::<f64>().unwrap_or(0.0);
    let low = data[5].parse::<f64>().unwrap_or(0.0);
    let volume = data[8].parse::<f64>().unwrap_or(0.0) / 100.0;
    let amount = data[9].parse::<f64>().unwrap_or(0.0);
    let change = price - close;
    let pct_change = if close.abs() > f64::EPSILON {
        change / close * 100.0
    } else {
        0.0
    };

    let mut bid_prices = Vec::new();
    let mut bid_volumes = Vec::new();
    for i in 0..5 {
        let vol_idx = 10 + i * 2;
        let price_idx = 11 + i * 2;
        if price_idx < data.len() {
            bid_volumes.push(parse_sina_volume_hands(data[vol_idx]));
            bid_prices.push(data[price_idx].parse::<f64>().unwrap_or(0.0));
        }
    }

    let mut ask_prices = Vec::new();
    let mut ask_volumes = Vec::new();
    for i in 0..5 {
        let vol_idx = 20 + i * 2;
        let price_idx = 21 + i * 2;
        if price_idx < data.len() {
            ask_volumes.push(parse_sina_volume_hands(data[vol_idx]));
            ask_prices.push(data[price_idx].parse::<f64>().unwrap_or(0.0));
        }
    }

    Some(Stock {
        code,
        name,
        price,
        change,
        pct_change,
        open,
        high,
        low,
        close,
        volume,
        amount,
        bid_prices,
        bid_volumes,
        ask_prices,
        ask_volumes,
        quote_source: QuoteSource::Sina,
        turnover_rate: None,
        volume_ratio: None,
        amplitude: if close.abs() > f64::EPSILON {
            Some((high - low) / close * 100.0)
        } else {
            None
        },
        market_cap: None,
        limit_up: None,
        limit_down: None,
    })
}

/// Parses a finite optional f64 field, treating empty and zero-like missing fields as unavailable.
fn parse_optional_f64(value: Option<&str>) -> Option<f64> {
    value
        .and_then(|value| value.parse::<f64>().ok())
        .filter(|value| value.is_finite() && value.abs() > f64::EPSILON)
}

/// Converts Sina share volume fields into A-share hands.
fn parse_sina_volume_hands(value: &str) -> i64 {
    (value.parse::<f64>().unwrap_or(0.0) / 100.0).round() as i64
}

/// Parses Tencent intraday minute JSON into ordered minute points.
fn parse_tencent_intraday_json(value: &Value, code: &str) -> Vec<MinutePoint> {
    let Some(rows) = value
        .get("data")
        .and_then(|data| data.get(code))
        .and_then(|stock| stock.get("data"))
        .and_then(|data| data.get("data"))
        .and_then(|rows| rows.as_array())
    else {
        return Vec::new();
    };

    rows.iter()
        .filter_map(|row| {
            let text = row.as_str()?;
            let fields: Vec<&str> = text.split_whitespace().collect();
            if fields.len() < 4 {
                return None;
            }

            Some(MinutePoint {
                time: fields[0].to_string(),
                price: fields[1].parse::<f64>().unwrap_or(0.0),
                volume: fields[2].parse::<f64>().unwrap_or(0.0),
                amount: fields[3].parse::<f64>().unwrap_or(0.0),
            })
        })
        .collect()
}

/// Extracts Tencent quote turnover amount as yuan from composite or fallback fields.
fn parse_tencent_amount_yuan(data: &[&str]) -> f64 {
    if let Some(amount) = data
        .get(35)
        .and_then(|field| field.split('/').nth(2))
        .and_then(|amount| amount.parse::<f64>().ok())
    {
        return amount;
    }

    if let Some(amount_10k_yuan) = data.get(57).and_then(|amount| amount.parse::<f64>().ok()) {
        return amount_10k_yuan * 10_000.0;
    }

    data.get(37)
        .and_then(|amount| amount.parse::<f64>().ok())
        .map(|amount_10k_yuan| amount_10k_yuan * 10_000.0)
        .unwrap_or(0.0)
}

/// Parses Tencent smartbox JavaScript response into A-share candidates.
fn parse_stock_search_response(body: &str) -> Vec<StockSearchResult> {
    let Some(raw_hint) = body
        .trim()
        .strip_prefix("v_hint=\"")
        .and_then(|text| text.strip_suffix("\""))
    else {
        return Vec::new();
    };

    let decoded_hint = serde_json::from_str::<String>(&format!("\"{}\"", raw_hint))
        .unwrap_or_else(|_| raw_hint.to_string());

    decoded_hint
        .split('^')
        .filter_map(|entry| {
            let fields: Vec<&str> = entry.split('~').collect();
            if fields.len() < 5 {
                return None;
            }

            let market = fields[0];
            let code = fields[1];
            let name = fields[2];
            let pinyin = fields[3];
            let category = fields[4];

            if !matches!(market, "sh" | "sz" | "bj")
                || code.len() != 6
                || !code.chars().all(|c| c.is_ascii_digit())
                || !category.starts_with("GP")
            {
                return None;
            }

            Some(StockSearchResult {
                code: format!("{}{}", market, code),
                name: name.to_string(),
                market: market.to_string(),
                pinyin: pinyin.to_string(),
            })
        })
        .take(10)
        .collect()
}

/// Starts a one-shot fuzzy stock search and sends results back to the event loop.
pub fn start_stock_search(tx: mpsc::Sender<Event>, keyword: String) {
    tokio::spawn(async move {
        let client = ApiClient::new();
        let result = client.search_stocks(&keyword).await;
        match result {
            Ok(results) => {
                let _ = tx.send(Event::SearchResultsUpdate(keyword, results)).await;
            }
            Err(e) => {
                let _ = tx
                    .send(Event::ApiError(format!("股票搜索失败: {}", e)))
                    .await;
            }
        }
    });
}

/// Parses Tencent K-line JSON into ordered bars for one stock code and period.
fn parse_kline_json(value: &Value, code: &str, period: ChartMode) -> Vec<KLine> {
    let Some(stock_node) = value
        .get("data")
        .and_then(|data| data.get(code))
        .or_else(|| {
            value
                .get("data")
                .and_then(|data| data.as_object())
                .and_then(|stocks| stocks.values().next())
        })
    else {
        return Vec::new();
    };

    let period_key = period.tencent_period().unwrap_or("day");
    let qfq_key = format!("qfq{}", period_key);
    let Some(rows) = stock_node
        .get(qfq_key.as_str())
        .or_else(|| stock_node.get(period_key))
        .or_else(|| stock_node.get("qfqday"))
        .or_else(|| stock_node.get("day"))
        .and_then(|day| day.as_array())
    else {
        return Vec::new();
    };

    rows.iter()
        .filter_map(|row| {
            let row = row.as_array()?;
            Some(KLine {
                date: row.first()?.as_str()?.to_string(),
                open: parse_json_f64(row.get(1)),
                close: parse_json_f64(row.get(2)),
                high: parse_json_f64(row.get(3)),
                low: parse_json_f64(row.get(4)),
                volume: parse_json_f64(row.get(5)),
            })
        })
        .collect()
}

/// Converts a JSON string or number field into f64, returning 0.0 for malformed values.
fn parse_json_f64(value: Option<&Value>) -> f64 {
    match value {
        Some(Value::Number(number)) => number.as_f64().unwrap_or(0.0),
        Some(Value::String(text)) => text.parse::<f64>().unwrap_or(0.0),
        _ => 0.0,
    }
}

/// Returns true during regular A-share continuous trading sessions.
fn is_a_share_market_open_now() -> bool {
    let now = Local::now();
    is_a_share_market_open(now.date_naive(), now.hour(), now.minute(), now.second())
}

/// Checks whether a China-local date/time is inside regular A-share trading hours.
fn is_a_share_market_open(date: NaiveDate, hour: u32, minute: u32, second: u32) -> bool {
    if !is_a_share_trading_day(date) {
        return false;
    }

    let seconds_since_midnight = hour * 3600 + minute * 60 + second;
    let morning_open = 9 * 3600 + 30 * 60;
    let morning_close = 11 * 3600 + 30 * 60;
    let afternoon_open = 13 * 3600;
    let afternoon_close = 15 * 3600;

    (morning_open..=morning_close).contains(&seconds_since_midnight)
        || (afternoon_open..=afternoon_close).contains(&seconds_since_midnight)
}

/// Returns true for known A-share trading days after weekend and holiday overrides.
fn is_a_share_trading_day(date: NaiveDate) -> bool {
    if static_holiday_overrides().contains(&date) {
        return false;
    }
    if static_workday_overrides().contains(&date) {
        return true;
    }
    !matches!(date.weekday(), Weekday::Sat | Weekday::Sun)
}

/// Returns known full-day A-share closures maintained in-code for current-year scheduling.
fn static_holiday_overrides() -> Vec<NaiveDate> {
    [
        "2026-01-01",
        "2026-02-16",
        "2026-02-17",
        "2026-02-18",
        "2026-02-19",
        "2026-02-20",
        "2026-04-06",
        "2026-05-01",
        "2026-05-04",
        "2026-05-05",
        "2026-06-19",
        "2026-09-25",
        "2026-10-01",
        "2026-10-02",
        "2026-10-05",
        "2026-10-06",
        "2026-10-07",
    ]
    .iter()
    .filter_map(|date| NaiveDate::parse_from_str(date, "%Y-%m-%d").ok())
    .collect()
}

/// Returns known make-up trading days when they apply to exchanges.
fn static_workday_overrides() -> Vec<NaiveDate> {
    Vec::new()
}

/// Fetches index and stock snapshots, then forwards updates to the event loop.
async fn fetch_and_send_snapshot(
    client: &ApiClient,
    tx: &mpsc::Sender<Event>,
    codes: Vec<String>,
    include_history: bool,
) {
    match client.fetch_indices().await {
        Ok(indices) => {
            let _ = tx.send(Event::IndicesUpdate(indices)).await;
        }
        Err(e) => {
            let _ = tx
                .send(Event::ApiError(format!("指数刷新失败: {}", e)))
                .await;
        }
    }

    if codes.is_empty() {
        return;
    }

    let quote_state = if is_a_share_market_open_now() {
        QuoteSessionState::Live
    } else {
        QuoteSessionState::ClosedSnapshot
    };

    match client.fetch_stocks(&codes).await {
        Ok(stocks) => {
            for stock in stocks {
                let _ = tx
                    .send(Event::StockUpdate(stock.code.clone(), stock, quote_state))
                    .await;
            }
        }
        Err(e) => {
            let _ = tx
                .send(Event::ApiError(format!("个股数据刷新失败: {}", e)))
                .await;
        }
    }

    if !include_history {
        return;
    }

    for code in codes {
        client.fetch_and_send_all_klines(tx, &code).await;
    }
}

/// Fetches one stock quote and optional daily K-line history, then forwards updates.
async fn fetch_and_send_stock_snapshot(
    client: &ApiClient,
    tx: &mpsc::Sender<Event>,
    code: String,
    include_history: bool,
) {
    match client.fetch_stocks(std::slice::from_ref(&code)).await {
        Ok(stocks) => {
            for stock in stocks {
                let quote_state = if include_history {
                    QuoteSessionState::ManualSnapshot
                } else {
                    QuoteSessionState::ClosedSnapshot
                };
                let _ = tx
                    .send(Event::StockUpdate(stock.code.clone(), stock, quote_state))
                    .await;
            }
        }
        Err(e) => {
            let error = format!("个股数据刷新失败({}): {}", code, e);
            let _ = tx.send(Event::StockError(code.clone(), error)).await;
        }
    }

    match client.fetch_intraday_minutes(&code).await {
        Ok(points) if !points.is_empty() => {
            let _ = tx.send(Event::MinuteUpdate(code.clone(), points)).await;
        }
        Ok(_) => {}
        Err(e) => {
            let _ = tx
                .send(Event::ApiError(format!("分时刷新失败({}): {}", code, e)))
                .await;
        }
    }

    if !include_history {
        return;
    }

    client.fetch_and_send_all_klines(tx, &code).await;
}

/// Starts a one-shot quote and daily K-line refresh for the supplied stock codes.
pub fn start_snapshot_refresh(tx: mpsc::Sender<Event>, codes: Vec<String>, include_history: bool) {
    tokio::spawn(async move {
        let client = ApiClient::new();
        fetch_and_send_snapshot(&client, &tx, codes, include_history).await;
    });
}

/// Starts a one-shot quote and daily K-line refresh for one stock code.
pub fn start_stock_snapshot_refresh(tx: mpsc::Sender<Event>, code: String, include_history: bool) {
    tokio::spawn(async move {
        let client = ApiClient::new();
        fetch_and_send_stock_snapshot(&client, &tx, code, include_history).await;
    });
}

/// Starts a one-shot K-line refresh for one stock and chart period.
pub fn start_kline_refresh(tx: mpsc::Sender<Event>, code: String, period: ChartMode) {
    tokio::spawn(async move {
        let client = ApiClient::new();
        client.fetch_selected_kline(&tx, code, period).await;
    });
}

/// Starts the background quote loop, using live cadence during sessions and snapshot cadence when closed.
pub fn start_api_loop(
    tx: mpsc::Sender<Event>,
    codes_to_poll: Arc<Mutex<Vec<String>>>,
    poll_interval: Duration,
) {
    let client = ApiClient::new();

    tokio::spawn(async move {
        let mut was_market_open = false;
        let mut last_closed_snapshot_codes: Option<Vec<String>> = None;
        loop {
            let market_open = is_a_share_market_open_now();
            let codes = {
                let lock = codes_to_poll.lock().unwrap();
                lock.clone()
            };

            if market_open && !was_market_open {
                let _ = tx
                    .send(Event::MarketStatus(
                        "A股交易时段，开始刷新真实行情".to_string(),
                    ))
                    .await;
            }
            if !market_open && was_market_open {
                let _ = tx
                    .send(Event::MarketStatus(
                        "A股已收盘，刷新一次最近行情和日K历史".to_string(),
                    ))
                    .await;
            }
            if !market_open && !was_market_open {
                let _ = tx
                    .send(Event::MarketStatus(
                        "非交易时段，准备刷新一次最近行情和日K历史".to_string(),
                    ))
                    .await;
            }

            if market_open {
                was_market_open = true;
                last_closed_snapshot_codes = None;
                fetch_and_send_snapshot(&client, &tx, codes, false).await;
                tokio::time::sleep(poll_interval).await;
            } else {
                let should_fetch_closed_snapshot =
                    last_closed_snapshot_codes.as_ref() != Some(&codes);
                if should_fetch_closed_snapshot {
                    fetch_and_send_snapshot(&client, &tx, codes.clone(), true).await;
                    last_closed_snapshot_codes = Some(codes);
                }
                was_market_open = false;
                tokio::time::sleep(CLOSED_MARKET_POLL_INTERVAL).await;
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn market_open_during_regular_sessions() {
        assert!(is_a_share_market_open(date("2026-07-06"), 9, 30, 0));
        assert!(is_a_share_market_open(date("2026-07-07"), 10, 0, 0));
        assert!(is_a_share_market_open(date("2026-07-08"), 13, 0, 0));
        assert!(is_a_share_market_open(date("2026-07-09"), 14, 59, 59));
    }

    #[test]
    fn market_closed_outside_regular_sessions() {
        assert!(!is_a_share_market_open(date("2026-07-10"), 9, 29, 59));
        assert!(!is_a_share_market_open(date("2026-07-10"), 11, 30, 1));
        assert!(!is_a_share_market_open(date("2026-07-10"), 12, 30, 0));
        assert!(!is_a_share_market_open(date("2026-07-10"), 15, 0, 1));
        assert!(!is_a_share_market_open(date("2026-07-11"), 10, 0, 0));
        assert!(!is_a_share_market_open(date("2026-10-01"), 10, 0, 0));
    }

    /// Parses a YYYY-MM-DD fixture date for trading-calendar tests.
    fn date(raw: &str) -> NaiveDate {
        NaiveDate::parse_from_str(raw, "%Y-%m-%d").expect("valid date fixture")
    }

    #[test]
    fn parses_a_share_search_results() {
        let body = r#"v_hint="sz~000001~\u5e73\u5b89\u94f6\u884c~payh~GP-A^hk~01833~\u5e73\u5b89\u597d\u533b\u751f~pahys~GP^sh~600519~\u8d35\u5dde\u8305\u53f0~gzmt~GP-A""#;
        let results = parse_stock_search_response(body);

        assert_eq!(results.len(), 2);
        assert_eq!(results[0].code, "sz000001");
        assert_eq!(results[0].name, "平安银行");
        assert_eq!(results[1].code, "sh600519");
        assert_eq!(results[1].name, "贵州茅台");
    }

    #[test]
    fn parses_tencent_amount_from_composite_field() {
        let raw = "1~贵州茅台~600519~1206.29~1168.63~1169.00~50363~30158~20205~1206.29~1~1206.01~2~1206.00~41~1205.85~24~1205.00~2~1206.58~2~1206.89~1~1206.90~15~1207.00~3~1207.01~4~~20260629113629~37.66~3.22~1215.00~1151.01~1206.29/50363/5975724837~50363~597572~0.40~18.23~~1215.00~1151.01~5.48~15079.61~15079.61~6.48~1285.49~1051.77~1.94~45~1186.54~13.84~18.32~~~0.35~597572.4837~0.0000~0";
        let fields: Vec<&str> = raw.split('~').collect();

        assert_eq!(parse_tencent_amount_yuan(&fields), 5_975_724_837.0);
    }

    #[test]
    fn parses_tencent_extended_quote_fields() {
        let raw = "v_sh600519=\"1~贵州茅台~600519~1206.29~1168.63~1169.00~50363~30158~20205~1206.29~1~1206.01~2~1206.00~41~1205.85~24~1205.00~2~1206.58~2~1206.89~1~1206.90~15~1207.00~3~1207.01~4~~20260629113629~37.66~3.22~1215.00~1151.01~1206.29/50363/5975724837~50363~597572~0.40~18.23~~1215.00~1151.01~5.48~15079.61~15079.61~6.48~1285.49~1051.77~1.94~45~1186.54~13.84~18.32~~~0.35~597572.4837~0.0000~0\";";
        let stock = parse_tencent_stock_line(raw).expect("valid Tencent quote");

        assert_eq!(stock.turnover_rate, Some(0.40));
        assert_eq!(stock.amplitude, Some(5.48));
        assert_eq!(stock.market_cap, Some(15079.61));
        assert_eq!(stock.limit_up, Some(1285.49));
        assert_eq!(stock.limit_down, Some(1051.77));
        assert_eq!(stock.volume_ratio, Some(1.94));
    }

    #[test]
    fn providers_declare_explicit_capabilities() {
        assert!(
            QuoteProvider::Tencent
                .capabilities()
                .contains(&ProviderCapability::KLine)
        );
        assert!(
            !QuoteProvider::Sina
                .capabilities()
                .contains(&ProviderCapability::KLine)
        );
        assert!(
            QuoteProvider::Sina
                .capabilities()
                .contains(&ProviderCapability::Quote)
        );
    }

    #[test]
    fn parses_sina_quote_into_internal_stock_model() {
        let raw = r#"var hq_str_sh600519="贵州茅台,1169.000,1168.630,1206.290,1215.000,1151.010,1206.290,1206.580,5036261,5975724837.000,100,1206.290,200,1206.010,4100,1206.000,2400,1205.850,200,1205.000,200,1206.580,100,1206.890,1500,1206.900,300,1207.000,400,1207.010,2026-06-29,11:30:00,00,";"#;
        let stock = parse_sina_stock_line(raw).expect("valid Sina quote");

        assert_eq!(stock.code, "sh600519");
        assert_eq!(stock.name, "贵州茅台");
        assert_eq!(stock.price, 1206.29);
        assert_eq!(stock.close, 1168.63);
        assert_eq!(stock.volume, 50362.61);
        assert_eq!(stock.amount, 5_975_724_837.0);
        assert_eq!(stock.bid_volumes[0], 1);
        assert_eq!(stock.ask_volumes[0], 2);
        assert_eq!(
            stock.amplitude.map(|value| (value * 100.0).round() / 100.0),
            Some(5.48)
        );
        assert!(stock.turnover_rate.is_none());
    }

    #[test]
    fn parses_tencent_intraday_points() {
        let value: Value = serde_json::from_str(
            r#"{"code":0,"data":{"sh600519":{"data":{"data":["0930 1169.00 525 61372500.00","0931 1161.60 1375 160552270.00"]}}}}"#,
        )
        .expect("valid json");
        let points = parse_tencent_intraday_json(&value, "sh600519");

        assert_eq!(points.len(), 2);
        assert_eq!(points[0].time, "0930");
        assert_eq!(points[0].price, 1169.0);
        assert_eq!(points[1].amount, 160_552_270.0);
    }

    #[test]
    fn parses_tencent_week_kline_points() {
        let value: Value = serde_json::from_str(
            r#"{"data":{"sh600519":{"qfqweek":[["2026-07-03","10.00","11.00","12.00","9.00","1000"]]}}}"#,
        )
        .expect("valid json");
        let bars = parse_kline_json(&value, "sh600519", ChartMode::WeeklyK);

        assert_eq!(bars.len(), 1);
        assert_eq!(bars[0].date, "2026-07-03");
        assert_eq!(bars[0].close, 11.0);
        assert_eq!(bars[0].volume, 1000.0);
    }

    #[test]
    fn parses_tencent_minute_kline_points() {
        let value: Value = serde_json::from_str(
            r#"{"data":{"sh600519":{"m5":[["202607061420","1208.57","1207.41","1208.57","1207.19","122.00",{},"0.10"]]}}}"#,
        )
        .expect("valid json");
        let bars = parse_kline_json(&value, "sh600519", ChartMode::Minute5);

        assert_eq!(bars.len(), 1);
        assert_eq!(bars[0].date, "202607061420");
        assert_eq!(bars[0].open, 1208.57);
        assert_eq!(bars[0].close, 1207.41);
        assert_eq!(bars[0].volume, 122.0);
    }
}
