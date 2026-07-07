use crate::config::{Config, StockGroup};
use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;

#[derive(Debug, Clone, PartialEq)]
pub enum InputMode {
    Normal,
    Search,
    GroupName(GroupNameAction),
    FilterText,
}

#[derive(Debug, Clone, PartialEq)]
pub enum GroupNameAction {
    Create,
    Rename,
}

#[derive(Debug, Clone, Copy, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub enum ChartMode {
    Intraday,
    Minute5,
    Minute15,
    Minute30,
    Minute60,
    DailyK,
    WeeklyK,
    MonthlyK,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SortMode {
    ConfigOrder,
    Code,
    Change,
    PctChange,
    Volume,
    Amount,
}

#[derive(Debug, Clone, PartialEq)]
pub enum FilterMode {
    All,
    Rising,
    Falling,
    Unchanged,
    Missing,
    Text(String),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LayoutMode {
    Balanced,
    Compact,
    LargeChart,
    OrderBook,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum QuoteSessionState {
    Live,
    ClosedSnapshot,
    ManualSnapshot,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum QuoteSource {
    Tencent,
    Sina,
}

#[derive(Debug, Clone)]
pub struct QuoteMeta {
    pub source: QuoteSource,
    pub received_at: DateTime<Local>,
    pub session_state: QuoteSessionState,
    pub last_error: Option<String>,
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
    pub quote_source: QuoteSource,
    pub turnover_rate: Option<f64>,
    pub volume_ratio: Option<f64>,
    pub amplitude: Option<f64>,
    pub market_cap: Option<f64>,
    pub limit_up: Option<f64>,
    pub limit_down: Option<f64>,
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

#[derive(Debug, Clone, Default, PartialEq)]
pub struct GroupOverview {
    pub loaded: usize,
    pub missing: usize,
    pub rising: usize,
    pub falling: usize,
    pub unchanged: usize,
    pub avg_pct_change: f64,
    pub total_amount: f64,
    pub leader: Option<String>,
    pub laggard: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ImportPreview {
    pub additions: usize,
    pub duplicates: usize,
    pub invalid: usize,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HighlightThresholds {
    pub pct_change: f64,
    pub amount: f64,
}

pub struct App {
    pub config: Config,
    pub active_group_idx: usize,
    pub selected_stock_idx: usize,
    pub stock_data: HashMap<String, Stock>,
    pub quote_meta: HashMap<String, QuoteMeta>,
    pub kline_data: HashMap<(String, ChartMode), Vec<KLine>>,
    pub intraday_data: HashMap<String, Vec<MinutePoint>>,
    pub indices: Vec<MarketIndex>,
    pub input_mode: InputMode,
    pub search_input: String,
    pub search_results: Vec<StockSearchResult>,
    pub selected_search_result_idx: usize,
    pub chart_mode: ChartMode,
    pub layout_mode: LayoutMode,
    pub sort_mode: SortMode,
    pub filter_mode: FilterMode,
    pub text_input: String,
    pub show_help_popup: bool,
    pub show_opening_auction_popup: bool,
    pub status_message: Option<String>,
    pub should_quit: bool,
    pub highlight_thresholds: HighlightThresholds,
    persist_config: bool,
}

impl App {
    /// Creates application state from persisted config with empty live-data caches.
    pub fn new() -> Self {
        let config = Config::load();
        let kline_data = Self::load_cached_kline_data(&config);
        Self::from_config(config, kline_data, true)
    }

    /// Creates application state from an explicit config for tests and controlled setup paths.
    fn from_config(
        config: Config,
        kline_data: HashMap<(String, ChartMode), Vec<KLine>>,
        persist_config: bool,
    ) -> Self {
        let chart_mode = ChartMode::from_config_key(&config.settings.chart_mode);
        let layout_mode = LayoutMode::from_config_key(&config.settings.layout_mode);
        let highlight_thresholds = HighlightThresholds {
            pct_change: config.settings.highlight_pct_threshold,
            amount: config.settings.highlight_amount_threshold,
        };
        Self {
            config,
            active_group_idx: 0,
            selected_stock_idx: 0,
            stock_data: HashMap::new(),
            quote_meta: HashMap::new(),
            kline_data,
            intraday_data: HashMap::new(),
            indices: Vec::new(),
            input_mode: InputMode::Normal,
            search_input: String::new(),
            search_results: Vec::new(),
            selected_search_result_idx: 0,
            chart_mode,
            layout_mode,
            sort_mode: SortMode::ConfigOrder,
            filter_mode: FilterMode::All,
            text_input: String::new(),
            show_help_popup: false,
            show_opening_auction_popup: false,
            status_message: Some("正在加载最近可用行情...".to_string()),
            should_quit: false,
            highlight_thresholds,
            persist_config,
        }
    }

    /// Loads cached K-line history for every stock and supported period currently present in config.
    fn load_cached_kline_data(config: &Config) -> HashMap<(String, ChartMode), Vec<KLine>> {
        let mut cache = HashMap::new();
        let Some(cache_dir) = Config::kline_cache_dir() else {
            return cache;
        };

        for group in &config.groups {
            for code in &group.stocks {
                for period in ChartMode::historical_periods() {
                    let path = cache_dir.join(format!("{}-{}.json", code, period.cache_key()));
                    let fallback_path = cache_dir.join(format!("{}.json", code));
                    let content = fs::read_to_string(&path)
                        .or_else(|_| {
                            if period == ChartMode::DailyK {
                                fs::read_to_string(fallback_path)
                            } else {
                                Err(std::io::Error::new(
                                    std::io::ErrorKind::NotFound,
                                    "period cache not found",
                                ))
                            }
                        })
                        .ok();
                    let Some(content) = content else {
                        continue;
                    };
                    let Ok(kline) = serde_json::from_str::<Vec<KLine>>(&content) else {
                        continue;
                    };
                    if !kline.is_empty() {
                        cache.insert((code.clone(), period), kline);
                    }
                }
            }
        }

        cache
    }

    /// Updates in-memory K-line history for one period and persists it to the local cache directory.
    pub fn update_kline_data(&mut self, code: String, period: ChartMode, kline: Vec<KLine>) {
        self.kline_data
            .insert((code.clone(), period), kline.clone());
        if !self.persist_config {
            return;
        }
        if let Err(err) = Self::save_kline_cache(&code, period, &kline) {
            self.status_message =
                Some(format!("{}缓存保存失败({}): {}", period.label(), code, err));
        }
    }

    /// Writes one stock's K-line history for one period to a JSON cache file.
    fn save_kline_cache(
        code: &str,
        period: ChartMode,
        kline: &[KLine],
    ) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(cache_dir) = Config::kline_cache_dir() {
            fs::create_dir_all(&cache_dir)?;
            let content = serde_json::to_string_pretty(kline)?;
            fs::write(
                cache_dir.join(format!("{}-{}.json", code, period.cache_key())),
                content,
            )?;
        }
        Ok(())
    }

    /// Records a successful quote update and its source/freshness metadata.
    pub fn record_stock_update(&mut self, code: String, stock: Stock, state: QuoteSessionState) {
        let source = stock.quote_source;
        self.stock_data.insert(code.clone(), stock);
        self.quote_meta.insert(
            code,
            QuoteMeta {
                source,
                received_at: Local::now(),
                session_state: state,
                last_error: None,
            },
        );
    }

    /// Records a per-stock quote refresh failure without discarding the last good quote.
    pub fn record_stock_error(&mut self, code: String, error: String) {
        if let Some(meta) = self.quote_meta.get_mut(&code) {
            meta.last_error = Some(error.clone());
        } else {
            self.quote_meta.insert(
                code,
                QuoteMeta {
                    source: QuoteSource::Tencent,
                    received_at: Local::now(),
                    session_state: QuoteSessionState::ManualSnapshot,
                    last_error: Some(error.clone()),
                },
            );
        }
        self.status_message = Some(error);
    }

    /// Returns the currently active watchlist group, if any.
    pub fn current_group(&self) -> Option<&StockGroup> {
        self.config.groups.get(self.active_group_idx)
    }

    /// Returns the mutable active stock group, if it exists.
    fn current_group_mut(&mut self) -> Option<&mut StockGroup> {
        self.config.groups.get_mut(self.active_group_idx)
    }

    /// Returns the selected stock code in the active group, if any.
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

    /// Clears generic text input used by group and filter dialogs.
    pub fn clear_text_input(&mut self) {
        self.text_input.clear();
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

    /// Moves selection to the next visible stock under the active sort/filter modes.
    pub fn next_stock(&mut self) {
        let visible = self.visible_stock_indices();
        if visible.is_empty() {
            return;
        }
        let position = visible
            .iter()
            .position(|idx| *idx == self.selected_stock_idx)
            .unwrap_or(0);
        self.selected_stock_idx = visible[(position + 1) % visible.len()];
    }

    /// Moves selection to the previous visible stock under the active sort/filter modes.
    pub fn prev_stock(&mut self) {
        let visible = self.visible_stock_indices();
        if visible.is_empty() {
            return;
        }
        let position = visible
            .iter()
            .position(|idx| *idx == self.selected_stock_idx)
            .unwrap_or(0);
        self.selected_stock_idx = if position == 0 {
            *visible.last().expect("visible stock")
        } else {
            visible[position - 1]
        };
    }

    /// Moves active selection to the next group.
    pub fn next_group(&mut self) {
        if !self.config.groups.is_empty() {
            self.active_group_idx = (self.active_group_idx + 1) % self.config.groups.len();
            self.selected_stock_idx = self.visible_stock_indices().first().copied().unwrap_or(0);
        }
    }

    /// Moves active selection to the previous group.
    pub fn prev_group(&mut self) {
        if !self.config.groups.is_empty() {
            if self.active_group_idx == 0 {
                self.active_group_idx = self.config.groups.len() - 1;
            } else {
                self.active_group_idx -= 1;
            }
            self.selected_stock_idx = self.visible_stock_indices().first().copied().unwrap_or(0);
        }
    }

    /// Cycles through supported watchlist sort modes and preserves current stock when possible.
    pub fn cycle_sort_mode(&mut self) {
        let selected = self.selected_stock_code();
        self.sort_mode = match self.sort_mode {
            SortMode::ConfigOrder => SortMode::Code,
            SortMode::Code => SortMode::Change,
            SortMode::Change => SortMode::PctChange,
            SortMode::PctChange => SortMode::Volume,
            SortMode::Volume => SortMode::Amount,
            SortMode::Amount => SortMode::ConfigOrder,
        };
        self.restore_selection_or_first_visible(selected);
        self.status_message = Some(format!("排序: {}", self.sort_mode.label()));
    }

    /// Cycles through non-text watchlist filters and preserves current stock when possible.
    pub fn cycle_filter_mode(&mut self) {
        let selected = self.selected_stock_code();
        self.filter_mode = match self.filter_mode {
            FilterMode::All => FilterMode::Rising,
            FilterMode::Rising => FilterMode::Falling,
            FilterMode::Falling => FilterMode::Unchanged,
            FilterMode::Unchanged => FilterMode::Missing,
            FilterMode::Missing | FilterMode::Text(_) => FilterMode::All,
        };
        self.restore_selection_or_first_visible(selected);
        self.status_message = Some(format!("过滤: {}", self.filter_mode.label()));
    }

    /// Applies a text filter to codes and loaded stock names.
    pub fn apply_text_filter(&mut self, query: String) {
        let selected = self.selected_stock_code();
        if query.trim().is_empty() {
            self.filter_mode = FilterMode::All;
        } else {
            self.filter_mode = FilterMode::Text(query.trim().to_lowercase());
        }
        self.restore_selection_or_first_visible(selected);
        self.status_message = Some(format!("过滤: {}", self.filter_mode.label()));
    }

    /// Cycles through supported layout modes and persists the preference.
    pub fn cycle_layout_mode(&mut self) {
        self.layout_mode = self.layout_mode.next();
        self.config.settings.layout_mode = self.layout_mode.config_key().to_string();
        self.save_config_with_status(format!("布局: {}", self.layout_mode.label()));
    }

    /// Persists the selected chart mode in user settings.
    pub fn persist_chart_mode(&mut self) {
        self.config.settings.chart_mode = self.chart_mode.config_key().to_string();
        self.save_config_with_status(format!("图表周期: {}", self.chart_mode.label()));
    }

    /// Returns aggregate market breadth and amount for the active group.
    pub fn current_group_overview(&self) -> GroupOverview {
        let Some(group) = self.current_group() else {
            return GroupOverview::default();
        };
        group_overview(group, &self.stock_data)
    }

    /// Exports watchlist groups to the default CSV file under the app config directory.
    pub fn export_watchlists(&mut self) {
        let Some(path) = Config::watchlist_export_path() else {
            self.status_message = Some("无法确定自选股导出路径".to_string());
            return;
        };
        if let Some(parent) = path.parent() {
            if let Err(err) = fs::create_dir_all(parent) {
                self.status_message = Some(format!("导出目录创建失败: {}", err));
                return;
            }
        }
        match fs::write(&path, self.watchlists_csv()) {
            Ok(()) => {
                self.status_message = Some(format!("已导出自选股: {}", path.display()));
            }
            Err(err) => {
                self.status_message = Some(format!("自选股导出失败: {}", err));
            }
        }
    }

    /// Imports watchlist rows from the default CSV/text file and reports a preview summary.
    pub fn import_watchlists(&mut self) -> ImportPreview {
        let Some(path) = Config::watchlist_import_path() else {
            self.status_message = Some("无法确定自选股导入路径".to_string());
            return ImportPreview::default();
        };
        let Ok(content) = fs::read_to_string(&path) else {
            self.status_message = Some(format!("导入文件不存在: {}", path.display()));
            return ImportPreview::default();
        };
        let preview = self.apply_watchlist_import(&content);
        self.save_config_with_status(format!(
            "导入完成: 新增{} 重复{} 无效{}",
            preview.additions, preview.duplicates, preview.invalid
        ));
        preview
    }

    /// Serializes current watchlists as group,code CSV rows.
    fn watchlists_csv(&self) -> String {
        let mut rows = String::from("group,code\n");
        for group in &self.config.groups {
            for code in &group.stocks {
                rows.push_str(&format!("{},{}\n", group.name, code));
            }
        }
        rows
    }

    /// Applies text/CSV watchlist rows and returns addition/duplicate/invalid counts.
    fn apply_watchlist_import(&mut self, content: &str) -> ImportPreview {
        let mut preview = ImportPreview::default();
        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.eq_ignore_ascii_case("group,code") {
                continue;
            }
            let fields: Vec<&str> = line.split(',').map(str::trim).collect();
            let (group_name, raw_code): (String, &str) = match fields.as_slice() {
                [code] => (
                    self.current_group()
                        .map(|group| group.name.clone())
                        .unwrap_or_else(|| "默认自选".to_string()),
                    *code,
                ),
                [group, code, ..] => ((*group).to_string(), *code),
                _ => {
                    preview.invalid += 1;
                    continue;
                }
            };
            let Some(code) = Self::normalize_stock_code(raw_code) else {
                preview.invalid += 1;
                continue;
            };
            let group_idx = self.ensure_group_index(&group_name);
            if self.config.groups[group_idx].stocks.contains(&code) {
                preview.duplicates += 1;
            } else {
                self.config.groups[group_idx].stocks.push(code);
                preview.additions += 1;
            }
        }
        preview
    }

    /// Returns the group index for a name, creating the group if needed.
    fn ensure_group_index(&mut self, name: &str) -> usize {
        if let Some(idx) = self
            .config
            .groups
            .iter()
            .position(|group| group.name == name)
        {
            return idx;
        }
        self.config.groups.push(StockGroup {
            name: if name.trim().is_empty() {
                "导入分组".to_string()
            } else {
                name.trim().to_string()
            },
            stocks: Vec::new(),
        });
        self.config.groups.len() - 1
    }

    /// Returns stock indices in the current group after applying filter and sort modes.
    pub fn visible_stock_indices(&self) -> Vec<usize> {
        let Some(group) = self.current_group() else {
            return Vec::new();
        };
        let mut indices: Vec<usize> = group
            .stocks
            .iter()
            .enumerate()
            .filter_map(|(idx, code)| self.stock_matches_filter(code).then_some(idx))
            .collect();

        match self.sort_mode {
            SortMode::ConfigOrder => {}
            SortMode::Code => indices.sort_by(|a, b| group.stocks[*a].cmp(&group.stocks[*b])),
            SortMode::Change => {
                sort_by_metric(&mut indices, group, &self.stock_data, SortMode::Change)
            }
            SortMode::PctChange => {
                sort_by_metric(&mut indices, group, &self.stock_data, SortMode::PctChange)
            }
            SortMode::Volume => {
                sort_by_metric(&mut indices, group, &self.stock_data, SortMode::Volume)
            }
            SortMode::Amount => {
                sort_by_metric(&mut indices, group, &self.stock_data, SortMode::Amount)
            }
        }
        indices
    }

    /// Moves current selection to its actual group index or to the first visible row.
    fn restore_selection_or_first_visible(&mut self, selected: Option<String>) {
        if let (Some(group), Some(selected)) = (self.current_group(), selected.as_ref()) {
            if let Some(idx) = group.stocks.iter().position(|code| code == selected) {
                if self.visible_stock_indices().contains(&idx) {
                    self.selected_stock_idx = idx;
                    return;
                }
            }
        }
        if let Some(idx) = self.visible_stock_indices().first().copied() {
            self.selected_stock_idx = idx;
        }
    }

    /// Returns true when a stock code passes the active watchlist filter.
    fn stock_matches_filter(&self, code: &str) -> bool {
        let stock = self.stock_data.get(code);
        match &self.filter_mode {
            FilterMode::All => true,
            FilterMode::Rising => stock.is_some_and(|stock| stock.pct_change > 0.0),
            FilterMode::Falling => stock.is_some_and(|stock| stock.pct_change < 0.0),
            FilterMode::Unchanged => stock.is_some_and(|stock| stock.pct_change == 0.0),
            FilterMode::Missing => stock.is_none(),
            FilterMode::Text(query) => {
                let query = query.to_lowercase();
                code.to_lowercase().contains(&query)
                    || stock.is_some_and(|stock| stock.name.to_lowercase().contains(&query))
            }
        }
    }

    /// Creates a watchlist group with a unique non-empty name and persists the config.
    pub fn create_group(&mut self, name: String) {
        let name = self.unique_group_name(name);
        self.config.groups.push(StockGroup {
            name: name.clone(),
            stocks: Vec::new(),
        });
        self.active_group_idx = self.config.groups.len().saturating_sub(1);
        self.selected_stock_idx = 0;
        self.save_config_with_status(format!("已创建分组: {}", name));
    }

    /// Renames the active group to a unique non-empty name and persists the config.
    pub fn rename_current_group(&mut self, name: String) {
        let name = self.unique_group_name(name);
        if let Some(group) = self.current_group_mut() {
            group.name = name.clone();
            self.save_config_with_status(format!("已重命名分组: {}", name));
        }
    }

    /// Deletes the active group when at least one other group remains.
    pub fn delete_current_group(&mut self) {
        if self.config.groups.len() <= 1 {
            self.status_message = Some("至少保留一个自选分组".to_string());
            return;
        }
        let removed = self.config.groups.remove(self.active_group_idx);
        if self.active_group_idx >= self.config.groups.len() {
            self.active_group_idx = self.config.groups.len().saturating_sub(1);
        }
        self.selected_stock_idx = self.visible_stock_indices().first().copied().unwrap_or(0);
        self.save_config_with_status(format!("已删除分组: {}", removed.name));
    }

    /// Moves the active group one position earlier and persists the config.
    pub fn move_current_group_left(&mut self) {
        if self.active_group_idx == 0 || self.config.groups.is_empty() {
            return;
        }
        self.config
            .groups
            .swap(self.active_group_idx, self.active_group_idx - 1);
        self.active_group_idx -= 1;
        self.save_config_with_status("已前移分组".to_string());
    }

    /// Moves the active group one position later and persists the config.
    pub fn move_current_group_right(&mut self) {
        if self.active_group_idx + 1 >= self.config.groups.len() {
            return;
        }
        self.config
            .groups
            .swap(self.active_group_idx, self.active_group_idx + 1);
        self.active_group_idx += 1;
        self.save_config_with_status("已后移分组".to_string());
    }

    /// Moves selected stock into the next group, preserving one copy only.
    pub fn move_selected_stock_to_next_group(&mut self) -> Option<String> {
        self.transfer_selected_stock_to_next_group(true)
    }

    /// Copies selected stock into the next group if it is not already there.
    pub fn copy_selected_stock_to_next_group(&mut self) -> Option<String> {
        self.transfer_selected_stock_to_next_group(false)
    }

    /// Transfers selected stock to the next group, optionally removing it from the source group.
    fn transfer_selected_stock_to_next_group(
        &mut self,
        remove_from_source: bool,
    ) -> Option<String> {
        if self.config.groups.len() < 2 {
            self.status_message = Some("需要至少两个分组才能移动或复制股票".to_string());
            return None;
        }
        let source_idx = self.active_group_idx;
        let target_idx = (source_idx + 1) % self.config.groups.len();
        let code = self.selected_stock_code()?;
        if !self.config.groups[target_idx].stocks.contains(&code) {
            self.config.groups[target_idx].stocks.push(code.clone());
        }
        if remove_from_source {
            if let Some(pos) = self.config.groups[source_idx]
                .stocks
                .iter()
                .position(|stock| stock == &code)
            {
                self.config.groups[source_idx].stocks.remove(pos);
            }
            self.selected_stock_idx = self.visible_stock_indices().first().copied().unwrap_or(0);
        }
        let action = if remove_from_source {
            "移动"
        } else {
            "复制"
        };
        let target_name = self.config.groups[target_idx].name.clone();
        self.save_config_with_status(format!("已{} {} 到 {}", action, code, target_name));
        Some(code)
    }

    /// Returns a non-empty group name that does not collide with existing names.
    fn unique_group_name(&self, name: String) -> String {
        let base = if name.trim().is_empty() {
            "新分组".to_string()
        } else {
            name.trim().to_string()
        };
        if !self.config.groups.iter().any(|group| group.name == base) {
            return base;
        }
        for suffix in 2.. {
            let candidate = format!("{}{}", base, suffix);
            if !self
                .config
                .groups
                .iter()
                .any(|group| group.name == candidate)
            {
                return candidate;
            }
        }
        unreachable!("infinite suffix search returns a unique group name")
    }

    /// Saves config and reports success or failure in the status bar.
    fn save_config_with_status(&mut self, success_message: String) {
        if !self.persist_config {
            self.status_message = Some(success_message);
            return;
        }
        match self.config.save() {
            Ok(()) => {
                self.status_message = Some(success_message);
            }
            Err(err) => {
                self.status_message = Some(format!("配置保存失败: {}", err));
            }
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
            self.save_config_with_status(format!("已添加股票: {}", code));
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

    /// Deletes the currently selected stock from the active group.
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
            self.save_config_with_status(format!("已删除股票: {}", code_str));
        }
    }
}

impl ChartMode {
    /// Returns all historical chart periods backed by K-line data.
    pub fn historical_periods() -> [ChartMode; 7] {
        [
            ChartMode::Minute5,
            ChartMode::Minute15,
            ChartMode::Minute30,
            ChartMode::Minute60,
            ChartMode::DailyK,
            ChartMode::WeeklyK,
            ChartMode::MonthlyK,
        ]
    }

    /// Returns true when this chart mode uses K-line bars instead of intraday minutes.
    pub fn is_kline(self) -> bool {
        !matches!(self, ChartMode::Intraday)
    }

    /// Returns true when this chart mode is backed by Tencent's minute K-line endpoint.
    pub fn is_minute_kline(self) -> bool {
        matches!(
            self,
            ChartMode::Minute5 | ChartMode::Minute15 | ChartMode::Minute30 | ChartMode::Minute60
        )
    }

    /// Cycles to the next supported chart period.
    pub fn next(self) -> Self {
        match self {
            ChartMode::Intraday => ChartMode::Minute5,
            ChartMode::Minute5 => ChartMode::Minute15,
            ChartMode::Minute15 => ChartMode::Minute30,
            ChartMode::Minute30 => ChartMode::Minute60,
            ChartMode::Minute60 => ChartMode::DailyK,
            ChartMode::DailyK => ChartMode::WeeklyK,
            ChartMode::WeeklyK => ChartMode::MonthlyK,
            ChartMode::MonthlyK => ChartMode::Intraday,
        }
    }

    /// Returns a chart mode parsed from persisted settings.
    pub fn from_config_key(key: &str) -> Self {
        match key {
            "m5" => ChartMode::Minute5,
            "m15" => ChartMode::Minute15,
            "m30" => ChartMode::Minute30,
            "m60" => ChartMode::Minute60,
            "day" => ChartMode::DailyK,
            "week" => ChartMode::WeeklyK,
            "month" => ChartMode::MonthlyK,
            _ => ChartMode::Intraday,
        }
    }

    /// Returns a user-facing short label for the chart mode.
    pub fn label(self) -> &'static str {
        match self {
            ChartMode::Intraday => "分时",
            ChartMode::Minute5 => "5分K",
            ChartMode::Minute15 => "15分K",
            ChartMode::Minute30 => "30分K",
            ChartMode::Minute60 => "60分K",
            ChartMode::DailyK => "日K",
            ChartMode::WeeklyK => "周K",
            ChartMode::MonthlyK => "月K",
        }
    }

    /// Returns the stable cache key for this chart period.
    pub fn cache_key(self) -> &'static str {
        self.config_key()
    }

    /// Returns the stable configuration key for this chart period.
    pub fn config_key(self) -> &'static str {
        match self {
            ChartMode::Intraday => "intraday",
            ChartMode::Minute5 => "m5",
            ChartMode::Minute15 => "m15",
            ChartMode::Minute30 => "m30",
            ChartMode::Minute60 => "m60",
            ChartMode::DailyK => "day",
            ChartMode::WeeklyK => "week",
            ChartMode::MonthlyK => "month",
        }
    }

    /// Returns Tencent K-line period key for provider requests.
    pub fn tencent_period(self) -> Option<&'static str> {
        match self {
            ChartMode::Intraday => None,
            ChartMode::Minute5 => Some("m5"),
            ChartMode::Minute15 => Some("m15"),
            ChartMode::Minute30 => Some("m30"),
            ChartMode::Minute60 => Some("m60"),
            ChartMode::DailyK => Some("day"),
            ChartMode::WeeklyK => Some("week"),
            ChartMode::MonthlyK => Some("month"),
        }
    }
}

impl LayoutMode {
    /// Cycles to the next supported TUI layout.
    pub fn next(self) -> Self {
        match self {
            LayoutMode::Balanced => LayoutMode::Compact,
            LayoutMode::Compact => LayoutMode::LargeChart,
            LayoutMode::LargeChart => LayoutMode::OrderBook,
            LayoutMode::OrderBook => LayoutMode::Balanced,
        }
    }

