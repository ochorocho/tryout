import { defineConfig } from 'vitepress'

const repo = 'https://github.com/bmack/tryout'

export default defineConfig({
  title: 'tryout',
  description: 'Every branch of your DDEV project, served side by side.',
  // Served from https://bmack.github.io/tryout/
  base: '/tryout/',
  cleanUrls: true,
  lastUpdated: true,
  head: [['link', { rel: 'icon', type: 'image/svg+xml', href: '/tryout/logo.svg' }]],

  themeConfig: {
    logo: '/logo.svg',
    nav: [
      { text: 'Guide', link: '/guide/introduction', activeMatch: '/guide/' },
      { text: 'Frameworks', link: '/frameworks/', activeMatch: '/frameworks/' },
      { text: 'Reference', link: '/reference/commands', activeMatch: '/reference/' },
    ],
    sidebar: {
      '/guide/': [
        {
          text: 'Getting started',
          items: [
            { text: 'Introduction', link: '/guide/introduction' },
            { text: 'Installation', link: '/guide/install' },
          ],
        },
        {
          text: 'Your own project',
          items: [
            { text: 'Project mode', link: '/guide/project-mode' },
            { text: 'Pull requests', link: '/guide/pull-requests' },
          ],
        },
        {
          text: 'TYPO3 Core',
          items: [
            { text: 'TYPO3 Core mode', link: '/guide/typo3-core' },
            { text: 'Gerrit patches', link: '/guide/gerrit' },
            { text: 'Contributing to TYPO3 Core', link: '/guide/contributing-to-core' },
          ],
        },
        {
          text: 'Everyday use',
          items: [
            { text: 'Served sites', link: '/guide/sites' },
            { text: 'Terminal UI', link: '/guide/terminal-ui' },
            { text: 'Troubleshooting', link: '/guide/troubleshooting' },
          ],
        },
      ],
      '/frameworks/': [
        {
          text: 'Frameworks',
          items: [
            { text: 'Overview', link: '/frameworks/' },
            { text: 'Laravel', link: '/frameworks/laravel' },
            { text: 'Symfony', link: '/frameworks/symfony' },
            { text: 'Drupal', link: '/frameworks/drupal' },
            { text: 'WordPress', link: '/frameworks/wordpress' },
            { text: 'WordPress with Bedrock', link: '/frameworks/wp-bedrock' },
            { text: 'TYPO3 site projects', link: '/frameworks/typo3' },
            { text: 'Craft CMS', link: '/frameworks/craftcms' },
            { text: 'Shopware 6', link: '/frameworks/shopware' },
            { text: 'Silverstripe', link: '/frameworks/silverstripe' },
            { text: 'CodeIgniter 4', link: '/frameworks/codeigniter' },
            { text: 'CakePHP', link: '/frameworks/cakephp' },
            { text: 'Backdrop', link: '/frameworks/backdrop' },
            { text: 'Asterios', link: '/frameworks/asterios' },
            { text: 'PHP and generic', link: '/frameworks/php' },
            { text: 'Not supported yet', link: '/frameworks/not-supported' },
          ],
        },
      ],
      '/reference/': [
        {
          text: 'Reference',
          items: [
            { text: 'Commands', link: '/reference/commands' },
            { text: 'Configuration', link: '/reference/configuration' },
            { text: 'Architecture', link: '/reference/architecture' },
            { text: 'Development', link: '/reference/development' },
          ],
        },
      ],
    },
    search: { provider: 'local' },
    socialLinks: [{ icon: 'github', link: repo }],
    editLink: {
      pattern: `${repo}/edit/main/docs/:path`,
      text: 'Edit this page on GitHub',
    },
    footer: {
      message: 'A DDEV add-on. Released under the MIT License.',
    },
  },
})
