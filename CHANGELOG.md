# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Docs

- `docs/guide/reader.md`'s field table had drifted behind `RawFileReader`
  (listed 5 of 21 public fields). Regenerated the table from the struct,
  documented `controllers()`/`ControllerInfo`, and added a table for the
  scan-read methods (`read_scan`, `read_scan_labels`, `read_peaks_only`,
  `read_scan_flat`, `read_scan_srm_v66`) alongside `read_scan_peaks`. (#22)

### Fixed

- `to_msc_record` now populates `SpectrumRecord.faims_cv` from this
  crate's own `ScanParams::faims_cv()` decoding instead of hardcoding
  `None`. (#27)
- The workspace `openmassspec-core` dependency constraint was still
  `"1.0.0"`, even though `to_msc_record` has required the `faims_cv`
  field added in 1.2.0 since the previous release. A fresh resolution
  against 1.0.x or 1.1.x would fail to compile. Bumped the constraint
  to `"1.2.0"` to match what the code actually requires.

## [1.3.4] - 2026-07-15

### Fixed

- Bumped `openmassspec-core` to 1.2.0. That release added a required
  `SpectrumRecord.faims_cv` field, which broke this crate's build under
  any dependency resolution that picked up 1.2.0 (`to_msc_record`
  constructed the struct literal without it). Wiring real FAIMS CV data
  through (this crate's `ScanParams::faims_cv()` already decodes it) is
  tracked separately in #27; this release only restores the build.

## [1.3.3] - 2026-07-14

### Fixed

- `RunMetadata.start_timestamp` (mzML export) was hardcoded to `None`; it
  is now populated from `RawFileInfoPreamble::acquisition_date`, formatted
  as RFC 3339 via a new `acquisition_date_rfc3339()`. (#24)
- `RawFileInfoPreamble::acquisition_date()` previously only bounds-checked
  year/month/day (day against a flat 1-31), so out-of-range time-of-day
  fields (hour, minute, second, millisecond) or impossible dates (Feb 29
  on a non-leap year, Apr 31) were silently accepted and produced a wrong
  timestamp instead of `None`. All fields are now validated, including a
  leap-year-aware day-of-month bound.

## [1.3.2] - 2026-07-11

### Security

- Upgraded `pyo3` and `numpy` from 0.22 to 0.29, clearing RUSTSEC-2025-0020
  and RUSTSEC-2026-0177. The `cargo audit` CI job no longer needs to
  `--ignore` those advisories.

## [1.3.1] - 2026-07-10

### Changed

- Dependency renamed `openproteo-core` -> `openmassspec-core` (1.0.0),
  following the umbrella's rename from OpenProteo to OpenMassSpec.
  No behavioral change.
- `opentfraw-py` no longer opts out of the workspace's `unsafe_code = "forbid"`
  lint; it never contained an `unsafe` block, so this was a stale exception
  that had drifted out of sync with CONTRIBUTING.md's description of the
  policy. Also wires up the same `clippy::unwrap_used`/`expect_used` warn
  lint the core crate already carries.
- CI (`ci.yml`) now builds and tests on `windows-latest` in addition to
  Linux/macOS, matching the wheel targets `publish.yml` already ships.
- New `audit.yml` workflow runs `cargo audit` against the RustSec Advisory
  DB on dependency changes and weekly. Two pre-existing advisories against
  `pyo3` are temporarily ignored pending a version bump (#20).

### Internal

- `opentfraw-py`: extracted the repeated source-mutex-lock pattern (6 call
  sites) into a `RawFile::locked_source` helper.

## [1.3.0] - 2026-07-09

### Added

- `RawFile.sample_info` (Python): returns the sample-sheet / sequence-row
  metadata for the acquisition as a dict (`id`, `comment`, `vial`,
  `injection_volume`, `sample_weight`, `sample_volume`, `istd_amount`,
  `dilution_factor`, `user_labels`, `inst_method`, `proc_method`,
  `file_name`, `path`). The Rust core already decoded this
  (`RawFileReader::seq_row`); this exposes it to Python. (@Nabejo)
- `RawFile.error_log()` (Python): returns the acquisition error log as a
  list of `{"time": ..., "message": ...}` dicts, in log order (`time` is
  the acquisition-relative time in minutes). The Rust core already decoded
  this (`RawFileReader::error_log`); this exposes it to Python. (@Nabejo)
- `RawFile.status_log(scan_number)` (Python): returns the per-scan
  instrument status log as a `{label: value}` dict (or `None`). This is
  the instrument-state-over-time log (temperatures, voltages, pressures,
  ion counts, etc.), distinct from the trailer-extra values already
  surfaced by `scan_parameters()`. The Rust core already decoded these
  (`inst_log_record` / `GenericRecord`); this exposes them to Python.
  (@Nabejo)
- `RawFile.controllers()` (Python): returns a list of dicts (`index`,
  `is_ms_controller`, `controller_type`, `first_scan`, `last_scan`,
  `start_time`, `end_time`) enumerating every controller in a
  multi-detector RAW file (MS, UV, PDA, Analog channels alongside the
  main MS controller). Single-controller files (the common case) return
  a one-element list. The Rust core already decoded this
  (`RawFileReader::controllers`); this exposes it to Python. (@Nabejo)
- `RawFile.instrument_method_text()` (Python): best-effort extraction of
  the embedded acquisition method (UTF-16LE text/XML blob) from the RAW
  file's metadata region, or `None` if no suitable text block is found
  or no method was embedded. Distinct from `sample_info()`'s
  `inst_method` field, which is just the method file name. The Rust core
  already decoded this (`RawFileReader::instrument_method_text`); this
  exposes it to Python. (@Nabejo)
- `RawFile.computer_name`, `RawFile.controller_count`, and
  `RawFile.acquisition_date` (Python): surface the remaining fields of
  `RawFileReader::raw_file_info` that weren't already exposed indirectly
  through other bindings. `acquisition_date` is a Unix timestamp in
  seconds decoded from the raw-file-info preamble's discrete
  year/month/day/hour/minute/second/millisecond fields (new
  `RawFileInfoPreamble::acquisition_date` in the Rust core, using
  Howard Hinnant's public-domain civil-calendar arithmetic) - a
  different decoded timestamp from `RawFile.created` (the Xcalibur
  audit-tag FILETIME); the two are expected to agree since they record
  the same acquisition event, but come from independently-decoded
  fields. (@Nabejo)

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
