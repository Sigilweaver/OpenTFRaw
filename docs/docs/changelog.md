---
sidebar_position: 98
---

# Changelog

The canonical changelog lives at
[`CHANGELOG.md`](https://github.com/Sigilweaver/OpenTFRaw/blob/main/CHANGELOG.md)
in the repository root. The notes below mirror the latest release.

## 0.1.0

Initial release.

- Rust parser for the Thermo Fisher RAW mass-spectrometry file format,
  covering versions 8 through 66.
- Section parsers for file header, audit tags, sample information,
  sequence row, RAW file info, run header, scan event hierarchy, scan
  index, scan-data packets (header and flat variants), error log,
  scan parameters, and the generic data section.
- `DeviceFamily` instrument classification.
- Python bindings (`opentfraw`) via PyO3 + maturin (abi3, Python 3.8+).
- mzML export example.
