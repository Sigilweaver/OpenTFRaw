---
sidebar_position: 1
slug: /
---

# OpenTFRaw

:::info Part of the OpenMassSpec stack

OpenTFRaw is one of the vendor readers in
[OpenMassSpec](https://sigilweaver.app/openmassspec/docs/), a Rust- and
Python-native stack for proteomics raw-file access. Sibling readers:
[OpenWRaw](https://sigilweaver.app/openwraw/docs/) (Waters `.raw/`),
[OpenTimsTDF](https://sigilweaver.app/OpenTimsTDF/docs/) (Bruker `.d/`).

:::

OpenTFRaw is a Rust library that reads Thermo Fisher `.raw`
mass-spectrometry files - the binary format produced by Thermo
ion traps, Orbitrap instruments and TSQ triple quadrupoles.

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

Tested format versions: 63, 64 and 66, on files from LTQ ion traps
through Orbitrap Astral and TSQ Altis. The reader also has code paths for
versions 8, 47, 57, 60 and 62, but the public test corpus has no file of
those versions, so they are untested. The
[README](https://github.com/Sigilweaver/OpenTFRaw#known-issues-and-gaps)
lists known issues and coverage gaps.

## Next steps

- [Install](./install) the Rust crate or the Python package.
- Run through the [Quickstart](./quickstart).
- Read the [Format specification](./format/overview) for the binary
  layer.
- Browse the API on [docs.rs](https://docs.rs/opentfraw).

## License

OpenTFRaw is Apache-2.0 licensed. See [License](./license).
