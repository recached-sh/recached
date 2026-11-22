//! 6O-C1: reply coalescing yields only where it can gain.
//!
//! Two corrections to 6O-B's cross-connection yield (`reply_coalesce`):
//! - **(A)** a read batch that ran more than one command flushes without
//!   yielding: its replies already leave in one `send(2)`, and the yield only
//!   delayed them (the 6O-B P=16 p99 regressions);
//! - **(B)** back-offs with no gain between them double, up to
//!   `MAX_BACKOFF`, so a lone connection on a multi-worker runtime rarely
//!   probes (6O-B: +3% CPU per op at one client and three workers). A yield
//!   resumed on another worker stays neutral, as in 6O-B.
//!
//! The 6O-B tests in `reply_coalesce_6ob` still cover the single-command
//! gain, the back-off state machine, a gain clearing the misses, the switch,
//! the overflow arm and the lone connection on one worker.

use super::reply_coalesce_6ob::{first_reply_sees_requests, switch_lock};
use super::request_path::Shared;
use super::*;
use std::future::Future;
use std::task::Poll;

fn encode_frame(args: &[&str]) -> Vec<u8> {
    let mut out = format!("*{}\r\n", args.len()).into_bytes();
    for a in args {
        out.extend_from_slice(format!("${}\r\n{a}\r\n", a.len()).as_bytes());
    }
    out
}

/// `conns` connections, each with `per_conn` pipelined `SET`s of its own
/// keys already waiting: how many of all the keys had been set when each
/// connection's first reply byte left, in the order the replies left.
async fn pipelined_first_reply_sees(conns: usize, per_conn: usize) -> Vec<usize> {
    let mut keys = Vec::new();
    let mut requests = Vec::new();
    for c in 0..conns {
        let mut request = Vec::new();
        for j in 0..per_conn {
            let key = format!("p:{c}:{j}");
            request.extend_from_slice(&encode_frame(&["SET", &key, "v"]));
            keys.push(key);
        }
        requests.push((request, b"+OK\r\n".repeat(per_conn)));
    }
    first_reply_sees_requests(requests, keys, Shared::new(false)).await
}

fn current_thread() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime")
}

// ── (A) multi-command batches ─────────────────────────────────────────────────

#[test]
fn only_a_batch_of_exactly_one_command_is_eligible() {
    assert!(
        !reply_coalesce::batch_eligible(0),
        "nothing ran: no reply to coalesce"
    );
    assert!(reply_coalesce::batch_eligible(1));
    for n in [2, 3, 16, 1_000] {
        assert!(!reply_coalesce::batch_eligible(n), "{n} commands");
    }
}

/// Each connection's two pipelined `SET`s run and their replies leave at
/// once, before the next connection runs anything. With 6O-B's yield, every
/// reply waited for every connection: `[6, 6, 6]`.
#[test]
fn a_batch_of_several_commands_flushes_without_yielding() {
    let _g = switch_lock();
    current_thread().block_on(async {
        reply_coalesce::reset();
        let seen = pipelined_first_reply_sees(3, 2).await;
        assert_eq!(
            seen,
            vec![2, 4, 6],
            "each batch's replies left right after it ran"
        );
        reply_coalesce::reset();
    });
}

/// P=16 on several connections: each batch's sixteen replies arrive whole
/// and in order (checked by the harness) and leave right after the batch.
/// The back-off state is untouched: a multi-command batch neither yields nor
/// scores anything.
#[test]
fn pipelined_batches_on_several_connections_reply_in_order_without_yielding() {
    let _g = switch_lock();
    current_thread().block_on(async {
        reply_coalesce::reset();
        let backoffs = reply_coalesce::backoffs();
        for pass in 0..3 {
            let seen = pipelined_first_reply_sees(4, 16).await;
            assert_eq!(seen, vec![16, 32, 48, 64], "pass {pass}");
        }
        assert_eq!(reply_coalesce::state(), (0, 0), "nothing was scored");
        assert_eq!(reply_coalesce::backoffs(), backoffs);
    });
}

