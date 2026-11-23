| Mutant | Outcome | Killed by |
|---|---|---|
| M1 single-key read never waits for a multi-key write | **KILLED** | store guards (10 s) |
| M2 multi-key read ignores writes in progress (validates only) | **KILLED** | store guards (5 s) |
| M3 multi-key read skips its after-validation | **KILLED** | store guards (4 s) |
| M4 locked fallback takes stripes in descending order | **KILLED** | store guards (43 s) |
| M5 multi-key write marks only its first stripe | **KILLED** | store guards (6 s) |
| M6 multi-key write not marked multi-key | **KILLED** | store guards (5 s) |
| M7 writers never mark a write in progress | **KILLED** | store guards (4 s) |
| M8 release clears the flags without advancing the version | **KILLED** | store guards (4 s) |
| M9 MGET reads the clock per key | **KILLED** | store guards (3 s) |
| M10 owned one-key reads bypass the protocol | **KILLED** | store guards (25 s) |
| M11 borrowed GET bypasses the protocol | **KILLED** | store guards (26 s) |
| M12 borrowed MGET bypasses the protocol | **KILLED** | store guards (7 s) |
| M13 by-reference one-key reads bypass the protocol | **KILLED** | store guards (7 s) |
| M14 a retried borrowed MGET does not restart its reply | **KILLED** | store guards (7 s) |
| M15 owned multi-key reads bypass the protocol | **KILLED** | store guards (22 s) |
| M16 a one-key write naming its key twice is not marked multi-key | **KILLED** | store guards (5 s) |
| M17 a write that may be rolled back is not marked multi-key | **KILLED** | store guards (4 s) |
| M18 SINTER/SUNION/SDIFF bypass the protocol | **KILLED** | store guards (5 s) |
| M19 a DEDUP-wrapped multi-key read runs (as before review round 1) | **KILLED** | store guards (6 s) |
| M20 borrowed HGET bypasses the protocol | **KILLED** | store guards (9 s) |
| M21 a pop that may empty a collection is not marked multi-key | **KILLED** | store guards (6 s) |
| M23 a single-key read skips its after-validation | **KILLED** | store guards (4 s) |
| M24 multi-step writes never mark the multi word | **KILLED** | store guards (4 s) |
| M25 a multi-step release does not advance the epoch | **SURVIVED** | all guards passed |
| M26 a retried borrowed GET does not restart its reply | **KILLED** | store guards (6 s) |
| M27 HINCRBYFLOAT creates the hash before refusing a non-finite increment | **KILLED** | store guards (5 s) |
| M28 LPUSH with nothing to add creates the list | **KILLED** | store guards (6 s) |
| M22 a multi-key EXISTS is read as its first key only | **SURVIVED** | all guards passed |
