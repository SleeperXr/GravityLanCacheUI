use serde::Serialize;

/// Setup wizard: validates that all required paths and services are accessible.
#[derive(Debug, Clone, Serialize)]
pub struct SetupCheck {
    pub name: String,
    pub status: CheckStatus,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum CheckStatus {
    Ok,
    Warning,
    Error,
}

/// Run all setup validation checks.
pub async fn run_checks(config: &crate::config::AppConfig) -> Vec<SetupCheck> {
    let mut checks = Vec::new();

    // 1. Logs directory
    checks.push(check_directory(
        "Lancache Logs Directory",
        &config.lancache_logs_dir,
    ));

    // 2. access.log file
    let access_log = std::path::Path::new(&config.lancache_logs_dir).join("access.log");
    if access_log.exists() {
        checks.push(SetupCheck {
            name: "access.log File".to_string(),
            status: CheckStatus::Ok,
            message: format!("Found at {}", access_log.display()),
        });
    } else {
        checks.push(SetupCheck {
            name: "access.log File".to_string(),
            status: CheckStatus::Warning,
            message: format!("Not found at {}. Will wait for it.", access_log.display()),
        });
    }

    // 3. Cache directory
    checks.push(check_directory(
        "Lancache Cache Directory",
        &config.lancache_cache_dir,
    ));

    // 4. Database path (parent writable)
    let db_parent = std::path::Path::new(&config.db_path)
        .parent()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();
    checks.push(check_directory_writable("Database Directory", &db_parent));

    // 5. Steam API key
    if let Some(ref key) = config.steam_api_key {
        if key.len() > 10 {
            checks.push(SetupCheck {
                name: "Steam API Key".to_string(),
                status: CheckStatus::Ok,
                message: "Configured".to_string(),
            });
        } else {
            checks.push(SetupCheck {
                name: "Steam API Key".to_string(),
                status: CheckStatus::Warning,
                message: "Key looks too short".to_string(),
            });
        }
    } else {
        checks.push(SetupCheck {
            name: "Steam API Key".to_string(),
            status: CheckStatus::Warning,
            message: "Not configured. Game name resolution will use local mapping only.".to_string(),
        });
    }

    checks
}

fn check_directory(name: &str, path: &str) -> SetupCheck {
    if std::path::Path::new(path).is_dir() {
        SetupCheck {
            name: name.to_string(),
            status: CheckStatus::Ok,
            message: format!("Found: {}", path),
        }
    } else {
        SetupCheck {
            name: name.to_string(),
            status: CheckStatus::Error,
            message: format!("Not found: {}", path),
        }
    }
}

fn check_directory_writable(name: &str, path: &str) -> SetupCheck {
    let p = std::path::Path::new(path);
    if !p.exists() {
        match std::fs::create_dir_all(p) {
            Ok(()) => SetupCheck {
                name: name.to_string(),
                status: CheckStatus::Ok,
                message: format!("Created: {}", path),
            },
            Err(e) => SetupCheck {
                name: name.to_string(),
                status: CheckStatus::Error,
                message: format!("Cannot create {}: {}", path, e),
            },
        }
    } else if p.is_dir() {
        SetupCheck {
            name: name.to_string(),
            status: CheckStatus::Ok,
            message: format!("Exists: {}", path),
        }
    } else {
        SetupCheck {
            name: name.to_string(),
            status: CheckStatus::Error,
            message: format!("Path is not a directory: {}", path),
        }
    }
}