/// Single-command batches still coalesce when a pipelined connection is
/// ready alongside them: the pipelined one flushes as soon as it ran, the
/// single ones wait for the pass and see every command of it.
#[test]
fn single_command_batches_still_coalesce_next_to_a_pipelined_one() {
    let _g = switch_lock();
    current_thread().block_on(async {
        reply_coalesce::reset();
        let mut keys = Vec::new();
        let mut requests = Vec::new();
        for c in 0..2 {
            let key = format!("one:{c}");
            requests.push((encode_frame(&["SET", &key, "v"]), b"+OK\r\n".to_vec()));
            keys.push(key);
        }
        let mut pipelined = Vec::new();
        for j in 0..8 {
            let key = format!("many:{j}");
            pipelined.extend_from_slice(&encode_frame(&["SET", &key, "v"]));
            keys.push(key);
        }
        requests.push((pipelined, b"+OK\r\n".repeat(8)));
        let seen = first_reply_sees_requests(requests, keys, Shared::new(false)).await;
        // Replies leave in this order: the pipelined connection's (it did
        // not yield), then the two single ones, resumed after the pass.
        assert_eq!(seen, vec![10, 10, 10]);
        reply_coalesce::reset();
    });
}

/// A transaction pipelined in one read batch (`MULTI`, `SET`, `EXEC`) is a
/// multi-command batch: its replies leave at once and are exactly the
/// transaction's. With the yield, every connection's first reply saw all
/// three keys.
#[test]
fn a_pipelined_transaction_flushes_without_yielding_and_replies_exactly() {
    let _g = switch_lock();
    current_thread().block_on(async {
        reply_coalesce::reset();
        let mut keys = Vec::new();
        let mut requests = Vec::new();
        for c in 0..3 {
            let key = format!("tx:{c}");
            let mut request = encode_frame(&["MULTI"]);
            request.extend_from_slice(&encode_frame(&["SET", &key, "v"]));
            request.extend_from_slice(&encode_frame(&["EXEC"]));
            requests.push((request, b"+OK\r\n+QUEUED\r\n*1\r\n+OK\r\n".to_vec()));
            keys.push(key);
        }
        let seen = first_reply_sees_requests(requests, keys, Shared::new(false)).await;
        // The replies reach the socket at the batch's flush, after its
        // `EXEC` ran: each connection sees its own key and those of the
        // connections before it, not the ones after.
        assert_eq!(seen, vec![1, 2, 3]);
        reply_coalesce::reset();
    });
}

/// A DEDUP envelope is one frame and one command: a batch of one still
/// coalesces, and the replies are the wrapped command's.
#[test]
fn a_single_dedup_write_per_batch_still_coalesces() {
    let _g = switch_lock();
    current_thread().block_on(async {
        reply_coalesce::reset();
        let mut keys = Vec::new();
        let mut requests = Vec::new();
        for c in 0..3 {
            let key = format!("dd:{c}");
            let client = format!("client-{c}");
            requests.push((
                encode_frame(&["DEDUP", &client, "1", "SET", &key, "v"]),
                b"+OK\r\n".to_vec(),
            ));
            keys.push(key);
        }
        let seen = first_reply_sees_requests(requests, keys, Shared::new(false)).await;
        assert_eq!(seen, vec![3, 3, 3], "each reply waited for the pass");
        reply_coalesce::reset();
    });
}

