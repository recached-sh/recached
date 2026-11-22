<script setup lang="ts">
import { useData, withBase } from 'vitepress'
import HeroVisual from './HeroVisual.vue'

// Copy lives in docs/index.md frontmatter (`rcHero`) so it can be edited
// without touching the component.
const { frontmatter } = useData()
const hero = frontmatter.value.rcHero as {
  eyebrow: string
  line1: string
  line2: string
  lead: string
  primary: { text: string; link: string }
  secondary: { text: string; link: string }
  github: string
}
</script>

<template>
  <section class="rc-hero" aria-labelledby="rc-hero-heading">
    <div class="rc-hero-grid" aria-hidden="true" />
    <div class="rc-hero-inner">
      <div class="rc-hero-copy">
        <p class="rc-hero-eyebrow">
          <img :src="withBase('/brand/logo.svg')" alt="" width="22" height="22" />
          <span>{{ hero.eyebrow }}</span>
        </p>

        <h1 id="rc-hero-heading" class="rc-hero-slogan">
          <span class="rc-hero-line1">{{ hero.line1 }}</span>
          <span class="rc-hero-line2">{{ hero.line2 }}</span>
        </h1>

        <p class="rc-hero-lead">{{ hero.lead }}</p>

        <div class="rc-hero-actions">
          <a class="rc-btn rc-btn-primary" :href="withBase(hero.primary.link)">
            {{ hero.primary.text }}
            <svg viewBox="0 0 16 16" width="16" height="16" aria-hidden="true"><path d="M3 8h10M9 4l4 4-4 4" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round" /></svg>
          </a>
          <a class="rc-btn rc-btn-secondary" :href="withBase(hero.secondary.link)">{{ hero.secondary.text }}</a>
        </div>

        <a class="rc-hero-github" :href="hero.github" target="_blank" rel="noopener">
          <svg viewBox="0 0 24 24" width="16" height="16" aria-hidden="true"><path fill="currentColor" d="M12 .3a12 12 0 0 0-3.8 23.4c.6.1.8-.3.8-.6v-2.2c-3.3.7-4-1.4-4-1.4-.6-1.4-1.4-1.8-1.4-1.8-1-.7.1-.7.1-.7 1.2.1 1.8 1.2 1.8 1.2 1 1.8 2.8 1.3 3.5 1 0-.8.4-1.3.7-1.6-2.7-.3-5.5-1.3-5.5-6 0-1.2.5-2.3 1.3-3.1-.2-.4-.6-1.6 0-3.2 0 0 1-.3 3.4 1.2a11.5 11.5 0 0 1 6 0C17.3 4.6 18.3 5 18.3 5c.7 1.6.2 2.9.1 3.2.8.8 1.3 1.9 1.3 3.2 0 4.6-2.8 5.6-5.5 5.9.4.4.8 1.1.8 2.2v3.3c0 .3.2.7.8.6A12 12 0 0 0 12 .3" /></svg>
          <span>recached-sh/recached on GitHub</span>
        </a>
      </div>

      <div class="rc-hero-visual">
        <HeroVisual />
      </div>
    </div>
  </section>
</template>

<style scoped>
.rc-hero {
  position: relative;
  overflow: hidden;
  border-bottom: 1px solid var(--rc-divider);
}

