//! SQLite metadata store.
//!
//! All state transitions are transactional; the database runs in WAL mode for
//! crash safety and concurrent readers. One connection guards all writes via a
//! mutex; individual operations are sub-millisecond so they are safe to call
//! from async contexts without `spawn_blocking`.

use std::path::Path;
use std::sync::{Arc, Mutex};

use anyhow::{Context, Result};
use rusqlite::{params, Connection, OptionalExtension, Row};

use crate::models::{ChunkRecord, ChunkStatus, DownloadRecord, DownloadState};
use crate::storage::metadata::DownloadMeta;
use crate::utils::now_ms;

pub const SCHEMA_VERSION: i32 = 1;

const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS downloads (
    id                 INTEGER PRIMARY KEY AUTOINCREMENT,
    public_id          TEXT NOT NULL UNIQUE,
    url                TEXT NOT NULL,
    effective_url      TEXT,
    filename           TEXT NOT NULL,
    output_path        TEXT NOT NULL,
    chunk_dir          TEXT NOT NULL,
    state              TEXT NOT NULL DEFAULT 'queued',
    total_size         INTEGER,
    downloaded_size    INTEGER NOT NULL DEFAULT 0,
    retries            INTEGER NOT NULL DEFAULT 5,
    max_connections    INTEGER NOT NULL DEFAULT 8,
    user_agent         TEXT,
    etag               TEXT,
    last_modified      TEXT,
    accept_ranges      INTEGER NOT NULL DEFAULT 0,
    checksum_algorithm TEXT,
    checksum_expected  TEXT,
    error              TEXT,
    created_at         INTEGER NOT NULL,
    updated_at         INTEGER NOT NULL,
    started_at         INTEGER,
    finished_at        INTEGER,
    UNIQUE(url, output_path)
);

CREATE TABLE IF NOT EXISTS chunks (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    download_id   INTEGER NOT NULL REFERENCES downloads(id) ON DELETE CASCADE,
    idx           INTEGER NOT NULL,
    start         INTEGER NOT NULL,
    end           INTEGER NOT NULL,
    downloaded    INTEGER NOT NULL DEFAULT 0,
    status        TEXT NOT NULL DEFAULT 'pending',
    retries       INTEGER NOT NULL DEFAULT 0,
    error         TEXT,
    file_path     TEXT NOT NULL,
    last_activity INTEGER,
    finished_at   INTEGER,
    UNIQUE(download_id, idx)
);