/// A pipeline split across reads (6O-C review): the first read runs fifteen
/// commands and ends inside the sixteenth frame, the second read completes
/// only that frame. That batch ran one command, but it is the pipeline's
/// tail and its client is waiting for it: no yield. Nor does a read that
/// runs one command and stops inside the next frame. Control: one large
/// command split the same way is a lone command, and does yield.
#[test]
fn the_tail_of_a_pipeline_split_across_reads_does_not_yield() {
    let _g = switch_lock();
    current_thread().block_on(async {
        let value = "v".repeat(512);
        let mut pipeline = Vec::new();
        for j in 0..16 {
            pipeline.extend_from_slice(&encode_frame(&["SET", &format!("tail:{j}"), &value]));
        }
        let single = encode_frame(&["SET", "lone", &value]);
        let first = encode_frame(&["SET", "lead:0", &value]);
        let mut leading = first.clone();
        leading.extend_from_slice(&encode_frame(&["SET", "lead:1", &value]));
        let tail_cut = pipeline.len() - 100;
        let single_cut = single.len() - 100;
        // A leading split: the first read is frame 1 and a few bytes of
        // frame 2 (it leaves part of a frame), the second completes frame 2
        // (the tail). Neither yields.
        let lead_cut = first.len() + 5;
        for (request, cut, replies, want_yields) in [
            (pipeline, tail_cut, 16, 0),
            (leading, lead_cut, 2, 0),
            (single, single_cut, 1, 1),
        ] {
            reply_coalesce::reset();
            let shared = Shared::new(false);
            let (mut client, server) = tokio::io::duplex(1 << 20);
            let task = tokio::spawn(shared.serve(server));
            let yields = reply_coalesce::yields();
            client.write_all(&request[..cut]).await.unwrap();
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
            client.write_all(&request[cut..]).await.unwrap();
            let mut got = vec![0u8; 5 * replies];
            client.read_exact(&mut got).await.unwrap();
            assert_eq!(got, b"+OK\r\n".repeat(replies));
            assert_eq!(
                reply_coalesce::yields() - yields,
                want_yields,
                "{replies} command(s) split across two reads"
            );
            drop(client);
            let _ = task.await;
        }
        reply_coalesce::reset();
    });
}

/// A read batch that completes no frame is not another connection's company
/// (6O-C review): a connection yielding while another one only receives part
/// of a frame scores a miss, not a gain.
#[test]
fn a_batch_that_ran_nothing_is_not_a_gain_for_another_connections_yield() {
    let _g = switch_lock();
    current_thread().block_on(async {
        reply_coalesce::reset();
        let shared = Shared::new(false);
        let (mut a, a_server) = tokio::io::duplex(1 << 16);
        let (mut b, b_server) = tokio::io::duplex(1 << 16);
        a.write_all(&encode_frame(&["SET", "alone", "v"]))
            .await
            .unwrap();
        let partial = encode_frame(&["SET", "partial", "v"]);
        b.write_all(&partial[..partial.len() - 3]).await.unwrap();
        // `a` runs its command and yields; `b` reads part of a frame and runs
        // nothing; `a` resumes.
        let a_task = tokio::spawn(shared.serve(a_server));
        let b_task = tokio::spawn(shared.serve(b_server));
        let mut reply = [0u8; 5];
        a.read_exact(&mut reply).await.unwrap();
        assert_eq!(&reply, b"+OK\r\n");
        assert_eq!(
            reply_coalesce::state(),
            (0, 1),
            "a miss: nobody ran a command"
        );
        b.write_all(&partial[partial.len() - 3..]).await.unwrap();
        b.read_exact(&mut reply).await.unwrap();
        assert_eq!(&reply, b"+OK\r\n");
        drop((a, b));
        let _ = a_task.await;
        let _ = b_task.await;
        reply_coalesce::reset();
    });
}

// ── (B) a resume on another worker ────────────────────────────────────────────

/// A thread that scores yields begun elsewhere, standing in for a worker
/// that stole the yielded task.
struct OtherWorker {
    tx: std::sync::mpsc::Sender<reply_coalesce::Probe>,
    rx: std::sync::mpsc::Receiver<(bool, (u32, u32), u64)>,
    handle: std::thread::JoinHandle<()>,
}

impl OtherWorker {
    fn start() -> OtherWorker {
        let (tx, probes) = std::sync::mpsc::channel::<reply_coalesce::Probe>();
        let (results, rx) = std::sync::mpsc::channel();
        let handle = std::thread::spawn(move || {
            reply_coalesce::reset();
            for probe in probes {
                let gained = reply_coalesce::end(probe);
                let _ = results.send((
                    gained,
                    reply_coalesce::state(),
                    reply_coalesce::migrations(),
                ));
            }
        });
        OtherWorker { tx, rx, handle }
    }

    /// Score `probe` on the other thread: whether it gained, that thread's
    /// `(skip, misses)` afterwards, and its migrations so far.
    fn end(&self, probe: reply_coalesce::Probe) -> (bool, (u32, u32), u64) {
        self.tx.send(probe).unwrap();
        self.rx.recv().unwrap()
    }

