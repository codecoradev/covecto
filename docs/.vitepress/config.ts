import { createConfig } from '@codecora/theme/vitepress/config'

export default createConfig({
  product: 'covecto',
  title: 'Covecto',
  description: 'Dual-engine image vectorization — pixel-exact for icons, smooth curves for art.',
  accent: 'orange',
  repo: 'covecto',
  head: [
    ['meta', { property: 'og:title', content: 'Covecto — Image to SVG Vectorization' }],
    ['meta', { property: 'og:description', content: 'Dual-engine image vectorizer: pixel-exact for icons, Bézier splines for art. CLI + HTTP API.' }],
    ['meta', { name: 'twitter:card', content: 'summary_large_image' }],
  ],
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
