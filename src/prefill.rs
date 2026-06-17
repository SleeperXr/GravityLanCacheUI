use serde::{Deserialize, Serialize};

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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct PrefillConfig {
    pub steam_enabled: bool,
    pub battlenet_enabled: bool,
    pub epic_enabled: bool,
    pub cron_schedule: String,
    pub run_on_startup: bool,
}

impl Default for PrefillConfig {
    fn default() -> Self {
        Self {
            steam_enabled: false,
            battlenet_enabled: false,
            epic_enabled: false,
            cron_schedule: "0 2 * * *".to_string(),
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
    pub async fn get_status(&self) -> Vec<PrefillStatus> {
        vec![
            self.platform_status("steam", "SteamPrefill").await,
            self.platform_status("battlenet", "BattleNetPrefill").await,
            self.platform_status("epic", "EpicPrefill").await,
        ]
    }

    async fn platform_status(&self, platform: &str, _binary_name: &str) -> PrefillStatus {
        PrefillStatus {
            platform: platform.to_string(),
            running: false,
            last_run: None,
            selected_apps: Vec::new(),
            cron_schedule: None,
        }
    }

    /// Trigger a prefill run for a specific platform.
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
}
