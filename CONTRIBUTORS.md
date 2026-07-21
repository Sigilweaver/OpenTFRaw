# Contributors

Thank you to everyone who has contributed to OpenTFRaw.

## Benjamin Riley ([@Nabejo](https://github.com/Nabejo))

Contributed in [Unreleased]:

- **`OpenTfRawSource::iter_chromatograms`** - wired the already-decoded
  per-scan TIC / base-peak fields (and, for flat-peak SRM files, the Q1/Q3
  transition maps) into `openmassspec_core::ChromatogramRecord`s so TIC, BPC,
  and SRM chromatograms reach mzML output. (Sigilweaver/OpenTFRaw#39)

Contributed in v1.3.0:

- **`RawFile.sample_info`** - exposed the sample-sheet / sequence-row
  metadata for the acquisition as a dict.
- **`RawFile.error_log()`** - exposed the acquisition error log as a list
  of `{"time": ..., "message": ...}` dicts.
- **`RawFile.status_log()`** - exposed the per-scan instrument status log
  (temperatures, voltages, pressures, ion counts, etc.), distinct from
  `scan_parameters()`.
- **`RawFile.controllers()`** - exposed multi-controller info (MS, UV, PDA,
  Analog channels) for multi-detector RAW files.
- **`RawFile.instrument_method_text()`** - best-effort extraction of the
  embedded acquisition method text/XML blob from the RAW file's metadata
  region.
- **`RawFile.computer_name`, `RawFile.controller_count`,
  `RawFile.acquisition_date`** - surfaced the remaining `raw_file_info`
  fields, including a new `RawFileInfoPreamble::acquisition_date` helper
  in the Rust core using Howard Hinnant's public-domain civil-calendar
  arithmetic.

## Oskari Kausiala ([@oskarsari](https://github.com/oskarsari)) - [Karsa Oy](https://github.com/karsa-oy)

Contributed in v1.2.0:

- **Exploris scan-event fix** - decoded the frequency-to-m/z calibration
  coefficients and MS2 precursor m/z for Orbitrap Exploris files, which were
  previously missing due to a body-offset mismatch.
- **Per-peak FT label data** - decoded the descriptor / unknown / triplet
  streams that trail the centroid peak list, exposing per-peak resolution,
  noise, and baseline via `ScanDataPacket.resolutions`, `noise_nodes`, and
  `noise_at(mz)`, with a Python `centroid_labels()` accessor.
- **`RawFile.profile()`** - surfaced raw profile spectra to the Python
  bindings, converting frequency-domain bins to m/z via the scan event's
  calibration coefficients.
- **`RawFile.scan_parameters()`** - exposed per-scan trailer parameters as a
  `{label: value}` dict in the Python bindings.
- **`RawFile.created`** - exposed the file creation / acquisition start
  timestamp from the Xcalibur audit tag.
