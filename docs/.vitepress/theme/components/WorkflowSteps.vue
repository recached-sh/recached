<script setup lang="ts">
// Numbered steps beside a code sample. The default slot holds Markdown (a
// `::: code-group`), so code is highlighted at build time by Shiki and no
// highlighter ships to the browser.
defineProps<{ steps: { title: string; body: string }[] }>()
</script>

<template>
  <div class="rc-flow">
    <ol class="rc-flow-steps">
      <li v-for="(s, i) in steps" :key="i" class="rc-flow-step">
        <span class="rc-flow-num" aria-hidden="true">{{ String(i + 1).padStart(2, '0') }}</span>
        <div>
          <h3 class="rc-flow-title">{{ s.title }}</h3>
          <p class="rc-flow-body">{{ s.body }}</p>
        </div>
      </li>
    </ol>
    <div class="rc-flow-code vp-doc">
      <slot />
    </div>
  </div>
</template>

<style scoped>
.rc-flow {
  display: grid;
  gap: 40px;
}

@media (min-width: 960px) {
  .rc-flow {
    grid-template-columns: minmax(0, 5fr) minmax(0, 7fr);
    gap: 56px;
    align-items: start;
  }
}

.rc-flow-steps {
  list-style: none;
  margin: 0;
  padding: 0;
  display: grid;
  gap: 4px;
}

.rc-flow-step {
  display: grid;
  grid-template-columns: 40px 1fr;
  gap: 12px;
  padding: 16px 0;
  border-top: 1px solid var(--rc-divider);
}

.rc-flow-step:first-child {
  border-top: 0;
  padding-top: 0;
}

.rc-flow-num {
  font-family: var(--rc-font-mono);
  font-size: 13px;
  font-weight: 600;
  line-height: 26px;
  color: var(--rc-accent);
}

.rc-flow-title {
  margin: 0;
  font-size: 17px;
  font-weight: 650;
  letter-spacing: -0.012em;
  line-height: 26px;
  color: var(--rc-text-1);
}

.rc-flow-body {
  margin: 4px 0 0;
  font-size: 15px;
  line-height: 1.6;
  color: var(--rc-text-2);
}

.rc-flow-code {
  min-width: 0;
}

.rc-flow-code :deep(.vp-code-group) {
  margin-top: 0;
}

.rc-flow-code :deep(> p) {
  font-size: 14px;
  color: var(--rc-text-3);
}
</style>
