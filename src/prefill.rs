use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::sync::Mutex;
use std::sync::Arc;
use once_cell::sync::Lazy;

pub static PREFILL_RUNNING_PLATFORMS: Lazy<Mutex<HashSet<String>>> = Lazy::new(|| {
    Mutex::new(HashSet::new())
});

/// Manages LanCache prefill operations (SteamPrefill, BattleNetPrefill, EpicPrefill).
/// Wraps the CLI tools from the ich777/lancache-prefill container.
pub struct PrefillManager {
    pub data_dir: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrefillStatus {
    pub platform: String,
    pub running: bool,
    pub last_run: Option<String>,
    pub selected_apps: Vec<String>,
    pub cron_schedule: Option<String>,
    pub last_log_summary: Option<String>,
    pub completed_apps: Vec<String>,
    pub active_app: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrefillConfig {
    pub steam_enabled: bool,
    pub battlenet_enabled: bool,
    pub epic_enabled: bool,
    pub cron_schedule: String, // format "HH:MM" local time
    pub run_on_startup: bool,
}

impl Default for PrefillConfig {
    fn default() -> Self {
        Self {
            steam_enabled: false,
            battlenet_enabled: false,
            epic_enabled: false,
            cron_schedule: "02:00".to_string(), // Default to 2:00 AM
            run_on_startup: false,
        }
    }
}

impl PrefillManager {
    pub fn new(data_dir: &str) -> Self {
        Self {
            data_dir: data_dir.to_string(),
        }
    }

    /// Get the status of all prefill platforms.
    pub async fn get_status(&self, db_parent: &str) -> Vec<PrefillStatus> {
        vec![
            self.platform_status("steam", "SteamPrefill", db_parent).await,
            self.platform_status("battlenet", "BattleNetPrefill", db_parent).await,
            self.platform_status("epic", "EpicPrefill", db_parent).await,
        ]
    }

    async fn platform_status(&self, platform: &str, binary_name: &str, db_parent: &str) -> PrefillStatus {
        let config_path = format!("{}/{}/selectedAppsToPrefill.json", self.data_dir, binary_name);
        
        let selected_apps = if std::path::Path::new(&config_path).exists() {
            if let Ok(data) = std::fs::read_to_string(&config_path) {
                serde_json::from_str::<Vec<serde_json::Value>>(&data)
                    .ok()
                    .map(|v| v.into_iter().map(|val| {
                        if let Some(s) = val.as_str() {
                            s.to_string()
                        } else if let Some(n) = val.as_f64() {
                            n.to_string()
                        } else if let Some(i) = val.as_i64() {
                            i.to_string()
                        } else {
                            val.to_string()
                        }
                    }).collect())
                    .unwrap_or_default()
            } else {
                Vec::new()
            }
        } else {
            Vec::new()
        };

        let running = if let Ok(running_set) = PREFILL_RUNNING_PLATFORMS.lock() {
            running_set.contains(platform)
        } else {
            false
        };

        let log_file_path = std::path::Path::new(db_parent)
            .join(format!("prefill_{}_last.log", platform));
        
        let mut last_run = None;
        let mut last_log_summary = None;
        let mut completed_apps = Vec::new();
        let mut active_app = None;

        if log_file_path.exists() {
            if let Ok(metadata) = std::fs::metadata(&log_file_path) {
                if let Ok(modified) = metadata.modified() {
                    let datetime: chrono::DateTime<chrono::Local> = modified.into();
                    last_run = Some(datetime.format("%Y-%m-%d %H:%M:%S").to_string());
                }
            }

            if let Ok(content) = std::fs::read_to_string(&log_file_path) {
                let mut current_prefilling = None;
                
                for line in content.lines() {
                    let line_trimmed = line.trim();
                    if line_trimmed.is_empty() {
                        continue;
                    }
                    
                    // Look for apps currently prefilling
                    if line_trimmed.contains("Prefilling App") || line_trimmed.contains("Prefilling app:") || line_trimmed.contains("Prefilling ") {
                        if let Some(pos) = line_trimmed.find("Prefilling") {
                            let part = &line_trimmed[pos + "Prefilling".len()..];
                            let app_name = part.trim_start_matches(" App")
                                .trim_start_matches(" app:")
                                .trim_start_matches(" ID")
                                .trim_start_matches(" id:")
                                .trim_start_matches(":")
                                .trim()
                                .trim_end_matches("...")
                                .to_string();
                            current_prefilling = Some(app_name.clone());
                            active_app = Some(app_name);
                        }
                    }
                    
                    // Look for completed apps
                    if line_trimmed.contains("Finished") || line_trimmed.contains("Up to date") {
                        if let Some(ref app) = current_prefilling {
                            if !completed_apps.contains(app) {
                                completed_apps.push(app.clone());
                            }
                            active_app = None;
                        }
                    }
                    
                    // Look for summary
                    if line_trimmed.contains("Prefilled") && (line_trimmed.contains("apps") || line_trimmed.contains("in")) {
                        last_log_summary = Some(line_trimmed.to_string());
                    }
                    
                    // Fallback to last line for status summary if no specific summary found
                    let clean_line = if line_trimmed.starts_with('[') {
                        if let Some(close_bracket) = line_trimmed.find(']') {
                            line_trimmed[close_bracket + 1..].trim().to_string()
                        } else {
                            line_trimmed.to_string()
                        }
                    } else {
                        line_trimmed.to_string()
                    };
                    if !clean_line.is_empty() && !clean_line.contains("[====") {
                        last_log_summary = Some(clean_line);
                    }
                }
                
                // If the run completed, active_app should be cleared
                if let Some(ref summary) = last_log_summary {
                    if summary.contains("Prefilled") || summary.contains("Finished prefilling") || summary.contains("complete") {
                        active_app = None;
                    }
                }
            }
        }

        PrefillStatus {
            platform: platform.to_string(),
            running,
            last_run,
            selected_apps,
            cron_schedule: None,
            last_log_summary,
            completed_apps,
            active_app,
        }
    }

    /// Read the raw selected app IDs from the selectedAppsToPrefill.json file.
    pub fn get_selected_apps_raw(&self, platform: &str) -> Result<Vec<serde_json::Value>, String> {
        let dir_name = Self::platform_dir(platform).ok_or_else(|| format!("Unknown platform: {}", platform))?;
        let path = format!("{}/{}/selectedAppsToPrefill.json", self.data_dir, dir_name);

        if !std::path::Path::new(&path).exists() {
            return Ok(Vec::new());
        }

        let data = std::fs::read_to_string(&path).map_err(|e| format!("Failed to read file: {}", e))?;
        let ids: Vec<serde_json::Value> = serde_json::from_str(&data).map_err(|e| format!("Invalid JSON: {}", e))?;
        Ok(ids)
    }

    /// Save a list of app IDs to the selectedAppsToPrefill.json file.
    pub fn save_selected_apps(&self, platform: &str, app_ids: &[serde_json::Value]) -> Result<(), String> {
        let dir_name = Self::platform_dir(platform).ok_or_else(|| format!("Unknown platform: {}", platform))?;
        let dir_path = format!("{}/{}", self.data_dir, dir_name);

        std::fs::create_dir_all(&dir_path).map_err(|e| format!("Failed to create directory: {}", e))?;

        let path = format!("{}/selectedAppsToPrefill.json", dir_path);
        let json = serde_json::to_string_pretty(app_ids).map_err(|e| format!("Failed to serialize: {}", e))?;
        std::fs::write(&path, json).map_err(|e| format!("Failed to write file: {}", e))?;
        Ok(())
    }

    /// Map platform key to directory name.
    fn platform_dir(platform: &str) -> Option<&'static str> {
        match platform {
            "steam" => Some("SteamPrefill"),
            "battlenet" => Some("BattleNetPrefill"),
            "epic" => Some("EpicPrefill"),
            _ => None,
        }
    }

    /// Trigger a prefill run for a specific platform. (Used for testing/fallback)
    #[allow(dead_code)]
    pub async fn run_prefill(
        &self,
        platform: &str,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let (dir_name, binary) = match platform {
            "steam" => ("SteamPrefill", "SteamPrefill"),
            "battlenet" => ("BattleNetPrefill", "BattleNetPrefill"),
            "epic" => ("EpicPrefill", "EpicPrefill"),
            _ => return Err(format!("Unknown platform: {}", platform).into()),
        };

        let binary_path = format!("{}/{}/{}", self.data_dir, dir_name, binary);

        if !std::path::Path::new(&binary_path).exists() {
            return Err(format!("Prefill binary not found: {}", binary_path).into());
        }

        let output = tokio::process::Command::new(&binary_path)
            .arg("prefill")
            .current_dir(format!("{}/{}", self.data_dir, dir_name))
            .output()
            .await?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        if output.status.success() {
            tracing::info!("Prefill {} completed successfully", platform);
            Ok(stdout)
        } else {
            tracing::error!("Prefill {} failed: {}", platform, stderr);
            Err(format!("Prefill failed: {}", stderr).into())
        }
    }

    /// Run prefill in background, logging to files
    pub async fn run_prefill_async(
        &self,
        platform: &str,
        log_dir: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let (dir_name, binary) = match platform {
            "steam" => ("SteamPrefill", "SteamPrefill"),
            "battlenet" => ("BattleNetPrefill", "BattleNetPrefill"),
            "epic" => ("EpicPrefill", "EpicPrefill"),
            _ => return Err(format!("Unknown platform: {}", platform).into()),
        };

        let binary_path = format!("{}/{}/{}", self.data_dir, dir_name, binary);
        let working_dir = format!("{}/{}", self.data_dir, dir_name);

        if !std::path::Path::new(&binary_path).exists() {
            return Err(format!("Prefill binary not found: {}", binary_path).into());
        }

        // Check if already running
        {
            let mut running = PREFILL_RUNNING_PLATFORMS.lock().map_err(|e| e.to_string())?;
            if running.contains(platform) {
                return Err("Prefill already running".into());
            }
            running.insert(platform.to_string());
        }

        let platform_string = platform.to_string();
        let log_dir_string = log_dir.to_string();

        tokio::spawn(async move {
            let log_file_path = std::path::PathBuf::from(&log_dir_string)
                .join(format!("prefill_{}_last.log", platform_string));

            if let Some(parent) = log_file_path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }

            let log_file = match std::fs::File::create(&log_file_path) {
                Ok(f) => f,
                Err(e) => {
                    tracing::error!("Failed to create prefill log file: {}", e);
                    if let Ok(mut running) = PREFILL_RUNNING_PLATFORMS.lock() {
                        running.remove(&platform_string);
                    }
                    return;
                }
            };

            tracing::info!("Starting background prefill for {}", platform_string);

            let mut child = match tokio::process::Command::new(&binary_path)
                .arg("prefill")
                .arg("--no-ansi")
                .current_dir(&working_dir)
                .stdout(std::process::Stdio::from(log_file.try_clone().unwrap()))
                .stderr(std::process::Stdio::from(log_file))
                .spawn()
            {
                Ok(c) => c,
                Err(e) => {
                    let _ = std::fs::write(&log_file_path, format!("Failed to spawn process: {}\n", e));
                    tracing::error!("Failed to spawn prefill for {}: {}", platform_string, e);
                    if let Ok(mut running) = PREFILL_RUNNING_PLATFORMS.lock() {
                        running.remove(&platform_string);
                    }
                    return;
                }
            };

            let status = child.wait().await;
            
            if let Ok(mut running) = PREFILL_RUNNING_PLATFORMS.lock() {
                running.remove(&platform_string);
            }

            match status {
                Ok(s) if s.success() => {
                    tracing::info!("Background prefill for {} completed successfully", platform_string);
                }
                Ok(s) => {
                    tracing::error!("Background prefill for {} failed with exit status: {}", platform_string, s);
                }
                Err(e) => {
                    tracing::error!("Background prefill for {} wait failed: {}", platform_string, e);
                }
            }
        });

        Ok(())
    }

