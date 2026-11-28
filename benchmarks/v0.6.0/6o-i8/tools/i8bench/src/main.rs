//! 6O-I8 transaction load generator and keyspace verifier. std only.
//!
//! ```text
//! i8bench contend PORT TX_N CLIENTS WRITERS WARMUP_S MEASURE_S SERIES
//! i8bench paced   PORT RATE CONNS TX_N KEYS WARMUP_S MEASURE_S PREFIX
//! i8bench prefill PORT KEYS
//! i8bench verify  PORT KEYS EXPECTED|none
//! ```
//!
//! `contend` is 6O-G's `txbench setn unrelated` with outside writers, the
//! workload of the 6O-I campaign C cell, unchanged: the same key pool
//! (`key:{i:012}`, 100,000 keys), the same per-client seeds, the same
//! transaction (`MULTI`, `TX_N` × `SET key <32 × v>`, `EXEC`, pipelined) and
//! the same writers (`MSET` of 2 keys, `<32 × w>`). What it adds: a warm-up
//! whose transactions are not counted, a measured window with its own
//! latencies, and a once-a-second series (transactions and writer operations
//! completed, worst latency in the second).
//!
//! `paced` offers transactions at a fixed rate (open loop: each connection
//! has a schedule, `CONNS / RATE` apart, and a late transaction is sent as
//! soon as the previous one completes; nothing is skipped), and records
//! offered (due) against accepted (completed) transactions. Connection `c`
//! writes only keys `i` with `i % CONNS == c`, and its transactions are
//! sequential, so the final value of every key is known: the last write of
//! the last acknowledged transaction that touched it. Values carry the
//! connection, the transaction's sequence number and the member index. At
//! the end it writes `PREFIX.expected`: one value per key, `KEYS` lines.
//!
//! `prefill` sets every key `i < KEYS` to `init:<i>`, padded to 32 bytes.
//! `verify` reads every key (pipelined `MGET`s) and every TTL (`PTTL`),
//! compares them with an expected file (or only digests them, `none`), and
//! prints the mismatches and an FNV-1a digest of every key and value.
use std::io::{BufRead, BufReader, BufWriter, Read, Write};
use std::net::TcpStream;
use std::sync::atomic::{AtomicU64, AtomicU8, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
}

const POOL: usize = 100_000;

fn key_pool() -> &'static [String] {
    static POOL_KEYS: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();
    POOL_KEYS.get_or_init(|| (0..POOL).map(|i| format!("key:{i:012}")).collect())
}

fn key_name(i: usize) -> String {
    format!("key:{i:012}")
}

fn enc(out: &mut Vec<u8>, args: &[&[u8]]) {
    out.extend_from_slice(format!("*{}\r\n", args.len()).as_bytes());
    for a in args {
        out.extend_from_slice(format!("${}\r\n", a.len()).as_bytes());
        out.extend_from_slice(a);
        out.extend_from_slice(b"\r\n");
    }
}

fn unix_ms() -> u128 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis()
}

#[derive(PartialEq, Debug)]
enum Reply {
    Ok,
    Nil,
    Error,
}

fn read_line<R: Read>(r: &mut BufReader<R>) -> Vec<u8> {
    let mut line = Vec::with_capacity(32);
    loop {
        let n = r.read_until(b'\n', &mut line).expect("read");
        assert!(n > 0, "connection closed");
        if line.ends_with(b"\r\n") {
            line.truncate(line.len() - 2);
            return line;
        }
    }
}

fn num(line: &[u8]) -> i64 {
    std::str::from_utf8(line).unwrap().parse().unwrap()
}

fn read_reply<R: Read>(r: &mut BufReader<R>) -> Reply {
    let line = read_line(r);
    match line[0] {
        b'+' | b':' => Reply::Ok,
        b'-' => Reply::Error,
        b'$' => {
            let n = num(&line[1..]);
            if n >= 0 {
                let mut buf = vec![0u8; n as usize + 2];
                r.read_exact(&mut buf).unwrap();
            }
            Reply::Ok
        }
        b'*' => {
            let n = num(&line[1..]);
            if n < 0 {
                return Reply::Nil;
            }
            let mut kind = Reply::Ok;
            for _ in 0..n {
                if read_reply(r) == Reply::Error {
                    kind = Reply::Error;
                }
            }
            kind
        }
        other => panic!("bad reply byte {other}"),
    }
}

