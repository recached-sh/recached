MUTANTS = [("R3-1 replica keeps the format at startup", "server-native/src/persistence.rs",
  "        // (6O-H review C, R3-1: an acknowledged write after a promotion\n        // was lost).\n        aof.leave_expiry_log();",
  "        // (6O-H review C, R3-1: an acknowledged write after a promotion\n        // was lost)."),
 ("R3-2 failed truncation keeps the mode", "server-native/src/persistence.rs",
  "            self.shared.lock().mode = AofFileMode::Legacy;\n            return Err(error);",
  "            return Err(error);")]
