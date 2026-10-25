//! The connection task: owns the WebSocket and the [`SyncClient`] state
//! machine, and is the only place either is touched.
//!
//! `SyncClient` is a single-threaded state machine returning *effects*; this
//! task is the native adapter that executes them, exactly as `wasm-edge` does
//! in the browser. Reads never come through here — they hit the shared
//! `Arc<KeyValueStore>` directly, with no channel and no lock.

use crate::error::{Error, Result};
use core_engine::resp::Value;
use core_engine::store::KeyValueStore;
use futures_util::stream::SplitSink;
use futures_util::{SinkExt, StreamExt};
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::time::Duration;
use sync_client::{
    EnqueueError, Enqueued, Incoming, SubscriptionState, SyncClient, SyncStatus, to_resp,
};
use tokio::net::TcpStream;
use tokio::sync::{broadcast, mpsc, oneshot, watch};
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};

/// Buffered messages per pub/sub channel before a slow subscriber starts
/// losing them. Lag is surfaced to the subscriber by `broadcast::Receiver`.
const PUBSUB_BUFFER: usize = 256;

/// How often the local store drops entries whose TTL has passed and, if a cap
/// is configured, evicts down to it.
///
/// The server sweeps its own keyspace on the same cadence. Without a sweep
/// here an expired entry is only *masked* on read, never removed, so a
/// long-lived process accumulates dead keys for as long as it runs.
const MAINTENANCE_INTERVAL: Duration = Duration::from_secs(1);

type Ws = WebSocketStream<MaybeTlsStream<TcpStream>>;
type WsWrite = SplitSink<Ws, Message>;

pub(crate) type Ack = oneshot::Sender<Result<Value>>;

/// Work the connection task performs on behalf of a [`Cache`](crate::Cache)
/// handle. Everything that touches the socket arrives as one of these.
pub(crate) enum Op {
    /// A durable mutation: goes through the outbox and is replayed on
    /// reconnect until the server acknowledges it.
    Write { encoded: Vec<u8>, ack: Option<Ack> },
    /// A one-shot round-trip that must never be replayed (read-through `GET`).
    Read { frame: Vec<u8>, ack: Ack },
    /// Register a live query and resolve once its `qstate` has been applied.
    Watch { pattern: String, ack: Ack },
    /// Drop a live query.
    Unwatch { pattern: String, ack: Ack },
    Subscribe {
        channel: String,
        ack: oneshot::Sender<Result<broadcast::Receiver<Vec<u8>>>>,
    },
}

/// State a `Cache` handle reads without going through the task.
pub(crate) struct Shared {
    pub(crate) connected: AtomicBool,
    pub(crate) outbox_len: AtomicUsize,
    /// Writes evicted from a full outbox under the legacy
    /// [`OverflowPolicy::EvictOldest`](sync_client::OverflowPolicy), cumulative.
    /// Never resets, so a caller polling it can tell "none yet" from "none
    /// since I last looked".
    pub(crate) dropped_writes: AtomicU64,
    /// The synchronization status, published on every change.
    pub(crate) status: watch::Sender<SyncStatus>,
    /// Each live query's progress, as last reported.
    pub(crate) subscriptions: std::sync::Mutex<HashMap<String, SubscriptionState>>,
}

pub(crate) struct Task {
    client: SyncClient,
    shared: Arc<Shared>,
    url: String,
    /// One slot per frame written to the current socket, in send order.
    ///
    /// This mirrors `SyncClient`'s own inflight FIFO: the protocol guarantees
    /// exactly one reply per command in send order, so the front slot always
    /// belongs to the next reply. `None` marks a frame nobody is awaiting.
    /// Every push here must pair with a frame the client also counted, or
    /// replies would be handed to the wrong caller.
    waiters: VecDeque<Option<Ack>>,
    /// Pub/sub fan-out. `SyncClient` does not track subscriptions (the browser
    /// SDK re-subscribes itself), so the adapter owns re-subscription.
    channels: HashMap<String, broadcast::Sender<Vec<u8>>>,
    first_open: Option<oneshot::Sender<Result<()>>>,
    /// Held for the periodic sweep. Reads do not come through here.
    store: Arc<KeyValueStore>,
    connect_timeout: Duration,
    /// Keys of refused writes whose live queries must re-snapshot.
    resync: Vec<String>,
}

impl Task {
    pub(crate) fn new(
        client: SyncClient,
        shared: Arc<Shared>,
        url: String,
        first_open: oneshot::Sender<Result<()>>,
        connect_timeout: Duration,
    ) -> Self {
        let store = Arc::clone(client.store());
        Self {
            client,
            shared,
            url,
            waiters: VecDeque::new(),
            channels: HashMap::new(),
            first_open: Some(first_open),
            store,
            connect_timeout,
            resync: Vec::new(),
        }
    }

