use anyhow::{Context, Result};
use parking_lot::Mutex;
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

pub const HISTORY_CAP: usize = 50;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    pub id: i64,
    pub kind: String,
    pub text: Option<String>,
    pub image_path: Option<String>,
    pub thumb_b64: Option<String>,
    pub width: Option<i64>,
    pub height: Option<i64>,
    pub size_bytes: i64,
    pub content_hash: String,
    pub pinned: bool,
    pub created_at: i64,
    pub last_used_at: i64,
}

pub struct Repo {
    conn: Arc<Mutex<Connection>>,
    images_dir: PathBuf,
}

impl Repo {
    pub fn open(db_path: &Path, images_dir: PathBuf) -> Result<Self> {
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent).ok();
        }
        std::fs::create_dir_all(&images_dir).ok();
        let conn = Connection::open(db_path).context("open sqlite")?;
        conn.pragma_update(None, "journal_mode", "WAL").ok();
        conn.pragma_update(None, "foreign_keys", "ON").ok();
        Self::migrate(&conn)?;
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
            images_dir,
        })
    }

    fn migrate(conn: &Connection) -> Result<()> {
        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS entries (
                id            INTEGER PRIMARY KEY AUTOINCREMENT,
                kind          TEXT NOT NULL CHECK (kind IN ('text','image')),
                text          TEXT,
                image_path    TEXT,
                thumb_png     BLOB,
                width         INTEGER,
                height        INTEGER,
                size_bytes    INTEGER NOT NULL,
                content_hash  TEXT NOT NULL UNIQUE,
                pinned        INTEGER NOT NULL DEFAULT 0,
                created_at    INTEGER NOT NULL,
                last_used_at  INTEGER NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_entries_pinned_lastused
                ON entries(pinned, last_used_at DESC);
            CREATE INDEX IF NOT EXISTS idx_entries_hash ON entries(content_hash);
            "#,
        )
        .context("migrate")?;
        Ok(())
    }

    pub fn images_dir(&self) -> &Path {
        &self.images_dir
    }

    /// Insert a new entry or bump last_used_at if hash already exists.
    /// Returns (entry_id, was_new).
    pub fn upsert(
        &self,
        kind: &str,
        text: Option<&str>,
        image_path: Option<&str>,
        thumb_png: Option<&[u8]>,
        width: Option<i64>,
        height: Option<i64>,
        size_bytes: i64,
        content_hash: &str,
    ) -> Result<(i64, bool)> {
        let now = now_ms();
        let mut conn = self.conn.lock();
        let tx = conn.transaction()?;

        let existing: Option<i64> = tx
            .query_row(
                "SELECT id FROM entries WHERE content_hash = ?1",
                params![content_hash],
                |r| r.get::<_, i64>(0),
            )
            .optional()?;

        let (id, is_new) = if let Some(id) = existing {
            tx.execute(
                "UPDATE entries SET last_used_at = ?1 WHERE id = ?2",
                params![now, id],
            )?;
            (id, false)
        } else {
            tx.execute(
                "INSERT INTO entries (kind, text, image_path, thumb_png, width, height,
                                      size_bytes, content_hash, pinned, created_at, last_used_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 0, ?9, ?9)",
                params![
                    kind,
                    text,
                    image_path,
                    thumb_png,
                    width,
                    height,
                    size_bytes,
                    content_hash,
                    now
                ],
            )?;
            (tx.last_insert_rowid(), true)
        };

        // Evict oldest unpinned beyond HISTORY_CAP.
        let evicted_paths: Vec<String> = {
            let mut stmt = tx.prepare(
                "SELECT image_path FROM entries
                 WHERE pinned = 0
                   AND id NOT IN (
                       SELECT id FROM entries WHERE pinned = 0
                       ORDER BY last_used_at DESC LIMIT ?1
                   )
                   AND image_path IS NOT NULL",
            )?;
            let rows = stmt.query_map(params![HISTORY_CAP as i64], |r| {
                r.get::<_, String>(0)
            })?;
            rows.filter_map(|r| r.ok()).collect()
        };
        tx.execute(
            "DELETE FROM entries
             WHERE pinned = 0
               AND id NOT IN (
                   SELECT id FROM entries WHERE pinned = 0
                   ORDER BY last_used_at DESC LIMIT ?1
               )",
            params![HISTORY_CAP as i64],
        )?;

        tx.commit()?;

        for rel in evicted_paths {
            let full = self.images_dir.join(&rel);
            let _ = std::fs::remove_file(full);
        }

        Ok((id, is_new))
    }

    pub fn list(&self) -> Result<Vec<Entry>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT id, kind, text, image_path, thumb_png, width, height,
                    size_bytes, content_hash, pinned, created_at, last_used_at
             FROM entries
             ORDER BY pinned DESC, last_used_at DESC",
        )?;
        let rows = stmt.query_map([], row_to_entry)?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    pub fn get(&self, id: i64) -> Result<Option<Entry>> {
        let conn = self.conn.lock();
        let entry = conn
            .query_row(
                "SELECT id, kind, text, image_path, thumb_png, width, height,
                        size_bytes, content_hash, pinned, created_at, last_used_at
                 FROM entries WHERE id = ?1",
                params![id],
                row_to_entry,
            )
            .optional()?;
        Ok(entry)
    }

    pub fn toggle_pin(&self, id: i64) -> Result<bool> {
        let conn = self.conn.lock();
        let pinned: Option<i64> = conn
            .query_row(
                "SELECT pinned FROM entries WHERE id = ?1",
                params![id],
                |r| r.get(0),
            )
            .optional()?;
        let new = match pinned {
            Some(0) => 1,
            Some(_) => 0,
            None => return Ok(false),
        };
        conn.execute(
            "UPDATE entries SET pinned = ?1 WHERE id = ?2",
            params![new, id],
        )?;
        Ok(new == 1)
    }

    pub fn delete(&self, id: i64) -> Result<Option<String>> {
        let conn = self.conn.lock();
        let image_path: Option<String> = conn
            .query_row(
                "SELECT image_path FROM entries WHERE id = ?1",
                params![id],
                |r| r.get(0),
            )
            .optional()?
            .flatten();
        conn.execute("DELETE FROM entries WHERE id = ?1", params![id])?;
        if let Some(rel) = &image_path {
            let _ = std::fs::remove_file(self.images_dir.join(rel));
        }
        Ok(image_path)
    }

    pub fn touch(&self, id: i64) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute(
            "UPDATE entries SET last_used_at = ?1 WHERE id = ?2",
            params![now_ms(), id],
        )?;
        Ok(())
    }

    pub fn clear_unpinned(&self) -> Result<Vec<String>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT image_path FROM entries WHERE pinned = 0 AND image_path IS NOT NULL",
        )?;
        let paths: Vec<String> = stmt
            .query_map([], |r| r.get::<_, String>(0))?
            .filter_map(|r| r.ok())
            .collect();
        drop(stmt);
        conn.execute("DELETE FROM entries WHERE pinned = 0", [])?;
        for rel in &paths {
            let _ = std::fs::remove_file(self.images_dir.join(rel));
        }
        Ok(paths)
    }
}

