use crate::config::{Config, StockGroup};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;

#[derive(Debug, Clone, PartialEq)]
pub enum InputMode {
    Normal,
    Search,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ChartMode {
    Intraday,
    DailyK,
}

#[derive(Debug, Clone)]
pub struct Stock {
    pub code: String, // e.g. "sh600519"
    pub name: String, // e.g. "贵州茅台"
    pub price: f64,
    pub change: f64,
    pub pct_change: f64,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: f64,           // in hands (手)
    pub amount: f64,           // in Yuan (元)
    pub bid_prices: Vec<f64>,  // Buy 1 to Buy 5
    pub bid_volumes: Vec<i64>, // Buy 1 to Buy 5 (in hands)
    pub ask_prices: Vec<f64>,  // Sell 1 to Sell 5
    pub ask_volumes: Vec<i64>, // Sell 1 to Sell 5
}

#[derive(Debug, Clone, PartialEq)]
pub struct StockSearchResult {
    pub code: String,
    pub name: String,
    pub market: String,
    pub pinyin: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AddStockStatus {
    Added,
    AlreadyExists,
    Invalid,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AddStockResult {
    pub status: AddStockStatus,
    pub code: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KLine {
    pub date: String,
    pub open: f64,
    pub close: f64,
    pub high: f64,
    pub low: f64,
    pub volume: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MinutePoint {
    pub time: String,
    pub price: f64,
    pub volume: f64,
    pub amount: f64,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct MarketIndex {
    pub code: String,
    pub name: String,
    pub price: f64,
    pub change: f64,
    pub pct_change: f64,
}

pub struct App {
    pub config: Config,
    pub active_group_idx: usize,
    pub selected_stock_idx: usize,
    pub stock_data: HashMap<String, Stock>,
    pub kline_data: HashMap<String, Vec<KLine>>,
    pub intraday_data: HashMap<String, Vec<MinutePoint>>,
    pub indices: Vec<MarketIndex>,
    pub input_mode: InputMode,
    pub search_input: String,
    pub search_results: Vec<StockSearchResult>,
    pub selected_search_result_idx: usize,
    pub chart_mode: ChartMode,
    pub show_help_popup: bool,
    pub show_opening_auction_popup: bool,
    pub status_message: Option<String>,
    pub should_quit: bool,
}

impl App {
    /// Creates application state from persisted config with empty live-data caches.
    pub fn new() -> Self {
        let config = Config::load();
        let kline_data = Self::load_cached_kline_data(&config);
        Self {
            config,
            active_group_idx: 0,
            selected_stock_idx: 0,
            stock_data: HashMap::new(),
            kline_data,
            intraday_data: HashMap::new(),
            indices: Vec::new(),
            input_mode: InputMode::Normal,
            search_input: String::new(),
            search_results: Vec::new(),
            selected_search_result_idx: 0,
            chart_mode: ChartMode::Intraday,
            show_help_popup: false,
            show_opening_auction_popup: false,
            status_message: Some("正在加载最近可用行情...".to_string()),
            should_quit: false,
        }
    }

    /// Loads cached daily K-line history for every stock currently present in config.
    fn load_cached_kline_data(config: &Config) -> HashMap<String, Vec<KLine>> {
        let mut cache = HashMap::new();
        let Some(cache_dir) = Config::kline_cache_dir() else {
            return cache;
        };

        for group in &config.groups {
            for code in &group.stocks {
                if cache.contains_key(code) {
                    continue;
                }

                let path = cache_dir.join(format!("{}.json", code));
                let Ok(content) = fs::read_to_string(path) else {
                    continue;
                };
                let Ok(kline) = serde_json::from_str::<Vec<KLine>>(&content) else {
                    continue;
                };
                if !kline.is_empty() {
                    cache.insert(code.clone(), kline);
                }
            }
        }

        cache
    }

    /// Updates in-memory daily K-line history and persists it to the local cache directory.
    pub fn update_kline_data(&mut self, code: String, kline: Vec<KLine>) {
        self.kline_data.insert(code.clone(), kline.clone());
        if let Err(err) = Self::save_kline_cache(&code, &kline) {
            self.status_message = Some(format!("日K缓存保存失败({}): {}", code, err));
        }
    }

    /// Writes one stock's daily K-line history to a JSON cache file.
    fn save_kline_cache(code: &str, kline: &[KLine]) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(cache_dir) = Config::kline_cache_dir() {
            fs::create_dir_all(&cache_dir)?;
            let content = serde_json::to_string_pretty(kline)?;
            fs::write(cache_dir.join(format!("{}.json", code)), content)?;
        }
        Ok(())
    }

    pub fn current_group(&self) -> Option<&StockGroup> {
        self.config.groups.get(self.active_group_idx)
    }

    pub fn selected_stock_code(&self) -> Option<String> {
        let group = self.current_group()?;
        group.stocks.get(self.selected_stock_idx).cloned()
    }

    /// Replaces fuzzy search results and resets the selected candidate.
    pub fn update_search_results(&mut self, results: Vec<StockSearchResult>) {
        self.search_results = results;
        self.selected_search_result_idx = 0;
    }

    /// Clears the add-stock search input and candidate state.
    pub fn clear_search_state(&mut self) {
        self.search_input.clear();
        self.search_results.clear();
        self.selected_search_result_idx = 0;
    }

    /// Toggles the keyboard help popup overlay.
    pub fn toggle_help_popup(&mut self) {
        self.show_help_popup = !self.show_help_popup;
    }

    /// Closes any active keyboard help popup overlay.
    pub fn close_help_popup(&mut self) {
        self.show_help_popup = false;
    }

    /// Toggles the opening auction popup for the currently selected stock.
    pub fn toggle_opening_auction_popup(&mut self) {
        self.show_opening_auction_popup = !self.show_opening_auction_popup;
    }

    /// Closes any active opening auction popup overlay.
    pub fn close_opening_auction_popup(&mut self) {
        self.show_opening_auction_popup = false;
    }

    /// Returns the currently selected fuzzy search candidate, if any.
    pub fn selected_search_result(&self) -> Option<&StockSearchResult> {
        self.search_results.get(self.selected_search_result_idx)
    }

    /// Moves selection to the next fuzzy search candidate.
    pub fn next_search_result(&mut self) {
        if !self.search_results.is_empty() {
            self.selected_search_result_idx =
                (self.selected_search_result_idx + 1) % self.search_results.len();
        }
    }

    /// Moves selection to the previous fuzzy search candidate.
    pub fn prev_search_result(&mut self) {
        if self.search_results.is_empty() {
            return;
        }
        if self.selected_search_result_idx == 0 {
            self.selected_search_result_idx = self.search_results.len() - 1;
        } else {
            self.selected_search_result_idx -= 1;
        }
    }

    pub fn next_stock(&mut self) {
        if let Some(group) = self.current_group() {
            if !group.stocks.is_empty() {
                self.selected_stock_idx = (self.selected_stock_idx + 1) % group.stocks.len();
            }
        }
    }

    pub fn prev_stock(&mut self) {
        if let Some(group) = self.current_group() {
            if !group.stocks.is_empty() {
                if self.selected_stock_idx == 0 {
                    self.selected_stock_idx = group.stocks.len() - 1;
                } else {
                    self.selected_stock_idx -= 1;
                }
            }
        }
    }

    pub fn next_group(&mut self) {
        if !self.config.groups.is_empty() {
            self.active_group_idx = (self.active_group_idx + 1) % self.config.groups.len();
            self.selected_stock_idx = 0;
        }
    }

    pub fn prev_group(&mut self) {
        if !self.config.groups.is_empty() {
            if self.active_group_idx == 0 {
                self.active_group_idx = self.config.groups.len() - 1;
            } else {
                self.active_group_idx -= 1;
            }
            self.selected_stock_idx = 0;
        }
    }

    /// Adds a normalized A-share code and reports whether it changed the watchlist.
    pub fn add_stock(&mut self, code: String) -> AddStockResult {
        let Some(code) = Self::normalize_stock_code(&code) else {
            self.status_message = Some("请输入股票代码，或从搜索结果中选择股票".to_string());
            return AddStockResult {
                status: AddStockStatus::Invalid,
                code: None,
            };
        };

        let mut added = false;
        let mut already_exists = false;
        if let Some(group) = self.config.groups.get_mut(self.active_group_idx) {
            if !group.stocks.contains(&code) {
                group.stocks.push(code.clone());
                self.selected_stock_idx = group.stocks.len().saturating_sub(1);
                added = true;
            } else {
                if let Some(idx) = group.stocks.iter().position(|stock| stock == &code) {
                    self.selected_stock_idx = idx;
                }
                already_exists = true;
            }
        }

        if added {
            let _ = self.config.save();
            self.status_message = Some(format!("已添加股票: {}", code));
            AddStockResult {
                status: AddStockStatus::Added,
                code: Some(code),
            }
        } else if already_exists {
            self.status_message = Some(format!("股票已在当前自选股中: {}", code));
            AddStockResult {
                status: AddStockStatus::AlreadyExists,
                code: Some(code),
            }
        } else {
            self.status_message = Some("当前自选分组不可用，无法添加股票".to_string());
            AddStockResult {
                status: AddStockStatus::Invalid,
                code: None,
            }
        }
    }

    /// Converts user-entered stock text into a supported A-share code.
    fn normalize_stock_code(input: &str) -> Option<String> {
        let code = input.trim().to_lowercase();
        if code.is_empty() {
            return None;
        }

        if code.len() == 8 {
            let (market, digits) = code.split_at(2);
            if matches!(market, "sh" | "sz" | "bj")
                && digits.len() == 6
                && digits.chars().all(|c| c.is_ascii_digit())
            {
                return Some(code);
            }
        }

        if code.len() == 6 && code.chars().all(|c| c.is_ascii_digit()) {
            if code.starts_with('6') || code.starts_with('9') || code.starts_with('5') {
                return Some(format!("sh{}", code));
            }
            if code.starts_with('0')
                || code.starts_with('3')
                || code.starts_with('2')
                || code.starts_with('1')
            {
                return Some(format!("sz{}", code));
            }
            if code.starts_with('8') || code.starts_with('4') {
                return Some(format!("bj{}", code));
            }
        }

        None
    }

    pub fn delete_selected_stock(&mut self) {
        let mut deleted = false;
        let mut code_str = String::new();
        if let Some(group) = self.config.groups.get_mut(self.active_group_idx) {
            if !group.stocks.is_empty() && self.selected_stock_idx < group.stocks.len() {
                let code = group.stocks.remove(self.selected_stock_idx);
                code_str = code;
                deleted = true;
                if self.selected_stock_idx >= group.stocks.len() && !group.stocks.is_empty() {
                    self.selected_stock_idx = group.stocks.len() - 1;
                } else if group.stocks.is_empty() {
                    self.selected_stock_idx = 0;
                }
            }
        }

        if deleted {
            let _ = self.config.save();
            self.status_message = Some(format!("已删除股票: {}", code_str));
        }
    }
}
