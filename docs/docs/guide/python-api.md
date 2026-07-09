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
raw.scan_filter(3)                        # canonical Thermo filter string, or None
raw.profile(3)                            # (mz, intensity) from the raw profile signal
raw.centroid_labels(3)                    # mz/intensity/resolution/noise/baseline/signal_to_noise
```

## Per-scan and acquisition metadata

```python
raw.scan_parameters(3)        # {label: value} trailer-extra dict, or None
raw.status_log(3)             # {label: value} instrument status log (temps, voltages, ...), or None
raw.error_log()               # [{"time": ..., "message": ...}, ...] in log order
raw.controllers()             # [{"index", "is_ms_controller", "controller_type", ...}, ...]
raw.instrument_method_text()  # best-effort UTF-16LE text/XML acquisition method blob, or None
```

`status_log` and `scan_parameters` are both per-scan generic-record
streams decoded from the file, but distinct ones: `scan_parameters`
mirrors the vendor reader's trailer-extra values, while `status_log` is
the instrument-state-over-time log.

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
