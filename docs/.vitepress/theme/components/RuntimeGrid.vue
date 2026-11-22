<script setup lang="ts">
import { withBase } from 'vitepress'

// Status labels must match the maturity statements in guide/introduction.md
// and guide/client-support.md. Tones only change the badge color.
defineProps<{
  items: {
    name: string
    package: string
    status: string
    tone: 'rc' | 'beta' | 'preview' | 'unpublished'
    link: string
  }[]
}>()
</script>

<template>
  <ul class="rc-runtimes">
    <li v-for="r in items" :key="r.name">
      <a class="rc-runtime" :href="withBase(r.link)">
        <span class="rc-runtime-name">{{ r.name }}</span>
        <code class="rc-runtime-pkg">{{ r.package }}</code>
        <span class="rc-runtime-status" :class="`is-${r.tone}`">{{ r.status }}</span>
      </a>
    </li>
  </ul>
</template>

<style scoped>
.rc-runtimes {
  list-style: none;
  margin: 0;
  padding: 0;
  display: grid;
  gap: 12px;
  grid-template-columns: repeat(auto-fill, minmax(min(100%, 280px), 1fr));
}

.rc-runtime {
  display: grid;
  gap: 6px;
  height: 100%;
  padding: 18px 20px;
  border: 1px solid var(--rc-border);
  border-radius: var(--rc-radius-md);
  background: var(--rc-surface);
  text-decoration: none;
  transition: border-color var(--rc-duration) var(--rc-ease), background-color var(--rc-duration) var(--rc-ease);
}

.rc-runtime:hover {
  border-color: var(--rc-accent-border);
  background: var(--rc-surface-raised);
}

.rc-runtime-name {
  font-size: 16px;
  font-weight: 650;
  color: var(--rc-text-1);
}

.rc-runtime-pkg {
  font-family: var(--rc-font-mono);
  font-size: 12.5px;
  color: var(--rc-text-2);
  overflow-wrap: anywhere;
}

.rc-runtime-status {
  justify-self: start;
  margin-top: 6px;
  padding: 2px 9px;
  border-radius: 999px;
  border: 1px solid var(--rc-border-strong);
  font-size: 12px;
  font-weight: 600;
  color: var(--rc-text-2);
}

.rc-runtime-status.is-rc,
.rc-runtime-status.is-beta {
  border-color: var(--rc-accent-border);
  background: var(--rc-accent-soft);
  color: var(--rc-accent-strong);
}
</style>
