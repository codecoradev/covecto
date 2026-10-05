import { createConfig } from '@codecora/theme/vitepress/config'

export default createConfig({
  product: 'covecto',
  title: 'Covecto: Image to SVG Vectorizer CLI | CodeCora',
  description: 'Covecto converts images to SVG with a dual-engine vectorizer CLI: pixel-exact for icons, smooth Bézier curves for art. SVG, PDF, EPS output. Zero dependencies.',
  accent: 'peach',
  repo: 'covecto',
  // NOTE: do NOT pass `head` here — @codecora/theme createConfig() drops
  // opts.head (never merged into its hardcoded head array). OG/Twitter meta
  // lives in docs/index.md frontmatter instead; theme fix tracked separately.
  ignoreDeadLinks: true,
  sidebar: [
    {
      text: 'Getting Started',
      items: [
        { text: 'Installation', link: '/install' },
        { text: 'Quick Start', link: '/getting-started' },
        { text: 'Docker', link: '/docker' },
      ],
    },
    {
      text: 'Engines',
      items: [
        { text: 'Engine Comparison', link: '/engines' },
        { text: 'PixelExact', link: '/engines/pixel-exact' },
        { text: 'Spline (vtracer)', link: '/engines/spline' },
      ],
    },
    {
      text: 'CLI',
      items: [
        { text: 'CLI Reference', link: '/cli' },
        { text: 'Profiles', link: '/profiles' },
        { text: 'Batch Processing', link: '/batch' },
      ],
    },
    {
      text: 'API',
      items: [
        { text: 'API Reference', link: '/api' },
        { text: 'OpenAPI Spec', link: 'https://github.com/codecoradev/covecto/blob/main/openapi.yaml' },
      ],
    },
    {
      text: 'Configuration',
      items: [
        { text: 'Vectorization Options', link: '/configuration' },
        { text: 'Optimization', link: '/optimization' },
        { text: 'Output Formats', link: '/formats' },
      ],
    },
    {
      text: 'Rust Library',
      items: [
        { text: 'Using as a Crate', link: '/library' },
      ],
    },
  ],
})