    /// Returns a layout parsed from persisted settings.
    pub fn from_config_key(key: &str) -> Self {
        match key {
            "compact" => LayoutMode::Compact,
            "large_chart" => LayoutMode::LargeChart,
            "order_book" => LayoutMode::OrderBook,
            _ => LayoutMode::Balanced,
        }
    }

    /// Returns the stable configuration key for this layout.
    pub fn config_key(self) -> &'static str {
        match self {
            LayoutMode::Balanced => "balanced",
            LayoutMode::Compact => "compact",
            LayoutMode::LargeChart => "large_chart",
            LayoutMode::OrderBook => "order_book",
        }
    }

    /// Returns the user-facing layout label.
    pub fn label(self) -> &'static str {
        match self {
            LayoutMode::Balanced => "平衡",
            LayoutMode::Compact => "紧凑",
            LayoutMode::LargeChart => "大图",
            LayoutMode::OrderBook => "盘口",
        }
    }
}

impl Stock {
    /// Returns true when quote metrics should be highlighted as a watchlist anomaly.
    pub fn is_anomaly(&self, _thresholds: HighlightThresholds) -> bool {
        self.pct_change.abs() >= self.attention_pct_threshold()
    }

    /// Returns a compact anomaly tag for the watchlist.
    pub fn anomaly_tag(&self, _thresholds: HighlightThresholds) -> Option<&'static str> {
        let threshold = self.attention_pct_threshold();
        if self.pct_change >= threshold {
            Some("强势")
        } else if self.pct_change <= -threshold {
            Some("弱势")
        } else {
            None
        }
    }

    /// Returns the percent-move threshold that marks this stock as needing attention.
    pub fn attention_pct_threshold(&self) -> f64 {
        if self.is_twenty_percent_limit_stock() {
            12.0
        } else {
            6.0
        }
    }

    /// Returns true for common A-share 20cm boards using code prefix and limit prices.
    fn is_twenty_percent_limit_stock(&self) -> bool {
        if self.code.starts_with("sh688")
            || self.code.starts_with("sh689")
            || self.code.starts_with("sz300")
            || self.code.starts_with("sz301")
        {
            return true;
        }
        if let Some(limit_up) = self.limit_up {
            if self.close > 0.0 && ((limit_up - self.close) / self.close * 100.0) >= 15.0 {
                return true;
            }
        }
        false
    }

    /// Returns the total bid volume across available order-book levels.
    pub fn total_bid_volume(&self) -> i64 {
        self.bid_volumes
            .iter()
            .copied()
            .filter(|vol| *vol > 0)
            .sum()
    }

    /// Returns the total ask volume across available order-book levels.
    pub fn total_ask_volume(&self) -> i64 {
        self.ask_volumes
            .iter()
            .copied()
            .filter(|vol| *vol > 0)
            .sum()
    }

    /// Returns the best bid-ask spread when both sides have valid prices.
    pub fn bid_ask_spread(&self) -> Option<f64> {
        let bid = self.bid_prices.iter().copied().find(|price| *price > 0.0)?;
        let ask = self.ask_prices.iter().copied().find(|price| *price > 0.0)?;
        Some((ask - bid).max(0.0))
    }

    /// Returns order-book imbalance as (bid - ask) / (bid + ask).
    pub fn order_book_imbalance(&self) -> Option<f64> {
        let bid = self.total_bid_volume() as f64;
        let ask = self.total_ask_volume() as f64;
        let total = bid + ask;
        (total > 0.0).then_some((bid - ask) / total)
    }
}

