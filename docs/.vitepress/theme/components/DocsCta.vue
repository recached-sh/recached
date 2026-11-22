<script setup lang="ts">
import { withBase } from 'vitepress'

defineProps<{
  title: string
  body: string
  primary: { text: string; link: string }
  secondary?: { text: string; link: string }
}>()
</script>

<template>
  <section class="rc-cta" aria-labelledby="rc-cta-title">
    <div class="rc-cta-inner">
      <div>
        <h2 id="rc-cta-title" class="rc-cta-title">{{ title }}</h2>
        <p class="rc-cta-body">{{ body }}</p>
      </div>
      <div class="rc-cta-actions">
        <a class="rc-cta-btn is-primary" :href="withBase(primary.link)">{{ primary.text }}</a>
        <a v-if="secondary" class="rc-cta-btn is-secondary" :href="withBase(secondary.link)">{{ secondary.text }}</a>
      </div>
    </div>
  </section>
</template>

<style scoped>
.rc-cta {
  padding: 64px var(--rc-gutter) 0;
}

@media (min-width: 640px) {
  .rc-cta {
    padding: 88px 48px 0;
  }
}

/* A midnight panel in both themes, matching the README banner. */
.rc-cta-inner {
  max-width: var(--rc-content-max);
  margin: 0 auto;
  display: grid;
  gap: 28px;
  align-items: center;
  padding: 40px 28px;
  border-radius: var(--rc-radius-xl);
  background-color: var(--rc-midnight);
  background-image: radial-gradient(rgba(253, 230, 138, 0.07) 1px, transparent 1px);
  background-size: 24px 24px;
  border: 1px solid rgba(245, 158, 11, 0.22);
}

@media (min-width: 768px) {
  .rc-cta-inner {
    grid-template-columns: 1fr auto;
    padding: 48px 56px;
  }
}

.rc-cta-title {
  margin: 0;
  font-size: clamp(24px, 3vw, 32px);
  line-height: 1.2;
  letter-spacing: -0.03em;
  font-weight: 750;
  color: #f5f6f8;
}

.rc-cta-body {
  margin: 10px 0 0;
  max-width: 40em;
  font-size: 16px;
  line-height: 1.6;
  color: #b9c0ce;
}

.rc-cta-actions {
  display: flex;
  flex-wrap: wrap;
  gap: 12px;
}

.rc-cta-btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  min-height: 44px;
  padding: 0 22px;
  border-radius: 10px;
  font-size: 15px;
  font-weight: 600;
  text-decoration: none;
  border: 1px solid transparent;
  transition: background-color var(--rc-duration) var(--rc-ease), border-color var(--rc-duration) var(--rc-ease);
}

.rc-cta-btn.is-primary {
  background: var(--rc-amber);
  color: var(--rc-midnight);
}

.rc-cta-btn.is-primary:hover {
  background: var(--rc-gold);
}

.rc-cta-btn.is-secondary {
  color: #f5f6f8;
  border-color: rgba(255, 255, 255, 0.22);
}

.rc-cta-btn.is-secondary:hover {
  border-color: var(--rc-gold);
}

.rc-cta-btn:focus-visible {
  box-shadow: 0 0 0 2px var(--rc-midnight), 0 0 0 4px var(--rc-gold);
}

@media (max-width: 639px) {
  .rc-cta-btn {
    flex: 1 1 100%;
  }
}
</style>
