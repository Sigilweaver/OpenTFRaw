# OpenTFRaw

An open, pure-Rust parser for **Thermo Fisher RAW** mass-spectrometry
files — reverse-engineered from scratch, without the vendor SDK.

## Status

**Experimental.** Reads file-format versions **8, 47, 57, 60, 62, 63, 64,
and 66** — covering every Thermo instrument line from the LCQ Classic
(mid-1990s) through to the Orbitrap Astral (2023) and the latest TSQ
triple quadrupoles.

A 27-file validation corpus spanning every major instrument family is
built by [`Sigilweaver/TFRaw-Sources`](https://github.com/Sigilweaver/TFRaw-Sources)
and fetched locally via [`scripts/fetch_corpus.py`](scripts/fetch_corpus.py)
— see [CORPUS.md](CORPUS.md).

## What's Parsed

| Component              | Status |
| ---------------------- | ------ |
| File header + audit tags           | ✅     |
| Sample information + sequence row  | ✅     |
| RAW file info + instrument method  | ✅     |
| Run header (multi-controller)      | ✅     |
| Scan event hierarchy + filters     | ✅     |
| Scan index / trailer               | ✅     |
| Scan peak data (packet + flat)     | ✅     |
| Error log                          | ✅     |
| Scan parameters / instrument log   | ✅     |
| Generic data section               | ✅     |
| Device / instrument classification | 🚧 best-effort |
| Full tune-method block             | ⏳     |
| Status log fine-grained schema     | ⏳     |

Format versions covered: **8, 47, 57, 60, 62, 63, 64, 66**.

## Quick start

```rust
use opentfraw::RawFileReader;

let raw = RawFileReader::open_path("sample.raw")?;
println!("{} — {} scans", raw.device_family.display_name(), raw.num_scans);

let mut file = std::fs::File::open("sample.raw")?;
for scan_num in 1..=raw.num_scans {
    let peaks = raw.read_scan_peaks(&mut file, scan_num)?;
    // peaks: Vec<Peak { mz, intensity, ... }>
}
```

The `dump` example prints a full human-readable summary:

```sh
cargo run --release --example dump -- path/to/file.raw [--max-scans N]
```

## Architecture

Three scan-data codecs are dispatched automatically by a format router:

| Router variant | When it applies                                  | Instruments                  |
| -------------- | ------------------------------------------------ | ---------------------------- |
| `PacketHeader` | Profile + centroid scans with a header packet    | All Orbitrap / ion trap      |
| `FlatV63`      | Variable-size flat peaks, version ≤ 63           | Older TSQ / SRM              |
| `FlatV66`      | Fixed 12-byte peak triplets, version ≥ 64        | TSQ Quantiva / Altis SRM     |

Entry point is `RawFileReader::read_scan_peaks`, which dispatches on
`scan_format: ScanDataFormat` populated during `open()`.

`DeviceFamily` (see [`src/device.rs`](src/device.rs)) classifies the
instrument into one of 10 families using a heuristic over the audit
tag + instrument method path + first-scan analyzer.

## Repository layout

```
OpenTFRaw/
├── src/                    # Rust crate
│   ├── lib.rs              # public API (RawFileReader, DeviceFamily, …)
│   ├── reader.rs           # main open() + dispatch logic
│   ├── header.rs           # file header
│   ├── audit_tag.rs        # audit tag blocks
│   ├── sample_info.rs      # sample info section
│   ├── seq_row.rs          # sequence row
│   ├── raw_file_info.rs    # RAW-file preamble + inst_method
│   ├── run_header.rs       # run header (single or multi-controller)
│   ├── scan_event.rs       # scan event hierarchy (filter / polarity / etc.)
│   ├── scan_index.rs       # scan index + trailer
│   ├── scan_data.rs        # packet-header and flat-peak decoders
│   ├── scan_format.rs      # format-dispatch enum
│   ├── device.rs           # DeviceFamily taxonomy + detection
│   ├── error_log.rs        # error log section
│   ├── generic_data.rs     # generic data section
│   ├── types.rs            # shared type aliases + Analyzer/Detector/etc.
│   └── error.rs            # Error / Result
├── examples/dump.rs        # CLI pretty-printer + validator
├── scripts/fetch_corpus.py # pulls PRIDE corpus (see CORPUS.md)
├── SPEC.md                 # evolving binary format specification
└── CORPUS.md               # validation corpus methodology + provenance
```

## Corpus

The validation corpus is not redistributed in this repo — it's pulled
from the [PRIDE Archive](https://www.ebi.ac.uk/pride/) on demand.

Project discovery and metadata is provided by the sibling repository
[`Sigilweaver/TFRaw-Sources`](https://github.com/Sigilweaver/TFRaw-Sources),
which catalogues 3,400+ Thermo-instrument PRIDE projects with 169,000+
.raw files. See [CORPUS.md](CORPUS.md) for the methodology.

```sh
python3 scripts/fetch_corpus.py             # default 400 MB cap
python3 scripts/fetch_corpus.py --max-mb 800 # raise cap for Astral etc.
```

## Why

Thermo RAW is the dominant proprietary format in bottom-up and top-down
proteomics. Existing readers depend on Thermo's Windows-only .NET SDK
wrapped via Mono or PythonNet — painful to deploy, awkward on
ARM/macOS, and opaque when things go wrong. A pure-Rust reader unlocks
cross-platform tooling (conversion, indexing, search) without the
vendor runtime dependency.

## Related

- [SPEC.md](SPEC.md) — binary format specification
- [CORPUS.md](CORPUS.md) — validation corpus methodology
- [`Sigilweaver/TFRaw-Sources`](https://github.com/Sigilweaver/TFRaw-Sources) — source catalogue

## License

Not yet licensed.
