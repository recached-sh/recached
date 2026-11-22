---
layout: home
markdownStyles: false
title: 'Recached — More than a cache. Built to sync.'
titleTemplate: false
description: 'A Rust-powered caching engine with real-time synchronization built in. Keep cached data close to your applications and synchronize updates across connected clients.'

# Homepage copy. Every claim here must match guide/introduction.md and
# guide/client-support.md; statuses in `runtimes` are the maturity labels used
# there.
rcHero:
  eyebrow: 'Recached — Rust-powered caching & sync'
  line1: 'More than a cache.'
  line2: 'Built to sync.'
  lead: 'A Rust-powered caching engine with real-time synchronization built in. Keep cached data close to your applications and synchronize updates across connected clients.'
  primary: { text: 'Get Started', link: '/guide/quick-start.html' }
  secondary: { text: 'Explore Documentation', link: '/guide/introduction.html' }
  github: 'https://github.com/recached-sh/recached'
---

<HomeHero />

<HomeSection id="capabilities" eyebrow="Capabilities" title="A cache server and the clients that read it locally" lead="Recached is an in-memory key-value cache. The same Rust engine also runs inside your browser, mobile and Rust clients, which hold the keys they watch and receive changes as they happen.">
  <div class="rc-cards">
    <FeatureCard icon="server" title="In-memory cache server" details="Strings, expiry, counters, hashes, lists, sets, sorted sets and JSON documents. Your backend connects with ioredis, node-redis, redis-py or redis-cli over a supported RESP command subset." link="/server/commands.html" link-text="Supported commands" />
    <FeatureCard icon="sync" title="Real-time sync, built in" details="Clients subscribe to key patterns, receive a snapshot and then each change over WebSocket, and re-hydrate what they watch after reconnecting." link="/guide/how-it-works.html#connect-and-hydrate" link-text="Connect and hydrate" />
    <FeatureCard icon="local" title="Local reads" details="Browser, Android, Apple and embedded Rust clients read cached values from their own memory instead of making a network request for every read." link="/guide/how-it-works.html#read-and-write-locally" link-text="Read and write locally" />
    <FeatureCard icon="offline" title="Offline-capable clients" details="Browser data can persist to IndexedDB and native data to SQLite. Writes made while disconnected queue and replay on reconnect, within outbox limits." link="/guide/client-support.html#write-durability-and-replay" link-text="Durability and replay" />
    <FeatureCard icon="scope" title="Scoped sync access" details="Tokens minted by your backend limit which keys each connection may read and write. Watching a pattern never grants access to it." link="/server/sync-scopes.html" link-text="Sync scopes" />
    <FeatureCard icon="engine" title="One Rust engine" details="The server and every client run the same core-engine. Each platform adds its own transport, persistence and reactive API, so capabilities differ by client." link="/guide/client-support.html" link-text="Compare clients" />
  </div>
</HomeSection>

<HomeSection id="workflow" tone="alt" eyebrow="Workflow" title="From docker run to a live UI" lead="Your backend keeps writing with the Redis client it already has. Subscribed clients receive the change and read it from local memory.">
<WorkflowSteps :steps="[
  { title: 'Run Recached', body: 'Start the server with Docker, Homebrew or Cargo. Port 6379 speaks RESP; port 6380 serves WebSocket sync.' },
  { title: 'Write from your backend', body: 'Use any Redis client with the supported command subset. No Recached-specific driver is needed.' },
  { title: 'Watch a key pattern', body: 'A client registers a live query, receives the matching keys, then every later change to them.' },
  { title: 'Read locally', body: 'Reads come from the client’s own memory. Change listeners fire when the server pushes an update.' },
]">

::: code-group

```bash [Server]
docker run -p 6379:6379 -p 6380:6380 ghcr.io/recached-sh/recached:latest

redis-cli ping
# PONG
```

```ts [Backend]
import Redis from 'ioredis'

const cache = new Redis('redis://127.0.0.1:6379')
await cache.set('inventory:item:99', '42')
```

```ts [Browser]
import { createCache } from 'recached-edge'

const cache = await createCache({
  connect: { url: 'ws://127.0.0.1:6380' },
})

cache.liveQuery('inventory:*') // snapshot, then live changes
cache.onKeyChange('inventory:item:99', () => {
  render(cache.get('inventory:item:99')) // a local read
})
```