    /// Publish whatever the client's status and live queries did since the
    /// last call. Called after every event that can move them.
    fn publish(&mut self) {
        if let Some(status) = self.client.take_status_change() {
            self.shared.status.send_replace(status);
        }
        let changes = self.client.take_subscription_changes();
        if !changes.is_empty() {
            let mut subs = self
                .shared
                .subscriptions
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            for (pattern, state) in changes {
                match state {
                    Some(state) => {
                        subs.insert(pattern, state);
                    }
                    None => {
                        subs.remove(&pattern);
                    }
                }
            }
        }
    }

    /// Drop expired entries and evict down to any configured cap.
    fn maintain(&self) {
        self.store.sweep_expired();
        self.store.try_evict_for_memory();
    }

    pub(crate) async fn run(mut self, mut ops: mpsc::Receiver<Op>) {
        loop {
            self.client.on_connecting();
            self.publish();
            // A TCP connect to a black-holed address otherwise hangs for the
            // OS timeout — around two minutes — which the caller sees as
            // `connect()` never returning.
            let attempt = tokio::time::timeout(
                self.connect_timeout,
                tokio_tungstenite::connect_async(self.url.as_str()),
            )
            .await
            .unwrap_or_else(|_| {
                Err(tokio_tungstenite::tungstenite::Error::Io(
                    std::io::ErrorKind::TimedOut.into(),
                ))
            });
            match attempt {
                Ok((ws, _)) => {
                    if let Some(tx) = self.first_open.take() {
                        let _ = tx.send(Ok(()));
                    }
                    if self.serve(ws, &mut ops).await {
                        return; // every Cache handle dropped
                    }
                }
                Err(e) => {
                    // The first attempt is the caller's `connect().await`. Fail
                    // it loudly rather than retrying behind their back — a
                    // typo'd URL should not look like a slow start-up.
                    if let Some(tx) = self.first_open.take() {
                        let _ = tx.send(Err(Error::Connect(e.to_string())));
                        return;
                    }
                }
            }

            self.shared.connected.store(false, Ordering::Release);
            self.fail_waiters(Error::Disconnected);

            if self.backoff(&mut ops).await {
                return;
            }
        }
    }

    /// Serve one connection. Returns true when the caller should stop entirely
    /// (all `Cache` handles dropped), false to reconnect.
    async fn serve(&mut self, ws: Ws, ops: &mut mpsc::Receiver<Op>) -> bool {
        let (mut write, mut read) = ws.split();

        self.waiters.clear();
        self.shared.connected.store(true, Ordering::Release);

        // Session re-establishment: AUTH -> SYNC -> QSUB -> outbox replay, in
        // that order. The client counts each of these as inflight itself, so
        // the waiter slots here are pure padding to stay aligned.
        let frames = self.client.on_open();
        self.publish();
        for frame in frames {
            self.waiters.push_back(None);
            if !send(&mut write, frame).await {
                return false;
            }
        }
        // Pub/sub is not part of on_open — re-subscribe here.
        let channels: Vec<String> = self.channels.keys().cloned().collect();
        for channel in channels {
            if let Some(frame) = self
                .client
                .session_command(to_resp(&["SUBSCRIBE", &channel]), true)
            {
                self.waiters.push_back(None);
                if !send(&mut write, frame).await {
                    return false;
                }
            }
        }

        let mut upkeep = tokio::time::interval(MAINTENANCE_INTERVAL);
        upkeep.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

        loop {
            tokio::select! {
                _ = upkeep.tick() => self.maintain(),
                incoming = read.next() => {
                    match incoming {
                        Some(Ok(msg)) => {
                            let alive = self.on_message(msg);
                            // Frames the client asked for in response, each
                            // holding a reply slot nobody awaits.
                            let extra = self.take_extra_frames(alive);
                            self.publish();
                            if !alive {
                                return false; // server closed
                            }
                            for frame in extra {
                                self.waiters.push_back(None);
                                if !send(&mut write, frame).await {
                                    return false;
                                }
                            }
                        }
                        // A transport error or clean end of stream both mean
                        // this socket is finished; reconnect handles the rest.
                        Some(Err(_)) | None => return false,
                    }
                }
                op = ops.recv() => {
                    match op {
                        Some(op) => {
                            if !self.on_op(op, &mut write).await {
                                return false;
                            }
                        }
                        None => return true,
                    }
                }
            }
        }
    }

