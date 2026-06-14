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

    // 2. access.log file and other logs in directory
    let logs_dir = std::path::Path::new(&config.lancache_logs_dir);
    let mut log_files = Vec::new();
    let mut has_access_log = false;
    
    if logs_dir.is_dir() {
        if let Ok(entries) = std::fs::read_dir(logs_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() {
                    let filename = path.file_name().unwrap_or_default().to_string_lossy().to_string();
                    if filename.ends_with(".log") {
                        let size = path.metadata().map(|m| m.len()).unwrap_or(0);
                        log_files.push(format!("{} ({})", filename, format_bytes(size)));
                        if filename == "access.log" {
                            has_access_log = true;
                        }
                    }
                }
            }
        }
    }

    if has_access_log {
        checks.push(SetupCheck {
            name: "access.log File".to_string(),
            status: CheckStatus::Ok,
            message: format!("Gefunden! Vorhandene Log-Dateien: {}", log_files.join(", ")),
        });
    } else if !log_files.is_empty() {
        checks.push(SetupCheck {
            name: "access.log File".to_string(),
            status: CheckStatus::Warning,
            message: format!("access.log fehlt! Andere Dateien gefunden: {}. LanCache schreibt standardmäßig in access.log.", log_files.join(", ")),
        });
    } else {
        checks.push(SetupCheck {
            name: "access.log File".to_string(),
            status: CheckStatus::Warning,
            message: format!("Keine Log-Dateien im Verzeichnis {} gefunden. Backend wartet auf Log-Generierung.", config.lancache_logs_dir),
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

fn format_bytes(bytes: u64) -> String {
    let k = 1024.0;
    const SIZES: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    if bytes == 0 {
        return "0 B".to_string();
    }
    let i = (bytes as f64).log(k).floor() as usize;
    if i >= SIZES.len() {
        return format!("{} B", bytes);
    }
    let val = bytes as f64 / k.powi(i as i32);
    format!("{:.2} {}", val, SIZES[i])
}