    fn stop(self) {
        drop(self.tx);
        self.handle.join().unwrap();
    }
}

/// A yield resumed on another worker is neutral (6O-C review): no gain, no
/// miss, on either worker. Scored as a miss, the stolen tasks of a busy
/// worker — which did gain there — backed their thief off.
#[test]
fn a_yield_resumed_on_another_worker_changes_nothing_on_either() {
    let _g = switch_lock();
    reply_coalesce::reset();
    let other = OtherWorker::start();

    let probe = reply_coalesce::begin().expect("a fresh worker yields");
    let (gained, state, migrations) = other.end(probe);
    assert!(!gained, "a resume elsewhere is not a gain");
    assert_eq!(state, (0, 0), "nor a miss on the resuming worker");
    assert_eq!(migrations, 1);

    // Many in a row never back the thief off.
    for n in 2..=8 {
        let probe = reply_coalesce::begin().expect("the origin is not backed off");
        let (gained, state, migrations) = other.end(probe);
        assert!(!gained);
        assert_eq!(state, (0, 0));
        assert_eq!(migrations, n);
    }
    assert_eq!(
        reply_coalesce::state(),
        (0, 0),
        "the origin's state is unchanged"
    );
    other.stop();
    reply_coalesce::reset();
}

/// A migration between two misses does not break the run: misses on this
/// worker still back it off, and a real gain still clears them.
#[test]
fn a_migration_between_two_misses_neither_breaks_nor_adds_to_the_run() {
    let _g = switch_lock();
    reply_coalesce::reset();
    let probe = reply_coalesce::begin().unwrap();
    assert!(!reply_coalesce::end(probe));
    assert_eq!(reply_coalesce::state(), (0, 1));
    // This thread resumes a yield begun on another one.
    let (probes_tx, probes_rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        reply_coalesce::reset();
        probes_tx.send(reply_coalesce::begin().unwrap()).unwrap();
    })
    .join()
    .unwrap();
    assert!(!reply_coalesce::end(probes_rx.recv().unwrap()));
    assert_eq!(reply_coalesce::state(), (0, 1), "neutral");
    let probe = reply_coalesce::begin().unwrap();
    reply_coalesce::note_batch(u64::MAX);
    assert!(reply_coalesce::end(probe), "a real gain");
    assert_eq!(reply_coalesce::state(), (0, 0), "the gain cleared the miss");
    reply_coalesce::reset();
}

/// Back-offs with no gain between them double, up to `MAX_BACKOFF`; a gain
/// starts over at `BACKOFF`.
#[test]
fn back_offs_with_no_gain_between_them_double_up_to_the_maximum() {
    let _g = switch_lock();
    reply_coalesce::reset();
    let miss_twice = || {
        for _ in 0..reply_coalesce::MISSES {
            let probe = reply_coalesce::begin().expect("probing");
            assert!(!reply_coalesce::end(probe));
        }
        let (skip, _) = reply_coalesce::state();
        for _ in 0..skip {
            assert!(reply_coalesce::begin().is_none(), "backed off");
        }
        skip
    };
    let mut periods = Vec::new();
    for _ in 0..8 {
        periods.push(miss_twice());
    }
    assert_eq!(periods, vec![32, 64, 128, 256, 512, 1024, 1024, 1024]);
    let probe = reply_coalesce::begin().unwrap();
    reply_coalesce::note_batch(u64::MAX);
    assert!(reply_coalesce::end(probe));
    assert_eq!(miss_twice(), reply_coalesce::BACKOFF, "a gain starts over");
    reply_coalesce::reset();
}