/// An `MGET` reply: one value per key.
fn read_values<R: Read>(r: &mut BufReader<R>) -> Vec<Option<Vec<u8>>> {
    let line = read_line(r);
    assert_eq!(line[0], b'*', "MGET reply {:?}", String::from_utf8_lossy(&line));
    let n = num(&line[1..]);
    (0..n)
        .map(|_| {
            let line = read_line(r);
            assert_eq!(line[0], b'$');
            let len = num(&line[1..]);
            if len < 0 {
                return None;
            }
            let mut buf = vec![0u8; len as usize + 2];
            r.read_exact(&mut buf).unwrap();
            buf.truncate(len as usize);
            Some(buf)
        })
        .collect()
}

fn read_int<R: Read>(r: &mut BufReader<R>) -> i64 {
    let line = read_line(r);
    assert_eq!(line[0], b':', "integer reply {:?}", String::from_utf8_lossy(&line));
    num(&line[1..])
}

fn connect(port: u16) -> (BufReader<TcpStream>, BufWriter<TcpStream>) {
    let s = TcpStream::connect(("127.0.0.1", port)).expect("connect");
    s.set_nodelay(true).unwrap();
    (
        BufReader::with_capacity(256 * 1024, s.try_clone().unwrap()),
        BufWriter::with_capacity(256 * 1024, s),
    )
}

fn pct(sorted: &[u64], q: f64) -> f64 {
    sorted
        .get(((sorted.len() as f64 - 1.0) * q) as usize)
        .copied()
        .unwrap_or(0) as f64
        / 1e3
}

/// Phase of a run: 0 warm-up, 1 measured, 2 stopping.
static PHASE: AtomicU8 = AtomicU8::new(0);

// ── contend ─────────────────────────────────────────────────────────────

struct Counters {
    tx: AtomicU64,
    wops: AtomicU64,
    tx_max_ns: AtomicU64,
    w_max_ns: AtomicU64,
    aborts: AtomicU64,
    errors: AtomicU64,
}

fn contend_client(port: u16, tx_n: usize, seed: u64, c: Arc<Counters>) -> Vec<u64> {
    let (mut r, mut w) = connect(port);
    let mut rng = Rng(seed | 1);
    let v = "v".repeat(32);
    let mut lat = Vec::with_capacity(1 << 16);
    let mut buf = Vec::with_capacity(1 << 20);
    let pool = key_pool();
    while PHASE.load(Ordering::Relaxed) < 2 {
        // As txbench `setn unrelated`: max(20, TX_N) keys drawn, TX_N used.
        let k: Vec<&str> = (0..20.max(tx_n))
            .map(|_| pool[(rng.next() % POOL as u64) as usize].as_str())
            .collect();
        buf.clear();
        enc(&mut buf, &[b"MULTI"]);
        for kk in &k[..tx_n] {
            enc(&mut buf, &[b"SET", kk.as_bytes(), v.as_bytes()]);
        }
        enc(&mut buf, &[b"EXEC"]);
        let t0 = Instant::now();
        w.write_all(&buf).unwrap();
        w.flush().unwrap();
        for _ in 0..tx_n + 1 {
            read_reply(&mut r);
        }
        match read_reply(&mut r) {
            Reply::Nil => c.aborts.fetch_add(1, Ordering::Relaxed),
            Reply::Error => c.errors.fetch_add(1, Ordering::Relaxed),
            Reply::Ok => 0,
        };
        let ns = t0.elapsed().as_nanos() as u64;
        c.tx.fetch_add(1, Ordering::Relaxed);
        c.tx_max_ns.fetch_max(ns, Ordering::Relaxed);
        if PHASE.load(Ordering::Relaxed) == 1 {
            lat.push(ns);
        }
    }
    lat
}

