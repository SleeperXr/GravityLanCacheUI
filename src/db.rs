use rusqlite::params;
use tokio_rusqlite::Connection;

/// SQLite database wrapper with async operations via tokio-rusqlite.
#[derive(Clone)]
pub struct Database {
    conn: Connection,
}

impl Database {
    /// Open or create the SQLite database at the given path.
    pub async fn new(path: &str) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        if let Some(parent) = std::path::Path::new(path).parent() {
            std::fs::create_dir_all(parent)?;
        }

        let conn = Connection::open(path).await?;

        // Enable WAL mode for concurrent reads during writes
        conn.call(|conn| {
            conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")?;
            Ok(())
        })
        .await?;

        Ok(Self { conn })
    }

    /// Run all schema migrations.
    pub async fn run_migrations(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.conn
            .call(|conn| {
                conn.execute_batch(SCHEMA)?;
                Ok(())
            })
            .await?;
        tracing::info!("Database migrations complete");
        Ok(())
    }

    // ── Download Events ──────────────────────────────────────────────

    pub async fn insert_download_event(
        &self,
        event: &DownloadEvent,
    ) -> Result<i64, Box<dyn std::error::Error + Send + Sync>> {
        let e = event.clone();
        let id = self
            .conn
            .call(move |conn| {
                conn.execute(
                    "INSERT INTO download_events
                     (client_ip, service, download_id, game_name, started_at, ended_at,
                      total_bytes, hit_bytes, miss_bytes, request_count, hit_rate)
                     VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)",
                    params![
                        e.client_ip,
                        e.service,
                        e.download_id,
                        e.game_name,
                        e.started_at,
                        e.ended_at,
                        e.total_bytes,
                        e.hit_bytes,
                        e.miss_bytes,
                        e.request_count,
                        e.hit_rate,
                    ],
                )?;
                Ok(conn.last_insert_rowid())
            })
            .await?;
        Ok(id)
    }

    pub async fn update_download_event(
        &self,
        event: &DownloadEvent,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let e = event.clone();
        self.conn
            .call(move |conn| {
                conn.execute(
                    "UPDATE download_events
                     SET ended_at=?1, total_bytes=?2, hit_bytes=?3, miss_bytes=?4,
                         request_count=?5, hit_rate=?6, game_name=?7
                     WHERE id=?8",
                    params![
                        e.ended_at,
                        e.total_bytes,
                        e.hit_bytes,
                        e.miss_bytes,
                        e.request_count,
                        e.hit_rate,
                        e.game_name,
                        e.id,
                    ],
                )?;
                Ok(())
            })
            .await?;
        Ok(())
    }

    pub async fn find_active_download(
        &self,
        client_ip: String,
        service: String,
        download_id: Option<String>,
        cutoff: String,
    ) -> Result<Option<DownloadEvent>, Box<dyn std::error::Error + Send + Sync>> {
        let result = self
            .conn
            .call(move |conn| {
                let mut stmt = conn.prepare(
                    "SELECT d.id, d.client_ip, d.service, d.download_id, COALESCE(d.game_name, m.game_name),
                            d.started_at, d.ended_at, d.total_bytes, d.hit_bytes, d.miss_bytes,
                            d.request_count, d.hit_rate, m.app_id
                     FROM download_events d
                     LEFT JOIN game_mappings m ON d.service = m.service AND d.download_id = m.download_id
                     WHERE d.client_ip=?1 AND d.service=?2 AND d.download_id IS ?3 AND d.ended_at > ?4
                     ORDER BY d.ended_at DESC LIMIT 1",
                )?;
                let event = stmt
                    .query_row(params![client_ip, service, download_id, cutoff], |row| {
                        Ok(DownloadEvent {
                            id: row.get(0)?,
                            client_ip: row.get(1)?,
                            service: row.get(2)?,
                            download_id: row.get(3)?,
                            game_name: row.get(4)?,
                            started_at: row.get(5)?,
                            ended_at: row.get(6)?,
                            total_bytes: row.get(7)?,
                            hit_bytes: row.get(8)?,
                            miss_bytes: row.get(9)?,
                            request_count: row.get(10)?,
                            hit_rate: row.get(11)?,
                            app_id: row.get(12)?,
                        })
                    })
                    .ok();
                Ok(event)
            })
            .await?;
        Ok(result)
    }

    pub async fn get_recent_downloads(
        &self,
        limit: u32,
        offset: u32,
    ) -> Result<Vec<DownloadEvent>, Box<dyn std::error::Error + Send + Sync>> {
        let events = self
            .conn
            .call(move |conn| {
                let mut stmt = conn.prepare(
                    "SELECT d.id, d.client_ip, d.service, d.download_id, COALESCE(d.game_name, m.game_name),
                            d.started_at, d.ended_at, d.total_bytes, d.hit_bytes, d.miss_bytes,
                            d.request_count, d.hit_rate, m.app_id
                     FROM download_events d
                     LEFT JOIN game_mappings m ON d.service = m.service AND d.download_id = m.download_id
                     ORDER BY d.ended_at DESC LIMIT ?1 OFFSET ?2",
                )?;
                let rows = stmt
                    .query_map(params![limit, offset], |row| {
                        Ok(DownloadEvent {
                            id: row.get(0)?,
                            client_ip: row.get(1)?,
                            service: row.get(2)?,
                            download_id: row.get(3)?,
                            game_name: row.get(4)?,
                            started_at: row.get(5)?,
                            ended_at: row.get(6)?,
                            total_bytes: row.get(7)?,
                            hit_bytes: row.get(8)?,
                            miss_bytes: row.get(9)?,
                            request_count: row.get(10)?,
                            hit_rate: row.get(11)?,
                            app_id: row.get(12)?,
                        })
                    })?
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(rows)
            })
            .await?;
        Ok(events)
    }

    // ── Hourly Stats ─────────────────────────────────────────────────

    pub async fn upsert_hourly_stats(
        &self,
        stats: &HourlyStats,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let s = stats.clone();
        self.conn
            .call(move |conn| {
                conn.execute(
                    "INSERT INTO hourly_stats (hour, service, total_bytes, hit_bytes, miss_bytes, request_count, unique_clients)
                     VALUES (?1,?2,?3,?4,?5,?6,?7)
                     ON CONFLICT(hour, service) DO UPDATE SET
                       total_bytes = total_bytes + excluded.total_bytes,
                       hit_bytes = hit_bytes + excluded.hit_bytes,
                       miss_bytes = miss_bytes + excluded.miss_bytes,
                       request_count = request_count + excluded.request_count",
                    params![s.hour, s.service, s.total_bytes, s.hit_bytes, s.miss_bytes, s.request_count, s.unique_clients],
                )?;
                Ok(())
            })
            .await?;
        Ok(())
    }

    // ── Parse State ──────────────────────────────────────────────────

    pub async fn get_parse_state(
        &self,
    ) -> Result<ParseState, Box<dyn std::error::Error + Send + Sync>> {
        let state = self
            .conn
            .call(|conn| {
                let result = conn
                    .query_row(
                        "SELECT last_offset, last_inode FROM parse_state WHERE id=1",
                        [],
                        |row| {
                            Ok(ParseState {
                                last_offset: row.get(0)?,
                                last_inode: row.get(1)?,
                            })
                        },
                    )
                    .ok()
                    .unwrap_or_default();
                Ok(result)
            })
            .await?;
        Ok(state)
    }

    pub async fn save_parse_state(
        &self,
        state: &ParseState,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let s = state.clone();
        self.conn
            .call(move |conn| {
                conn.execute(
                    "INSERT INTO parse_state (id, last_offset, last_inode, last_parsed_at)
                     VALUES (1, ?1, ?2, datetime('now'))
                     ON CONFLICT(id) DO UPDATE SET
                       last_offset=excluded.last_offset,
                       last_inode=excluded.last_inode,
                       last_parsed_at=excluded.last_parsed_at",
                    params![s.last_offset, s.last_inode],
                )?;
                Ok(())
            })
            .await?;
        Ok(())
    }

    // ── Game Mappings ────────────────────────────────────────────────

    pub async fn get_game_name(
        &self,
        service: String,
        download_id: String,
    ) -> Result<Option<String>, Box<dyn std::error::Error + Send + Sync>> {
        let name = self
            .conn
            .call(move |conn| {
                let result = conn
                    .query_row(
                        "SELECT game_name FROM game_mappings WHERE service=?1 AND download_id=?2",
                        params![service, download_id],
                        |row| row.get(0),
                    )
                    .ok();
                Ok(result)
            })
            .await?;
        Ok(name)
    }

    pub async fn get_steam_app_id(
        &self,
        depot_id: String,
    ) -> Result<Option<String>, Box<dyn std::error::Error + Send + Sync>> {
        let app_id = self
            .conn
            .call(move |conn| {
                let result = conn
                    .query_row(
                        "SELECT app_id FROM game_mappings WHERE service='steam' AND download_id=?1",
                        params![depot_id],
                        |row| row.get(0),
                    )
                    .ok();
                Ok(result)
            })
            .await?;
        Ok(app_id)
    }

    pub async fn save_game_mapping(
        &self,
        service: String,
        download_id: String,
        game_name: String,
        app_id: Option<String>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.conn
            .call(move |conn| {
                conn.execute(
                    "INSERT INTO game_mappings (service, download_id, game_name, app_id, updated_at)
                     VALUES (?1,?2,?3,?4,datetime('now'))
                     ON CONFLICT(service, download_id) DO UPDATE SET
                       game_name=excluded.game_name, app_id=excluded.app_id, updated_at=excluded.updated_at",
                    params![service, download_id, game_name, app_id],
                )?;
                Ok(())
            })
            .await?;
        Ok(())
    }

    // ── Cache Snapshots ──────────────────────────────────────────────

    pub async fn insert_cache_snapshot(
        &self,
        snapshot: &CacheSnapshot,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let s = snapshot.clone();
        self.conn
            .call(move |conn| {
                conn.execute(
                    "INSERT INTO cache_snapshots (taken_at, total_size_bytes, total_files, details_json)
                     VALUES (datetime('now'), ?1, ?2, ?3)",
                    params![s.total_size_bytes, s.total_files, s.details_json],
                )?;
                Ok(())
            })
            .await?;
        Ok(())
    }

    pub async fn get_latest_cache_snapshot(
        &self,
    ) -> Result<Option<CacheSnapshot>, Box<dyn std::error::Error + Send + Sync>> {
        let snapshot = self
            .conn
            .call(|conn| {
                let result = conn
                    .query_row(
                        "SELECT total_size_bytes, total_files, details_json, taken_at FROM cache_snapshots ORDER BY id DESC LIMIT 1",
                        [],
                        |row| {
                            Ok(CacheSnapshot {
                                total_size_bytes: row.get(0)?,
                                total_files: row.get(1)?,
                                details_json: row.get(2)?,
                                taken_at: Some(row.get(3)?),
                            })
                        },
                    )
                    .ok();
                Ok(result)
            })
            .await?;
        Ok(snapshot)
    }

    pub async fn get_cached_games_summary(
        &self,
    ) -> Result<Vec<CachedGameSummary>, Box<dyn std::error::Error + Send + Sync>> {
        let list = self
            .conn
            .call(|conn| {
                let mut stmt = conn.prepare(
                    "SELECT COALESCE(d.game_name, m.game_name, d.download_id, d.service) AS name,
                            d.service,
                            m.app_id,
                            SUM(d.total_bytes) AS total_bytes,
                            SUM(d.hit_bytes) AS hit_bytes,
                            SUM(d.miss_bytes) AS miss_bytes,
                            MAX(d.ended_at) AS last_downloaded
                     FROM download_events d
                     LEFT JOIN game_mappings m ON d.service = m.service AND d.download_id = m.download_id
                     GROUP BY name, d.service, m.app_id
                     ORDER BY total_bytes DESC",
                )?;
                let rows = stmt
                    .query_map([], |row| {
                        Ok(CachedGameSummary {
                            name: row.get(0)?,
                            service: row.get(1)?,
                            app_id: row.get(2)?,
                            total_bytes: row.get(3)?,
                            hit_bytes: row.get(4)?,
                            miss_bytes: row.get(5)?,
                            last_downloaded: row.get(6)?,
                        })
                    })?
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(rows)
            })
            .await?;
        Ok(list)
    }

    // ── Counts & Batch Imports ────────────────────────────────────────

    pub async fn get_download_events_count(
        &self,
    ) -> Result<i64, Box<dyn std::error::Error + Send + Sync>> {
        let count = self
            .conn
            .call(|conn| {
                let val: i64 = conn
                    .query_row("SELECT COUNT(*) FROM download_events", [], |r| r.get(0))
                    .unwrap_or(0);
                Ok(val)
            })
            .await?;
        Ok(count)
    }

    pub async fn get_game_mappings_count(
        &self,
    ) -> Result<i64, Box<dyn std::error::Error + Send + Sync>> {
        let count = self
            .conn
            .call(|conn| {
                let val: i64 = conn
                    .query_row("SELECT COUNT(*) FROM game_mappings", [], |r| r.get(0))
                    .unwrap_or(0);
                Ok(val)
            })
            .await?;
        Ok(count)
    }

    pub async fn batch_insert_game_mappings(
        &self,
        service: String,
        mappings: Vec<(String, String, String)>,
    ) -> Result<usize, Box<dyn std::error::Error + Send + Sync>> {
        let count = self
            .conn
            .call(move |conn| {
                let tx = conn.transaction()?;
                let mut stmt = tx.prepare(
                    "INSERT INTO game_mappings (service, download_id, game_name, app_id, updated_at)
                     VALUES (?1, ?2, ?3, ?4, datetime('now'))
                     ON CONFLICT(service, download_id) DO UPDATE SET
                       game_name=excluded.game_name, app_id=excluded.app_id, updated_at=excluded.updated_at",
                )?;
                let mut inserted = 0;
                for (depot_id, game_name, app_id) in mappings {
                    stmt.execute(params![service, depot_id, game_name, Some(app_id)])?;
                    inserted += 1;
                }
                stmt.finalize()?;
                tx.commit()?;
                Ok(inserted)
            })
            .await?;
        Ok(count)
    }

    // ── Dashboard Aggregations ───────────────────────────────────────

    pub async fn get_dashboard_stats(
        &self,
    ) -> Result<DashboardStats, Box<dyn std::error::Error + Send + Sync>> {
        let stats = self
            .conn
            .call(|conn| {
                let row = conn.query_row(
                    "SELECT 
                        COALESCE(SUM(total_bytes), 0),
                        COALESCE(SUM(hit_bytes), 0),
                        COALESCE(SUM(miss_bytes), 0),
                        COUNT(*),
                        COUNT(DISTINCT client_ip)
                     FROM download_events",
                    [],
                    |r| {
                        Ok((
                            r.get::<_, i64>(0)?,
                            r.get::<_, i64>(1)?,
                            r.get::<_, i64>(2)?,
                            r.get::<_, i64>(3)?,
                            r.get::<_, i64>(4)?,
                        ))
                    },
                ).unwrap_or((0, 0, 0, 0, 0));

                let total_bytes = row.0;
                let hit_bytes = row.1;
                let miss_bytes = row.2;
                let total_downloads = row.3;
                let unique_clients = row.4;

                let hit_rate = if total_bytes > 0 {
                    (hit_bytes as f64 / total_bytes as f64) * 100.0
                } else {
                    0.0
                };

                Ok(DashboardStats {
                    total_bytes,
                    hit_bytes,
                    miss_bytes,
                    bandwidth_saved: hit_bytes,
                    hit_rate,
                    total_downloads,
                    unique_clients,
                })
            })
            .await?;
        Ok(stats)
    }
}