/// A lone connection on a three-worker runtime, its client on a thread of
/// its own: nobody else ever runs a batch, so no yield can gain. Wherever
/// each yield resumes, the workers back off, and with the doubling back-off
/// the yields are a few dozen in 1,200 flushes (6O-B's fixed back-off gave
/// about 74).
#[test]
fn a_lone_connection_on_several_workers_backs_off() {
    let _g = switch_lock();
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(3)
        .thread_name(reply_coalesce::tally::PREFIX)
        .enable_all()
        .build()
        .expect("runtime");
    let server = rt.block_on(spawn_server());
    let addr = server.tcp_addr;
    const ROUND_TRIPS: u64 = 1_200;
    let before = reply_coalesce::tally::snapshot();
    // The client runs on this (unnamed) thread, so it is neither counted
    // nor a task on the server's workers.
    current_thread().block_on(async {
        let mut c = RespClient::connect(addr).await;
        for i in 0..ROUND_TRIPS {
            let key = format!("lone-mw:{i}");
            assert_eq!(
                c.cmd(&["SET", &key, "v"]).await,
                Value::SimpleString("OK".into())
            );
        }
    });
    let after = reply_coalesce::tally::snapshot();
    let (yields, migrations, backoffs) =
        (after.0 - before.0, after.1 - before.1, after.2 - before.2);
    println!(
        "lone connection, 3 workers: {ROUND_TRIPS} flushes, {yields} yields, {migrations} resumed elsewhere, {backoffs} back-offs"
    );
    assert!(backoffs > 0, "the lone connection's workers backed off");
    // No yield can gain and none is neutral unless it migrated, so each
    // worker backs off after every two of its own yields, for 32, 64, ...,
    // 1,024 flushes: 2 * 6 yields cover the first 2,000 flushes it begins,
    // whatever the split between workers.
    let bound = 3 * 2 * 6 + 2 * migrations;
    assert!(
        yields <= bound,
        "{yields} yields for {ROUND_TRIPS} flushes (bound {bound}; {migrations} migrated)"
    );
    drop(server);
    rt.shutdown_background();
}

/// P=16 on a three-worker runtime: no batch of sixteen yields, every reply
/// arrives in order.
#[test]
fn pipelined_batches_on_several_workers_never_yield() {
    let _g = switch_lock();
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(3)
        .thread_name(reply_coalesce::tally::PREFIX)
        .enable_all()
        .build()
        .expect("runtime");
    let before = reply_coalesce::tally::snapshot();
    rt.block_on(async {
        let shared = Arc::new(Shared::new(false));
        let mut clients = Vec::new();
        for c in 0..6 {
            let shared = Arc::clone(&shared);
            clients.push(tokio::spawn(async move {
                let (mut client, server) = tokio::io::duplex(1 << 20);
                let server = tokio::spawn(shared.serve(server));
                for round in 0..40 {
                    let mut request = Vec::new();
                    for j in 0..16 {
                        request.extend_from_slice(&encode_frame(&["INCR", &format!("mw:{c}:{j}")]));
                    }
                    // One write: the server reads the whole batch at once.
                    client.write_all(&request).await.unwrap();
                    let mut want = Vec::new();
                    for _ in 0..16 {
                        want.extend_from_slice(format!(":{}\r\n", round + 1).as_bytes());
                    }
                    let mut got = vec![0u8; want.len()];
                    client.read_exact(&mut got).await.unwrap();
                    assert_eq!(got, want, "connection {c} round {round}");
                }
                drop(client);
                let _ = server.await;
            }));
        }
        for c in clients {
            c.await.unwrap();
        }
    });
    let after = reply_coalesce::tally::snapshot();
    assert_eq!(after.0 - before.0, 0, "no batch of sixteen yielded");
    rt.shutdown_background();
}

/// The back-off escalates only for a worker that served one connection
/// since its last back-off began (6O-C review): with company, its yields may
/// still gain, so it keeps 6O-B's fixed back-off rather than settling at the
/// maximum.
#[test]
fn a_worker_with_company_does_not_escalate_its_back_off() {
    let _g = switch_lock();
    reply_coalesce::reset();
    let back_off = |batches_by: &[u64]| {
        for _ in 0..reply_coalesce::MISSES {
            let probe = reply_coalesce::begin().expect("probing");
            assert!(!reply_coalesce::end(probe));
        }
        let (skip, _) = reply_coalesce::state();
        for conn in batches_by {
            reply_coalesce::note_batch(*conn);
        }
        for _ in 0..skip {
            assert!(reply_coalesce::begin().is_none());
        }
        skip
    };
    // Two connections ran batches during each back-off: it stays at 32.
    assert_eq!(back_off(&[1, 2]), 32);
    assert_eq!(back_off(&[1, 2]), 32);
    assert_eq!(back_off(&[1, 2]), 32);
    // From now on only connection 3: the first change of connection counts
    // once, then it doubles.
    assert_eq!(back_off(&[3, 3]), 32);
    assert_eq!(back_off(&[3, 3]), 32);
    assert_eq!(back_off(&[3]), 64);
    assert_eq!(back_off(&[3]), 128);
    reply_coalesce::reset();
}

