import DefaultTheme from 'vitepress/theme'
import type { Theme } from 'vitepress'
import './style.css'

import HomeHero from './components/HomeHero.vue'
import HomeSection from './components/HomeSection.vue'
import FeatureCard from './components/FeatureCard.vue'
import WorkflowSteps from './components/WorkflowSteps.vue'
import RuntimeGrid from './components/RuntimeGrid.vue'
import DocsCta from './components/DocsCta.vue'

export default {
  extends: DefaultTheme,
  enhanceApp({ app }) {
    // Homepage building blocks. They are plain components with no runtime
    // dependencies; any Markdown page may use them.
    app.component('HomeHero', HomeHero)
    app.component('HomeSection', HomeSection)
    app.component('FeatureCard', FeatureCard)
    app.component('WorkflowSteps', WorkflowSteps)
    app.component('RuntimeGrid', RuntimeGrid)
    app.component('DocsCta', DocsCta)
  },
} satisfies Theme
