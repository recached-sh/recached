//! The on-device database: the local copy of the store, the outbox, and the
//! client's identity.
//!
//! The local copy is kept *materialised* — one row per key holding that key's
//! current [`SnapshotEntry`] — rather than as a log of commands. Every change
//! rewrites only the keys it touched, so nothing ever needs compacting, and a
//! cold start is a single `restore` of whatever rows exist.
//!
//! Each change commits in one transaction: a local write lands together with
//! its outbox row, and a server frame together with the outbox rows its reply
//! retires. There is no moment at which one is on disk without the other.

use core_engine::store::{KeyValueStore, SnapshotEntry};
use rusqlite::{Connection, OptionalExtension, params};

/// Bumped when the schema changes; `open` migrates from older versions.
const SCHEMA_VERSION: i32 = 1;

const SCHEMA: &str = "
    CREATE TABLE IF NOT EXISTS kv (
        key   TEXT PRIMARY KEY NOT NULL,
        entry BLOB NOT NULL
    ) WITHOUT ROWID;
    CREATE TABLE IF NOT EXISTS outbox (
        id    INTEGER PRIMARY KEY,
        frame BLOB NOT NULL
    );
    CREATE TABLE IF NOT EXISTS meta (
        key   TEXT PRIMARY KEY NOT NULL,
        value TEXT NOT NULL
    ) WITHOUT ROWID;
";

pub(crate) struct Db {
    conn: Connection,
}

impl Db {
    pub(crate) fn open(path: &str) -> rusqlite::Result<Self> {
        let conn = Connection::open(path)?;
        // WAL with `synchronous = NORMAL`: a committed transaction survives the
        // app being killed or crashing, which is the guarantee queued writes
        // need. Only an OS crash or power loss can roll back the last few
        // commits, and `FULL` would pay an fsync per streamed server frame to
        // close that window.
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        let version: i32 = conn.pragma_query_value(None, "user_version", |row| row.get(0))?;
        if version > SCHEMA_VERSION {
            return Err(rusqlite::Error::InvalidParameterName(format!(
                "database schema {version} is newer than this library ({SCHEMA_VERSION})"
            )));
        }
        conn.execute_batch(SCHEMA)?;
        conn.pragma_update(None, "user_version", SCHEMA_VERSION)?;
        Ok(Self { conn })
    }

    pub(crate) fn meta_get(&self, key: &str) -> rusqlite::Result<Option<String>> {
        self.conn
            .query_row("SELECT value FROM meta WHERE key = ?1", [key], |row| {
                row.get(0)
            })
            .optional()
    }

    pub(crate) fn meta_put(&self, key: &str, value: &str) -> rusqlite::Result<()> {
        self.conn.execute(
            "INSERT INTO meta (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )?;
        Ok(())
    }

    /// Every persisted key. A row that no longer decodes — written by a future
    /// format, or damaged — is skipped rather than failing the whole open:
    /// this is a cache of server state, and the next live-query snapshot
    /// replaces it.
    pub(crate) fn load_entries(&self) -> rusqlite::Result<Vec<SnapshotEntry>> {
        let mut stmt = self.conn.prepare("SELECT entry FROM kv")?;
        let rows = stmt.query_map([], |row| row.get::<_, Vec<u8>>(0))?;
        let mut entries = Vec::new();
        for bytes in rows {
            if let Ok(entry) = rmp_serde::from_slice::<SnapshotEntry>(&bytes?) {
                entries.push(entry);
            }
        }
        Ok(entries)
    }

    pub(crate) fn load_outbox(&self) -> rusqlite::Result<Vec<(u64, Vec<u8>)>> {
        let mut stmt = self.conn.prepare("SELECT id, frame FROM outbox")?;
        let rows = stmt.query_map([], |row| Ok((row.get::<_, i64>(0)? as u64, row.get(1)?)))?;
        rows.collect()
    }

    /// Replace the whole outbox with renumbered rows, atomically — see
    /// `SyncClient::restore_outbox` for why it cannot be done in place.
    pub(crate) fn replace_outbox(&mut self, rows: &[(u64, u64, Vec<u8>)]) -> rusqlite::Result<()> {
        let tx = self.conn.transaction()?;
        tx.execute("DELETE FROM outbox", [])?;
        {
            let mut insert = tx.prepare("INSERT INTO outbox (id, frame) VALUES (?1, ?2)")?;
            for (_, id, frame) in rows {
                insert.execute(params![*id as i64, frame])?;
            }
        }
        tx.commit()
    }

    /// Persist one change in a single transaction: the current state of every
    /// key in `keys` (deleting the rows of keys that are gone), an outbox row
    /// to add, and outbox rows to remove.
    pub(crate) fn commit(
        &mut self,
        store: &KeyValueStore,
        keys: &[String],
        add: Option<(u64, &[u8])>,
        remove: &[u64],
    ) -> Result<(), crate::RecachedError> {
        let tx = self.conn.transaction()?;
        {
            let mut upsert = tx.prepare_cached(
                "INSERT INTO kv (key, entry) VALUES (?1, ?2)
                 ON CONFLICT(key) DO UPDATE SET entry = excluded.entry",
            )?;
            let mut delete = tx.prepare_cached("DELETE FROM kv WHERE key = ?1")?;
            for key in keys {
                match store.snapshot_key(key) {
                    Some(entry) => {
                        let bytes = rmp_serde::to_vec(&entry).map_err(|e| {
                            crate::RecachedError::Storage {
                                message: format!("encoding {key:?}: {e}"),
                            }
                        })?;
                        upsert.execute(params![key, bytes])?;
                    }
                    None => {
                        delete.execute([key])?;
                    }
                }
            }
            if let Some((id, frame)) = add {
                tx.execute(
                    "INSERT OR REPLACE INTO outbox (id, frame) VALUES (?1, ?2)",
                    params![id as i64, frame],
                )?;
            }
            for id in remove {
                tx.execute("DELETE FROM outbox WHERE id = ?1", [*id as i64])?;
            }
        }
        tx.commit()?;
        Ok(())
    }
}