// ── cancellation and shutdown ─────────────────────────────────────────────────

/// A flush's yield dropped while pending (the connection's task cancelled
/// mid-yield) leaves the worker's state consistent: nothing was scored, and
/// the next flush probes as usual.
#[test]
fn a_yield_cancelled_while_pending_scores_nothing() {
    let _g = switch_lock();
    current_thread().block_on(async {
        reply_coalesce::reset();
        let mut fut = Box::pin(reply_coalesce::before_flush());
        let first = std::future::poll_fn(|cx| Poll::Ready(fut.as_mut().poll(cx))).await;
        assert!(first.is_pending(), "the flush is yielded");
        drop(fut);
        assert_eq!(reply_coalesce::state(), (0, 0), "nothing scored");
        let probe = reply_coalesce::begin().expect("the next flush probes");
        assert!(!reply_coalesce::end(probe));
        assert_eq!(reply_coalesce::state(), (0, 1));
        reply_coalesce::reset();
    });
}

/// A connection task aborted while its flush is yielded: its command ran,
/// its reply never left, its socket closes, and the other connection of the
/// pass is answered.
#[test]
fn a_connection_aborted_during_its_yield_never_replies_and_others_are_answered() {
    let _g = switch_lock();
    current_thread().block_on(async {
        reply_coalesce::reset();
        let shared = Shared::new(false);
        let (mut a, a_server) = tokio::io::duplex(1 << 16);
        let (mut b, b_server) = tokio::io::duplex(1 << 16);
        a.write_all(&encode_frame(&["SET", "abort:a", "v"]))
            .await
            .unwrap();
        b.write_all(&encode_frame(&["SET", "abort:b", "v"]))
            .await
            .unwrap();
        let a_task = tokio::spawn(shared.serve(a_server));
        let b_task = tokio::spawn(shared.serve(b_server));
        // After the park that runs the two connections (each runs its
        // command and yields before its flush), a current-thread `block_on`
        // polls its own future first: this one resumes before either
        // connection does. (A spawned task would not.)
        tokio::task::yield_now().await;
        assert_ne!(
            shared.store.get_current("abort:a"),
            Value::BulkString(None),
            "a's command ran before its yield"
        );
        a_task.abort();
        let err = a_task.await.expect_err("aborted");
        assert!(err.is_cancelled());
        let mut reply = [0u8; 5];
        b.read_exact(&mut reply).await.unwrap();
        assert_eq!(&reply, b"+OK\r\n", "b was answered");
        let mut rest = Vec::new();
        a.read_to_end(&mut rest).await.unwrap();
        assert!(rest.is_empty(), "a's reply never left: {rest:?}");
        drop(b);
        let _ = b_task.await;
        reply_coalesce::reset();
    });
}

/// A client that sends a command and goes away at once: the server runs it,
/// yields, fails the flush on the closed socket and cleans up, including the
/// watch the command registered.
#[test]
fn a_client_gone_during_the_pending_flush_is_cleaned_up() {
    let _g = switch_lock();
    current_thread().block_on(async {
        reply_coalesce::reset();
        let shared = Shared::new(false);
        let (mut client, server) = tokio::io::duplex(1 << 16);
        client
            .write_all(&encode_frame(&["WATCH", "gone:k"]))
            .await
            .unwrap();
        drop(client);
        let task = tokio::spawn(shared.serve(server));
        tokio::time::timeout(std::time::Duration::from_secs(5), task)
            .await
            .expect("the connection ended")
            .expect("without a panic");
        assert_eq!(
            shared.watch_registry.watched_keys.load(Ordering::Relaxed),
            0,
            "its watch was unregistered"
        );
        reply_coalesce::reset();
    });
}