fn contend_writer(port: u16, seed: u64, c: Arc<Counters>) -> Vec<u64> {
    let (mut r, mut w) = connect(port);
    let mut rng = Rng(seed | 1);
    let v = "w".repeat(32);
    let mut lat = Vec::with_capacity(1 << 16);
    let mut buf = Vec::new();
    let pool = key_pool();
    while PHASE.load(Ordering::Relaxed) < 2 {
        let a = &pool[(rng.next() % POOL as u64) as usize];
        let b = &pool[(rng.next() % POOL as u64) as usize];
        buf.clear();
        enc(&mut buf, &[b"MSET", a.as_bytes(), v.as_bytes(), b.as_bytes(), v.as_bytes()]);
        let t0 = Instant::now();
        w.write_all(&buf).unwrap();
        w.flush().unwrap();
        read_reply(&mut r);
        let ns = t0.elapsed().as_nanos() as u64;
        c.wops.fetch_add(1, Ordering::Relaxed);
        c.w_max_ns.fetch_max(ns, Ordering::Relaxed);
        if PHASE.load(Ordering::Relaxed) == 1 {
            lat.push(ns);
        }
    }
    lat
}

fn contend(a: &[String]) {
    let port: u16 = a[0].parse().unwrap();
    let tx_n: usize = a[1].parse().unwrap();
    let clients: usize = a[2].parse().unwrap();
    let writers: usize = a[3].parse().unwrap();
    let warm: f64 = a[4].parse().unwrap();
    let measure: f64 = a[5].parse().unwrap();
    let mut series = BufWriter::new(std::fs::File::create(&a[6]).unwrap());
    writeln!(series, "unix_ms\tphase\ttx\twops\ttx_max_us\tw_max_us").unwrap();
    key_pool();
    let c = Arc::new(Counters {
        tx: AtomicU64::new(0),
        wops: AtomicU64::new(0),
        tx_max_ns: AtomicU64::new(0),
        w_max_ns: AtomicU64::new(0),
        aborts: AtomicU64::new(0),
        errors: AtomicU64::new(0),
    });
    let ct: Vec<_> = (0..clients)
        .map(|i| {
            let c = Arc::clone(&c);
            std::thread::spawn(move || contend_client(port, tx_n, 0x9E37_79B9_7F4A_7C15 ^ (i as u64 + 1), c))
        })
        .collect();
    let wt: Vec<_> = (0..writers)
        .map(|i| {
            let c = Arc::clone(&c);
            std::thread::spawn(move || contend_writer(port, 0xD1B5_4A32_D192_ED03 ^ (i as u64 + 7), c))
        })
        .collect();
    let t0 = Instant::now();
    let total = warm + measure;
    let (mut m_start, mut m_end) = (0u128, 0u128);
    let (mut tx0, mut w0, mut a0, mut e0) = (0, 0, 0, 0);
    let mut next = 1.0;
    loop {
        let el = t0.elapsed().as_secs_f64();
        if PHASE.load(Ordering::Relaxed) == 0 && el >= warm {
            PHASE.store(1, Ordering::Relaxed);
            m_start = unix_ms();
            tx0 = c.tx.load(Ordering::Relaxed);
            w0 = c.wops.load(Ordering::Relaxed);
            a0 = c.aborts.load(Ordering::Relaxed);
            e0 = c.errors.load(Ordering::Relaxed);
        }
        if el >= total {
            break;
        }
        if el >= next {
            writeln!(
                series,
                "{}\t{}\t{}\t{}\t{:.0}\t{:.0}",
                unix_ms(),
                PHASE.load(Ordering::Relaxed),
                c.tx.load(Ordering::Relaxed),
                c.wops.load(Ordering::Relaxed),
                c.tx_max_ns.swap(0, Ordering::Relaxed) as f64 / 1e3,
                c.w_max_ns.swap(0, Ordering::Relaxed) as f64 / 1e3
            )
            .unwrap();
            series.flush().unwrap();
            next += 1.0;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    m_end = m_end.max(unix_ms());
    let tx1 = c.tx.load(Ordering::Relaxed);
    let w1 = c.wops.load(Ordering::Relaxed);
    let a1 = c.aborts.load(Ordering::Relaxed);
    let e1 = c.errors.load(Ordering::Relaxed);
    PHASE.store(2, Ordering::Relaxed);
    let mut lat: Vec<u64> = ct.into_iter().flat_map(|t| t.join().unwrap()).collect();
    let mut wlat: Vec<u64> = wt.into_iter().flat_map(|t| t.join().unwrap()).collect();
    lat.sort_unstable();
    wlat.sort_unstable();
    let secs = (m_end - m_start) as f64 / 1e3;
    println!(
        "mode=contend tx_n={tx_n} clients={clients} writers={writers} warm_s={warm} measure_s={secs:.2} \
         measure_start_ms={m_start} measure_end_ms={m_end} tx={} tx_per_s={:.2} p50_us={:.1} p95_us={:.1} \
         p99_us={:.1} max_us={:.1} aborts={} errors={} wops={} writer_ops_per_s={:.1} w_p50_us={:.1} \
         w_p99_us={:.1} w_max_us={:.1} total_tx={}",
        tx1 - tx0,
        (tx1 - tx0) as f64 / secs,
        pct(&lat, 0.5),
        pct(&lat, 0.95),
        pct(&lat, 0.99),
        pct(&lat, 1.0),
        a1 - a0,
        e1 - e0,
        w1 - w0,
        (w1 - w0) as f64 / secs,
        pct(&wlat, 0.5),
        pct(&wlat, 0.99),
        pct(&wlat, 1.0),
        c.tx.load(Ordering::Relaxed),
    );
}

// ── paced ───────────────────────────────────────────────────────────────

fn value_for(conn: usize, seq: u64, member: usize) -> Vec<u8> {
    let mut v = format!("{conn:02x}:{seq:010}:{member:05}:").into_bytes();
    v.resize(32, b'v');
    v
}

fn init_value(i: usize) -> Vec<u8> {
    let mut v = format!("init:{i}:").into_bytes();
    v.resize(32, b'p');
    v
}

struct PacedConn {
    /// Per local key: (seq, member) of the last acknowledged write.
    last: Vec<Option<(u64, u32)>>,
    lat_due: Vec<u64>,
    lat_sent: Vec<u64>,
    errors: u64,
    sent: u64,
    done: u64,
}

struct PacedShared {
    done: AtomicU64,
    max_due_ns: AtomicU64,
}

#[allow(clippy::too_many_arguments)]
fn paced_conn(
    port: u16,
    conn: usize,
    conns: usize,
    tx_n: usize,
    keys: usize,
    interval: f64,
    start: Instant,
    end: f64,
    shared: Arc<PacedShared>,
) -> PacedConn {
    let (mut r, mut w) = connect(port);
    let local = keys / conns;
    let mut rng = Rng(0x5851_F42D_4C95_7F2D ^ ((conn as u64 + 1) * 0x2545_F491_4F6C_DD1D));
    let mut st = PacedConn {
        last: vec![None; local],
        lat_due: Vec::with_capacity(1 << 16),
        lat_sent: Vec::with_capacity(1 << 16),
        errors: 0,
        sent: 0,
        done: 0,
    };
    let mut buf = Vec::with_capacity(tx_n * 80 + 64);
    let mut picks: Vec<usize> = Vec::with_capacity(tx_n);
    // Spread the connections' schedules across one interval.
    let phase = interval * conn as f64 / conns as f64;
    let mut k: u64 = 0;
    loop {
        let due = phase + k as f64 * interval;
        if due >= end {
            break;
        }
        let el = start.elapsed().as_secs_f64();
        if due > el {
            let wait = due - el;
            if wait > 0.000_3 {
                std::thread::sleep(Duration::from_secs_f64(wait - 0.000_2));
            }
            while start.elapsed().as_secs_f64() < due {
                std::hint::spin_loop();
            }
        }
        let seq = k + 1;
        picks.clear();
        buf.clear();
        enc(&mut buf, &[b"MULTI"]);
        for m in 0..tx_n {
            let j = (rng.next() % local as u64) as usize;
            picks.push(j);
            let key = key_name(j * conns + conn);
            enc(&mut buf, &[b"SET", key.as_bytes(), &value_for(conn, seq, m)]);
        }
        enc(&mut buf, &[b"EXEC"]);
        let sent_at = Instant::now();
        w.write_all(&buf).unwrap();
        w.flush().unwrap();
        st.sent += 1;
        let mut ok = true;
        for _ in 0..tx_n + 1 {
            if read_reply(&mut r) != Reply::Ok {
                ok = false;
            }
        }
        if read_reply(&mut r) != Reply::Ok {
            ok = false;
        }
        let now = start.elapsed().as_secs_f64();
        if ok {
            for (m, &j) in picks.iter().enumerate() {
                st.last[j] = Some((seq, m as u32));
            }
            st.done += 1;
            shared.done.fetch_add(1, Ordering::Relaxed);
        } else {
            st.errors += 1;
        }
        let due_ns = ((now - due) * 1e9) as u64;
        shared.max_due_ns.fetch_max(due_ns, Ordering::Relaxed);
        if PHASE.load(Ordering::Relaxed) == 1 {
            st.lat_due.push(due_ns);
            st.lat_sent.push(sent_at.elapsed().as_nanos() as u64);
        }
        k += 1;
    }
    st
}

/// Transactions due by `t` seconds after the start, summed over connections.
fn due_by(t: f64, interval: f64, conns: usize, end: f64) -> u64 {
    let t = t.min(end);
    (0..conns)
        .map(|c| {
            let phase = interval * c as f64 / conns as f64;
            if t <= phase {
                0
            } else {
                ((t - phase) / interval).ceil() as u64
            }
        })
        .sum()
}

fn paced(a: &[String]) {
    let port: u16 = a[0].parse().unwrap();
    let rate: f64 = a[1].parse().unwrap();
    let conns: usize = a[2].parse().unwrap();
    let tx_n: usize = a[3].parse().unwrap();
    let keys: usize = a[4].parse().unwrap();
    let warm: f64 = a[5].parse().unwrap();
    let measure: f64 = a[6].parse().unwrap();
    let prefix = &a[7];
    assert!(keys % conns == 0, "KEYS must be a multiple of CONNS");
    let interval = conns as f64 / rate;
    let end = warm + measure;
    let shared = Arc::new(PacedShared {
        done: AtomicU64::new(0),
        max_due_ns: AtomicU64::new(0),
    });
    let start = Instant::now() + Duration::from_millis(200);
    let start_ms = unix_ms() + 200;
    let ts: Vec<_> = (0..conns)
        .map(|c| {
            let s = Arc::clone(&shared);
            std::thread::spawn(move || paced_conn(port, c, conns, tx_n, keys, interval, start, end, s))
        })
        .collect();
    let mut series = BufWriter::new(std::fs::File::create(format!("{prefix}.series.tsv")).unwrap());
    writeln!(series, "unix_ms\tt_s\tphase\toffered\taccepted\tmax_due_lat_us").unwrap();
    while Instant::now() < start {
        std::thread::sleep(Duration::from_millis(1));
    }
    let (mut done0, mut m_start) = (0u64, 0u128);
    let mut next = 1.0;
    loop {
        let el = start.elapsed().as_secs_f64();
        if PHASE.load(Ordering::Relaxed) == 0 && el >= warm {
            PHASE.store(1, Ordering::Relaxed);
            done0 = shared.done.load(Ordering::Relaxed);
            m_start = unix_ms();
        }
        if el >= end {
            break;
        }
        if el >= next {
            writeln!(
                series,
                "{}\t{:.3}\t{}\t{}\t{}\t{:.0}",
                unix_ms(),
                el,
                PHASE.load(Ordering::Relaxed),
                due_by(el, interval, conns, end),
                shared.done.load(Ordering::Relaxed),
                shared.max_due_ns.swap(0, Ordering::Relaxed) as f64 / 1e3
            )
            .unwrap();
            series.flush().unwrap();
            next += 1.0;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    let done1 = shared.done.load(Ordering::Relaxed);
    let m_end = unix_ms();
    PHASE.store(2, Ordering::Relaxed);
    let offered = due_by(end, interval, conns, end) - due_by(warm, interval, conns, end);
    let results: Vec<PacedConn> = ts.into_iter().map(|t| t.join().unwrap()).collect();
    let drained_ms = unix_ms();
    let mut lat_due: Vec<u64> = results.iter().flat_map(|r| r.lat_due.iter().copied()).collect();
    let mut lat_sent: Vec<u64> = results.iter().flat_map(|r| r.lat_sent.iter().copied()).collect();
    lat_due.sort_unstable();
    lat_sent.sort_unstable();
    let errors: u64 = results.iter().map(|r| r.errors).sum();
    let sent: u64 = results.iter().map(|r| r.sent).sum();
    let done: u64 = results.iter().map(|r| r.done).sum();
    // The expected keyspace: every key's last acknowledged value, or its
    // prefill value.
    let mut out = BufWriter::new(std::fs::File::create(format!("{prefix}.expected")).unwrap());
    let mut written = 0u64;
    for i in 0..keys {
        let (c, j) = (i % conns, i / conns);
        let v = match results[c].last.get(j).copied().flatten() {
            Some((seq, m)) => {
                written += 1;
                value_for(c, seq, m as usize)
            }
            None => init_value(i),
        };
        out.write_all(&v).unwrap();
        out.write_all(b"\n").unwrap();
    }
    out.flush().unwrap();
    let secs = (m_end - m_start) as f64 / 1e3;
    println!(
        "mode=paced rate={rate} conns={conns} tx_n={tx_n} keys={keys} warm_s={warm} measure_s={secs:.2} \
         start_ms={start_ms} measure_start_ms={m_start} measure_end_ms={m_end} drained_ms={drained_ms} \
         offered={offered} offered_per_s={:.1} accepted={} accepted_per_s={:.1} accepted_ratio={:.4} \
         frames_per_s={:.0} sent_total={sent} done_total={done} errors={errors} keys_written={written} \
         due_p50_us={:.1} due_p99_us={:.1} due_max_us={:.1} sent_p50_us={:.1} sent_p99_us={:.1} sent_max_us={:.1}",
        offered as f64 / measure,
        done1 - done0,
        (done1 - done0) as f64 / secs,
        (done1 - done0) as f64 / offered.max(1) as f64,
        (done1 - done0) as f64 * tx_n as f64 / secs,
        pct(&lat_due, 0.5),
        pct(&lat_due, 0.99),
        pct(&lat_due, 1.0),
        pct(&lat_sent, 0.5),
        pct(&lat_sent, 0.99),
        pct(&lat_sent, 1.0),
    );
}

// ── prefill and verify ──────────────────────────────────────────────────

fn prefill(a: &[String]) {
    let port: u16 = a[0].parse().unwrap();
    let keys: usize = a[1].parse().unwrap();
    let (mut r, mut w) = connect(port);
    let mut sent = 0usize;
    for chunk in (0..keys).collect::<Vec<_>>().chunks(100) {
        let mut buf = Vec::new();
        let names: Vec<(String, Vec<u8>)> = chunk.iter().map(|&i| (key_name(i), init_value(i))).collect();
        let mut args: Vec<&[u8]> = vec![b"MSET"];
        for (k, v) in &names {
            args.push(k.as_bytes());
            args.push(v);
        }
        enc(&mut buf, &args);
        w.write_all(&buf).unwrap();
        sent += 1;
        if sent % 50 == 0 {
            w.flush().unwrap();
            for _ in 0..50 {
                assert_eq!(read_reply(&mut r), Reply::Ok);
            }
            sent = 0;
        }
    }
    w.flush().unwrap();
    for _ in 0..sent {
        assert_eq!(read_reply(&mut r), Reply::Ok);
    }
    println!("prefill keys={keys}");
}

fn verify(a: &[String]) {
    let port: u16 = a[0].parse().unwrap();
    let keys: usize = a[1].parse().unwrap();
    let expected: Option<Vec<Vec<u8>>> = if a[2] == "none" {
        None
    } else {
        let text = std::fs::read(&a[2]).unwrap();
        let lines: Vec<Vec<u8>> = text.split(|b| *b == b'\n').filter(|l| !l.is_empty()).map(<[u8]>::to_vec).collect();
        assert_eq!(lines.len(), keys, "expected file has {} lines", lines.len());
        Some(lines)
    };
    let (mut r, mut w) = connect(port);
    let t0 = Instant::now();
    let mut digest: u64 = 0xcbf2_9ce4_8422_2325;
    let mut fnv = |bytes: &[u8]| {
        for b in bytes {
            digest ^= u64::from(*b);
            digest = digest.wrapping_mul(0x0000_0100_0000_01b3);
        }
    };
    let (mut missing, mut mismatched, mut ttl_set) = (0u64, 0u64, 0u64);
    let mut examples: Vec<String> = Vec::new();
    let batch = 1000usize;
    let idx: Vec<usize> = (0..keys).collect();
    let mut buf = Vec::new();
    for chunk in idx.chunks(batch * 8) {
        buf.clear();
        for sub in chunk.chunks(batch) {
            let names: Vec<String> = sub.iter().map(|&i| key_name(i)).collect();
            let mut args: Vec<&[u8]> = vec![b"MGET"];
            args.extend(names.iter().map(String::as_bytes));
            enc(&mut buf, &args);
        }
        for &i in chunk {
            enc(&mut buf, &[b"PTTL", key_name(i).as_bytes()]);
        }
        w.write_all(&buf).unwrap();
        w.flush().unwrap();
        let mut vals = Vec::with_capacity(chunk.len());
        for _ in chunk.chunks(batch) {
            vals.extend(read_values(&mut r));
        }
        for (&i, v) in chunk.iter().zip(&vals) {
            let name = key_name(i);
            fnv(name.as_bytes());
            fnv(b"=");
            match v {
                None => {
                    missing += 1;
                    fnv(b"<nil>");
                }
                Some(v) => fnv(v),
            }
            fnv(b"\n");
            if let Some(exp) = &expected
                && v.as_deref() != Some(exp[i].as_slice())
            {
                mismatched += 1;
                if examples.len() < 5 {
                    examples.push(format!(
                        "{name}:{}!={}",
                        v.as_deref().map_or("<nil>".into(), |v| String::from_utf8_lossy(v).into_owned()),
                        String::from_utf8_lossy(&exp[i])
                    ));
                }
            }
        }
        for _ in chunk {
            if read_int(&mut r) != -1 {
                ttl_set += 1;
            }
        }
    }
    let mut buf = Vec::new();
    enc(&mut buf, &[b"DBSIZE"]);
    w.write_all(&buf).unwrap();
    w.flush().unwrap();
    let dbsize = read_int(&mut r);
    println!(
        "verify port={port} keys={keys} dbsize={dbsize} missing={missing} mismatched={} ttl_set={ttl_set} \
         digest={digest:016x} ok={} secs={:.2} examples={}",
        if expected.is_some() { mismatched.to_string() } else { "na".into() },
        expected.is_some() && missing == 0 && mismatched == 0 && ttl_set == 0 && dbsize == keys as i64,
        t0.elapsed().as_secs_f64(),
        examples.join("|")
    );
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let rest = &a[2.min(a.len())..];
    match a.get(1).map(String::as_str) {
        Some("contend") if rest.len() == 7 => contend(rest),
        Some("paced") if rest.len() == 8 => paced(rest),
        Some("prefill") if rest.len() == 2 => prefill(rest),
        Some("verify") if rest.len() == 3 => verify(rest),
        _ => {
            eprintln!(
                "usage:\n  i8bench contend PORT TX_N CLIENTS WRITERS WARMUP_S MEASURE_S SERIES\n  \
                 i8bench paced PORT RATE CONNS TX_N KEYS WARMUP_S MEASURE_S PREFIX\n  \
                 i8bench prefill PORT KEYS\n  i8bench verify PORT KEYS EXPECTED|none"
            );
            std::process::exit(2);
        }
    }
}