fn row_to_entry(row: &rusqlite::Row) -> rusqlite::Result<Entry> {
    use base64::{engine::general_purpose::STANDARD, Engine as _};
    let thumb_blob: Option<Vec<u8>> = row.get(4)?;
    let thumb_b64 = thumb_blob.map(|b| STANDARD.encode(b));
    let pinned: i64 = row.get(9)?;
    Ok(Entry {
        id: row.get(0)?,
        kind: row.get(1)?,
        text: row.get(2)?,
        image_path: row.get(3)?,
        thumb_b64,
        width: row.get(5)?,
        height: row.get(6)?,
        size_bytes: row.get(7)?,
        content_hash: row.get(8)?,
        pinned: pinned != 0,
        created_at: row.get(10)?,
        last_used_at: row.get(11)?,
    })
}

pub fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha256};

    fn hash(s: &str) -> String {
        hex::encode(Sha256::digest(s.as_bytes()))
    }

    fn temp_repo() -> Repo {
        let dir = tempdir();
        Repo::open(&dir.join("db.sqlite"), dir.join("images")).unwrap()
    }

    fn tempdir() -> PathBuf {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let p = std::env::temp_dir().join(format!(
            "clipboard-test-{}-{}-{}",
            std::process::id(),
            now_ms(),
            n,
        ));
        std::fs::create_dir_all(&p).unwrap();
        p
    }

    #[test]
    fn upsert_dedupes_by_hash() {
        let repo = temp_repo();
        let h = hash("hello");
        let (id1, new1) = repo
            .upsert("text", Some("hello"), None, None, None, None, 5, &h)
            .unwrap();
        assert!(new1);
        let (id2, new2) = repo
            .upsert("text", Some("hello"), None, None, None, None, 5, &h)
            .unwrap();
        assert!(!new2);
        assert_eq!(id1, id2);
        assert_eq!(repo.list().unwrap().len(), 1);
    }

    #[test]
    fn evicts_oldest_unpinned_beyond_cap() {
        let repo = temp_repo();
        for i in 0..(HISTORY_CAP + 10) {
            let s = format!("item-{i}");
            let h = hash(&s);
            repo.upsert("text", Some(&s), None, None, None, None, s.len() as i64, &h)
                .unwrap();
        }
        assert_eq!(repo.list().unwrap().len(), HISTORY_CAP);
    }

    #[test]
    fn pinned_items_survive_eviction() {
        let repo = temp_repo();
        let pinned_ids: Vec<i64> = (0..5)
            .map(|i| {
                let s = format!("pinned-{i}");
                let h = hash(&s);
                let (id, _) = repo
                    .upsert("text", Some(&s), None, None, None, None, s.len() as i64, &h)
                    .unwrap();
                repo.toggle_pin(id).unwrap();
                id
            })
            .collect();
        for i in 0..(HISTORY_CAP + 20) {
            let s = format!("noise-{i}");
            let h = hash(&s);
            repo.upsert("text", Some(&s), None, None, None, None, s.len() as i64, &h)
                .unwrap();
        }
        let list = repo.list().unwrap();
        assert_eq!(list.len(), HISTORY_CAP + 5);
        for id in pinned_ids {
            assert!(repo.get(id).unwrap().is_some(), "pinned {id} survived");
        }
    }

    #[test]
    fn toggle_pin_flips() {
        let repo = temp_repo();
        let (id, _) = repo
            .upsert("text", Some("x"), None, None, None, None, 1, &hash("x"))
            .unwrap();
        assert_eq!(repo.toggle_pin(id).unwrap(), true);
        assert_eq!(repo.toggle_pin(id).unwrap(), false);
    }
}
