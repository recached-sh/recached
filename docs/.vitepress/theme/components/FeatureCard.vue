<script setup lang="ts">
import { computed } from 'vue'
import { withBase } from 'vitepress'

// Line icons share the geometry of docs/public/brand/icons/*.svg, inlined so
// they can take the theme's accent color instead of a fixed amber stroke.
const ICONS: Record<string, string> = {
  server:
    '<rect x="3" y="4" width="18" height="7" rx="2"/><rect x="3" y="13" width="18" height="7" rx="2"/><path d="M7 7.5h.01M7 16.5h.01M11 7.5h6M11 16.5h6"/>',
  sync:
    '<path d="M4 12a8 8 0 0 1 13.7-5.6L20 8.7"/><path d="M20 4v4.7h-4.7"/><path d="M20 12a8 8 0 0 1-13.7 5.6L4 15.3"/><path d="M4 20v-4.7h4.7"/>',
  local:
    '<rect x="5" y="5" width="14" height="14" rx="2"/><rect x="9.5" y="9.5" width="5" height="5" rx="1"/><path d="M9 2v3M15 2v3M9 19v3M15 19v3M2 9h3M2 15h3M19 9h3M19 15h3"/>',
  offline: '<path d="M7 18.5h10a4 4 0 0 0 .6-7.96A6 6 0 0 0 6.3 9.6 4.5 4.5 0 0 0 7 18.5z"/><path d="M3 3l18 18"/>',
  scope: '<rect x="5" y="11" width="14" height="10" rx="2"/><path d="M8 11V7a4 4 0 0 1 8 0v4"/><path d="M12 15v2"/>',
  engine: '<path d="M12 2.5l8.2 4.75v9.5L12 21.5l-8.2-4.75v-9.5z"/><circle cx="12" cy="12" r="3"/>',
}

const props = defineProps<{
  icon: keyof typeof ICONS
  title: string
  details: string
  link?: string
  linkText?: string
}>()

const svg = computed(() => ICONS[props.icon] ?? '')
const href = computed(() => (props.link ? withBase(props.link) : undefined))
</script>

<template>
  <article class="rc-card">
    <span class="rc-card-icon" aria-hidden="true">
      <svg viewBox="0 0 24 24" width="20" height="20" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" v-html="svg" />
    </span>
    <h3 class="rc-card-title">{{ title }}</h3>
    <p class="rc-card-details">{{ details }}</p>
    <a v-if="href" class="rc-card-link" :href="href">
      {{ linkText ?? 'Learn more' }}<span class="rc-visually-hidden"> about {{ title.toLowerCase() }}</span>
      <svg viewBox="0 0 16 16" width="14" height="14" aria-hidden="true"><path d="M3 8h10M9 4l4 4-4 4" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round" /></svg>
    </a>
  </article>
</template>

<style scoped>
.rc-card {
  position: relative;
  display: flex;
  flex-direction: column;
  padding: 24px;
  border: 1px solid var(--rc-border);
  border-radius: var(--rc-radius-lg);
  background: var(--rc-surface);
  box-shadow: var(--rc-shadow-sm);
  transition: border-color var(--rc-duration) var(--rc-ease), box-shadow var(--rc-duration) var(--rc-ease);
}

.rc-card:hover {
  border-color: var(--rc-accent-border);
  box-shadow: var(--rc-shadow-md);
}

.rc-card-icon {
  display: inline-grid;
  place-items: center;
  width: 40px;
  height: 40px;
  border-radius: 10px;
  color: var(--rc-amber);
  background: var(--rc-midnight);
  border: 1px solid rgba(245, 158, 11, 0.25);
}

.rc-card-title {
  margin: 20px 0 8px;
  font-size: 17px;
  font-weight: 650;
  letter-spacing: -0.015em;
  line-height: 1.35;
  color: var(--rc-text-1);
}

.rc-card-details {
  margin: 0;
  flex: 1;
  font-size: 15px;
  line-height: 1.6;
  color: var(--rc-text-2);
}

.rc-card-link {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  margin-top: 18px;
  font-size: 14px;
  font-weight: 600;
  color: var(--rc-accent);
  text-decoration: none;
}

.rc-card-link:hover {
  color: var(--rc-accent-strong);
}

.rc-card-link svg {
  transition: transform var(--rc-duration) var(--rc-ease);
}

.rc-card-link:hover svg {
  transform: translateX(2px);
}

.rc-visually-hidden {
  position: absolute;
  width: 1px;
  height: 1px;
  margin: -1px;
  padding: 0;
  overflow: hidden;
  clip: rect(0 0 0 0);
  white-space: nowrap;
  border: 0;
}
</style>
