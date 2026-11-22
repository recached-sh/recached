# Recached — Amber Signal

**Slogan:** More than a cache. Built to sync.

Always two sentences, sentence case, each ending in a full stop. On two lines,
"Built to sync." takes the amber accent. Do not replace it with a generic tagline.

**Description:** A Rust-powered caching engine with real-time synchronization built in.

| Color | Hex | Usage |
| --- | --- | --- |
| Amber | `#F59E0B` | Logo stroke, primary buttons, "Built to sync." on dark backgrounds, sync lines |
| Gold | `#FBBF24` | Dark-mode links and highlights |
| Pale gold | `#FDE68A` | Logo node, inline code on dark |
| Midnight | `#171C2B` | Logo tile, dark artwork, text on amber |
| Amber 600 | `#D97706` | "Built to sync." on light backgrounds (3.2:1, large text only) |
| Amber 700 | `#B45309` | Links on light backgrounds (5.0:1) |
| Amber 800 | `#92400E` | Small text and hover on light backgrounds (7.1:1) |

Amber itself is 2.1:1 on white, so it is never used for text on light surfaces.

## Assets

| File | Use |
| --- | --- |
| `logo.svg` | The Connected R mark. Its geometry is canonical: every other asset embeds this exact path. |
| `wordmark.svg` | Mark and wordmark, for dark backgrounds |
| `banner.svg` | 1600×600 README banner with the slogan |
| `social-card.svg` | Source of the 1200×630 `docs/public/brand/social-card.png`; re-render the PNG after editing it |
| `../docs/public/architecture-{light,dark}.svg` | Architecture diagram, used by the README and the docs |
| `../docs/public/brand/icons/` | Line icons; the homepage feature cards inline the same paths |

The homepage hero illustration is a Vue component
(`docs/.vitepress/theme/components/HeroVisual.vue`) so it follows the light/dark
switch.

Rendering the social card: open the SVG in headless Chromium with Inter loaded
(VitePress ships `inter-roman-latin.woff2`) and `text { font-optical-sizing: none }`,
screenshot at 1200×630, and save as PNG.

Diagrams illustrate cache synchronization, not database replication or full
Redis command parity. Avoid the previous orange-red/teal branding.