CREATE TABLE IF NOT EXISTS events (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    download_id INTEGER NOT NULL REFERENCES downloads(id) ON DELETE CASCADE,
    level       TEXT NOT NULL,
    message     TEXT NOT NULL,
    created_at  INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS statistics (
    id                 INTEGER PRIMARY KEY AUTOINCREMENT,
    download_id        INTEGER NOT NULL REFERENCES downloads(id) ON DELETE CASCADE,
    ts                 INTEGER NOT NULL,
    bytes              INTEGER NOT NULL,
    active_connections INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_chunks_download ON chunks(download_id);
CREATE INDEX IF NOT EXISTS idx_events_download ON events(download_id);
CREATE INDEX IF NOT EXISTS idx_statistics_download ON statistics(download_id, ts);
"#;

/// Thread-safe handle to the metadata database.
#[derive(Clone)]
pub struct Storage {
    inner: Arc<Mutex<Connection>>,
    db_path: std::path::PathBuf,
}

impl Storage {
    /// Open (or create) the database at `path`.
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("cannot create data dir {}", parent.display()))?;
        }
        let conn = Connection::open(path)
            .with_context(|| format!("cannot open metadata DB {}", path.display()))?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        conn.busy_timeout(std::time::Duration::from_secs(5))?;
        conn.execute_batch(SCHEMA)?;
        let version: i32 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
        if version < SCHEMA_VERSION {
            conn.execute_batch(&format!("PRAGMA user_version = {SCHEMA_VERSION}"))?;
        }
        Ok(Storage {
            inner: Arc::new(Mutex::new(conn)),
            db_path: path.to_path_buf(),
        })
    }

    pub fn db_path(&self) -> &Path {
        &self.db_path
    }

    fn conn(&self) -> std::sync::MutexGuard<'_, Connection> {
        match self.inner.lock() {
            Ok(g) => g,
            Err(poisoned) => poisoned.into_inner(),
        }
    }

    // ------------------------------------------------------------- download

    pub fn insert_download(
        &self,
        url: &str,
        filename: &str,
        output_path: &Path,
        chunk_dir: &Path,
        retries: i32,
        connections: i32,
        user_agent: Option<&str>,
    ) -> Result<DownloadRecord> {
        let now = now_ms();
        let public_id = crate::utils::new_public_id();
        let conn = self.conn();
        conn.execute(
            r#"INSERT INTO downloads
               (public_id, url, filename, output_path, chunk_dir, state, retries,
                max_connections, user_agent, created_at, updated_at)
               VALUES (?1, ?2, ?3, ?4, ?5, 'queued', ?6, ?7, ?8, ?9, ?9)"#,
            params![
                public_id,
                url,
                filename,
                output_path.to_string_lossy(),
                chunk_dir.to_string_lossy(),
                retries,
                connections,
                user_agent,
                now
            ],
        )?;
        let id = conn.last_insert_rowid();
        drop(conn);
        self.get_download_by_id(id)?.ok_or_else(|| {
            anyhow::anyhow!("download row vanished immediately after insert (id {id})")
        })
    }

    pub fn get_download_by_id(&self, id: i64) -> Result<Option<DownloadRecord>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            r#"SELECT id, public_id, url, effective_url, filename, output_path,
                      chunk_dir, state, total_size, downloaded_size, retries,
                      max_connections, user_agent, etag, last_modified, accept_ranges,
                      checksum_algorithm, checksum_expected, error, created_at,
                      updated_at, started_at, finished_at
               FROM downloads WHERE id = ?1"#,
        )?;
        let row = stmt.query_row(params![id], map_download).optional()?;
        Ok(row)
    }

    pub fn get_download_by_public_id(&self, public_id: &str) -> Result<Option<DownloadRecord>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            r#"SELECT id, public_id, url, effective_url, filename, output_path,
                      chunk_dir, state, total_size, downloaded_size, retries,
                      max_connections, user_agent, etag, last_modified, accept_ranges,
                      checksum_algorithm, checksum_expected, error, created_at,
                      updated_at, started_at, finished_at
               FROM downloads WHERE public_id = ?1"#,
        )?;
        let row = stmt.query_row(params![public_id], map_download).optional()?;
        Ok(row)
    }

    pub fn find_download_by_url_output(
        &self,
        url: &str,
        output: &Path,
    ) -> Result<Option<DownloadRecord>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            r#"SELECT id, public_id, url, effective_url, filename, output_path,
                      chunk_dir, state, total_size, downloaded_size, retries,
                      max_connections, user_agent, etag, last_modified, accept_ranges,
                      checksum_algorithm, checksum_expected, error, created_at,
                      updated_at, started_at, finished_at
               FROM downloads WHERE url = ?1 AND output_path = ?2"#,
        )?;
        let row = stmt
            .query_row(
                params![url, output.to_string_lossy()],
                map_download,
            )
            .optional()?;
        Ok(row)
    }

    pub fn list_downloads(&self) -> Result<Vec<DownloadRecord>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            r#"SELECT id, public_id, url, effective_url, filename, output_path,
                      chunk_dir, state, total_size, downloaded_size, retries,
                      max_connections, user_agent, etag, last_modified, accept_ranges,
                      checksum_algorithm, checksum_expected, error, created_at,
                      updated_at, started_at, finished_at
               FROM downloads ORDER BY id DESC"#,
        )?;
        let rows = stmt.query_map([], map_download)?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    pub fn set_chunk_dir(&self, id: i64, dir: &Path) -> Result<()> {
        let conn = self.conn();
        conn.execute(
            "UPDATE downloads SET chunk_dir = ?2, updated_at = ?3 WHERE id = ?1",
            params![id, dir.to_string_lossy(), now_ms()],
        )?;
        Ok(())
    }

    pub fn update_download_meta(
        &self,
        id: i64,
        effective_url: Option<&str>,
        total_size: Option<i64>,
        accept_ranges: bool,
        etag: Option<&str>,
        last_modified: Option<&str>,
        checksum_algorithm: Option<&str>,
        checksum_expected: Option<&str>,
    ) -> Result<()> {
        let conn = self.conn();
        conn.execute(
            r#"UPDATE downloads SET effective_url = ?2, total_size = ?3, accept_ranges = ?4,
               etag = ?5, last_modified = ?6, checksum_algorithm = ?7, checksum_expected = ?8,
               updated_at = ?9 WHERE id = ?1"#,
            params![
                id,
                effective_url,
                total_size,
                accept_ranges as i32,
                etag,
                last_modified,
                checksum_algorithm,
                checksum_expected,
                now_ms()
            ],
        )?;
        Ok(())
    }

    pub fn update_state(&self, id: i64, state: DownloadState, error: Option<&str>) -> Result<()> {
        let conn = self.conn();
        let now = now_ms();
        conn.execute(
            r#"UPDATE downloads SET state = ?2, error = ?3, updated_at = ?4,
               started_at = COALESCE(started_at, CASE WHEN ?2 IN ('running','merging') THEN ?4 END),
               finished_at = CASE WHEN ?2 IN ('completed','cancelled','failed') THEN ?4 END
               WHERE id = ?1"#,
            params![id, state.as_str(), error, now],
        )?;
        Ok(())
    }

    pub fn add_downloaded_bytes(&self, id: i64, delta: i64) -> Result<()> {
        let conn = self.conn();
        conn.execute(
            "UPDATE downloads SET downloaded_size = downloaded_size + ?2, updated_at = ?3 WHERE id = ?1",
            params![id, delta.max(0), now_ms()],
        )?;
        Ok(())
    }

    pub fn set_downloaded_bytes(&self, id: i64, bytes: i64) -> Result<()> {
        let conn = self.conn();
        conn.execute(
            "UPDATE downloads SET downloaded_size = ?2, updated_at = ?3 WHERE id = ?1",
            params![id, bytes.max(0), now_ms()],
        )?;
        Ok(())
    }

    // ---------------------------------------------------------------- chunks

    pub fn insert_chunk(
        &self,
        download_id: i64,
        idx: i64,
        start: i64,
        end: i64,
        file_path: &Path,
    ) -> Result<i64> {
        let conn = self.conn();
        conn.execute(
            "INSERT INTO chunks (download_id, idx, start, end, downloaded, status, file_path)
             VALUES (?1, ?2, ?3, ?4, 0, 'pending', ?5)",
            params![download_id, idx, start, end, file_path.to_string_lossy()],
        )?;
        Ok(conn.last_insert_rowid())
    }

    pub fn get_chunks(&self, download_id: i64) -> Result<Vec<ChunkRecord>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            r#"SELECT id, download_id, idx, start, end, downloaded, status, retries,
                      error, file_path, last_activity, finished_at
               FROM chunks WHERE download_id = ?1 ORDER BY idx"#,
        )?;
        let rows = stmt.query_map(params![download_id], map_chunk)?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    pub fn update_chunk_progress(&self, chunk_id: i64, downloaded: i64, status: ChunkStatus) -> Result<()> {
        let conn = self.conn();
        conn.execute(
            "UPDATE chunks SET downloaded = ?2, status = ?3, last_activity = ?4 WHERE id = ?1",
            params![chunk_id, downloaded, status.as_str(), now_ms()],
        )?;
        Ok(())
    }

    pub fn update_chunk_end(&self, chunk_id: i64, end: i64) -> Result<()> {
        let conn = self.conn();
        conn.execute(
            "UPDATE chunks SET end = ?2, last_activity = ?3 WHERE id = ?1",
            params![chunk_id, end, now_ms()],
        )?;
        Ok(())
    }

    pub fn mark_chunk_finished(&self, chunk_id: i64, error: Option<&str>) -> Result<()> {
        let conn = self.conn();
        let now = now_ms();
        conn.execute(
            r#"UPDATE chunks SET status = CASE WHEN ?2 IS NULL THEN 'completed' ELSE 'failed' END,
               error = ?2, retries = retries, last_activity = ?3, finished_at = ?3
               WHERE id = ?1"#,
            params![chunk_id, error, now],
        )?;
        Ok(())
    }

    pub fn mark_chunk_retry(&self, chunk_id: i64, error: &str) -> Result<()> {
        let conn = self.conn();
        conn.execute(
            "UPDATE chunks SET status = 'pending', retries = retries + 1, error = ?2, last_activity = ?3 WHERE id = ?1",
            params![chunk_id, error, now_ms()],
        )?;
        Ok(())
    }

    pub fn reset_chunk(&self, chunk_id: i64) -> Result<()> {
        let conn = self.conn();
        conn.execute(
            "UPDATE chunks SET downloaded = 0, status = 'pending', error = NULL WHERE id = ?1",
            params![chunk_id],
        )?;
        Ok(())
    }

    pub fn delete_chunks(&self, download_id: i64) -> Result<()> {
        let conn = self.conn();
        conn.execute("DELETE FROM chunks WHERE download_id = ?1", params![download_id])?;
        Ok(())
    }

    // ---------------------------------------------------------------- events

    pub fn log_event(&self, download_id: i64, level: &str, message: &str) -> Result<()> {
        let conn = self.conn();
        conn.execute(
            "INSERT INTO events (download_id, level, message, created_at) VALUES (?1, ?2, ?3, ?4)",
            params![download_id, level, message, now_ms()],
        )?;
        Ok(())
    }

    pub fn recent_events(&self, download_id: i64, limit: i64) -> Result<Vec<(String, String, i64)>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT level, message, created_at FROM events WHERE download_id = ?1 ORDER BY id DESC LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![download_id, limit], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, i64>(2)?))
        })?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    // ------------------------------------------------------------- statistics

    pub fn record_statistic(&self, download_id: i64, bytes: i64, active: i64) -> Result<()> {
        let conn = self.conn();
        conn.execute(
            "INSERT INTO statistics (download_id, ts, bytes, active_connections) VALUES (?1, ?2, ?3, ?4)",
            params![download_id, now_ms(), bytes.max(0), active],
        )?;
        Ok(())
    }

    pub fn last_statistics(&self, download_id: i64, limit: i64) -> Result<Vec<(i64, i64, i64)>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT ts, bytes, active_connections FROM statistics WHERE download_id = ?1 ORDER BY id DESC LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![download_id, limit], |r| {
            Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?, r.get::<_, i64>(2)?))
        })?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    // ---------------------------------------------------------------- remove

    pub fn delete_download(&self, id: i64) -> Result<()> {
        let conn = self.conn();
        conn.execute("DELETE FROM downloads WHERE id = ?1", params![id])?;
        Ok(())
    }

    /// Fetch a full metadata snapshot (download + chunks) for external APIs.
    pub fn snapshot(&self, id: i64) -> Result<Option<DownloadMeta>> {
        let Some(dl) = self.get_download_by_id(id)? else {
            return Ok(None);
        };
        let chunks = self.get_chunks(id)?;
        Ok(Some(DownloadMeta::from_records(&dl, &chunks)))
    }
}