    /// Load prefill configuration from disk.
    pub fn load_config(db_parent: &str) -> PrefillConfig {
        let path = std::path::Path::new(db_parent).join("prefill_config.json");
        if let Ok(data) = std::fs::read_to_string(&path) {
            if let Ok(config) = serde_json::from_str::<PrefillConfig>(&data) {
                return config;
            }
        }
        PrefillConfig::default()
    }

    /// Save prefill configuration to disk.
    pub fn save_config(db_parent: &str, config: &PrefillConfig) -> Result<(), std::io::Error> {
        let path = std::path::Path::new(db_parent).join("prefill_config.json");
        let json = serde_json::to_string_pretty(config)?;
        std::fs::write(path, json)
    }
}

pub async fn run_prefill_scheduler(state: Arc<crate::AppState>) {
    tracing::info!("Prefill scheduler task started");
    tokio::time::sleep(std::time::Duration::from_secs(10)).await;

    // Run on startup check
    {
        let db_parent = {
            let config = state.config.read().await;
            std::path::Path::new(&config.db_path)
                .parent()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|| "/data/gravitylancacheui".to_string())
        };
        let prefill_dir = {
            let config = state.config.read().await;
            config.prefill_dir.clone()
        };
        let prefill_config = PrefillManager::load_config(&db_parent);
        
        if prefill_config.run_on_startup {
            tracing::info!("Prefill run on startup is enabled. Triggering prefills...");
            let manager = PrefillManager::new(&prefill_dir);
            if prefill_config.steam_enabled {
                let _ = manager.run_prefill_async("steam", &db_parent).await;
            }
            if prefill_config.battlenet_enabled {
                let _ = manager.run_prefill_async("battlenet", &db_parent).await;
            }
            if prefill_config.epic_enabled {
                let _ = manager.run_prefill_async("epic", &db_parent).await;
            }
        }
    }

    loop {
        tokio::time::sleep(std::time::Duration::from_secs(60)).await;

        let db_parent = {
            let config = state.config.read().await;
            std::path::Path::new(&config.db_path)
                .parent()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|| "/data/gravitylancacheui".to_string())
        };
        let prefill_dir = {
            let config = state.config.read().await;
            config.prefill_dir.clone()
        };

        let prefill_config = PrefillManager::load_config(&db_parent);
        
        let now = chrono::Local::now();
        let current_time = now.format("%H:%M").to_string();

        if prefill_config.cron_schedule == current_time {
            tracing::info!("Scheduled prefill trigger time matched: {}", current_time);
            let manager = PrefillManager::new(&prefill_dir);
            
            if prefill_config.steam_enabled {
                let _ = manager.run_prefill_async("steam", &db_parent).await;
            }
            if prefill_config.battlenet_enabled {
                let _ = manager.run_prefill_async("battlenet", &db_parent).await;
            }
            if prefill_config.epic_enabled {
                let _ = manager.run_prefill_async("epic", &db_parent).await;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[tokio::test]
    async fn test_platform_status_parsing() {
        let rand_id = uuid::Uuid::new_v4().to_string();
        let test_dir = std::path::PathBuf::from("target").join(format!("test_prefill_{}", rand_id));
        
        let cache_dir = test_dir.join("cache");
        let db_dir = test_dir.join("db");
        
        fs::create_dir_all(&cache_dir).unwrap();
        fs::create_dir_all(&db_dir).unwrap();
        
        let cache_path = cache_dir.to_str().unwrap();
        let db_path = db_dir.to_str().unwrap();
        
        // Write a mockup prefill config for steam
        let steam_dir = cache_dir.join("SteamPrefill");
        fs::create_dir_all(&steam_dir).unwrap();
        fs::write(
            steam_dir.join("selectedAppsToPrefill.json"),
            r#"["400", "570"]"#
        ).unwrap();

        // Write mockup logs
        let log_file = db_dir.join("prefill_steam_last.log");
        let log_content = "\
[12:00:00] Starting login!
[12:00:05] Prefilling App Portal...
[12:00:10] App Portal - Finished (1.2 GB downloaded)
[12:00:11] Prefilling app: Dota 2...
[12:00:15] Downloading: 50% - 100 MB/s
";
        fs::write(&log_file, log_content).unwrap();

        let manager = PrefillManager::new(cache_path);
        let status = manager.platform_status("steam", "SteamPrefill", db_path).await;

        assert_eq!(status.platform, "steam");
        assert_eq!(status.selected_apps, vec!["400".to_string(), "570".to_string()]);
        assert_eq!(status.completed_apps, vec!["Portal".to_string()]);
        assert_eq!(status.active_app, Some("Dota 2".to_string()));
        assert!(status.last_run.is_some());
        assert_eq!(status.last_log_summary, Some("Downloading: 50% - 100 MB/s".to_string()));

        // Clean up
        let _ = fs::remove_dir_all(&test_dir);
    }
}
