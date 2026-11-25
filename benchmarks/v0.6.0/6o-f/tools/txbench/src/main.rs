//! 6O-E transaction load generator. std only.
//!
//! `txbench PORT MODE KEYSET CLIENTS SECS [WRITERS]`
//!
//! Each client connection loops: build one transaction, send it as one
//! pipelined write (`MULTI`, the queued commands, `EXEC`), read every reply,
//! record the latency. `watch` mode adds the CAS round trip first
//! (`WATCH a` + `GET a`, then `MULTI SET a v EXEC`). WRITERS extra
//! connections run `MSET` of 2 keys from the same key set in a loop, for
//! contention. Values are 32 bytes, as in the 6O kit.
//!
//! MODE:   get-get | mget-get | exists-get | get-set | mget-mset | watch | dbsize-get (6O-F)
//! KEYSET: unrelated (uniform over 100,000 keys) | overlap (16 keys) | same (2 keys)
//!
//! Prints one line: tx/s, latency p50/p95/p99/max in µs, aborted EXECs
//! (nil), error replies, writer ops/s.
use std::io::{BufReader, BufWriter, Read, Write};
use std::net::TcpStream;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
}

fn key(rng: &mut Rng, keyset: &str) -> String {
    let n = match keyset {
        "unrelated" => 100_000,
        "overlap" => 16,
        "same" => 2,
        other => panic!("keyset {other}"),
    };
    format!("key:{:012}", rng.next() % n)
}

fn enc(out: &mut Vec<u8>, args: &[&str]) {
    out.extend_from_slice(format!("*{}\r\n", args.len()).as_bytes());
    for a in args {
        out.extend_from_slice(format!("${}\r\n", a.len()).as_bytes());
        out.extend_from_slice(a.as_bytes());
        out.extend_from_slice(b"\r\n");
    }
}

/// Reply kinds that matter here.
#[derive(PartialEq)]
enum Reply {
    Ok,
    Nil,
    Error,
}

fn read_line<R: Read>(r: &mut BufReader<R>) -> Vec<u8> {
    let mut line = Vec::new();
    let mut b = [0u8; 1];
    loop {
        r.read_exact(&mut b).expect("read");
        line.push(b[0]);
        if line.ends_with(b"\r\n") {
            line.truncate(line.len() - 2);
            return line;
        }
    }
}

