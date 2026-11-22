<script setup lang="ts">
// The homepage illustration. It shows only connections that exist: backend
// RESP clients talk to the server on :6379; browser, native and embedded Rust
// clients sync over WebSocket on :6380 and keep a local cache they read from.
// Colors come from theme tokens, so it follows the light/dark switch.
</script>

<template>
  <svg
    class="rc-hv"
    viewBox="0 0 480 448"
    role="img"
    aria-labelledby="rc-hv-title rc-hv-desc"
    xmlns="http://www.w3.org/2000/svg"
  >
    <title id="rc-hv-title">How Recached connects your backend and clients</title>
    <desc id="rc-hv-desc">
      Your backend writes to the Recached server over RESP on port 6379. Browser, Android and iOS, and
      Rust service clients sync with the server over WebSocket on port 6380 and read from their own
      local cache.
    </desc>

    <!-- Links -->
    <g class="rc-hv-link">
      <path d="M240 72V140" />
      <path d="M240 252V300M80 300H400M80 300V336M240 300V336M400 300V336" />
    </g>
    <!-- Flow: updates travel from the backend through the server to clients. -->
    <g class="rc-hv-flow" aria-hidden="true">
      <path d="M240 72V140" />
      <path d="M240 252V300H80V336" />
      <path d="M240 252V336" />
      <path d="M240 252V300H400V336" />
    </g>

    <text class="rc-hv-label" x="252" y="112">RESP :6379</text>
    <text class="rc-hv-label" x="252" y="286">WebSocket :6380</text>

    <!-- Backend -->
    <rect class="rc-hv-node" x="130" y="8" width="220" height="64" rx="14" />
    <text class="rc-hv-title" x="240" y="36" text-anchor="middle">Your backend</text>
    <text class="rc-hv-sub" x="240" y="56" text-anchor="middle">any Redis client</text>

    <!-- Server, with the Connected R mark (original geometry) -->
    <rect class="rc-hv-server" x="80" y="140" width="320" height="112" rx="18" />
    <g transform="translate(102 166) scale(0.47)">
      <rect class="rc-hv-logo-tile" x="4" y="4" width="120" height="120" rx="28" />
      <path
        d="M38 94V34H61C84 34 86 64 63 67H56L83 94"
        stroke="#F59E0B"
        stroke-width="11"
        stroke-linecap="square"
        stroke-linejoin="round"
        fill="none"
      />
      <circle cx="92" cy="27" r="7" fill="#FDE68A" />
      <circle cx="94" cy="96" r="6" fill="#FBBF24" />
    </g>
    <text class="rc-hv-title rc-hv-title-lg" x="176" y="194">Recached server</text>
    <text class="rc-hv-sub" x="176" y="218">in-memory cache · sync fan-out</text>

    <!-- Clients -->
    <g v-for="(c, i) in [
      { x: 10, t: 'Browser', s: 'WebAssembly' },
      { x: 170, t: 'Android · iOS', s: 'Kotlin · Swift' },
      { x: 330, t: 'Rust service', s: 'recached-embed' },
    ]" :key="i">
      <rect class="rc-hv-node" :x="c.x" y="336" width="140" height="104" rx="14" />
      <text class="rc-hv-title" :x="c.x + 70" y="364" text-anchor="middle">{{ c.t }}</text>
      <text class="rc-hv-sub" :x="c.x + 70" y="383" text-anchor="middle">{{ c.s }}</text>
      <rect class="rc-hv-chip" :x="c.x + 16" y="400" width="108" height="26" rx="7" />
      <circle class="rc-hv-chip-dot" :cx="c.x + 31" cy="413" r="3" />
      <text class="rc-hv-chip-text" :x="c.x + 78" y="417" text-anchor="middle">local cache</text>
    </g>
  </svg>
</template>

<style scoped>
.rc-hv {
  display: block;
  width: 100%;
  height: auto;
  font-family: var(--rc-font-sans);
  overflow: visible;
}

.rc-hv-link path {
  fill: none;
  stroke: var(--rc-line);
  stroke-width: 1.5;
}

.rc-hv-flow path {
  fill: none;
  stroke: var(--rc-amber);
  stroke-width: 2;
  stroke-linecap: round;
  stroke-dasharray: 2 14;
  opacity: 0.9;
}

@media (prefers-reduced-motion: no-preference) {
  .rc-hv-flow path {
    animation: rc-hv-flow 1.6s linear infinite;
  }
}

@keyframes rc-hv-flow {
  to {
    stroke-dashoffset: -32;
  }
}

.rc-hv-node {
  fill: var(--rc-surface);
  stroke: var(--rc-border-strong);
  stroke-width: 1;
}

.rc-hv-server {
  fill: var(--rc-surface-raised);
  stroke: var(--rc-amber);
  stroke-width: 1.5;
}

.rc-hv-logo-tile {
  fill: #171c2b;
  stroke: rgba(245, 158, 11, 0.35);
  stroke-width: 2;
}

.rc-hv-title {
  fill: var(--rc-text-1);
  font-size: 14px;
  font-weight: 650;
  letter-spacing: -0.01em;
}

.rc-hv-title-lg {
  font-size: 17px;
  font-weight: 700;
}

.rc-hv-sub {
  fill: var(--rc-text-3);
  font-family: var(--rc-font-mono);
  font-size: 11px;
}

.rc-hv-label {
  fill: var(--rc-text-2);
  font-family: var(--rc-font-mono);
  font-size: 11px;
  font-weight: 600;
}

.rc-hv-chip {
  fill: var(--rc-accent-soft);
  stroke: var(--rc-accent-border);
  stroke-width: 1;
}

.rc-hv-chip-dot {
  fill: var(--rc-amber);
}

.rc-hv-chip-text {
  fill: var(--rc-accent-strong);
  font-family: var(--rc-font-mono);
  font-size: 11px;
  font-weight: 600;
}
</style>
