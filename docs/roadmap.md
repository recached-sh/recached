# Roadmap

Recached competes on **where the data can live** — the same engine on the server, in browser WebAssembly, and in native mobile apps and Rust services, with sync in between. The [benchmark guide](/guide/benchmarks) explains how to measure its throughput and memory cost on the commit you deploy. The project does not publish a current cross-project performance claim.


---

## Native clients: implemented previews and planned adapters

- **Kotlin + Swift: in preview.** One `uniffi`-annotated Rust crate (`recached-mobile`) generates bindings for both. The platform WebSocket (OkHttp / URLSession) feeds frames into `sync-client`, with no embedded async runtime. Data and outbox persist to SQLite, and reactivity is Kotlin `Flow` and Swift `AsyncStream` / Observation over keychange pushes. Both SDKs include demo apps, automated tests, and CI workflows. They are not published yet: see [Android](/android/getting-started) and [iOS](/ios/getting-started).
- **Flutter: planned, not implemented.** The proposed adapter uses `flutter_rust_bridge`: synchronous local reads into Rust memory, `watchKey()` → `Stream` for rebuilds.
- **React Native: planned, not implemented.** The proposed adapter reuses the UniFFI binding layer through `uniffi-bindgen-react-native`, with an API modeled on the browser React hooks.


## Planned WASM server-side scripting

Run `.wasm` stored procedures in place of Lua scripts. The scripting VM would be sandboxed (no network, no file I/O, bounded execution time), accept any WASM module that exports a specific entry function, and execute it against the cache store. The intended runtime accepts compatible modules compiled from languages such as Rust, TinyGo, or AssemblyScript.


## Planned WASI target

A `wasm32-wasip1` build of `wasm-edge` for Cloudflare Workers and Deno Deploy, running Recached as a cache layer at the edge with the same API as the browser client.

The current browser adapter uses Web APIs, including a JavaScript clock and `window` reconnect timers. A WASI target and platform-specific transport and persistence adapters are not implemented.


## AI-era features

Recached's unfair advantage is *where the data lives* — so the winning AI features put the intelligence layer **next to the user** instead of behind another network hop. Ordered by intended sequence.

### Token-cost rate limiting

The proposed `COST` argument would extend the existing limiter to weighted token budgets. It is not implemented:

```bash
RLCHECK user:42 100000 3600 COST 1850   # consume 1,850 tokens of a 100k/hour budget
```


### Semantic caching (`SEMSET` / `SEMGET`)

Proposed `SEMSET` and `SEMGET` commands would cache responses by embedding similarity. These commands are not implemented:

```bash
SEMSET prompts <embedding> "<cached LLM response>" EX 3600
SEMGET prompts <embedding> 0.92          # → cached response or nil
```

### Streaming values: implemented deltas, future client APIs

`APPEND` and negotiated `keydelta` frames are implemented. Shared clients enable `CLIENT DELTA ON`, and live-query snapshots restore state on reconnect. The server can stream append deltas instead of re-sending the whole value. See [key deltas](/server/protocol#key-deltas-client-delta-on).

The browser and native wrappers do not expose an `append` helper yet; backend RESP writers can use `APPEND` while clients observe the key. Additional streaming APIs remain future work.

### Computed keys — the reactive cache

Declare a key as a function of other keys; the server recomputes on change and the diff flows through live queries — cache becomes spreadsheet. `cart:42:total` recomputes when any `cart:42:item:*` changes, and every subscribed UI updates. Would use the planned WASM scripting runtime. Biggest lift, biggest ceiling.

Under consideration behind these: a CRDT text type for collaborative editing (likely embedding an existing Rust CRDT rather than building one), and per-key undo/history on top of the existing op-log machinery.

---

Feedback on priorities is welcome — [open an issue](https://github.com/recached-sh/recached/issues) or write to [dennis@thinkgrid.dev](mailto:dennis@thinkgrid.dev).
