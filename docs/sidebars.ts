import type { SidebarsConfig } from '@docusaurus/plugin-content-docs';

const sidebars: SidebarsConfig = {
  docsSidebar: [
    'intro',
    'install',
    'quickstart',
    {
      type: 'category',
      label: 'Guide',
      collapsed: false,
      items: [
        'guide/reader',
        'guide/scan-data',
        'guide/instrument-families',
        'guide/mzml-export',
      ],
    },
    {
      type: 'category',
      label: 'Format Specification',
      link: { type: 'doc', id: 'format/overview' },
      items: [
        'format/overview',
        'format/file-layout',
        'format/sample-and-sequence',
        'format/raw-file-info',
        'format/run-header',
        'format/scan-index-and-data',
        'format/scan-event',
        'format/scan-parameters',
        'format/logs',
        'format/enumerations',
        'format/frequency-to-mz',
        'format/references',
      ],
    },
    'changelog',
    'license',
  ],
};

export default sidebars;
