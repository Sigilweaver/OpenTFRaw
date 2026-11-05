import { createConfig } from '@sigilweaver/docs-kit';

export default createConfig({
  title: 'OpenTFRaw',
  tagline: 'Rust and Python reader for Thermo Fisher RAW mass spectrometry files',
  slug: 'opentfraw',
  repo: 'OpenTFRaw',
  keywords: ['OpenTFRaw', 'Thermo Fisher', 'RAW', 'mass spectrometry', 'Orbitrap', 'Rust', 'Python'],
  description: 'OpenTFRaw is a Rust and Python reader for Thermo Fisher RAW mass spectrometry files.',
  packages: { crate: 'opentfraw', docsRs: true },
  navbar: {
    projects: {
      label: 'OpenTFRaw',
      items: [
        'openmassspec',
        { id: 'opentimstdf', label: 'OpenTimsTDF (Bruker)' },
        { id: 'openwraw', label: 'OpenWRaw (Waters)' },
      ],
    },
    items: [{ href: 'https://docs.rs/opentfraw', label: 'API (docs.rs)' }],
  },
});
