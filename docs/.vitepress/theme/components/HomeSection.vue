<script setup lang="ts">
defineProps<{
  id?: string
  eyebrow?: string
  title: string
  lead?: string
  tone?: 'default' | 'alt'
}>()
</script>

<template>
  <section class="rc-section" :class="tone === 'alt' && 'rc-section-alt'" :aria-labelledby="id ? `${id}-title` : undefined">
    <div class="rc-section-inner">
      <header class="rc-section-head">
        <p v-if="eyebrow" class="rc-section-eyebrow">{{ eyebrow }}</p>
        <h2 :id="id ? `${id}-title` : undefined" class="rc-section-title">{{ title }}</h2>
        <p v-if="lead" class="rc-section-lead">{{ lead }}</p>
      </header>
      <slot />
    </div>
  </section>
</template>

<style scoped>
.rc-section {
  border-bottom: 1px solid var(--rc-divider);
}

.rc-section-alt {
  background: var(--rc-bg-alt);
}

.rc-section-inner {
  max-width: var(--rc-content-max);
  margin: 0 auto;
  padding: 64px var(--rc-gutter);
}

@media (min-width: 640px) {
  .rc-section-inner {
    padding: 88px 48px;
  }
}

@media (min-width: 960px) {
  .rc-section-inner {
    padding: 104px 64px;
  }
}

.rc-section-head {
  max-width: 720px;
  margin-bottom: 40px;
}

.rc-section-eyebrow {
  margin: 0 0 12px;
  font-family: var(--rc-font-mono);
  font-size: 12px;
  font-weight: 600;
  letter-spacing: 0.12em;
  text-transform: uppercase;
  color: var(--rc-accent);
}

.rc-section-title {
  margin: 0;
  font-size: clamp(26px, 3.4vw, 36px);
  line-height: 1.15;
  letter-spacing: -0.03em;
  font-weight: 750;
  color: var(--rc-text-1);
  text-wrap: balance;
}

.rc-section-lead {
  margin: 16px 0 0;
  font-size: 17px;
  line-height: 1.65;
  color: var(--rc-text-2);
}
</style>
