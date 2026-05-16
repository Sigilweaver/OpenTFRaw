# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] - 2026-05-16

### Added

- Rust parser for the Thermo Fisher RAW mass spectrometry file
  format, no native or system dependencies.
- Reader API for top-level structures: `FileHeader`, `AuditTag`,
  `SeqRow`, `InjectionData`, `ASInfo`, `RawFileInfo`, `InstID`,
  `RunHeader`, `SampleInfo`.
- Per-scan API: scan-index entries, packet headers, profile chunks,
  centroid peaks, scan events, scan parameters (generic records).
- Error log and instrument log decoders.
- Robust instrument-model detection via byte scan.
- Frequency-to-m/z conversion using the per-segment calibration table.
- `examples/dump.rs`: dump the contents of a RAW file as plain text.
- `examples/to_mzml.rs`: convert a RAW file to mzML (centroid or
  profile; optionally indexed).
- Validated against ProteoWizard `msconvert` mzML output for a
  multi-instrument PRIDE corpus (LTQ FT, Q Exactive HF, Orbitrap
  Fusion Lumos, Orbitrap Exploris 480, TSQ Vantage, TSQ Quantiva,
  TSQ Altis).
- Optional Python bindings (`opentfraw-py`, not published to crates.io).
- Format specification under `docs/docs/format/`.

### Out of scope

- Methods file (`MethodFile`) deep parse beyond byte-level layout.

[0.1.0]: https://github.com/Sigilweaver/OpenTFRaw/releases/tag/v0.1.0