fn map_download(r: &Row<'_>) -> rusqlite::Result<DownloadRecord> {
    let state: String = r.get(7)?;
    let accept_ranges: i32 = r.get(15)?;
    Ok(DownloadRecord {
        id: r.get(0)?,
        public_id: r.get(1)?,
        url: r.get(2)?,
        effective_url: r.get(3)?,
        filename: r.get(4)?,
        output_path: r.get(5)?,
        chunk_dir: r.get(6)?,
        state: DownloadState::from_str(&state).unwrap_or(DownloadState::Interrupted),
        total_size: r.get(8)?,
        downloaded_size: r.get(9)?,
        retries: r.get(10)?,
        max_connections: r.get(11)?,
        user_agent: r.get(12)?,
        etag: r.get(13)?,
        last_modified: r.get(14)?,
        accept_ranges: accept_ranges != 0,
        checksum_algorithm: r.get(16)?,
        checksum_expected: r.get(17)?,
        error: r.get(18)?,
        created_at: r.get(19)?,
        updated_at: r.get(20)?,
        started_at: r.get(21)?,
        finished_at: r.get(22)?,
    })
}

fn map_chunk(r: &Row<'_>) -> rusqlite::Result<ChunkRecord> {
    let status: String = r.get(6)?;
    Ok(ChunkRecord {
        id: r.get(0)?,
        download_id: r.get(1)?,
        idx: r.get(2)?,
        start: r.get(3)?,
        end: r.get(4)?,
        downloaded: r.get(5)?,
        status: ChunkStatus::from_str(&status).unwrap_or(ChunkStatus::Pending),
        retries: r.get(7)?,
        error: r.get(8)?,
        file_path: r.get(9)?,
        last_activity: r.get(10)?,
        finished_at: r.get(11)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        let dir = std::env::temp_dir().join(format!("rdm-db-test-{}", std::process::id()));
        let db = dir.join("metadata.db");
        let _ = std::fs::remove_dir_all(&dir);
        let storage = Storage::open(&db).unwrap();
        let dl = storage
            .insert_download(
                "https://example.com/a.bin",
                "a.bin",
                &dir.join("a.bin"),
                &dir.join(".rdm/dl-00000001"),
                3,
                4,
                Some("rdm-test"),
            )
            .unwrap();
        assert!(dl.public_id.starts_with("dl-"));
        assert_eq!(dl.state, DownloadState::Queued);

        let chunk_id = storage
            .insert_chunk(dl.id, 0, 0, 99, &dir.join("chunk-0000.tmp"))
            .unwrap();
        storage
            .update_chunk_progress(chunk_id, 42, ChunkStatus::Active)
            .unwrap();
        let chunks = storage.get_chunks(dl.id).unwrap();
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].downloaded, 42);
        assert_eq!(chunks[0].len(), 100);
        assert_eq!(chunks[0].remaining(), 58);

        storage.update_state(dl.id, DownloadState::Completed, None).unwrap();
        let reloaded = storage.get_download_by_id(dl.id).unwrap().unwrap();
        assert_eq!(reloaded.state, DownloadState::Completed);
        assert!(reloaded.finished_at.is_some());

        let found = storage
            .find_download_by_url_output("https://example.com/a.bin", &dir.join("a.bin"))
            .unwrap()
            .unwrap();
        assert_eq!(found.public_id, dl.public_id);

        let snap = storage.snapshot(dl.id).unwrap().unwrap();
        assert_eq!(snap.chunks.len(), 1);
        storage.delete_download(dl.id).unwrap();
        assert!(storage.get_download_by_id(dl.id).unwrap().is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn event_log() {
        let dir = std::env::temp_dir().join(format!("rdm-ev-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let storage = Storage::open(&dir.join("m.db")).unwrap();
        let dl = storage
            .insert_download("u", "f", &dir.join("f"), &dir.join("c"), 1, 1, None)
            .unwrap();
        storage.log_event(dl.id, "info", "hello").unwrap();
        let evs = storage.recent_events(dl.id, 10).unwrap();
        assert_eq!(evs.len(), 1);
        assert_eq!(evs[0].1, "hello");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