    /// Apply one incoming frame. Returns false when the socket is done.
    fn on_message(&mut self, msg: Message) -> bool {
        let raw = match msg {
            Message::Binary(b) => b.to_vec(),
            Message::Text(t) => t.as_bytes().to_vec(),
            Message::Close(_) => return false,
            // Ping/Pong are answered by tungstenite itself.
            _ => return true,
        };

        // Parsed twice on purpose: `handle_frame` classifies and applies but
        // discards the reply payload, and read-through needs that payload.
        // Replies are the rare path, so the second parse is not worth
        // restructuring the client's API for.
        let parsed = Value::parse(&raw).ok().map(|(v, _)| v);

        match self.client.handle_frame(&raw) {
            // A refused write resolves its caller with the server's error.
            Incoming::Reply { .. } | Incoming::AppliedReply { .. } => {
                if let Some(Some(ack)) = self.waiters.pop_front() {
                    let _ = ack.send(reply_result(parsed));
                }
            }
            Incoming::Refused {
                keys, unreconciled, ..
            } => {
                if let Some(Some(ack)) = self.waiters.pop_front() {
                    let _ = ack.send(reply_result(parsed));
                }
                if !unreconciled.is_empty() {
                    tracing::warn!(
                        ?unreconciled,
                        "recached: a refused write's keys are not covered by any live query; \
                         their local copies are not corrected"
                    );
                }
                // The live queries covering the keys re-snapshot, so the local
                // copy shows the server's values, not the refused write.
                self.resync.extend(keys);
            }
            Incoming::PubSub { channel, message } => {
                if let Some(tx) = self.channels.get(&channel) {
                    // Err only means nobody is listening right now.
                    let _ = tx.send(message);
                }
            }
            // A push applied to the local store, or a frame we do not model.
            // Neither consumes a reply slot.
            Incoming::Applied { .. } | Incoming::Ignored => {}
            // A frame we could not parse may have been a reply the server has
            // already counted, in which case both FIFOs are now one ahead of
            // reality and every later reply resolves the wrong caller and
            // retires the wrong outbox row. That cannot be repaired in place,
            // so drop the socket: the reconnect rebuilds both from a cleared
            // FIFO, and the outbox replays whatever was still queued.
            // A write refused for a reason that can clear stays queued for the
            // reconnect; its caller hears `Disconnected`, as for any write
            // still unacknowledged when a socket goes.
            Incoming::Deferred { reason } => {
                tracing::warn!(
                    "recached: the server deferred a write ({reason}); reconnecting to replay it"
                );
                return false;
            }
            Incoming::Malformed => {
                tracing::warn!(
                    "recached: unparseable frame from the server; dropping the socket to \
                     resynchronise replies"
                );
                return false;
            }
        }

        self.shared
            .outbox_len
            .store(self.client.outbox_len(), Ordering::Release);
        true
    }

    /// Frames to send after a frame was handled: re-subscriptions after a
    /// refusal, and any the client queued itself.
    fn take_extra_frames(&mut self, alive: bool) -> Vec<Vec<u8>> {
        if !alive {
            self.resync.clear();
            return Vec::new();
        }
        let keys = std::mem::take(&mut self.resync);
        let mut frames = self.client.resubscribe_covering(&keys, true);
        frames.extend(self.client.take_frames_to_send(true));
        frames
    }

    /// Queue a write, or refuse it with the reason. `connected` says whether
    /// the socket is up.
    fn enqueue(&mut self, encoded: &[u8], connected: bool) -> std::result::Result<Enqueued, Error> {
        let queued = self
            .client
            .try_enqueue_write(encoded, true, connected)
            .map_err(|e| match e {
                EnqueueError::OutboxFull { pending, capacity } => {
                    tracing::warn!(
                        pending,
                        capacity,
                        "recached: pending-write queue full — refused a new write"
                    );
                    Error::OutboxFull { pending, capacity }
                }
                // A process holds one session for its whole life; four billion
                // writes in it is not reachable in practice.
                EnqueueError::WireIdsExhausted => Error::OutboxFull {
                    pending: self.client.outbox_len(),
                    capacity: self.client.max_pending(),
                },
            });
        self.shared
            .outbox_len
            .store(self.client.outbox_len(), Ordering::Release);
        queued
    }

