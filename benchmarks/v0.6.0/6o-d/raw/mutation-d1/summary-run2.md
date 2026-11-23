| Mutant | Outcome | Killed by |
|---|---|---|
| M1 single-key read never waits for a multi-key write | **KILLED** | store guards (6 s) |
| M2 multi-key read ignores writes in progress (validates only) | **KILLED** | store guards (6 s) |
| M3 multi-key read skips its after-validation | **KILLED** | store guards (5 s) |
| M4 locked fallback takes stripes in descending order | **KILLED** | store guards (23 s) |
| M5 multi-key write marks only its first stripe | **KILLED** | store guards (5 s) |
| M6 multi-key write not marked multi-key | **KILLED** | store guards (5 s) |
| M7 writers never mark a write in progress | **KILLED** | store guards (4 s) |
| M8 release clears the flags without advancing the version | **KILLED** | store guards (5 s) |
| M9 MGET reads the clock per key | **KILLED** | store guards (3 s) |
| M10 owned one-key reads bypass the protocol | **KILLED** | store guards (5 s) |
| M11 borrowed GET bypasses the protocol | **KILLED** | server guards (6 s) |
| M12 borrowed MGET bypasses the protocol | **KILLED** | store guards (5 s) |
| M13 by-reference one-key reads bypass the protocol | **KILLED** | store guards (6 s) |
| M14 a retried borrowed MGET does not restart its reply | **KILLED** | store guards (5 s) |
| M15 owned multi-key reads bypass the protocol | **KILLED** | store guards (5 s) |