impl SortMode {
    /// Returns the user-facing sort label.
    pub fn label(self) -> &'static str {
        match self {
            SortMode::ConfigOrder => "自定义",
            SortMode::Code => "代码",
            SortMode::Change => "涨跌额",
            SortMode::PctChange => "涨跌幅",
            SortMode::Volume => "成交量",
            SortMode::Amount => "成交额",
        }
    }
}

impl FilterMode {
    /// Returns the user-facing filter label.
    pub fn label(&self) -> String {
        match self {
            FilterMode::All => "全部".to_string(),
            FilterMode::Rising => "上涨".to_string(),
            FilterMode::Falling => "下跌".to_string(),
            FilterMode::Unchanged => "平盘".to_string(),
            FilterMode::Missing => "缺数据".to_string(),
            FilterMode::Text(query) => format!("文本({})", query),
        }
    }
}

impl QuoteSource {
    /// Returns the user-facing quote source label.
    pub fn label(self) -> &'static str {
        match self {
            QuoteSource::Tencent => "腾讯",
            QuoteSource::Sina => "新浪",
        }
    }
}

/// Sorts watchlist indices by one loaded stock metric, missing data last.
fn sort_by_metric(
    indices: &mut [usize],
    group: &StockGroup,
    stocks: &HashMap<String, Stock>,
    sort_mode: SortMode,
) {
    indices.sort_by(|a, b| {
        stock_metric(stocks.get(&group.stocks[*b]), sort_mode)
            .total_cmp(&stock_metric(stocks.get(&group.stocks[*a]), sort_mode))
    });
}