    /// Perform one op against a live socket. Returns false if the socket died.
    async fn on_op(&mut self, op: Op, write: &mut WsWrite) -> bool {
        match op {
            Op::Write { encoded, ack } => {
                let queued = match self.enqueue(&encoded, true) {
                    Ok(queued) => queued,
                    Err(e) => {
                        if let Some(ack) = ack {
                            let _ = ack.send(Err(e));
                        }
                        return true;
                    }
                };
                self.note_dropped(queued.dropped);
                self.shared
                    .outbox_len
                    .store(self.client.outbox_len(), Ordering::Release);
                if !queued.send_now {
                    // This socket is being abandoned; the write replays on the next.
                    if let Some(ack) = ack {
                        let _ = ack.send(Err(Error::Disconnected));
                    }
                    return true;
                }
                self.waiters.push_back(ack);
                send(write, queued.frame).await
            }
            Op::Read { frame, ack } => match self.client.session_command(frame, true) {
                Some(frame) => {
                    self.waiters.push_back(Some(ack));
                    send(write, frame).await
                }
                None => {
                    let _ = ack.send(Err(Error::Disconnected));
                    true
                }
            },
            Op::Watch { pattern, ack } => {
                let frame = self.client.add_live_query(&pattern, true);
                self.publish();
                match frame {
                    Some(frame) => {
                        self.waiters.push_back(Some(ack));
                        send(write, frame).await
                    }
                    None => {
                        let _ = ack.send(Err(Error::Disconnected));
                        true
                    }
                }
            }
            Op::Unwatch { pattern, ack } => {
                let frame = self.client.remove_live_query(Some(&pattern), true);
                self.publish();
                match frame {
                    Some(frame) => {
                        self.waiters.push_back(Some(ack));
                        send(write, frame).await
                    }
                    None => {
                        let _ = ack.send(Err(Error::Disconnected));
                        true
                    }
                }
            }
            Op::Subscribe { channel, ack } => {
                let rx = match self.channels.get(&channel) {
                    Some(tx) => tx.subscribe(),
                    None => {
                        let (tx, rx) = broadcast::channel(PUBSUB_BUFFER);
                        self.channels.insert(channel.clone(), tx);
                        rx
                    }
                };
                let alive = match self
                    .client
                    .session_command(to_resp(&["SUBSCRIBE", &channel]), true)
                {
                    Some(frame) => {
                        self.waiters.push_back(None);
                        send(write, frame).await
                    }
                    None => true,
                };
                let _ = ack.send(Ok(rx));
                alive
            }
        }
    }

    /// Wait out the reconnect delay, still accepting writes into the durable
    /// outbox. Returns true when every handle has been dropped.
    ///
    /// Without this a caller's `set()` would block for the whole outage
    /// instead of being queued — the outbox exists precisely so it does not
    /// have to.
    async fn backoff(&mut self, ops: &mut mpsc::Receiver<Op>) -> bool {
        let delay = Duration::from_millis(u64::from(self.client.on_close()));
        self.publish();
        let deadline = tokio::time::sleep(delay);
        tokio::pin!(deadline);
        let mut upkeep = tokio::time::interval(MAINTENANCE_INTERVAL);
        upkeep.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

        loop {
            tokio::select! {
                _ = &mut deadline => return false,
                // An outage is exactly when expired entries would otherwise
                // pile up: nothing is arriving to trigger a read, and the
                // sweep is the only thing that ever removes them.
                _ = upkeep.tick() => self.maintain(),
                op = ops.recv() => match op {
                    Some(Op::Write { encoded, ack }) => {
                        // Queued in memory; it replays on the next `on_open`.
                        let result = self.enqueue(&encoded, false);
                        if let Ok(queued) = &result {
                            self.note_dropped(queued.dropped);
                        }
                        if let Some(ack) = ack {
                            let _ = ack.send(Err(match result {
                                Ok(_) => Error::Disconnected,
                                Err(e) => e,
                            }));
                        }
                    }
                    // Everything else needs the network to mean anything.
                    Some(Op::Read { ack, .. })
                    | Some(Op::Watch { ack, .. })
                    | Some(Op::Unwatch { ack, .. }) => {
                        let _ = ack.send(Err(Error::Disconnected));
                    }
                    Some(Op::Subscribe { channel, ack }) => {
                        let rx = self
                            .channels
                            .entry(channel)
                            .or_insert_with(|| broadcast::channel(PUBSUB_BUFFER).0)
                            .subscribe();
                        let _ = ack.send(Ok(rx));
                    }
                    None => return true,
                }
            }
        }
    }

    /// Under the legacy eviction policy a full outbox evicts its oldest entry
    /// to make room. That write is gone — it will never be replayed — so it
    /// has to be visible rather than silently absent.
    fn note_dropped(&self, dropped: Option<u64>) {
        if dropped.is_some() {
            let total = self
                .shared
                .dropped_writes
                .fetch_add(1, Ordering::Relaxed)
                .saturating_add(1);
            tracing::warn!(
                dropped_writes = total,
                max_pending = self.client.max_pending(),
                "recached: pending-write queue full — discarded the oldest queued write"
            );
        }
    }

    fn fail_waiters(&mut self, err: Error) {
        for waiter in self.waiters.drain(..).flatten() {
            let _ = waiter.send(Err(err.clone()));
        }
    }
}

async fn send(write: &mut WsWrite, frame: Vec<u8>) -> bool {
    write.send(Message::Binary(frame.into())).await.is_ok()
}

fn reply_result(parsed: Option<Value>) -> Result<Value> {
    match parsed {
        Some(Value::Error(msg)) => Err(Error::Server(msg)),
        Some(value) => Ok(value),
        None => Err(Error::Server("unparseable reply".to_string())),
    }
}
