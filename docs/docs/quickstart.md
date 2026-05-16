---
sidebar_position: 3
---

# Quickstart

## Rust

Open a file and read its peaks:

```rust
use opentfraw::RawFileReader;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let raw = RawFileReader::open_path("sample.raw")?;
    println!("{} -- {} scans", raw.device_family.display_name(), raw.num_scans);

    let mut file = std::fs::File::open("sample.raw")?;
    for scan_num in 1..=raw.num_scans {
        let peaks = raw.read_scan_peaks(&mut file, scan_num)?;
        println!("scan {scan_num}: {} peaks", peaks.mz.len());
    }
    Ok(())
}
```

The repo ships two examples:

```sh
cargo run --release --example dump -- path/to/file.raw [--max-scans N]
cargo run --release --example to_mzml -- path/to/file.raw output.mzML
```

`dump` walks every section of the file and prints a summary; `to_mzml`
writes a minimal mzML.

## Python

```python
import opentfraw

raw = opentfraw.RawFile("run.raw")
print(raw.num_scans, raw.instrument_model)

mz, intensity = raw.peaks(3)      # float64 / float32 numpy arrays
scan = raw.scan(3)                # dict: ms_level, RT, charge, filter, ...
print(scan["filter_string"])

raw.to_mzml("run.mzML")
```

## Next

- [Reader API](./guide/reader)
- [Scan data layouts](./guide/scan-data)
- [Instrument families](./guide/instrument-families)
- [Format specification](./format/overview)
