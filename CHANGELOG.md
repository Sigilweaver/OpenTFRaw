# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- `RawFile.controllers()` (Python): returns a list of dicts (`index`,
  `is_ms_controller`, `controller_type`, `first_scan`, `last_scan`,
  `start_time`, `end_time`) enumerating every controller in a
  multi-detector RAW file (MS, UV, PDA, Analog channels alongside the
  main MS controller). Single-controller files (the common case) return
  a one-element list. The Rust core already decoded this
  (`RawFileReader::controllers`); this exposes it to Python. (@Nabejo)

## [1.2.1] - 2026-07-06

### Changed

- PyPI package now declares `keywords` (`mass-spectrometry`, `thermo`,
  `raw`, `proteomics`, `orbitrap`) so the package is findable via PyPI
  search; previously only the crates.io side had them.

## [1.2.0] - 2026-06-23

### Added

- `RawFile.profile(scan_number)` (Python): returns the raw profile spectrum as
  `(mz, intensity)` NumPy arrays, converting the frequency-domain bins via the
  scan event's calibration coefficients. The Rust core already decoded profile
  data (`ScanDataPacket.profile`, `Profile::to_mz_intensity`); this exposes it to
  Python, which previously surfaced centroids only. (@oskarsari)
- Per-peak FT label data decoding for PacketHeader scans. The previously
  skipped descriptor / unknown / triplet streams are now decoded:
  `ScanDataPacket` gains `resolutions` (per-peak resolution) and
  `noise_nodes` (the noise-vs-m/z function), plus `noise_at(mz)` to
  interpolate per-peak noise/baseline. New `RawFileReader::read_scan_labels`
  reads peaks + labels while skipping the profile signal, and the Python
  binding exposes `RawFile.centroid_labels(scan_number)` returning
  `mz`/`intensity`/`resolution`/`noise`/`baseline`/`signal_to_noise` arrays.
  (@oskarsari)
- `RawFile.scan_parameters(scan_number)` (Python): returns the per-scan generic
  ("trailer") parameters as a `{label: value}` dict (or `None`), mirroring the
  vendor reader's trailer-extra information. Keys are the instrument's own
  labels (e.g. `"HCD Energy V:"`, `"MS2 Isolation Width:"`); values keep their
  stored type. The Rust core already decoded these (`scan_parameters` /
  `GenericRecord`); this surfaces them to Python. (@oskarsari)
- `RawFile.created`: file creation (acquisition start) time as a Unix timestamp
  in seconds, read from the Xcalibur audit tag (a Windows FILETIME). The Rust
  core already parses this (`header.audit_start.time`); the bindings did not
  surface it. Note: Thermo records the instrument's local wall-clock there with
  no timezone, so interpreting the value as UTC reproduces that local wall-clock
  rather than a true UTC instant. (@oskarsari)
- `Error::UnsupportedOperation` variant for operations that require a specific
  scan-data format (e.g. `read_scan_labels` on a TSQ/SRM file).

### Fixed

- Orbitrap Exploris scan events: the frequency->m/z calibration coefficients
  and the MS2 precursor m/z are now decoded. The nparam/coefficients block is
  read immediately after the scan-window FractionCollector (offset 80 for the
  offset-64 FC family) instead of a fixed `body_size - 64`, which missed it on
  Exploris (body_size 136) and left the profile m/z mis-converted. Dependent
  MS2 scans whose reaction record starts at body offset 4 (Exploris) now have
  their precursor m/z and activation energy recovered. Q Exactive / Fusion
  decoding is unchanged. (@oskarsari)
- `profile()` and `centroid_labels()` now return a clear error when called on
  TSQ/SRM files instead of attempting to parse flat-peak data as a PacketHeader.

## [1.1.0] - 2026-05-31

### Added

- `CITATION.cff`: author identity (Nathan Riley + ORCID) and a
  scaffolded `identifiers:` block ready for the Zenodo concept DOI.
- `CONTRIBUTING.md`.
- Docusaurus build job in CI.

### Changed

