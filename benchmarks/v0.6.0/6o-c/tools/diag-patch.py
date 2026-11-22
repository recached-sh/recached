#!/usr/bin/env python3
"""6O-C diagnostic patch (never merged): process-wide counters for the reply
coalescer's decisions, exposed as one INFO stats line `reply_coalesce_diag`.
Applies to 6O-B (d5563d7) and 6O-C1 sources alike. Relaxed fetch_adds on one
shared cache line: diagnostic cells only, never throughput cells."""
import sys, re
root = sys.argv[1]
p = f"{root}/server-native/src/reply_coalesce.rs"
s = open(p).read()
s += '''
pub(crate) mod diag {
    use std::sync::atomic::{AtomicU64, Ordering::Relaxed};
    pub(crate) static SKIPPED: AtomicU64 = AtomicU64::new(0);
    pub(crate) static YIELDS: AtomicU64 = AtomicU64::new(0);
    pub(crate) static MIGRATED: AtomicU64 = AtomicU64::new(0);
    pub(crate) static GAINS: AtomicU64 = AtomicU64::new(0);
    pub(crate) static BACKOFFS: AtomicU64 = AtomicU64::new(0);
    pub(crate) static FLUSHES: AtomicU64 = AtomicU64::new(0);
    pub(crate) fn bump(c: &AtomicU64) { c.fetch_add(1, Relaxed); }
    pub(crate) fn render() -> String {
        format!("reply_coalesce_diag:flushes={},skipped={},yields={},migrated={},gains={},backoffs={}\\r\\n",
            FLUSHES.load(Relaxed), SKIPPED.load(Relaxed), YIELDS.load(Relaxed),
            MIGRATED.load(Relaxed), GAINS.load(Relaxed), BACKOFFS.load(Relaxed))
    }
}
'''
s = s.replace("            w.skip.set(skip - 1);\n            return None;",
              "            w.skip.set(skip - 1);\n            diag::bump(&diag::SKIPPED);\n            return None;", 1)
s = s.replace("        Some(Probe {\n            worker:", "        diag::bump(&diag::YIELDS);\n        Some(Probe {\n            worker:", 1)
for old in ("            w.skip.set(BACKOFF);", "            w.skip.set(period);"):
    if old in s:
        s = s.replace(old, old + "\n            diag::bump(&diag::BACKOFFS);", 1)
        break
# wrap end(): count migrations and gains
s = s.replace("pub(crate) fn end(probe: Probe) -> bool {", "pub(crate) fn end(probe: Probe) -> bool {\n    let migrated = WORKER.with(|w| std::ptr::from_ref(w) as usize != probe.worker);\n    if migrated { diag::bump(&diag::MIGRATED); }\n    let gained = end_inner(probe);\n    if gained { diag::bump(&diag::GAINS); }\n    gained\n}\n\nfn end_inner(probe: Probe) -> bool {", 1)
for needle in ["diag::SKIPPED", "diag::YIELDS", "diag::BACKOFFS", "fn end_inner"]:
    assert needle in s, needle
open(p, "w").write(s)
# every flush at the end of a read batch that had replies (the call site)
c = f"{root}/server-native/src/connection.rs"
t = open(c).read()
old = "                        // Flush all responses for this read batch in one syscall.\n"
assert t.count(old) == 1
t = t.replace(old, "                        if !writer.buffer().is_empty() { reply_coalesce::diag::bump(&reply_coalesce::diag::FLUSHES); }\n" + old)
open(c, "w").write(t)
i = f"{root}/server-native/src/info.rs"
u = open(i).read()
old = "                     evicted_keys:{}\\r\\n\",\n"
assert u.count(old) == 1, "info"
u = u.replace(old, "                     evicted_keys:{}\\r\\n{}\",\n")
old = "                    store.evicted_count(),\n                )"
assert u.count(old) == 1
u = u.replace(old, "                    store.evicted_count(),\n                    crate::reply_coalesce::diag::render(),\n                )")
open(i, "w").write(u)
print("patched", root)
