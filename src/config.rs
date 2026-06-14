use serde::{Deserialize, Serialize};

/// Application configuration loaded from environment variables with sensible defaults.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub lancache_logs_dir: String,
    pub lancache_cache_dir: String,
    pub db_path: String,
    pub steam_api_key: Option<String>,
    pub cache_scan_interval_secs: u64,
    pub log_retention_days: u32,
    pub listen_port: u16,
    pub excluded_ips: Vec<String>,
    pub config_file_path: String,
}

impl AppConfig {
    /// Load configuration from environment variables, falling back to defaults.
    pub fn load() -> Self {
        dotenvy::dotenv().ok();

        let config_file_path = std::env::var("CONFIG_FILE")
            .unwrap_or_else(|_| "/data/gravitylancacheui/config.json".into());

        let mut config = Self {
            lancache_logs_dir: std::env::var("LANCACHE_LOGS_DIR")
                .unwrap_or_else(|_| "/data/logs".into()),
            lancache_cache_dir: std::env::var("LANCACHE_CACHE_DIR")
                .unwrap_or_else(|_| "/data/cache".into()),
            db_path: std::env::var("DB_PATH")
                .unwrap_or_else(|_| "/data/gravitylancacheui/db.sqlite".into()),
            steam_api_key: std::env::var("STEAM_API_KEY").ok(),
            cache_scan_interval_secs: std::env::var("CACHE_SCAN_INTERVAL_SECS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(300),
            log_retention_days: std::env::var("LOG_RETENTION_DAYS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(90),
            listen_port: std::env::var("LISTEN_PORT")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(8080),
            excluded_ips: std::env::var("EXCLUDED_IPS")
                .ok()
                .map(|v| v.split(',').map(|s| s.trim().to_string()).collect())
                .unwrap_or_default(),
            config_file_path,
        };

        // Overlay persisted config.json settings (user-editable via Settings UI)
        if let Ok(data) = std::fs::read_to_string(&config.config_file_path) {
            if let Ok(persisted) = serde_json::from_str::<PersistedConfig>(&data) {
                config.apply_persisted(persisted);
            }
        }

        config
    }

    /// Merge user-persisted settings into the running config.
    fn apply_persisted(&mut self, p: PersistedConfig) {
        if let Some(key) = p.steam_api_key {
            self.steam_api_key = Some(key);
        }
        if let Some(interval) = p.cache_scan_interval_secs {
            self.cache_scan_interval_secs = interval;
        }
        if let Some(days) = p.log_retention_days {
            self.log_retention_days = days;
        }
        if let Some(ips) = p.excluded_ips {
            self.excluded_ips = ips;
        }
        if let Some(db) = p.db_path {
            self.db_path = db;
        }
    }

    /// Save current user-editable settings to config.json.
    pub fn save_persisted(&self) -> Result<(), std::io::Error> {
        let persisted = PersistedConfig {
            steam_api_key: self.steam_api_key.clone(),
            cache_scan_interval_secs: Some(self.cache_scan_interval_secs),
            log_retention_days: Some(self.log_retention_days),
            excluded_ips: Some(self.excluded_ips.clone()),
            db_path: Some(self.db_path.clone()),
        };

        if let Some(parent) = std::path::Path::new(&self.config_file_path).parent() {
            std::fs::create_dir_all(parent)?;
        }

        let json = serde_json::to_string_pretty(&persisted)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        std::fs::write(&self.config_file_path, json)
    }
}

/// Subset of config that users can edit via the Settings UI and persist to disk.
#[derive(Debug, Serialize, Deserialize, Default)]
struct PersistedConfig {
    steam_api_key: Option<String>,
    cache_scan_interval_secs: Option<u64>,
    log_retention_days: Option<u32>,
    excluded_ips: Option<Vec<String>>,
    db_path: Option<String>,
}
