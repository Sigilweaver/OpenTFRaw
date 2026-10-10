---
sidebar_position: 5
---

# Python API

`opentfraw.RawFile` wraps a `RawFileReader` for ergonomic use from Python.
Every attribute and method below also carries a docstring in the wheel
itself (`help(opentfraw.RawFile)`); this page is a map of what's available.

```python
import opentfraw

raw = opentfraw.RawFile("run.raw")
```

## Attributes

| Attribute           | Type            | Description                                                    |
| -------------------- | --------------- | --------------------------------------------------------------- |
| `num_scans`          | `int`           | Total number of scans                                           |
| `first_scan`         | `int`           | Scan number of the first scan (usually 1)                       |
| `last_scan`          | `int`           | Scan number of the last scan                                    |
| `instrument_model`   | `str \| None`   | Detected instrument model (e.g. `"Orbitrap Fusion Lumos"`)      |
| `created`            | `float \| None` | Acquisition start time (Xcalibur audit tag FILETIME), Unix secs |
| `sample_info`        | `dict`          | Sample-sheet / sequence-row metadata for the acquisition        |
| `computer_name`      | `str`           | Acquisition workstation's computer name                         |
| `controller_count`   | `int`           | Number of controllers in the file (MS plus auxiliary detectors) |
| `acquisition_date`   | `float \| None` | Acquisition timestamp decoded from `raw_file_info`, Unix secs   |

`created` and `acquisition_date` are two independently-decoded timestamps
for the same acquisition event (one from the Xcalibur audit tag, the other
from the raw-file-info preamble); they're expected to agree but aren't
guaranteed to. Both are the instrument's local wall-clock time with no
timezone attached.

## Scan data

```python
mz, intensity = raw.peaks(3)              # centroid peaks, float64/float32 numpy arrays
scan = raw.scan(3)                        # dict: ms_level, RT, charge, filter_string, ...
for scan in raw.iter_scans():             # equivalent to scan(n) for n in range(first_scan, last_scan+1)
    ...
raw.scan_table()                          # {key: [one value per scan]} for every scan() key but mz/intensity; no peak reads
raw.scan_filter(3)                        # scan filter string, or None
raw.profile(3)                            # (mz, intensity) from the raw profile signal
raw.centroid_labels(3)                    # mz/intensity/resolution/noise/baseline/signal_to_noise
```

`scan()` reads its metadata from the same derivation the mzML writer uses,
so its precursor, collision-energy and scan-mode values match what
`to_mzml()` writes. Its `extra` key holds every other decoded value the scan
carries under normalized `opentfraw.*` keys; `opentfraw.extra_field_keys()`
lists them all.

## Lock-mass counts

`scan()["extra"]`, `iter_records()`, and mzML spectrum userParams distinguish
these trailer fields:

| Extra key | Trailer label | Meaning |
| --- | --- | --- |
| `opentfraw.number_of_matched_lock_masses` | `Number of LM Found:` | Peaks matched in this scan |
| `opentfraw.number_of_configured_lock_masses` | `Number of Lock Masses:` | Lock masses configured for acquisition |

Missing, mistyped, or negative counts are omitted; an explicit zero is retained.
The older `opentfraw.number_of_lock_masses` key retains its compatibility
fallback from matched to configured count, so it must not be interpreted as a
matched count when only configuration is available.

Counts do not establish whether correction was applied. In the public
[Q Exactive HF-X PXD071477](https://www.ebi.ac.uk/pride/archive/projects/PXD071477)
and [Exploris 480 PXD064947](https://www.ebi.ac.uk/pride/archive/projects/PXD064947)
acquisitions, zero-match scans retain the correction from the latest matched
scan, including zero and positive matches within the same MS1 event.
[Q Exactive Plus MTBLS5657](https://www.ebi.ac.uk/metabolights/MTBLS5657)
provides a separate negative-mode positive-match control. `scan_filter()` does
not yet render `lock`: a validated per-scan flag or equivalent correction-state
interpretation is still needed, including inherited and valid zero corrections.

## Per-scan and acquisition metadata

```python
raw.scan_parameters(3)        # {label: value} trailer-extra dict, or None
raw.status_log(3)             # {label: value} status-log record in effect for the scan, or None
raw.status_log_error          # why the status log could not be decoded, or None
raw.error_log()               # [{"time": ..., "message": ...}, ...] in log order
raw.controllers()             # [{"index", "is_ms_controller", "controller_type", ...}, ...]
raw.instrument_method_text()  # best-effort UTF-16LE text/XML acquisition method blob, or None
```

`status_log` and `scan_parameters` are distinct generic-record streams:
`scan_parameters` holds one trailer record per scan, while `status_log`
reads the instrument-state-over-time log, written every few seconds, and
returns the last record written at or before the scan's start time. Scans
before the first record return `None`. If the file's status log could not
be decoded, `status_log` raises `ValueError` and `status_log_error` says
why.

`controllers()` returns a one-element list for the common single-MS-
controller case; multi-detector files (UV, PDA, Analog channels
alongside MS) return one entry per controller.

## Export

```python
raw.to_mzml("run.mzML")
```

See [mzML export](./mzml-export) for what the output covers.

## Next

- [Reader API](./reader) (Rust)
- [Scan data layouts](./scan-data)