- **Panic surface eliminated (WP17).** Parsers no longer call
  `unwrap()` in production code: a new `bytes` helper module
  (`read_u32/f64_le`) returns `Error::UnexpectedEof { offset,
  needed }`, and a `find_map` closure now uses `.ok()?` to preserve
  `Option`. Library crate carries
  `#![cfg_attr(not(test), warn(clippy::unwrap_used,
  clippy::expect_used))]`.
- Manifest hygiene (WP13): `homepage` set to <https://sigilweaver.app>
  and `documentation` link added.
- README badge block unified across the Sigilweaver portfolio.

## [1.0.6] - 2026-05-21

### Changed

- Depend on `openproteo-core = "1.0.0"` (was `0.1.0`, yanked).
- MSRV bumped from 1.75 to 1.85 (tracks `openproteo-core 1.0.0`).

## [1.0.5] - 2026-05-18

### Changed

- Depend on `openproteo-core = "0.1.0"` from crates.io (no source change;
  workspace dependency now carries an explicit registry version so the
  crate can be published).
- `SECURITY.md` added; coordinated-disclosure contact documented.

## [1.0.4] - 2026-05-17

### Changed

- Restructured to a Cargo workspace layout. The library crate is now at
  `crates/opentfraw/` and the Python bindings crate at
  `crates/opentfraw-py/`. The `pyproject.toml` is now at the repository
  root. No public API changes.

## [1.0.3] - 2026-05-17

### Fixed

- `python/pyproject.toml`: revert `readme` to `"README.md"` and restore
  `python/README.md` stub. Maturin sdist packaging prohibits `..` in
  archive paths, causing the 1.0.2 sdist build to fail on CI.

## [1.0.2] - 2026-05-17

### Changed

- Docs and source comments: replace em-dashes, en-dashes, smart quotes,
  and ellipsis characters with ASCII equivalents.

## [1.0.1] - 2026-05-17

### Changed

- README: standardize structure and docs link format (consistent with
  OpenTimsTDF and OpenWRaw).

## [1.0.0] - 2026-05-17

First stable release. The public API of `opentfraw` is now considered
stable and will follow semantic versioning. Format coverage is unchanged
from 0.1.0 (LTQ FT, Q Exactive HF, Orbitrap Fusion Lumos, Orbitrap
Exploris 480, TSQ Vantage, TSQ Quantiva, TSQ Altis).

### Added

- `ATTRIBUTION.md` (replaces `CREDITS.md`): tracks third-party notices for
  bundled data and vendored code.
- `publish.yml` GitHub Actions workflow: publishes the `opentfraw` crate
  to crates.io and the Python wheel to PyPI via OIDC Trusted Publishing
  on every `v*` tag push.

### Changed

- CI migrated from WarpBuild runners to standard GitHub-hosted
  (`ubuntu-latest`, `macos-latest`, `windows-latest`).
- Removed the `tools/` vendor SDK tree and `corpus/mzml/` binary corpus
  from repository history (git history rewritten; total size reduced from
  ~1.5 GB to ~660 KB).
- Removed "Pure-Rust" marketing language from `README.md` and related
  documentation (Python bindings use PyO3/maturin which pulls in a C
  compiler at build time).
- Renamed `CREDITS.md` to `ATTRIBUTION.md`.

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
- Exercised on a multi-instrument PRIDE corpus (LTQ FT, Q Exactive HF,
  Orbitrap Fusion Lumos, Orbitrap Exploris 480, TSQ Vantage, TSQ
  Quantiva, TSQ Altis); mzML output checked for structural conformance
  against the PSI-MS mzML schema.
- Optional Python bindings (`opentfraw-py`, not published to crates.io).
- Format specification under `docs/docs/format/`.

### Out of scope

- Methods file (`MethodFile`) deep parse beyond byte-level layout.

[1.0.1]: https://github.com/Sigilweaver/OpenTFRaw/releases/tag/v1.0.1
[1.0.0]: https://github.com/Sigilweaver/OpenTFRaw/releases/tag/v1.0.0
[0.1.0]: https://github.com/Sigilweaver/OpenTFRaw/releases/tag/v0.1.0
