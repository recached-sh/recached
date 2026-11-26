P = "server-native/src/propagation.rs"
S = "server-native/src/server_state.rs"
T = "server-native/src/transaction.rs"
MUTANTS = [
    ("B-1 DEL pushed to the writer (origin 0)", P,
     "        tx.send(Arc::new(SyncPush::for_command(\n            origin,\n            &del,",
     "        tx.send(Arc::new(SyncPush::for_command(\n            0,\n            &del,"),
    ("B-1 EXEC DEL pushed to the writer", T,
     "                    let origin = if write.effective() { conn_id } else { 0 };",
     "                    let origin = 0;"),
    ("RB-M1 deferred branch [frame][DEL]", S,
     "            let frames = [expired, wrapped.as_deref().unwrap_or(resp)];",
     "            let frames = [wrapped.as_deref().unwrap_or(resp), expired];"),
    ("RB-M2 forget_expired a no-op", P,
     "    if !expired.is_empty() {\n        state.forget_ephemeral_keys(expired.iter().map(String::as_str));\n    }",
     "    let _ = (state, expired);"),
    ("RB-M3 expiry removal never removed_after_set", T,
     "    fn expiry(&self, at: usize, key: &str) -> Option<KeyChange<'static>> {\n        self.removal(at, key, Change::Expiry)\n    }",
     "    fn expiry(&self, at: usize, key: &str) -> Option<KeyChange<'static>> {\n        self.removal(at, key, Change::Expiry)\n            .map(|_| KeyChange::removed(key.to_string()))\n    }"),
    ("RB-M4 failed write's WS DEL dropped", P,
     "            // No frame of the write follows it: every client, the writer's\n            // too, learns the key is gone (origin 0, as for an eviction).\n            if expiry_ws && let Some((del, frame)) = expiry {",
     "            if false && let Some((del, frame)) = expiry {"),
    ("N-1 bound off by one", P,
     "            && (end == start || bytes + keys[end].len() <= EXPIRY_DEL_FRAME_KEY_BYTES)",
     "            && (end == start || bytes + keys[end].len() < EXPIRY_DEL_FRAME_KEY_BYTES)"),
]
