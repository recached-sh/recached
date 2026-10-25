//! The synchronization lifecycle, as Kotlin and Swift see it.
//!
//! Mirrors of `sync_client`'s status types, so UniFFI can export them without
//! the shared state machine depending on it.

use sync_client as sc;

/// Where the connection is in establishing synchronization. An open socket
/// is only the start: see [`RecachedClient::sync_status`](crate::RecachedClient::sync_status).
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum SyncPhase {
    /// No socket, or the last one closed.
    Disconnected,
    /// The app is opening a socket
    /// ([`connection_opening`](crate::RecachedClient::connection_opening)).
    Connecting,
    /// `AUTH`, the protocol opt-ins or the sync token are not yet answered.
    Authenticating,
    /// Writes queued while offline have been sent and not all answered.
    Replaying,
    /// The snapshots of the patterns watched at connect time have not all
    /// arrived.
    Hydrating,
    /// Synchronized. Patterns watched later report their own progress
    /// through [`SubscriptionState`].
    Ready,
    /// Open but impaired; [`SyncStatus::error`] says why.
    Degraded,
}

/// Why a watched pattern was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum SubscriptionFailure {
    /// More keys match than the server sends in one snapshot. Nothing local
    /// was reconciled against a partial one.
    SnapshotTooLarge,
    /// Outside the sync token's scope.
    OutOfScope,
    Other,
}

/// A connection-level synchronization problem. `reason` is the server's
/// error text, which never contains a password or token.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum SyncError {
    AuthFailed {
        reason: String,
    },
    ScopeRefused {
        reason: String,
    },
    /// A queued write was refused for a reason that can clear; it stays
    /// queued and replays on the next connection.
    WriteDeferred {
        reason: String,
    },
    SubscriptionFailed {
        pattern: String,
        failure: SubscriptionFailure,
        reason: String,
    },
    /// An incoming frame could not be parsed; the socket must be replaced.
    MalformedFrame,
}

/// The connection's synchronization status.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct SyncStatus {
    pub phase: SyncPhase,
    /// What is wrong, when something is. While disconnected, what ended the
    /// last connection.
    pub error: Option<SyncError>,
}

/// One watched pattern's progress.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum SubscriptionState {
    /// Waiting for a connection; local copies are what the last session left.
    Pending,
    /// Snapshot requested, not yet received.
    Hydrating,
    /// Snapshot applied; changes stream as they happen.
    Current,
    /// Refused by the server; retried on the next connection.
    Failed {
        failure: SubscriptionFailure,
        reason: String,
    },
}

/// Diagnostic counters since [`open`](crate::RecachedClient::open).
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Record)]
pub struct SyncCounters {
    pub connections: u64,
    pub replayed_writes: u64,
    pub deferred_writes: u64,
    pub refused_writes: u64,
    /// Writes refused locally because the outbox was full.
    pub rejected_writes: u64,
    /// Writes evicted under `evict_oldest_when_full`.
    pub evicted_writes: u64,
    pub snapshots_applied: u64,
    pub malformed_frames: u64,
}

impl From<sc::SyncPhase> for SyncPhase {
    fn from(p: sc::SyncPhase) -> Self {
        match p {
            sc::SyncPhase::Disconnected => Self::Disconnected,
            sc::SyncPhase::Connecting => Self::Connecting,
            sc::SyncPhase::Authenticating => Self::Authenticating,
            sc::SyncPhase::Replaying => Self::Replaying,
            sc::SyncPhase::Hydrating => Self::Hydrating,
            sc::SyncPhase::Ready => Self::Ready,
            sc::SyncPhase::Degraded => Self::Degraded,
        }
    }
}

impl From<sc::SubscriptionFailure> for SubscriptionFailure {
    fn from(f: sc::SubscriptionFailure) -> Self {
        match f {
            sc::SubscriptionFailure::SnapshotTooLarge => Self::SnapshotTooLarge,
            sc::SubscriptionFailure::OutOfScope => Self::OutOfScope,
            sc::SubscriptionFailure::Other => Self::Other,
        }
    }
}

impl From<sc::SyncError> for SyncError {
    fn from(e: sc::SyncError) -> Self {
        match e {
            sc::SyncError::AuthFailed { reason } => Self::AuthFailed { reason },
            sc::SyncError::ScopeRefused { reason } => Self::ScopeRefused { reason },
            sc::SyncError::WriteDeferred { reason } => Self::WriteDeferred { reason },
            sc::SyncError::SubscriptionFailed {
                pattern,
                failure,
                reason,
            } => Self::SubscriptionFailed {
                pattern,
                failure: failure.into(),
                reason,
            },
            sc::SyncError::MalformedFrame => Self::MalformedFrame,
        }
    }
}

impl From<sc::SyncStatus> for SyncStatus {
    fn from(s: sc::SyncStatus) -> Self {
        Self {
            phase: s.phase.into(),
            error: s.error.map(Into::into),
        }
    }
}

impl From<sc::SubscriptionState> for SubscriptionState {
    fn from(s: sc::SubscriptionState) -> Self {
        match s {
            sc::SubscriptionState::Pending => Self::Pending,
            sc::SubscriptionState::Hydrating => Self::Hydrating,
            sc::SubscriptionState::Current => Self::Current,
            sc::SubscriptionState::Failed { failure, reason } => Self::Failed {
                failure: failure.into(),
                reason,
            },
        }
    }
}

impl From<sc::SyncCounters> for SyncCounters {
    fn from(c: sc::SyncCounters) -> Self {
        Self {
            connections: c.connections,
            replayed_writes: c.replayed_writes,
            deferred_writes: c.deferred_writes,
            refused_writes: c.refused_writes,
            rejected_writes: c.rejected_writes,
            evicted_writes: c.evicted_writes,
            snapshots_applied: c.snapshots_applied,
            malformed_frames: c.malformed_frames,
        }
    }
}
