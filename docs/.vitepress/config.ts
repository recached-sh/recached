import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import { defineConfig, type HeadConfig } from 'vitepress'

const SITE_URL = 'https://recached.dev'
const SLOGAN = 'More than a cache. Built to sync.'
const DESCRIPTION =
  'A Rust-powered caching engine with real-time synchronization built in. Keep cached data close to your applications and synchronize updates across connected clients.'
const SOCIAL_IMAGE = `${SITE_URL}/brand/social-card.png`

// The published URL of a page. cleanUrls is off, so pages are served as
// `/guide/quick-start.html`, which is also what the sitemap lists — the
// canonical has to match it exactly or search engines see two URLs.
function pageUrl(relativePath: string): string {
  const path = relativePath.replace(/(^|\/)index\.md$/, '$1').replace(/\.md$/, '.html')
  return `${SITE_URL}/${path}`
}

// The first prose paragraph of a Markdown page, as plain text, for pages that
// set no `description` in frontmatter. Skips headings, code fences, custom
// containers (::: … :::), tables, lists, quotes and HTML blocks.
function firstParagraph(markdown: string): string | undefined {
  const body = markdown.replace(/^---\n[\s\S]*?\n---\n/, '')
  let fence = false
  let container = 0
  let para: string[] = []
  for (const raw of body.split('\n')) {
    const line = raw.trim()
    if (line.startsWith('```')) {
      fence = !fence
      continue
    }
    if (fence) continue
    if (line.startsWith(':::')) {
      container += line === ':::' ? -1 : 1
      continue
    }
    if (container > 0) continue
    if (line === '') {
      if (para.length) break
      continue
    }
    if (/^(#|\||[-*+] |\d+\. |>|<|!\[)/.test(line)) {
      if (para.length) break
      continue
    }
    para.push(line)
  }
  const text = para
    .join(' ')
    .replace(/\[([^\]]+)\]\([^)]*\)/g, '$1')
    .replace(/`([^`]+)`/g, '$1')
    .replace(/(\*\*|__|\*|_)(\S[^*_]*?)\1/g, '$2')
    .replace(/<[^>]+>/g, '')
    .replace(/\s+/g, ' ')
    .trim()
  if (!text) return undefined
  if (text.length <= 160) return text
  const cut = text.slice(0, 157)
  return cut.slice(0, cut.lastIndexOf(' ')) + '…'
}

export default defineConfig({
  // Served from the root of the custom domain (recached.dev). This was
  // '/recached/' for the github.io project-pages URL — leaving it set there
  // makes every asset 404 on a custom domain, which renders the site as
  // unstyled HTML.
  base: '/',
  // Working plans for upcoming milestones and internal reports live beside the
  // docs but are not part of the published site. reports/ is also gitignored.
  srcExclude: ['plans/**', 'reports/**'],
  lang: 'en-US',
  title: 'Recached',
  titleTemplate: ':title — Recached',
  description: DESCRIPTION,
  cleanUrls: false,
  lastUpdated: true,

  head: [
    ['link', { rel: 'icon', type: 'image/svg+xml', href: '/favicon.svg' }],
    ['meta', { name: 'theme-color', content: '#171C2B' }],
    ['meta', { name: 'color-scheme', content: 'light dark' }],
    ['meta', { property: 'og:type', content: 'website' }],
    ['meta', { property: 'og:site_name', content: 'Recached' }],
    ['meta', { property: 'og:image', content: SOCIAL_IMAGE }],
    ['meta', { property: 'og:image:width', content: '1200' }],
    ['meta', { property: 'og:image:height', content: '630' }],
    ['meta', { property: 'og:image:alt', content: `Recached — ${SLOGAN}` }],
    ['meta', { name: 'twitter:card', content: 'summary_large_image' }],
    ['meta', { name: 'twitter:image', content: SOCIAL_IMAGE }],
  ],

  transformPageData(pageData, { siteConfig }) {
    if (pageData.isNotFound || pageData.frontmatter.description) return
    try {
      const source = readFileSync(join(siteConfig.srcDir, pageData.filePath), 'utf8')
      const description = firstParagraph(source)
      if (description) pageData.description = description
    } catch {
      // Keep the site description.
    }
  },

  // Per-page canonical URL and social metadata. Titles and descriptions come
  // from each page's frontmatter (or its first heading), so a shared link
  // previews the page that was shared, not the homepage.
  transformHead({ pageData, title, description }) {
    if (pageData.isNotFound) return []
    const url = pageUrl(pageData.relativePath)
    const isHome = pageData.relativePath === 'index.md'
    const ogTitle = isHome ? `Recached — ${SLOGAN}` : title
    const head: HeadConfig[] = [
      ['link', { rel: 'canonical', href: url }],
      ['meta', { property: 'og:url', content: url }],
      ['meta', { property: 'og:title', content: ogTitle }],
      ['meta', { property: 'og:description', content: description }],
      ['meta', { name: 'twitter:title', content: ogTitle }],
      ['meta', { name: 'twitter:description', content: description }],
    ]
    if (isHome) {
      head.push([
        'script',
        { type: 'application/ld+json' },
        JSON.stringify({
          '@context': 'https://schema.org',
          '@type': 'SoftwareSourceCode',
          name: 'Recached',
          description: DESCRIPTION,
          url: SITE_URL,
          codeRepository: 'https://github.com/recached-sh/recached',
          programmingLanguage: 'Rust',
          license: 'https://www.apache.org/licenses/LICENSE-2.0',
        }),
      ])
    }
    return head
  },

  markdown: {
    theme: { light: 'github-light', dark: 'github-dark' },
  },

  themeConfig: {
    siteTitle: 'Recached',
    logo: { src: '/brand/logo.svg', alt: '' },

    search: {
      provider: 'local',
    },

    nav: [
      { text: 'Guide', link: '/guide/introduction', activeMatch: '^/guide/' },
      { text: 'Server', link: '/server/installation', activeMatch: '^/server/(?!commands|protocol)' },
      {
        text: 'SDKs',
        activeMatch: '^/(browser|react|vue|android|ios|rust)/(?!api-reference|hooks-reference|composables-reference)',
        items: [
          {
            text: 'Web',
            items: [
              { text: 'Browser (WebAssembly)', link: '/browser/getting-started' },
              { text: 'React', link: '/react/getting-started' },
              { text: 'Vue', link: '/vue/getting-started' },
            ],
          },
          {
            text: 'Native',
            items: [
              { text: 'Android (Kotlin preview)', link: '/android/getting-started' },
              { text: 'iOS & macOS (Swift preview)', link: '/ios/getting-started' },
              { text: 'Rust service (embedded)', link: '/rust/getting-started' },
            ],
          },
        ],
      },
      {
        text: 'Reference',
        activeMatch: '^/(server/(commands|protocol)|[a-z]+/(api-reference|hooks-reference|composables-reference))',
        items: [
          { text: 'Commands', link: '/server/commands' },
          { text: 'Wire protocol', link: '/server/protocol' },
          { text: 'Browser API', link: '/browser/api-reference' },
          { text: 'React hooks', link: '/react/hooks-reference' },
          { text: 'Vue composables', link: '/vue/composables-reference' },
          { text: 'Rust API', link: '/rust/api-reference' },
        ],
      },
      { text: 'Roadmap', link: '/roadmap' },
    ],

    // One sidebar for every page, so the whole documentation set is one click
    // away wherever a reader lands. Each link appears once: a page listed twice
    // would be highlighted twice. URLs are unchanged from the per-section
    // sidebars this replaces, so no redirects are needed.
    sidebar: [
      {
        text: 'Introduction',
        items: [
          { text: 'What is Recached?', link: '/guide/introduction' },
          { text: 'Use cases', link: '/guide/use-cases' },
          { text: 'Client support', link: '/guide/client-support' },
        ],
      },
      {
        text: 'Getting started',
        items: [
          { text: 'Quick start', link: '/guide/quick-start' },
          { text: 'Install the server', link: '/server/installation' },
        ],
      },
      {
        text: 'Sync & architecture',
        collapsed: false,
        items: [
          { text: 'How it works', link: '/guide/how-it-works' },
          { text: 'Sync scopes', link: '/server/sync-scopes' },
          { text: 'Database integration', link: '/guide/database-integration' },
        ],
      },
      {
        text: 'Server',
        collapsed: false,
        items: [
          { text: 'Configuration', link: '/server/configuration' },
          { text: 'Security', link: '/server/security' },
          { text: 'Operations', link: '/server/operations' },
          { text: 'Troubleshooting', link: '/server/troubleshooting' },
          { text: 'Upgrading to 0.6', link: '/server/upgrading-0.6' },
        ],
      },
      {
        text: 'Clients & SDKs',
        collapsed: false,
        items: [
          {
            text: 'Browser (WebAssembly)',
            collapsed: true,
            items: [
              { text: 'Getting started', link: '/browser/getting-started' },
              { text: 'Persistence', link: '/browser/persistence' },
              { text: 'Offline & reconnection', link: '/browser/offline' },
            ],
          },
          { text: 'React', link: '/react/getting-started' },
          { text: 'Vue', link: '/vue/getting-started' },
          { text: 'Android (Kotlin)', link: '/android/getting-started' },
          { text: 'iOS & macOS (Swift)', link: '/ios/getting-started' },
          { text: 'Rust service', link: '/rust/getting-started' },
        ],
      },
      {
        text: 'Reference',
        collapsed: true,
        items: [
          { text: 'Commands', link: '/server/commands' },
          { text: 'Wire protocol', link: '/server/protocol' },
          { text: 'Browser API', link: '/browser/api-reference' },
          { text: 'React hooks', link: '/react/hooks-reference' },
          { text: 'Vue composables', link: '/vue/composables-reference' },
          { text: 'Rust API', link: '/rust/api-reference' },
        ],
      },
      {
        text: 'Project',
        collapsed: true,
        items: [
          { text: 'Benchmarks', link: '/guide/benchmarks' },
          { text: 'Roadmap', link: '/roadmap' },
          { text: 'Changelog', link: 'https://github.com/recached-sh/recached/blob/main/CHANGELOG.md' },
          { text: 'Contributing', link: 'https://github.com/recached-sh/recached#contributing' },
        ],
      },
    ],

    outline: { level: [2, 3], label: 'On this page' },

    socialLinks: [{ icon: 'github', link: 'https://github.com/recached-sh/recached', ariaLabel: 'Recached on GitHub' }],

    footer: {
      message: 'Released under the Apache License 2.0.',
      copyright: 'Copyright © 2026 ThinkGrid Labs',
    },

    editLink: {
      pattern: 'https://github.com/recached-sh/recached/edit/main/docs/:path',
      text: 'Edit this page on GitHub',
    },

    docFooter: { prev: 'Previous', next: 'Next' },
    externalLinkIcon: true,
  },

  sitemap: {
    hostname: SITE_URL,
  },
})
