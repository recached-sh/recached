"""6O-G server-side mutants, run in the dedicated mutation worktree.

Each mutant edits files in SRC, builds the server tests and runs FILTER;
'killed' = some test failed (or the run timed out: a deadlock). Files are
restored after each mutant. Usage: python3 mutate-6og-server.py [names...]
"""
import os, subprocess, sys
SRC = os.path.expanduser("~/recached-6og-bench/src-mut")
TGT = os.path.expanduser("~/recached-6og-bench/target-mut-server")
FILTER = os.environ.get("FILTER", "6og")
T = "server-native/src/transaction.rs"
M = {
 # 6 (effects/publication before the last write) is store mutant M2, killed there.
 # 7: notification value read for an intermediate write (every effective write notified,
 # read lazily in phase 2 -> the final value under an earlier write's delta)
 "S07_notify_every_write": [(T, "            if writes.last != (at, false) {", "            if writes.last.1 {")],
 # 8: the last write's delta instead of the full value for a key written several times
 "S08_last_delta_not_full_value": [(T, "                (_, false) => KeyChange::current(key.to_string(), None, store),",
   "                (_, false) => KeyChange::current(key.to_string(), delta.as_ref().filter(|(k, _)| k == key).map(|(_, d)| d.clone()), store),")],
 # 9: WATCH invalidation lost when coalescing (removal after a set)
 "S09_lost_invalidation": [(T, "                (_, true) if writes.set => KeyChange::removed_after_set(key.to_string()),",
   "                (_, true) if writes.set => KeyChange::removed(key.to_string()),")],
 # effective-write definition: count non-effective writes
 "S09b_count_noop_writes": [(T, "            if write.effective() {\n                if matches!(write.cmd, Command::FlushDb) {",
   "            if true {\n                if matches!(write.cmd, Command::FlushDb) {")],
 # ephemeral bookkeeping of evictions moved out of command order (after the claim)
 # ephemeral bookkeeping of evictions moved to phase 2 (after later commands' claims)
 "S10b_eviction_forget_in_phase_2": [
   (T, "    for key in &evicted {\n        state.forget_ephemeral_keys(std::iter::once(key.as_str()));\n    }\n", ""),
   (T, "            let write = &mut writes[at];\n",
       "            let write = &mut writes[at];\n            for key in &write.evicted { state.forget_ephemeral_keys(std::iter::once(key.as_str())); }\n")],
 # AOF batch split per frame
 "S10c_aof_batch_split": [("server-native/src/server_state.rs",
   "                aof.append_frames(frames).await",
   "                { let mut r = Ok(()); for f in frames { r = r.and(aof.append(f).await); } r }")],
 # 13: replica resumes partially: links closed, run id kept
 "S13_no_run_id_rotation": [("server-native/src/replication.rs",
   ".unwrap_or_else(|poisoned| poisoned.into_inner()) = generate_run_id();",
   ".unwrap_or_else(|poisoned| poisoned.into_inner()) = self.run_id();")],
 # 13b: a replica's full resync does not reset its sub-replicas
 "S13b_no_sub_replica_reset": [("server-native/src/replication.rs",
   "            let sub_replicas = state.replicas.reset_stream();", "            let sub_replicas = 0usize;")],
 # convergence skipped entirely (fail-closed only)
 "S13c_no_converge": [(T, "        converge(&queue[failed], state, watch_registry, store).await;", "        let _ = (watch_registry, store);")],
 # fail open: a panicking write leaves persistence healthy
 "S14b_fail_open": [(T, '    state.record_internal_failure("transaction_panic");', '    let _ = state;')],
 # 15: the scope is not restored when the transaction ends
 "S15_scope_left_set": [("core-engine/src/store.rs", "        set_scope(self.previous);", "        let _ = self.previous;")],
}
names = sys.argv[1:] or list(M)
env = dict(os.environ, PATH=os.path.expanduser("~/.cargo/bin:") + os.environ["PATH"], CARGO_TARGET_DIR=TGT)
for name in names:
    originals = {}
    try:
        for path, a, b in M[name]:
            p = os.path.join(SRC, path)
            s = originals.get(p) or open(p).read()
            originals.setdefault(p, s)
            cur = open(p).read()
            assert cur.count(a) == 1, (name, path, a[:60], cur.count(a))
            open(p, "w").write(cur.replace(a, b))
        b = subprocess.run(["cargo", "test", "-q", "-p", "recached", "--bin", "recached-server", "--no-run"],
                           cwd=SRC, env=env, capture_output=True, text=True)
        if b.returncode != 0:
            print(f"=== {name}: DOES NOT COMPILE (not counted)\n{b.stderr[-800:]}"); sys.stdout.flush(); continue
        r = subprocess.run(["timeout", "-s", "KILL", "900", "cargo", "test", "-q", "-p", "recached", "--bin",
                            "recached-server", FILTER], cwd=SRC, env=env, capture_output=True, text=True)
        out = r.stdout + r.stderr
        failed = [l for l in out.splitlines() if l.startswith("    tests::")]
        verdict = "KILLED" if r.returncode != 0 else "SURVIVED"
        if r.returncode == -9 or r.returncode == 137: verdict = "KILLED (timeout: hang)"
        res = [l for l in out.splitlines() if "test result" in l]
        print(f"=== {name}: {verdict} rc={r.returncode} {res[-1] if res else ''}")
        for l in failed[:8]: print("   ", l.strip())
        sys.stdout.flush()
    finally:
        for p, s in originals.items():
            open(p, "w").write(s)
