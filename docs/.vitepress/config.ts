import { defineConfig } from 'vitepress'

export default defineConfig({
  // Served from the root of the custom domain (recached.dev). This was
  // '/recached/' for the github.io project-pages URL — leaving it set there
  // makes every asset 404 on a custom domain, which renders the site as
  // unstyled HTML.
  base: '/',
  // Working plans for upcoming milestones live beside the docs but are not
  // part of the published site.
  srcExclude: ['plans/**'],
  title: 'Recached',
  titleTemplate: ':title — Recached',
  description:
    'A Rust cache server that runs on your backend and inside the browser. Zero-latency local reads. Automatic WebSocket sync. Works as a Redis drop-in on the server and a WASM module in the browser.',

  head: [
    ['meta', { property: 'og:type', content: 'website' }],
    ['meta', { property: 'og:image', content: 'https://recached.dev/recached.jpg' }],
    [
      'meta',
      {
        property: 'og:description',
        content:
          'A Rust cache server that runs on your backend and inside the browser. Zero-latency local reads. Automatic WebSocket sync.',
      },
    ],
    ['meta', { name: 'twitter:card', content: 'summary_large_image' }],
    ['meta', { name: 'twitter:image', content: 'https://recached.dev/recached.jpg' }],
    ['meta', { name: 'keywords', content: 'rust cache, redis alternative, redis compatible, webassembly cache, wasm browser cache, local-first, zero-latency reads, websocket sync, in-memory cache, browser cache, edge cache, real-time sync, offline-first, indexeddb cache, pub-sub' }],
  ],

  themeConfig: {
    siteTitle: 'Recached ⚡',

    nav: [
      { text: 'Home', link: '/' },
      { text: 'Guide', link: '/guide/introduction' },
      { text: 'Server', link: '/server/installation' },
      { text: 'Browser', link: '/browser/getting-started' },
      { text: 'React', link: '/react/getting-started' },
      { text: 'Vue', link: '/vue/getting-started' },
      { text: 'Rust', link: '/rust/getting-started' },
      { text: 'Roadmap', link: '/roadmap' },
    ],

    sidebar: {
      '/guide/': [
        {
          text: 'Guide',
          items: [
            { text: 'Introduction', link: '/guide/introduction' },
            { text: 'Quick Start', link: '/guide/quick-start' },
            { text: 'Use Cases', link: '/guide/use-cases' },
            { text: 'How It Works', link: '/guide/how-it-works' },
            { text: 'Benchmarks', link: '/guide/benchmarks' },
          ],
        },
      ],
      '/server/': [
        {
          text: 'Server',
          items: [
            { text: 'Installation', link: '/server/installation' },
            { text: 'Configuration', link: '/server/configuration' },
            { text: 'Commands', link: '/server/commands' },
            { text: 'Sync Scopes', link: '/server/sync-scopes' },
            { text: 'Security', link: '/server/security' },
            { text: 'Operations', link: '/server/operations' },
            { text: 'Troubleshooting', link: '/server/troubleshooting' },
            { text: 'Wire Protocol', link: '/server/protocol' },
          ],
        },
      ],
      '/browser/': [
        {
          text: 'Browser (WASM)',
          items: [
            { text: 'Getting Started', link: '/browser/getting-started' },
            { text: 'API Reference', link: '/browser/api-reference' },
            { text: 'Persistence', link: '/browser/persistence' },
            { text: 'Offline & Reconnection', link: '/browser/offline' },
          ],
        },
      ],
      '/react/': [
        {
          text: 'React',
          items: [
            { text: 'Getting Started', link: '/react/getting-started' },
            { text: 'Hooks Reference', link: '/react/hooks-reference' },
          ],
        },
      ],
      '/vue/': [
        {
          text: 'Vue',
          items: [
            { text: 'Getting Started', link: '/vue/getting-started' },
            { text: 'Composables Reference', link: '/vue/composables-reference' },
          ],
        },
      ],
      '/rust/': [
        {
          text: 'Rust (Embedded)',
          items: [
            { text: 'Getting Started', link: '/rust/getting-started' },
            { text: 'API Reference', link: '/rust/api-reference' },
          ],
        },
      ],
    },

    socialLinks: [
      { icon: 'github', link: 'https://github.com/recached-sh/recached' },
    ],

    footer: {
      message: 'Released under the Apache License 2.0.',
      copyright: 'Copyright © 2026 ThinkGrid Labs',
    },

    editLink: {
      pattern: 'https://github.com/recached-sh/recached/edit/main/docs/:path',
      text: 'Edit this page on GitHub',
    },

    lastUpdated: true,
  },

  sitemap: {
    hostname: 'https://recached.dev',
  },
})
