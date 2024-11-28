use serde::Deserialize;
use std::path::PathBuf;

#[derive(Debug, Deserialize)]
pub struct Config {
    pub network: NetworkConfig,
    pub storage: StorageConfig,
    pub mempool: MempoolConfig,
}

#[derive(Debug, Deserialize)]
pub struct NetworkConfig {
    pub port: u16,
    pub max_peers: usize,
    pub bootstrap_nodes: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct StorageConfig {
    pub path: PathBuf,
    pub cache_size: usize,
}

#[derive(Debug, Deserialize)]
pub struct MempoolConfig {
    pub max_size: usize,
    pub ttl_seconds: u64,
}

impl Config {
    pub fn load() -> Result<Self> {
        let config_path = std::env::var("RUSTBTC_CONFIG")
            .unwrap_or_else(|_| "config.toml".to_string());
            
        let config_str = std::fs::read_to_string(&config_path)
            .map_err(|e| RustBtcError::ConfigError(format!("无法读取配置文件: {}", e)))?;
            
        toml::from_str(&config_str)
            .map_err(|e| RustBtcError::ConfigError(format!("配置文件格式错误: {}", e)))
    }
} 