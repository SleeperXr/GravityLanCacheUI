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
                    "SELECT id, client_ip, service, download_id, game_name,
                            started_at, ended_at, total_bytes, hit_bytes, miss_bytes,
                            request_count, hit_rate
                     FROM download_events
                     WHERE client_ip=?1 AND service=?2 AND download_id IS ?3 AND ended_at > ?4
                     ORDER BY ended_at DESC LIMIT 1",
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
                    "SELECT id, client_ip, service, download_id, game_name,
                            started_at, ended_at, total_bytes, hit_bytes, miss_bytes,
                            request_count, hit_rate
                     FROM download_events ORDER BY ended_at DESC LIMIT ?1 OFFSET ?2",
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

    // ── Dashboard Aggregations ───────────────────────────────────────

    pub async fn get_dashboard_stats(
        &self,
    ) -> Result<DashboardStats, Box<dyn std::error::Error + Send + Sync>> {
        let stats = self
            .conn
            .call(|conn| {
                let total_bytes: i64 = conn
                    .query_row("SELECT COALESCE(SUM(total_bytes),0) FROM download_events", [], |r| r.get(0))
                    .unwrap_or(0);
                let hit_bytes: i64 = conn
                    .query_row("SELECT COALESCE(SUM(hit_bytes),0) FROM download_events", [], |r| r.get(0))
                    .unwrap_or(0);
                let miss_bytes: i64 = conn
                    .query_row("SELECT COALESCE(SUM(miss_bytes),0) FROM download_events", [], |r| r.get(0))
                    .unwrap_or(0);
                let total_downloads: i64 = conn
                    .query_row("SELECT COUNT(*) FROM download_events", [], |r| r.get(0))
                    .unwrap_or(0);
                let unique_clients: i64 = conn
                    .query_row("SELECT COUNT(DISTINCT client_ip) FROM download_events", [], |r| r.get(0))
                    .unwrap_or(0);

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
