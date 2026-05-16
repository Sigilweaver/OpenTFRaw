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

| Field             | Type            | Description                                     |
| ----------------- | --------------- | ----------------------------------------------- |
| `num_scans`       | `u32`           | Scan count from the run header                  |
| `device_family`   | `DeviceFamily`  | Heuristic instrument classification             |
| `scan_format`     | `ScanDataFormat`| Which decoder will be used for peaks            |
| `instrument_model`| `String`        | Reported instrument model from `RawFileInfo`    |
| `scan_events`     | `Vec<ScanEvent>`| Per-scan event metadata (filter, polarity, ...) |

Peaks are read with `read_scan_peaks`:

```rust
let mut file = std::fs::File::open("sample.raw")?;
let peaks = raw.read_scan_peaks(&mut file, 1)?;
for (mz, intensity) in peaks.mz.iter().zip(peaks.intensity.iter()) {
    println!("{mz:.4}\t{intensity:.0}");
}
```

`read_scan_peaks` dispatches on `scan_format` to one of the three
decoders described in [Scan data](./scan-data).

## Error handling

Public functions return `opentfraw::Result<T>`. The error type is
`opentfraw::Error`, which wraps the failure category (`Io`, `Parse`,
`UnsupportedVersion`, `BadMagic`, ...) and a message.
