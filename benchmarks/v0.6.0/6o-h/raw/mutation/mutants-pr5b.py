MUTANTS = [
    ("R2 capable replica sweeps on its own", "server-native/src/propagation.rs",
     "    if state.is_replica() && state.replicas.upstream_expirylog.load(Ordering::Acquire) {\n        return 0;\n    }",
     ""),
    ("R13 replica pushes a multi-key DEL as one frame", "server-native/src/replication.rs",
     "        && keys.len() > 1\n        && tx.is_active()",
     "        && keys.len() > 1\n        && false"),
]
