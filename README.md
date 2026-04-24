# OpenTFRaw

Pure-Rust parser for Thermo Fisher RAW mass-spectrometry files, reverse-engineered without the vendor SDK.

Reads format versions 8, 47, 57, 60, 62, 63, 64, and 66 -- covering every Thermo instrument
line from the LCQ Classic (mid-1990s) through the Orbitrap Astral (2023) and the latest TSQ
triple quadrupoles.

## Status

Experimental. A 27-file validation corpus spanning every major instrument family is fetched
on demand via `scripts/fetch_corpus.py`. See [CORPUS.md](CORPUS.md).

## What is parsed

| Component                           | Status       |
| ----------------------------------- | ------------ |
| File header + audit tags            | yes          |
| Sample information + sequence row   | yes          |
| RAW file info + instrument method   | yes          |
| Run header (multi-controller)       | yes          |
| Scan event hierarchy + filters      | yes          |
| Scan index / trailer                | yes          |
| Scan peak data (packet + flat)      | yes          |
| Error log                           | yes          |
| Scan parameters / instrument log    | yes          |
| Scan filter strings (Thermo syntax) | yes          |
| Generic data section                | yes          |
| Device / instrument classification  | best-effort  |
| Full tune-method block              | not yet      |
| Status log fine-grained schema      | not yet      |

## Quick start

### Rust

```rust
use opentfraw::RawFileReader;

let raw = RawFileReader::open_path("sample.raw")?;
println!("{} -- {} scans", raw.device_family.display_name(), raw.num_scans);

let mut file = std::fs::File::open("sample.raw")?;
for scan_num in 1..=raw.num_scans {
    let peaks = raw.read_scan_peaks(&mut file, scan_num)?;
}
```

```sh
cargo run --release --example dump -- path/to/file.raw [--max-scans N]
cargo run --release --example to_mzml -- path/to/file.raw output.mzML
```

### Python

The `python/` sub-crate wraps the Rust library via PyO3. Build with
[maturin](https://www.maturin.rs/):

```sh
cd python && maturin develop --release
```

```python
import opentfraw

raw = opentfraw.RawFile("run.raw")
print(raw.num_scans, raw.instrument_model)

mz, intensity = raw.peaks(3)        # float64 / float32 numpy arrays
s = raw.scan(3)                     # dict with ms_level, RT, charge, etc.
print(s["filter_string"])           # e.g. "ITMS + c NSI d Full ms2 384.8@cid30.00 [110-1166]"

raw.to_mzml("run.mzML")
```

## Architecture

Scan data is dispatched to one of three decoders based on the file's scan-data format:

| Variant        | When it applies                                | Instruments              |
| -------------- | ---------------------------------------------- | ------------------------ |
| `PacketHeader` | Profile + centroid scans with a header packet  | All Orbitrap / ion trap  |
| `FlatV63`      | Variable-size flat peaks, version <= 63        | Older TSQ / SRM          |
| `FlatV66`      | Fixed 12-byte peak triplets, version >= 64     | TSQ Quantiva / Altis SRM |

Entry point is `RawFileReader::read_scan_peaks`, which dispatches on
`scan_format: ScanDataFormat` set during `open()`.

`DeviceFamily` (`src/device.rs`) classifies the instrument into one of 10 families using a
heuristic over the audit tag, instrument method path, and first-scan analyzer.

## Repository layout

```
src/
  lib.rs              public API (RawFileReader, DeviceFamily, ...)
  reader.rs           open() and dispatch logic
  header.rs           file header
  audit_tag.rs        audit tag blocks
  sample_info.rs      sample info section
  seq_row.rs          sequence row
  raw_file_info.rs    RAW-file preamble + inst_method
  run_header.rs       run header (single or multi-controller)
  scan_event.rs       scan event hierarchy (filter / polarity / etc.)
  scan_index.rs       scan index + trailer
  scan_data.rs        packet-header and flat-peak decoders
  scan_format.rs      format-dispatch enum
  device.rs           DeviceFamily taxonomy + detection
  error_log.rs        error log section
  generic_data.rs     generic data section
  types.rs            shared enums (Analyzer, Polarity, Activation, ...)
  error.rs            Error / Result
examples/
  dump.rs             CLI pretty-printer + validator
  to_mzml.rs          mzML export
python/               PyO3 Python bindings (opentfraw wheel)
scripts/
  fetch_corpus.py     pulls PRIDE corpus (see CORPUS.md)
SPEC.md               binary format specification
CORPUS.md             validation corpus methodology + provenance
CREDITS.md            prior art and third-party acknowledgements
```

## Corpus

The validation corpus is pulled from the [PRIDE Archive](https://www.ebi.ac.uk/pride/) on demand
and is not redistributed in this repo. The file `scripts/sources.json` records the exact PRIDE
accession and filename for each instrument; the fetcher downloads any entry not already on disk.

```sh
python3 scripts/fetch_corpus.py
python3 scripts/fetch_corpus.py --dry-run
```

## Why

Existing open-source RAW readers depend on Thermo's Windows-only .NET SDK (wrapped via Mono or
PythonNet), which limits deployment to Windows or requires a non-trivial runtime. OpenTFRaw parses
the binary format directly with no vendor dependency, enabling cross-platform use on Linux, macOS,
and Windows without the .NET runtime.

## Related

- [SPEC.md](SPEC.md) -- binary format specification
- [CORPUS.md](CORPUS.md) -- validation corpus methodology
- [CREDITS.md](CREDITS.md) -- prior art and acknowledgements

## License

Copyright 2026 Sigilweaver Holdings LLC

Licensed under the Apache License, Version 2.0. See [LICENSE](LICENSE).
