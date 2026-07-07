use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct StockGroup {
    pub name: String,
    pub stocks: Vec<String>, // e.g. ["sh600519", "sz000001"]
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Config {
    pub groups: Vec<StockGroup>,
    #[serde(default)]
    pub settings: AppSettings,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AppSettings {
    pub layout_mode: String,
    pub chart_mode: String,
    pub highlight_pct_threshold: f64,
    pub highlight_amount_threshold: f64,
    pub holiday_overrides: Vec<String>,
    pub workday_overrides: Vec<String>,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            layout_mode: "balanced".to_string(),
            chart_mode: "intraday".to_string(),
            highlight_pct_threshold: 5.0,
            highlight_amount_threshold: 1_000_000_000.0,
            holiday_overrides: Vec::new(),
            workday_overrides: Vec::new(),
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            groups: vec![
                StockGroup {
                    name: "默认自选".to_string(),
                    stocks: vec![
                        "sh600519".to_string(), // 贵州茅台
                        "sz000001".to_string(), // 平安银行
                        "sh601318".to_string(), // 中国平安
                    ],
                },
                StockGroup {
                    name: "科技半导体".to_string(),
                    stocks: vec![
                        "sh688981".to_string(), // 中芯国际
                        "sz300750".to_string(), // 宁德时代
                        "sz002415".to_string(), // 海康威视
                    ],
                },
                StockGroup {
                    name: "消费白酒".to_string(),
                    stocks: vec![
                        "sh600519".to_string(), // 贵州茅台
                        "sz000858".to_string(), // 五粮液
                        "sh600887".to_string(), // 伊利股份
                    ],
                },
            ],
            settings: AppSettings::default(),
        }
    }
}

impl Config {
    /// Returns the application support directory used for config and runtime caches.
    pub fn app_dir() -> Option<PathBuf> {
        dirs::config_dir().map(|mut p| {
            p.push("lazytrade");
            p
        })
    }

    /// Returns the TOML config file path in the platform config directory.
    pub fn config_path() -> Option<PathBuf> {
        Self::app_dir().map(|mut p| {
            p.push("config.toml");
            p
        })
    }

    /// Returns the directory used to persist per-stock daily K-line cache files.
    pub fn kline_cache_dir() -> Option<PathBuf> {
        Self::app_dir().map(|mut p| {
            p.push("kline-cache");
            p
        })
    }

    /// Returns the default CSV export path for watchlist groups.
    pub fn watchlist_export_path() -> Option<PathBuf> {
        Self::app_dir().map(|mut p| {
            p.push("watchlist-export.csv");
            p
        })
    }

    /// Returns the default text/CSV import path for watchlist groups.
    pub fn watchlist_import_path() -> Option<PathBuf> {
        Self::app_dir().map(|mut p| {
            p.push("watchlist-import.csv");
            p
        })
    }

    /// Loads user configuration, creating a default config when none exists.
    pub fn load() -> Self {
        if let Some(path) = Self::config_path() {
            if path.exists() {
                if let Ok(content) = fs::read_to_string(&path) {
                    if let Ok(config) = toml::from_str::<Config>(&content) {
                        return config;
                    }
                }
            }
        }
        let default_config = Self::default();
        let _ = default_config.save();
        default_config
    }

    /// Saves user configuration to the platform config directory.
    pub fn save(&self) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(path) = Self::config_path() {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }
            let content = toml::to_string_pretty(self)?;
            fs::write(path, content)?;
        }
        Ok(())
    }
}
