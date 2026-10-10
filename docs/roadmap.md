# Roadmap

Recached competes on **where the data can live** — the same engine on the server and in the browser, with sync in between. The [benchmark guide](/guide/benchmarks) explains how to measure its throughput and memory cost on the commit you deploy. The project does not publish a current cross-project performance claim.


---

## Mobile SDKs — React Native, Flutter, Kotlin, Swift

- **Kotlin + Swift: in preview.** One `uniffi`-annotated Rust crate (`recached-mobile`) generates bindings for both. The platform WebSocket (OkHttp / URLSession) feeds frames into `sync-client`, with no embedded async runtime. Data and outbox persist to SQLite, and reactivity is Kotlin `Flow` and Swift `AsyncStream` / Observation over keychange pushes. Both SDKs have a demo app and pass their CI; the Android one also passes on a phone. They are not published yet: see [Android](/android/getting-started) and [iOS](/ios/getting-started).
- **Flutter** via `flutter_rust_bridge`: synchronous local reads into Rust memory, `watchKey()` → `Stream` for rebuilds.
- **React Native** last (Hermes has no WASM): `uniffi-bindgen-react-native` reuses the same binding layer, and the existing React hooks API carries over — same `useKey` in React DOM and React Native.


## WASM server-side scripting

Run `.wasm` stored procedures in place of Lua scripts. The scripting VM would be sandboxed (no network, no file I/O, bounded execution time), accept any WASM module that exports a specific entry function, and execute it against the cache store. Supports any language that compiles to WASM: Rust, Go (TinyGo), AssemblyScript, Python.


## WASI target

A `wasm32-wasip1` build of `wasm-edge` for Cloudflare Workers and Deno Deploy, running Recached as a cache layer at the edge with the same API as the browser client.

`core-engine` is already `wasm32`-compatible; the work is adapting the WebSocket and persistence layers to WASI. Last on the list because the platform fights the model — Workers cannot hold persistent WebSockets outside Durable Objects — and edge platforms ship native KV stores.


## AI-era features

Recached's unfair advantage is *where the data lives* — so the winning AI features put the intelligence layer **next to the user** instead of behind another network hop. Ordered by intended sequence.

### Token-cost rate limiting

AI providers meter tokens, not requests. One optional argument extends the existing limiter to weighted budgets:

```bash
RLCHECK user:42 100000 3600 COST 1850   # consume 1,850 tokens of a 100k/hour budget
```


### Semantic caching (`SEMSET` / `SEMGET`)

LLM calls are expensive and repeats are *paraphrases*, so exact-key caching misses them. A semantic cache returns a hit when a query's embedding is close enough to a cached one:

```bash
SEMSET prompts <embedding> "<cached LLM response>" EX 3600
SEMGET prompts <embedding> 0.92          # → cached response or nil
```

### Streaming values — "watch the agent think"

An agent streams tokens into a key with `APPEND`; every subscribed browser renders it live. Live queries already deliver the subscription — the missing piece is an append *delta* frame (keychange currently re-sends the whole value) plus catch-up-then-follow on reconnect. `useKey('agent:run:42:output')` becomes a live-typing agent visible to any number of viewers. Redis Streams end at the backend; this reaches the UI.


### Computed keys — the reactive cache

Declare a key as a function of other keys; the server recomputes on change and the diff flows through live queries — cache becomes spreadsheet. `cart:42:total` recomputes when any `cart:42:item:*` changes, and every subscribed UI updates. Uses WASM scripting (#7) as the function runtime. Biggest lift, biggest ceiling.

Under consideration behind these: a CRDT text type for collaborative editing (likely embedding an existing Rust CRDT rather than building one), and per-key undo/history on top of the existing op-log machinery.

---

Feedback on priorities is welcome — [open an issue](https://github.com/recached-sh/recached/issues) or write to [dennis@thinkgrid.dev](mailto:dennis@thinkgrid.dev).
