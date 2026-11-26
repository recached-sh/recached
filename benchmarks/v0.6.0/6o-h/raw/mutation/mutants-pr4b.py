P = "server-native/src/persistence.rs"
MUTANTS = [
    ("Md unknown non-ignorable control ignored", P,
     "                        timeless_end = timeless_end.min(record_at);\n                    }\n                    // A checkpoint marker says",
     "                    }\n                    // A checkpoint marker says"),
    ("Mn a replica keeps writing the format (PR 4: at the handshake)", "server-native/src/replication.rs",
     "    if let Some(aof) = &state.aof {\n        aof.leave_expiry_log();\n    }",
     ""),
]
