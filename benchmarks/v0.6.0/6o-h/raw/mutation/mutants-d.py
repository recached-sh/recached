MUTANTS = [
 ("D1 mode not rechecked under the barrier", "server-native/src/propagation.rs",
  "            if state.is_replica() && state.replicas.upstream_expirylog.load(Ordering::Acquire) {\n                return total;\n            }\n            let expired",
  "            let expired"),
 ("D2 stale handshake not dropped", "server-native/src/replication.rs",
  "    if dedup_source.writes_expiry_log() != advertised_expirylog\n        || replicas.run_id() != advertised_run\n    {",
  "    if false {"),
]
