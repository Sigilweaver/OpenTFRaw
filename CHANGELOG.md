# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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
- Validated against ProteoWizard `msconvert` mzML output for a
  multi-instrument PRIDE corpus (LTQ FT, Q Exactive HF, Orbitrap
  Fusion Lumos, Orbitrap Exploris 480, TSQ Vantage, TSQ Quantiva,
  TSQ Altis).
- Optional Python bindings (`opentfraw-py`, not published to crates.io).
- Format specification under `docs/docs/format/`.

### Out of scope

- Methods file (`MethodFile`) deep parse beyond byte-level layout.

[1.0.1]: https://github.com/Sigilweaver/OpenTFRaw/releases/tag/v1.0.1
[1.0.0]: https://github.com/Sigilweaver/OpenTFRaw/releases/tag/v1.0.0
[0.1.0]: https://github.com/Sigilweaver/OpenTFRaw/releases/tag/v0.1.0
