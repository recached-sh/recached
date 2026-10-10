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
}

impl fmt::Display for RecachedError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Storage { reason } => write!(f, "storage: {reason}"),
            Self::Command { reason } => f.write_str(reason),
            Self::NotUtf8 { key } => write!(f, "value of {key:?} is not valid UTF-8"),
        }
    }
}

impl std::error::Error for RecachedError {}

impl From<rusqlite::Error> for RecachedError {
    fn from(e: rusqlite::Error) -> Self {
        Self::Storage {
            reason: e.to_string(),
        }
    }
}
