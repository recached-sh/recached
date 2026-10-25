//! Errors the embedded cache can return.

use std::fmt;

/// `Result` with this crate's [`Error`] as its error type.
pub type Result<T> = std::result::Result<T, Error>;

/// Everything the embedded cache can fail with.
///
/// `#[non_exhaustive]`: match with a `_` arm. New variants are expected as the
/// crate grows, and adding one should not be a breaking change.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum Error {
    /// The key is not covered by any live query, so the local store has no
    /// opinion about it.
    ///
    /// This is the error that keeps an embedded cache honest. A plain `None`
    /// here would be indistinguishable from "the key does not exist", and a
    /// service would serve a confidently wrong answer for every key it forgot
    /// to [`watch`](crate::Cache::watch). Either watch a pattern covering the
    /// key, or use [`get_or_fetch`](crate::Cache::get_or_fetch).
    NotHydrated {
        /// The key that was read.
        key: String,
    },

    /// The key holds a collection but was read as a string (or vice versa).
    WrongType {
        /// The key that was read.
        key: String,
    },

    /// The server replied with an error. Carries the server's message.
    Server(String),

    /// The socket is down. Writes are queued in memory and replayed on
    /// reconnect; reads that need the network cannot proceed.
    Disconnected,

    /// The connection task has shut down — the `Cache` handle is dead.
    Closed,

    /// A round-trip did not complete within
    /// [`CacheBuilder::request_timeout`](crate::CacheBuilder::request_timeout).
    Timeout,

    /// The initial connection could not be established. Carries the reason.
    Connect(String),

    /// The write was not queued: `pending` writes already wait for the server
    /// and the queue holds `capacity`
    /// ([`CacheBuilder::max_pending`](crate::CacheBuilder::max_pending)).
    /// Nothing was sent or applied, and every queued write is intact; retry
    /// once the queue drains ([`pending_writes`](crate::Cache::pending_writes)).
    OutboxFull {
        /// Writes waiting for the server.
        pending: usize,
        /// The queue's cap.
        capacity: usize,
    },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::NotHydrated { key } => write!(
                f,
                "key `{key}` is not covered by any live query \
                 (call `watch()` with a matching pattern, or use `get_or_fetch()`)"
            ),
            Error::WrongType { key } => write!(f, "key `{key}` holds a different type"),
            Error::Server(msg) => write!(f, "server error: {msg}"),
            Error::Disconnected => write!(f, "not connected to the recached server"),
            Error::Closed => write!(f, "cache connection task has shut down"),
            Error::Timeout => write!(f, "timed out waiting for the server"),
            Error::Connect(msg) => write!(f, "could not connect: {msg}"),
            Error::OutboxFull { pending, capacity } => write!(
                f,
                "pending-write queue full ({pending} of {capacity}); the write was not queued"
            ),
        }
    }
}

impl std::error::Error for Error {}