/// Returns a numeric stock metric for sorting, treating missing data as the lowest value.
fn stock_metric(stock: Option<&Stock>, sort_mode: SortMode) -> f64 {
    let Some(stock) = stock else {
        return f64::NEG_INFINITY;
    };
    match sort_mode {
        SortMode::ConfigOrder | SortMode::Code => 0.0,
        SortMode::Change => stock.change,
        SortMode::PctChange => stock.pct_change,
        SortMode::Volume => stock.volume,
        SortMode::Amount => stock.amount,
    }
}

/// Computes loaded/missing breadth, leaders, laggards, and total amount for a group.
fn group_overview(group: &StockGroup, stocks: &HashMap<String, Stock>) -> GroupOverview {
    let mut overview = GroupOverview {
        missing: group.stocks.len(),
        ..GroupOverview::default()
    };
    let mut pct_sum = 0.0;
    let mut leader: Option<&Stock> = None;
    let mut laggard: Option<&Stock> = None;

    for code in &group.stocks {
        let Some(stock) = stocks.get(code) else {
            continue;
        };
        overview.loaded += 1;
        overview.missing = overview.missing.saturating_sub(1);
        overview.total_amount += stock.amount;
        pct_sum += stock.pct_change;
        if stock.pct_change > 0.0 {
            overview.rising += 1;
        } else if stock.pct_change < 0.0 {
            overview.falling += 1;
        } else {
            overview.unchanged += 1;
        }
        if leader.is_none_or(|leader| stock.pct_change > leader.pct_change) {
            leader = Some(stock);
        }
        if laggard.is_none_or(|laggard| stock.pct_change < laggard.pct_change) {
            laggard = Some(stock);
        }
    }

    if overview.loaded > 0 {
        overview.avg_pct_change = pct_sum / overview.loaded as f64;
    }
    overview.leader = leader.map(|stock| stock.name.clone());
    overview.laggard = laggard.map(|stock| stock.name.clone());
    overview
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Builds a minimal app with deterministic watchlist groups for state tests.
    fn test_app() -> App {
        App::from_config(
            Config {
                groups: vec![
                    StockGroup {
                        name: "A".to_string(),
                        stocks: vec![
                            "sh600001".to_string(),
                            "sz000001".to_string(),
                            "bj430001".to_string(),
                        ],
                    },
                    StockGroup {
                        name: "B".to_string(),
                        stocks: vec!["sh600002".to_string()],
                    },
                ],
                settings: Default::default(),
            },
            HashMap::new(),
            false,
        )
    }

    /// Builds a stock fixture with the supplied market metrics.
    fn stock(code: &str, pct_change: f64, amount: f64) -> Stock {
        Stock {
            code: code.to_string(),
            name: code.to_string(),
            price: 10.0,
            change: pct_change,
            pct_change,
            open: 10.0,
            high: 10.0,
            low: 10.0,
            close: 10.0,
            volume: amount / 10.0,
            amount,
            bid_prices: vec![],
            bid_volumes: vec![],
            ask_prices: vec![],
            ask_volumes: vec![],
            quote_source: QuoteSource::Tencent,
            turnover_rate: Some(1.2),
            volume_ratio: Some(1.5),
            amplitude: Some(3.0),
            market_cap: Some(100.0),
            limit_up: Some(11.0),
            limit_down: Some(9.0),
        }
    }

    #[test]
    fn group_management_create_rename_reorder_and_transfer() {
        let mut app = test_app();

        app.create_group("A".to_string());
        assert_eq!(app.config.groups[2].name, "A2");

        app.rename_current_group("Renamed".to_string());
        assert_eq!(app.config.groups[2].name, "Renamed");

        app.move_current_group_left();
        assert_eq!(app.active_group_idx, 1);
        assert_eq!(app.config.groups[1].name, "Renamed");

        app.active_group_idx = 0;
        app.selected_stock_idx = 0;
        let moved = app.move_selected_stock_to_next_group();
        assert_eq!(moved.as_deref(), Some("sh600001"));
        assert!(
            !app.config.groups[0]
                .stocks
                .contains(&"sh600001".to_string())
        );
        assert!(
            app.config.groups[1]
                .stocks
                .contains(&"sh600001".to_string())
        );

        let copied = app.copy_selected_stock_to_next_group();
        assert!(copied.is_some());
    }

    #[test]
    fn sort_and_filter_keep_selection_on_same_stock_when_visible() {
        let mut app = test_app();
        app.stock_data.insert(
            "sh600001".to_string(),
            stock("sh600001", -1.0, 10_000_000.0),
        );
        app.stock_data
            .insert("sz000001".to_string(), stock("sz000001", 2.0, 1_000.0));
        app.selected_stock_idx = 1;

        app.sort_mode = SortMode::Amount;
        app.restore_selection_or_first_visible(Some("sz000001".to_string()));
        assert_eq!(app.selected_stock_code().as_deref(), Some("sz000001"));
        assert_eq!(app.visible_stock_indices(), vec![0, 1, 2]);

        app.filter_mode = FilterMode::Rising;
        app.restore_selection_or_first_visible(Some("sz000001".to_string()));
        assert_eq!(app.visible_stock_indices(), vec![1]);
        assert_eq!(app.selected_stock_code().as_deref(), Some("sz000001"));

        app.filter_mode = FilterMode::Missing;
        app.restore_selection_or_first_visible(Some("sz000001".to_string()));
        assert_eq!(app.visible_stock_indices(), vec![2]);
        assert_eq!(app.selected_stock_code().as_deref(), Some("bj430001"));
    }

    #[test]
    fn quote_metadata_tracks_success_and_failure_state() {
        let mut app = test_app();
        app.record_stock_update(
            "sh600001".to_string(),
            stock("sh600001", 1.0, 1.0),
            QuoteSessionState::Live,
        );
        let meta = app.quote_meta.get("sh600001").expect("quote meta");
        assert_eq!(meta.source, QuoteSource::Tencent);
        assert_eq!(meta.session_state, QuoteSessionState::Live);
        assert!(meta.last_error.is_none());

        app.record_stock_error("sh600001".to_string(), "刷新失败".to_string());
        assert_eq!(
            app.quote_meta
                .get("sh600001")
                .and_then(|meta| meta.last_error.as_deref()),
            Some("刷新失败")
        );
    }

    #[test]
    fn chart_modes_cycle_through_supported_periods() {
        let mut mode = ChartMode::Intraday;
        let mut labels = Vec::new();
        for _ in 0..8 {
            labels.push(mode.label());
            mode = mode.next();
        }

        assert_eq!(
            labels,
            vec![
                "分时", "5分K", "15分K", "30分K", "60分K", "日K", "周K", "月K"
            ]
        );
        assert_eq!(mode, ChartMode::Intraday);
        assert_eq!(ChartMode::Minute5.tencent_period(), Some("m5"));
        assert_eq!(ChartMode::WeeklyK.cache_key(), "week");
    }

    #[test]
    fn kline_updates_are_keyed_by_code_and_period() {
        let mut app = test_app();
        let day_bar = KLine {
            date: "2026-07-01".to_string(),
            open: 10.0,
            close: 11.0,
            high: 12.0,
            low: 9.0,
            volume: 100.0,
        };
        let week_bar = KLine {
            date: "2026-W27".to_string(),
            open: 20.0,
            close: 21.0,
            high: 22.0,
            low: 19.0,
            volume: 200.0,
        };

        app.update_kline_data("sh600001".to_string(), ChartMode::DailyK, vec![day_bar]);
        app.update_kline_data("sh600001".to_string(), ChartMode::WeeklyK, vec![week_bar]);

        assert_eq!(
            app.kline_data
                .get(&("sh600001".to_string(), ChartMode::DailyK))
                .and_then(|bars| bars.first())
                .map(|bar| bar.close),
            Some(11.0)
        );
        assert_eq!(
            app.kline_data
                .get(&("sh600001".to_string(), ChartMode::WeeklyK))
                .and_then(|bars| bars.first())
                .map(|bar| bar.close),
            Some(21.0)
        );
    }

    #[test]
    fn anomaly_highlighting_uses_board_aware_percent_thresholds() {
        let thresholds = HighlightThresholds {
            pct_change: 5.0,
            amount: 1_000_000_000.0,
        };
        let strong = stock("sh600001", 6.0, 10_000.0);
        let weak = stock("sh600002", -6.0, 10_000.0);
        let normal = stock("bj430001", 5.99, 10_000.0);
        let mut twenty_normal = stock("sz300001", 11.99, 10_000.0);
        let twenty_strong = stock("sh688001", 12.0, 10_000.0);
        twenty_normal.limit_up = Some(12.0);

        assert!(strong.is_anomaly(thresholds));
        assert_eq!(strong.anomaly_tag(thresholds), Some("强势"));
        assert!(weak.is_anomaly(thresholds));
        assert_eq!(weak.anomaly_tag(thresholds), Some("弱势"));
        assert!(!normal.is_anomaly(thresholds));
        assert_eq!(normal.anomaly_tag(thresholds), None);
        assert!(!twenty_normal.is_anomaly(thresholds));
        assert!(twenty_strong.is_anomaly(thresholds));
        assert_eq!(twenty_strong.anomaly_tag(thresholds), Some("强势"));
    }

    #[test]
    fn group_overview_handles_mixed_loaded_and_missing_rows() {
        let mut app = test_app();
        app.stock_data
            .insert("sh600001".to_string(), stock("leader", 3.0, 100.0));
        app.stock_data
            .insert("sz000001".to_string(), stock("laggard", -1.0, 200.0));

        let overview = app.current_group_overview();
        assert_eq!(overview.loaded, 2);
        assert_eq!(overview.missing, 1);
        assert_eq!(overview.rising, 1);
        assert_eq!(overview.falling, 1);
        assert_eq!(overview.avg_pct_change, 1.0);
        assert_eq!(overview.total_amount, 300.0);
        assert_eq!(overview.leader.as_deref(), Some("leader"));
        assert_eq!(overview.laggard.as_deref(), Some("laggard"));
    }

    #[test]
    fn import_preview_counts_additions_duplicates_and_invalid_rows() {
        let mut app = test_app();
        let preview = app.apply_watchlist_import("A,600001\nA,600003\nNewGroup,000001\nbadcode\n");

        assert_eq!(preview.additions, 2);
        assert_eq!(preview.duplicates, 1);
        assert_eq!(preview.invalid, 1);
        assert!(
            app.config.groups[0]
                .stocks
                .contains(&"sh600003".to_string())
        );
        assert!(app.config.groups.iter().any(
            |group| group.name == "NewGroup" && group.stocks.contains(&"sz000001".to_string())
        ));
    }

    #[test]
    fn layout_and_chart_modes_round_trip_config_keys() {
        assert_eq!(LayoutMode::from_config_key("compact"), LayoutMode::Compact);
        assert_eq!(LayoutMode::LargeChart.next(), LayoutMode::OrderBook);
        assert_eq!(LayoutMode::OrderBook.next(), LayoutMode::Balanced);
        assert_eq!(LayoutMode::OrderBook.config_key(), "order_book");
        assert_eq!(ChartMode::from_config_key("m60"), ChartMode::Minute60);
        assert_eq!(ChartMode::from_config_key("unknown"), ChartMode::Intraday);
    }
}