// ── Data Models ──────────────────────────────────────────────────────

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub struct DownloadEvent {
    pub id: i64,
    pub client_ip: String,
    pub service: String,
    pub download_id: Option<String>,
    pub game_name: Option<String>,
    pub started_at: String,
    pub ended_at: String,
    pub total_bytes: i64,
    pub hit_bytes: i64,
    pub miss_bytes: i64,
    pub request_count: i64,
    pub hit_rate: f64,
    pub app_id: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HourlyStats {
    pub hour: String,
    pub service: String,
    pub total_bytes: i64,
    pub hit_bytes: i64,
    pub miss_bytes: i64,
    pub request_count: i64,
    pub unique_clients: i64,
}

#[derive(Debug, Clone, Default)]
pub struct ParseState {
    pub last_offset: i64,
    pub last_inode: i64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CacheSnapshot {
    pub total_size_bytes: i64,
    pub total_files: i64,
    pub details_json: Option<String>,
    pub taken_at: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DashboardStats {
    pub total_bytes: i64,
    pub hit_bytes: i64,
    pub miss_bytes: i64,
    pub bandwidth_saved: i64,
    pub hit_rate: f64,
    pub total_downloads: i64,
    pub unique_clients: i64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CachedGameSummary {
    pub name: String,
    pub service: String,
    pub app_id: Option<String>,
    pub total_bytes: i64,
    pub hit_bytes: i64,
    pub miss_bytes: i64,
    pub last_downloaded: String,
}

// ── Schema ───────────────────────────────────────────────────────────

const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS download_events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    client_ip TEXT NOT NULL,
    service TEXT NOT NULL,
    download_id TEXT,
    game_name TEXT,
    started_at TEXT NOT NULL,
    ended_at TEXT NOT NULL,
    total_bytes INTEGER NOT NULL DEFAULT 0,
    hit_bytes INTEGER NOT NULL DEFAULT 0,
    miss_bytes INTEGER NOT NULL DEFAULT 0,
    request_count INTEGER NOT NULL DEFAULT 0,
    hit_rate REAL NOT NULL DEFAULT 0.0
);

CREATE INDEX IF NOT EXISTS idx_downloads_ended ON download_events(ended_at);
CREATE INDEX IF NOT EXISTS idx_downloads_service ON download_events(service);
CREATE INDEX IF NOT EXISTS idx_downloads_client ON download_events(client_ip);
CREATE INDEX IF NOT EXISTS idx_downloads_lookup ON download_events(client_ip, service, download_id, ended_at);

CREATE TABLE IF NOT EXISTS hourly_stats (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    hour TEXT NOT NULL,
    service TEXT NOT NULL,
    total_bytes INTEGER NOT NULL DEFAULT 0,
    hit_bytes INTEGER NOT NULL DEFAULT 0,
    miss_bytes INTEGER NOT NULL DEFAULT 0,
    request_count INTEGER NOT NULL DEFAULT 0,
    unique_clients INTEGER NOT NULL DEFAULT 0,
    UNIQUE(hour, service)
);

CREATE TABLE IF NOT EXISTS client_stats (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    client_ip TEXT NOT NULL,
    date TEXT NOT NULL,
    service TEXT NOT NULL,
    total_bytes INTEGER NOT NULL DEFAULT 0,
    hit_bytes INTEGER NOT NULL DEFAULT 0,
    miss_bytes INTEGER NOT NULL DEFAULT 0,
    request_count INTEGER NOT NULL DEFAULT 0,
    UNIQUE(client_ip, date, service)
);

CREATE TABLE IF NOT EXISTS game_mappings (
    service TEXT NOT NULL,
    download_id TEXT NOT NULL,
    game_name TEXT NOT NULL,
    app_id TEXT,
    updated_at TEXT DEFAULT (datetime('now')),
    PRIMARY KEY(service, download_id)
);

CREATE TABLE IF NOT EXISTS parse_state (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    last_offset INTEGER NOT NULL DEFAULT 0,
    last_inode INTEGER NOT NULL DEFAULT 0,
    last_parsed_at TEXT
);

CREATE TABLE IF NOT EXISTS cache_snapshots (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    taken_at TEXT NOT NULL,
    total_size_bytes INTEGER NOT NULL,
    total_files INTEGER NOT NULL,
    details_json TEXT
);
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_dashboard_stats() {
        let db = Database::new(":memory:").await.unwrap();
        db.run_migrations().await.unwrap();

        let event1 = DownloadEvent {
            id: 0,
            client_ip: "192.168.1.50".to_string(),
            service: "steam".to_string(),
            download_id: Some("100".to_string()),
            game_name: Some("Test Game".to_string()),
            started_at: "2026-06-17T12:00:00".to_string(),
            ended_at: "2026-06-17T12:05:00".to_string(),
            total_bytes: 1000,
            hit_bytes: 800,
            miss_bytes: 200,
            request_count: 5,
            hit_rate: 80.0,
            app_id: None,
        };

        let event2 = DownloadEvent {
            id: 0,
            client_ip: "192.168.1.60".to_string(),
            service: "steam".to_string(),
            download_id: Some("200".to_string()),
            game_name: Some("Test Game 2".to_string()),
            started_at: "2026-06-17T12:10:00".to_string(),
            ended_at: "2026-06-17T12:15:00".to_string(),
            total_bytes: 2000,
            hit_bytes: 2000,
            miss_bytes: 0,
            request_count: 10,
            hit_rate: 100.0,
            app_id: None,
        };

        db.insert_download_event(&event1).await.unwrap();
        db.insert_download_event(&event2).await.unwrap();

        let stats = db.get_dashboard_stats().await.unwrap();
        assert_eq!(stats.total_bytes, 3000);
        assert_eq!(stats.hit_bytes, 2800);
        assert_eq!(stats.miss_bytes, 200);
        assert_eq!(stats.bandwidth_saved, 2800);
        assert_eq!(stats.total_downloads, 2);
        assert_eq!(stats.unique_clients, 2);
        assert!((stats.hit_rate - 93.33333333333333).abs() < 0.001);
    }
}