fn read_reply<R: Read>(r: &mut BufReader<R>) -> Reply {
    let line = read_line(r);
    match line[0] {
        b'+' | b':' => Reply::Ok,
        b'-' => Reply::Error,
        b'$' => {
            let n: i64 = std::str::from_utf8(&line[1..]).unwrap().parse().unwrap();
            if n >= 0 {
                let mut buf = vec![0u8; n as usize + 2];
                r.read_exact(&mut buf).unwrap();
            }
            Reply::Ok
        }
        b'*' => {
            let n: i64 = std::str::from_utf8(&line[1..]).unwrap().parse().unwrap();
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

struct Stats {
    lat_ns: Vec<u64>,
    aborts: u64,
    errors: u64,
}

fn client(port: u16, mode: &str, keyset: &str, seed: u64, stop: Arc<AtomicBool>) -> Stats {
    let s = TcpStream::connect(("127.0.0.1", port)).expect("connect");
    s.set_nodelay(true).unwrap();
    let mut r = BufReader::with_capacity(64 * 1024, s.try_clone().unwrap());
    let mut w = BufWriter::with_capacity(64 * 1024, s);
    let mut rng = Rng(seed | 1);
    let v = "v".repeat(32);
    let mut st = Stats { lat_ns: Vec::with_capacity(1 << 20), aborts: 0, errors: 0 };
    let mut buf = Vec::with_capacity(4096);
    while !stop.load(Ordering::Relaxed) {
        let t0 = Instant::now();
        if mode == "watch" {
            let a = key(&mut rng, keyset);
            buf.clear();
            enc(&mut buf, &["WATCH", &a]);
            enc(&mut buf, &["GET", &a]);
            w.write_all(&buf).unwrap();
            w.flush().unwrap();
            read_reply(&mut r);
            read_reply(&mut r);
            buf.clear();
            enc(&mut buf, &["MULTI"]);
            enc(&mut buf, &["SET", &a, &v]);
            enc(&mut buf, &["EXEC"]);
            w.write_all(&buf).unwrap();
            w.flush().unwrap();
            read_reply(&mut r);
            read_reply(&mut r);
        } else {
            let keys: Vec<String> = (0..20).map(|_| key(&mut rng, keyset)).collect();
            let k: Vec<&str> = keys.iter().map(String::as_str).collect();
            buf.clear();
            enc(&mut buf, &["MULTI"]);
            let queued = match mode {
                "get-get" => {
                    enc(&mut buf, &["GET", k[0]]);
                    enc(&mut buf, &["GET", k[1]]);
                    2
                }
                "mget-get" => {
                    let mut args = vec!["MGET"];
                    args.extend(&k[..10]);
                    enc(&mut buf, &args);
                    enc(&mut buf, &["GET", k[10]]);
                    2
                }
                "exists-get" => {
                    enc(&mut buf, &["EXISTS", k[0]]);
                    enc(&mut buf, &["GET", k[1]]);
                    2
                }
                "get-set" => {
                    enc(&mut buf, &["GET", k[0]]);
                    enc(&mut buf, &["SET", k[1], &v]);
                    2
                }
                "mget-mset" => {
                    let mut args = vec!["MGET"];
                    args.extend(&k[..10]);
                    enc(&mut buf, &args);
                    let mut args = vec!["MSET"];
                    for kk in &k[10..20] {
                        args.push(kk);
                        args.push(&v);
                    }
                    enc(&mut buf, &args);
                    2
                }
                // 6O-F: a whole-keyspace read inside EXEC takes every
                // replication-order stripe.
                "dbsize-get" => {
                    enc(&mut buf, &["DBSIZE"]);
                    enc(&mut buf, &["GET", k[0]]);
                    2
                }
                other => panic!("mode {other}"),
            };
            enc(&mut buf, &["EXEC"]);
            w.write_all(&buf).unwrap();
            w.flush().unwrap();
            for _ in 0..queued + 1 {
                read_reply(&mut r); // OK, QUEUED…
            }
        }
        match read_reply(&mut r) {
            Reply::Nil => st.aborts += 1,
            Reply::Error => st.errors += 1,
            Reply::Ok => {}
        }
        st.lat_ns.push(t0.elapsed().as_nanos() as u64);
    }
    st
}

fn writer(port: u16, keyset: &str, seed: u64, stop: Arc<AtomicBool>) -> u64 {
    let s = TcpStream::connect(("127.0.0.1", port)).expect("connect");
    s.set_nodelay(true).unwrap();
    let mut r = BufReader::new(s.try_clone().unwrap());
    let mut w = BufWriter::new(s);
    let mut rng = Rng(seed | 1);
    let v = "w".repeat(32);
    let mut n = 0;
    let mut buf = Vec::new();
    while !stop.load(Ordering::Relaxed) {
        let (a, b) = (key(&mut rng, keyset), key(&mut rng, keyset));
        buf.clear();
        enc(&mut buf, &["MSET", &a, &v, &b, &v]);
        w.write_all(&buf).unwrap();
        w.flush().unwrap();
        read_reply(&mut r);
        n += 1;
    }
    n
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    if a.len() < 6 {
        eprintln!("usage: txbench PORT MODE KEYSET CLIENTS SECS [WRITERS]");
        std::process::exit(2);
    }
    let port: u16 = a[1].parse().unwrap();
    let (mode, keyset) = (a[2].clone(), a[3].clone());
    let clients: usize = a[4].parse().unwrap();
    let secs: f64 = a[5].parse().unwrap();
    let writers: usize = a.get(6).map_or(0, |w| w.parse().unwrap());
    let stop = Arc::new(AtomicBool::new(false));
    let mut ct = Vec::new();
    for i in 0..clients {
        let (m, k, s) = (mode.clone(), keyset.clone(), Arc::clone(&stop));
        ct.push(std::thread::spawn(move || client(port, &m, &k, 0x9E37_79B9_7F4A_7C15 ^ (i as u64 + 1), s)));
    }
    let mut wt = Vec::new();
    for i in 0..writers {
        let (k, s) = (keyset.clone(), Arc::clone(&stop));
        wt.push(std::thread::spawn(move || writer(port, &k, 0xD1B5_4A32_D192_ED03 ^ (i as u64 + 7), s)));
    }
    let t0 = Instant::now();
    std::thread::sleep(Duration::from_secs_f64(secs));
    stop.store(true, Ordering::Relaxed);
    let mut lat = Vec::new();
    let (mut aborts, mut errors) = (0, 0);
    for t in ct {
        let st = t.join().unwrap();
        lat.extend(st.lat_ns);
        aborts += st.aborts;
        errors += st.errors;
    }
    let wops: u64 = wt.into_iter().map(|t| t.join().unwrap()).sum();
    let el = t0.elapsed().as_secs_f64();
    lat.sort_unstable();
    let p = |q: f64| lat.get(((lat.len() as f64 - 1.0) * q) as usize).copied().unwrap_or(0) as f64 / 1e3;
    println!(
        "mode={mode} keyset={keyset} clients={clients} writers={writers} secs={el:.2} tx={} tx_per_s={:.0} p50_us={:.1} p95_us={:.1} p99_us={:.1} max_us={:.1} aborts={aborts} errors={errors} writer_ops_per_s={:.0}",
        lat.len(),
        lat.len() as f64 / el,
        p(0.50),
        p(0.95),
        p(0.99),
        p(1.0),
        wops as f64 / el
    );
}
