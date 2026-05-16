---
sidebar_position: 1
slug: /
---

# OpenTFRaw

OpenTFRaw is a Rust library that reads Thermo Fisher `.raw`
mass-spectrometry files - the binary format produced by Thermo
instruments from the LCQ Classic (mid-1990s) through the Orbitrap
Astral and modern TSQ triple quadrupoles.

It runs on Linux, macOS, and Windows, with no dependency on the Thermo
`.NET` runtime or `RawFileReader` library. The format was decoded by
binary analysis of a public corpus of mass-spectrometry datasets
(PRIDE accessions); see [`CORPUS.md`](https://github.com/Sigilweaver/OpenTFRaw/blob/main/CORPUS.md).

Optional Python bindings are available via the
[`opentfraw`](./install) wheel.

## What it covers

| Component                                  | Status      |
| ------------------------------------------ | ----------- |
| File header + audit tags                   | supported   |
| Sample information + sequence row          | supported   |
| RAW file info + instrument method block    | supported   |
| Run header (single + multi-controller)     | supported   |
| Scan index + trailer                       | supported   |
| Scan-data packets (header + flat variants) | supported   |
| Scan event hierarchy + filter strings      | supported   |
| Scan parameters + generic data section     | supported   |
| Error log + instrument log                 | supported   |
| Device / instrument classification         | best-effort |
| Full tune-method block                     | planned     |

Supported format versions: 8, 47, 57, 60, 62, 63, 64, 66 (covering LCQ
through Orbitrap Astral and modern TSQ).

## Next steps

- [Install](./install) the Rust crate or the Python package.
- Run through the [Quickstart](./quickstart).
- Read the [Format specification](./format/overview) for the binary
  layer.
- Browse the API on [docs.rs](https://docs.rs/opentfraw).

## License

OpenTFRaw is Apache-2.0 licensed. See [License](./license).