/* A faint dot grid that fades out toward the edges. */
.rc-hero-grid {
  position: absolute;
  inset: 0;
  background-image: radial-gradient(var(--rc-grid-dot) 1px, transparent 1px);
  background-size: 24px 24px;
  mask-image: radial-gradient(ellipse 80% 70% at 70% 40%, #000 30%, transparent 75%);
  -webkit-mask-image: radial-gradient(ellipse 80% 70% at 70% 40%, #000 30%, transparent 75%);
  pointer-events: none;
}

.rc-hero-inner {
  position: relative;
  max-width: var(--rc-content-max);
  margin: 0 auto;
  padding: 56px var(--rc-gutter) 64px;
  display: grid;
  gap: 48px;
}

@media (min-width: 640px) {
  .rc-hero-inner {
    padding: 80px 48px 88px;
  }
}

@media (min-width: 960px) {
  .rc-hero-inner {
    grid-template-columns: minmax(0, 58fr) minmax(0, 42fr);
    align-items: center;
    gap: 56px;
    padding: 104px 64px 112px;
  }
}

.rc-hero-eyebrow {
  display: inline-flex;
  align-items: center;
  gap: 10px;
  margin: 0 0 28px;
  font-family: var(--rc-font-mono);
  font-size: 12px;
  font-weight: 600;
  letter-spacing: 0.12em;
  text-transform: uppercase;
  color: var(--rc-text-2);
}

.rc-hero-eyebrow img {
  border-radius: 6px;
  flex: none;
}

/* The slogan is the largest text on the page, on two deliberate lines. Sizes
   are chosen so each line fits its column at every width without wrapping
   (measured at 320, 375, 768, 960, 1024, 1440 and 1920px). */
.rc-hero-slogan {
  margin: 0;
  font-size: clamp(32px, 9.4vw, 68px);
  line-height: 1.02;
  letter-spacing: -0.038em;
  font-weight: 800;
  color: var(--rc-text-1);
  text-wrap: balance;
}

@media (min-width: 960px) {
  .rc-hero-slogan {
    font-size: clamp(48px, 4.9vw, 68px);
  }
}

.rc-hero-slogan span {
  display: block;
  white-space: nowrap;
}

.rc-hero-line2 {
  color: var(--rc-slogan-accent);
}

.rc-hero-lead {
  margin: 28px 0 0;
  max-width: 34em;
  font-size: clamp(17px, 1.6vw, 19px);
  line-height: 1.6;
  color: var(--rc-text-2);
}

.rc-hero-actions {
  display: flex;
  flex-wrap: wrap;
  gap: 12px;
  margin-top: 36px;
}

.rc-btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 8px;
  min-height: 44px;
  padding: 0 22px;
  border-radius: 10px;
  border: 1px solid transparent;
  font-size: 15px;
  font-weight: 600;
  letter-spacing: -0.005em;
  text-decoration: none;
  transition:
    background-color var(--rc-duration) var(--rc-ease),
    border-color var(--rc-duration) var(--rc-ease),
    color var(--rc-duration) var(--rc-ease),
    transform var(--rc-duration) var(--rc-ease);
}

.rc-btn-primary {
  background: var(--rc-amber);
  color: var(--rc-midnight);
}

.rc-btn-primary:hover {
  background: var(--rc-gold);
}

.rc-btn-primary svg {
  transition: transform var(--rc-duration) var(--rc-ease);
}

.rc-btn-primary:hover svg {
  transform: translateX(2px);
}

.rc-btn-secondary {
  background: var(--rc-surface);
  border-color: var(--rc-border-strong);
  color: var(--rc-text-1);
}

.rc-btn-secondary:hover {
  border-color: var(--rc-accent-border);
  background: var(--rc-surface-raised);
}

.rc-hero-github {
  display: inline-flex;
  align-items: center;
  gap: 8px;
  margin-top: 24px;
  font-size: 14px;
  font-weight: 500;
  color: var(--rc-text-2);
  text-decoration: none;
}

.rc-hero-github:hover {
  color: var(--rc-text-1);
}

.rc-hero-visual {
  max-width: 520px;
  width: 100%;
  margin: 0 auto;
  padding: 24px;
  border: 1px solid var(--rc-border);
  border-radius: var(--rc-radius-xl);
  background: color-mix(in srgb, var(--rc-bg) 70%, transparent);
  box-shadow: var(--rc-shadow-lg);
}

@media (max-width: 479px) {
  .rc-hero-eyebrow {
    font-size: 11px;
    letter-spacing: 0.06em;
    gap: 8px;
  }
}

@media (max-width: 639px) {
  .rc-btn {
    flex: 1 1 100%;
  }
  .rc-hero-visual {
    padding: 14px;
  }
}
</style>
