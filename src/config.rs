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
