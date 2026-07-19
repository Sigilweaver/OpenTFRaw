---
sidebar_position: 1
---

# Reader

The entry point is `RawFileReader`. Opening a file parses the file
header, audit tags, sample information, run header, scan event tree,
scan index, error log, and generic data section. Scan peak data is
read on demand from a separate `std::fs::File` handle.

```rust
use opentfraw::RawFileReader;

let raw = RawFileReader::open_path("sample.raw")?;
```

After `open_path` returns, the following fields are populated:

| Field                    | Type                            | Description                                                                      |
| ------------------------ | -------------------------------- | ----------------------------------------------------------------------------------- |
| `header`                 | `FileHeader`                     | Raw file magic/version header                                                     |
| `seq_row`                | `SeqRow`                         | Sequence-table row (sample id, comment, vial, ...); surfaced to Python as `sample_info` |
| `raw_file_info`          | `RawFileInfo`                    | Computer name, controller count, acquisition date, and other file-level metadata   |
| `run_header`             | `RunHeader`                      | Acquisition time range, first/last scan number                                    |
| `scan_index`             | `Vec<ScanIndexEntry>`            | Per-scan byte offsets into the data stream                                        |
| `scan_events`            | `Vec<ScanEvent>`                 | Per-scan event metadata (filter, polarity, ...)                                   |
| `scan_parameters_header` | `GenericDataHeader`              | Column layout for `scan_parameters`                                               |
| `scan_parameters`        | `Vec<GenericRecord>`             | Per-scan generic trailer values (`scan_parameters` in Python)                     |
| `error_log`              | `Vec<ErrorEntry>`                | Instrument error/status messages; surfaced to Python as `error_log()`              |
| `inst_log_header`        | `GenericDataHeader`              | Column layout for `inst_log`                                                      |
| `inst_log`               | `Vec<GenericRecord>`             | Instrument status-log records; surfaced to Python as `status_log()`               |
| `version`                | `u32`                            | Raw file format version                                                            |
| `num_scans`              | `u32`                            | Scan count from the run header                                                    |
| `data_addr`              | `u64`                            | Data stream base address (for computing absolute scan offsets)                    |
| `flat_peaks`             | `bool`                           | True if scan data uses flat-peak format (TSQ/SRM) instead of `PacketHeader`        |
| `scan_format`            | `ScanDataFormat`                 | Which decoder will be used for peaks                                              |
| `device_family`          | `DeviceFamily`                   | Heuristic instrument classification                                               |
| `instrument_model`       | `Option<&'static str>`           | Reported instrument model from file metadata, if detected                         |
| `srm_q1_by_event`        | `HashMap<u16, f64>`              | SRM only: scan_event -> Q1 precursor m/z                                           |
| `srm_q3_windows`         | `HashMap<u16, Vec<(f32, f32)>>`  | SRM only: scan_event -> Q3 isolation windows                                       |
| `srm_ce_by_event`        | `HashMap<u16, f64>`              | SRM only: scan_event -> collision energy (eV); empty for v66/TSQ Altis files       |

For multi-controller files (MS + UV/PDA/Analog), `controllers` enumerates
each controller's metadata as `Vec<ControllerInfo>` (index, run-header
address, `is_ms_controller`, `ControllerType`, first/last scan, start/end
time):

```rust
let mut file = std::fs::File::open("sample.raw")?;
let controllers = raw.controllers(&mut file)?;
```

Peaks are read with `read_scan_peaks`:

```rust
let mut file = std::fs::File::open("sample.raw")?;
let peaks = raw.read_scan_peaks(&mut file, 1)?;
for (mz, intensity) in peaks.mz.iter().zip(peaks.intensity.iter()) {
    println!("{mz:.4}\t{intensity:.0}");
}
```

`read_scan_peaks` dispatches on `scan_format` to one of the three
decoders described in [Scan data](./scan-data). Related read methods:

| Method              | Returns          | Notes                                                                          |
| ------------------- | ----------------- | --------------------------------------------------------------------------------- |
| `read_scan`         | `ScanDataPacket` | Full packet (profile + centroids + FT labels); `PacketHeader` format only          |
| `read_scan_labels`  | `ScanDataPacket` | Centroids + FT labels, skipping the profile signal; `PacketHeader` format only     |
| `read_scan_peaks`   | `Vec<Peak>`      | Centroided peaks, dispatching on `scan_format`; the recommended default           |
| `read_peaks_only`   | `Vec<Peak>`      | Like `read_scan_peaks` but skips profile data on `PacketHeader` files for a 2-10x speedup |
| `read_scan_flat`    | `Vec<Peak>`      | Low-level: TSQ/SRM (v63) flat-peak format                                          |
| `read_scan_srm_v66` | `Vec<Peak>`      | Low-level: TSQ Quantiva/Altis (v66) SRM format                                     |

## Error handling

Public functions return `opentfraw::Result<T>`. The error type is
`opentfraw::Error`, which wraps the failure category (`Io`, `Parse`,
`UnsupportedVersion`, `BadMagic`, ...) and a message.
