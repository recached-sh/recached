//! Errors the client can return across the FFI boundary.

use std::fmt;

/// Everything a `RecachedClient` call can fail with. Kotlin and Swift see it as
/// an exception / `Error` with these cases.
///
/// The text field is `reason`, not `message`: a Kotlin exception already has a
/// `message`, and UniFFI's generated class would declare it twice.
#[derive(Debug, uniffi::Error)]
pub enum RecachedError {
    /// The on-device database could not be opened, read or written.
    ///
    /// On a write this means the change was applied in memory, and will sync
    /// while the app keeps running, but would not survive a restart.
    Storage { reason: String },

    /// The engine refused the command — a `WRONGTYPE`, an invalid JSON path, a
    /// non-integer counter. Nothing was applied or queued.
    Command { reason: String },

    /// The key holds bytes that are not valid UTF-8, so it has no string form.
    /// Read it with `get` instead.
    NotUtf8 { key: String },

    /// The write was refused before anything changed: `pending` writes wait
    /// for the server and the outbox holds `capacity`
    /// (`ClientConfig.max_pending_writes`). Nothing was applied, stored or
    /// sent, and every queued write is intact. Retry once it drains.
    OutboxFull { pending: u64, capacity: u64 },

    /// This session has used every write id it can carry (four billion).
    /// Nothing was applied; reopen the client to start a new session.
    WriteIdsExhausted,
}

impl fmt::Display for RecachedError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Storage { reason } => write!(f, "storage: {reason}"),
            Self::Command { reason } => f.write_str(reason),
            Self::NotUtf8 { key } => write!(f, "value of {key:?} is not valid UTF-8"),
            Self::OutboxFull { pending, capacity } => write!(
                f,
                "outbox full: {pending} writes pending, capacity {capacity}; nothing was written"
            ),
            Self::WriteIdsExhausted => f.write_str("write ids exhausted; reopen the client"),
        }
    }
}

impl std::error::Error for RecachedError {}

impl From<sync_client::EnqueueError> for RecachedError {
    fn from(e: sync_client::EnqueueError) -> Self {
        match e {
            sync_client::EnqueueError::OutboxFull { pending, capacity } => Self::OutboxFull {
                pending: pending as u64,
                capacity: capacity as u64,
            },
            sync_client::EnqueueError::WireIdsExhausted => Self::WriteIdsExhausted,
        }
    }
}

impl From<rusqlite::Error> for RecachedError {
    fn from(e: rusqlite::Error) -> Self {
        Self::Storage {
            reason: e.to_string(),
        }
    }
}
