use std::collections::HashMap;
use std::sync::Arc;

use crate::AppState;

/// Resolves download identifiers (e.g. Steam depot IDs) to human-readable game names.
pub struct GameResolver;

impl GameResolver {
    /// Attempt to resolve a download_id to a game name for the given service.
    pub async fn resolve(
        state: &Arc<AppState>,
        service: &str,
        download_id: &str,
    ) -> Option<String> {
        // Check local DB cache first
        if let Ok(Some(name)) = state
            .db
            .get_game_name(service.to_string(), download_id.to_string())
            .await
        {
            return Some(name);
        }

        // Attempt online resolution
        let resolved = match service {
            "steam" => resolve_steam_depot(state, download_id).await,
            _ => None,
        };

        // Cache the result if found
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

        resolved
    }
}

/// Use the Steam Web API to resolve a depot ID to an app name.
async fn resolve_steam_depot(state: &Arc<AppState>, depot_id: &str) -> Option<String> {
    // The Steam API doesn't provide a direct depot→app mapping.
    // We use the GetAppList endpoint and maintain a local lookup table.
    // For efficiency, we lazy-load the full app list once and cache it.
    let api_key = state.config.steam_api_key.as_ref()?;

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
