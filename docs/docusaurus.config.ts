import { themes as prismThemes } from 'prism-react-renderer';
import type { Config } from '@docusaurus/types';
import type * as Preset from '@docusaurus/preset-classic';

const config: Config = {
  title: 'OpenTFRaw',
  tagline: 'Rust and Python reader for Thermo Fisher RAW mass spectrometry files',
  favicon: 'img/favicon.ico',

  markdown: {
    mermaid: true,
    hooks: {
      onBrokenMarkdownLinks: 'warn',
    },
  },
  themes: ['@docusaurus/theme-mermaid'],

  url: 'https://sigilweaver.app',
  baseUrl: '/opentfraw/docs/',

  organizationName: 'Sigilweaver',
  projectName: 'OpenTFRaw',

  onBrokenLinks: 'throw',

  i18n: {
    defaultLocale: 'en',
    locales: ['en'],
  },

  presets: [
    [
      'classic',
      {
        docs: {
          routeBasePath: '/',
          sidebarPath: './sidebars.ts',
          editUrl: 'https://github.com/Sigilweaver/OpenTFRaw/tree/main/docs/',
        },
        blog: false,
        sitemap: {
          changefreq: 'weekly',
          priority: 0.5,
          filename: 'sitemap.xml',
        },
        theme: {
          customCss: './src/css/custom.css',
        },
      } satisfies Preset.Options,
    ],
  ],

  themeConfig: {
    metadata: [
      { name: 'keywords', content: 'OpenTFRaw, Thermo Fisher, RAW, mass spectrometry, Orbitrap, Rust, Python' },
      { name: 'description', content: 'OpenTFRaw is a Rust and Python reader for Thermo Fisher RAW mass spectrometry files.' },
    ],
    colorMode: {
      defaultMode: 'dark',
      disableSwitch: false,
      respectPrefersColorScheme: true,
    },
    navbar: {
      title: 'OpenTFRaw',
      logo: {
        alt: 'Sigilweaver logo',
        src: 'img/logo.svg',
        href: 'https://sigilweaver.app',
        target: '_self',
      },
      items: [
        {
          type: 'dropdown',
          label: 'OpenTFRaw',
          position: 'left',
          items: [
            { label: 'All Docs', href: 'https://sigilweaver.app/docs/' },
            { label: 'OpenProteo', href: 'https://sigilweaver.app/openproteo/docs/' },
            { label: 'OpenTimsTDF (Bruker)', href: 'https://sigilweaver.app/opentimstdf/docs/' },
            { label: 'OpenWRaw (Waters)', href: 'https://sigilweaver.app/openwraw/docs/' },
          ],
        },
        {
          href: 'https://docs.rs/opentfraw',
          label: 'API (docs.rs)',
          position: 'right',
        },
        {
          href: 'https://sigilweaver.app',
          label: 'Website',
          position: 'right',
        },
        {
          href: 'https://github.com/Sigilweaver/OpenTFRaw',
          label: 'GitHub',
          position: 'right',
        },
      ],
    },
    footer: {
      style: 'dark',
      links: [
        {
          title: 'Project',
          items: [
            { label: 'GitHub', href: 'https://github.com/Sigilweaver/OpenTFRaw' },
            { label: 'Issues', href: 'https://github.com/Sigilweaver/OpenTFRaw/issues' },
            { label: 'crates.io', href: 'https://crates.io/crates/opentfraw' },
            { label: 'docs.rs', href: 'https://docs.rs/opentfraw' },
          ],
        },
        {
          title: 'Sigilweaver',
          items: [
            { label: 'Website', href: 'https://sigilweaver.app' },
            { label: 'Other projects', href: 'https://sigilweaver.app#projects' },
          ],
        },
        {
          title: 'Legal',
          items: [
            { label: 'Terms of Use', href: 'https://sigilweaver.app/terms' },
            { label: 'Privacy Policy', href: 'https://sigilweaver.app/privacy' },
          ],
        },
      ],
      copyright: `Copyright ${new Date().getFullYear()} Sigilweaver Holdings LLC. OpenTFRaw is Apache-2.0 licensed. Documentation licensed under <a href="https://creativecommons.org/licenses/by-sa/4.0/" target="_blank" rel="noopener noreferrer">CC-BY-SA 4.0</a>.`,
    },
    prism: {
      theme: prismThemes.github,
      darkTheme: prismThemes.dracula,
      additionalLanguages: ['rust', 'toml', 'bash'],
    },
  } satisfies Preset.ThemeConfig,
};

export default config;
