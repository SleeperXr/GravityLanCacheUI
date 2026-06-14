use std::collections::HashMap;
use std::sync::Arc;
use once_cell::sync::Lazy;
use tokio::sync::RwLock;

use crate::AppState;

/// In-memory cache for resolved (and unresolved/None) game mappings to avoid slamming the database during high-throughput log scanning.
static RESOLVE_CACHE: Lazy<RwLock<HashMap<(String, String), Option<String>>>> =
    Lazy::new(|| RwLock::new(HashMap::new()));

/// Resolves download identifiers (e.g. Steam depot IDs) to human-readable game names.
pub struct GameResolver;

impl GameResolver {
    /// Attempt to resolve a download_id to a game name for the given service.
    pub async fn resolve(
        state: &Arc<AppState>,
        service: &str,
        download_id: &str,
    ) -> Option<String> {
        let cache_key = (service.to_string(), download_id.to_string());

        // 1. Check in-memory cache first
        {
            let cache = RESOLVE_CACHE.read().await;
            if let Some(cached_val) = cache.get(&cache_key) {
                return cached_val.clone();
            }
        }

        // 2. Check local DB cache
        let resolved;
        let mut resolved_online = false;

        if let Ok(Some(name)) = state
            .db
            .get_game_name(service.to_string(), download_id.to_string())
            .await
        {
            resolved = Some(name);
        } else {
            // 3. Attempt online resolution
            resolved = match service {
                "steam" => resolve_steam_depot(state, download_id).await,
                _ => None,
            };
            if resolved.is_some() {
                resolved_online = true;
            }
        }

        // 4. Save to DB if resolved online
        if resolved_online {
            if let Some(ref name) = resolved {
                let _ = state
                    .db
                    .save_game_mapping(
                        service.to_string(),
                        download_id.to_string(),
                        name.clone(),
                        None,
                    )
                    .await;
            }
        }

        // 5. Populate in-memory cache (caches both Some and None)
        {
            let mut cache = RESOLVE_CACHE.write().await;
            cache.insert(cache_key, resolved.clone());
        }

        resolved
    }
}

/// Use the Steam Web API to resolve a depot ID to an app name.
async fn resolve_steam_depot(state: &Arc<AppState>, depot_id: &str) -> Option<String> {
    // The Steam API doesn't provide a direct depot→app mapping.
    // We use the GetAppList endpoint and maintain a local lookup table.
    // For efficiency, we lazy-load the full app list once and cache it.
    let api_key = state.config.read().await.steam_api_key.clone()?;

    let url = format!(
        "https://api.steampowered.com/ISteamApps/GetAppList/v2/?key={}",
        api_key
    );

    let resp = reqwest::get(&url).await.ok()?;
    let data: serde_json::Value = resp.json().await.ok()?;

    // The app list doesn't map depots, but having it allows us to match
    // app IDs that we might extract from other log patterns.
    // For now, return None — the mapping file approach is more reliable.
    let _ = data;
    let _ = depot_id;

    None
}

/// Load a community-maintained depot-to-game mapping JSON file.
#[allow(dead_code)]
pub async fn load_mapping_file(
    state: &Arc<AppState>,
    path: &str,
) -> Result<usize, Box<dyn std::error::Error + Send + Sync>> {
    let content = tokio::fs::read_to_string(path).await?;
    let mappings: HashMap<String, String> = serde_json::from_str(&content)?;

    let mut count = 0;
    for (depot_id, game_name) in mappings {
        state
            .db
            .save_game_mapping("steam".to_string(), depot_id, game_name, None)
            .await?;
        count += 1;
    }

    tracing::info!("Loaded {} game mappings from {}", count, path);
    Ok(count)
}

/// Automatically check if mapping database has mappings. If not, download and import them.
pub async fn check_and_download_depot_mappings(state: Arc<AppState>) {
    match state.db.get_game_mappings_count().await {
        Ok(count) => {
            if count > 0 {
                tracing::info!("Database already contains {} game mappings, skipping auto-download", count);
                return;
            }
        }
        Err(e) => {
            tracing::error!("Failed to check game mappings count: {}", e);
            return;
        }
    }

    if let Err(e) = download_and_import_mappings(state).await {
        tracing::error!("Failed to download and import game mappings: {}", e);
    }
}

/// Force download and update of mappings.
pub async fn force_download_depot_mappings(state: Arc<AppState>) -> Result<usize, Box<dyn std::error::Error + Send + Sync>> {
    tracing::info!("Force updating Steam depot mappings...");
    download_and_import_mappings(state).await
}

/// Download mapping CSV from GitHub and import into SQLite.
async fn download_and_import_mappings(state: Arc<AppState>) -> Result<usize, Box<dyn std::error::Error + Send + Sync>> {
    tracing::info!("Downloading Steam depot mappings (approx. 7.8 MB) from GitHub...");
    let url = "https://github.com/devedse/DeveLanCacheUI_SteamDepotFinder_Runner/releases/latest/download/app-depot-output-cleaned.csv";
    
    // Create reqwest client
    let client = reqwest::Client::builder()
        .user_agent("GravityLancacheUI/0.1.0")
        .build()?;
        
    let resp = client.get(url).send().await?;
    if !resp.status().is_success() {
        return Err(format!("GitHub returned HTTP {}", resp.status()).into());
    }

    let text = resp.text().await?;
    tracing::info!("Depot mappings file downloaded. Parsing and importing into database...");

    let state_clone = state.clone();
    let mappings = tokio::task::spawn_blocking(move || {
        let mut list = Vec::new();
        for line in text.lines() {
            let parts: Vec<&str> = line.split(';').collect();
            if parts.len() >= 3 {
                let app_id = parts[0].trim().to_string();
                let game_name = parts[1].trim().to_string();
                let depot_id = parts[2].trim().to_string();
                if !depot_id.is_empty() && !game_name.is_empty() {
                    list.push((depot_id, game_name, app_id));
                }
            }
        }
        list
    })
    .await?;

    let num_mappings = mappings.len();
    tracing::info!("Parsed {} mappings. Starting batch insert...", num_mappings);

    let inserted = state_clone.db.batch_insert_game_mappings("steam".to_string(), mappings).await?;
    tracing::info!("Successfully imported {}/{} Steam depot mappings into database", inserted, num_mappings);

    Ok(inserted)
}