:::

These are plaintext development endpoints. Read [security](/server/security) before exposing either port.

</WorkflowSteps>
</HomeSection>

<HomeSection id="architecture" eyebrow="Architecture" title="One engine, from the server to every client" lead="Backend services write over RESP. Clients sync over WebSocket and serve reads from a local cache. Your database stays the source of truth.">
  <figure class="rc-figure">
    <img class="light-only" src="/architecture-light.svg" width="1040" height="420" alt="Your database stays the source of truth; your backend publishes committed state to the Recached server over RESP on port 6379. Browser, Android and iOS, and Rust service clients sync over WebSocket on port 6380 and read from a local cache." />
    <img class="dark-only" src="/architecture-dark.svg" width="1040" height="420" alt="Your database stays the source of truth; your backend publishes committed state to the Recached server over RESP on port 6379. Browser, Android and iOS, and Rust service clients sync over WebSocket on port 6380 and read from a local cache." />
  </figure>
  <div class="rc-arch-notes">
    <p><strong>A cache, not a system of record.</strong> Keep PostgreSQL or MySQL as the source of truth and publish committed state into Recached. A Recached acknowledgment confirms a cache write, not a database commit.</p>
    <p><a href="/guide/how-it-works.html">How it works</a> covers hydration, reconnect replay and each client’s storage contract. <a href="/guide/database-integration.html">Database integration</a> shows the outbox pattern.</p>
  </div>
</HomeSection>

<HomeSection id="runtimes" tone="alt" eyebrow="Clients and SDKs" title="Choose your runtime" lead="APIs, persistence and guarantees differ by client. Each status below is the project’s own maturity label.">
<RuntimeGrid :items="[
  { name: 'Server', package: 'Docker · Homebrew · cargo install --git', status: 'Release candidate for cache workloads', tone: 'rc', link: '/server/installation.html' },
  { name: 'Browser', package: 'recached-edge (WebAssembly)', status: 'Sync layer beta', tone: 'beta', link: '/browser/getting-started.html' },
  { name: 'React and Vue', package: '@recached/react · @recached/vue', status: 'Sync layer beta', tone: 'beta', link: '/react/getting-started.html' },
  { name: 'Android', package: 'dev.recached:recached-android', status: '0.1.2 preview', tone: 'preview', link: '/android/getting-started.html' },
  { name: 'iOS and macOS', package: 'Recached via SwiftPM', status: '0.1.2 preview', tone: 'preview', link: '/ios/getting-started.html' },
  { name: 'Rust service', package: 'recached-embed (Git dependency)', status: 'Unpublished', tone: 'unpublished', link: '/rust/getting-started.html' },
]" />
  <p class="rc-runtimes-note">React Native, Flutter and a WASI build are <a href="/roadmap.html">planned</a>, not implemented. See <a href="/guide/client-support.html">client support and limits</a>.</p>
</HomeSection>

<DocsCta title="Start with the quick start" body="Run a server, write a key from your backend, and watch the change arrive in a browser client." :primary="{ text: 'Get Started', link: '/guide/quick-start.html' }" :secondary="{ text: 'Explore Documentation', link: '/guide/introduction.html' }" />

<style>
.rc-cards {
  display: grid;
  gap: 16px;
  grid-template-columns: repeat(auto-fill, minmax(min(100%, 300px), 1fr));
}

.rc-arch-notes {
  display: grid;
  gap: 8px 40px;
  margin-top: 32px;
  font-size: 15px;
  line-height: 1.65;
  color: var(--rc-text-2);
}

@media (min-width: 960px) {
  .rc-arch-notes {
    grid-template-columns: 1fr 1fr;
  }
}

.rc-arch-notes p,
.rc-runtimes-note {
  margin: 0;
}

.rc-arch-notes strong {
  color: var(--rc-text-1);
}

.rc-arch-notes a,
.rc-runtimes-note a,
.rc-flow-code > p a {
  color: var(--rc-accent);
  font-weight: 500;
  text-decoration: underline;
  text-decoration-color: var(--rc-accent-border);
  text-underline-offset: 3px;
}

.rc-runtimes-note {
  margin-top: 20px;
  font-size: 14px;
  color: var(--rc-text-3);
}
</style>
