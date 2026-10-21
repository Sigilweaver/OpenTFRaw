# OpenTFRaw

[![CI](https://github.com/Sigilweaver/OpenTFRaw/actions/workflows/ci.yml/badge.svg)](https://github.com/Sigilweaver/OpenTFRaw/actions/workflows/ci.yml)
[![DOI](https://zenodo.org/badge/DOI/10.5281/zenodo.20470601.svg)](https://doi.org/10.5281/zenodo.20470601)
[![crates.io](https://img.shields.io/crates/v/opentfraw.svg)](https://crates.io/crates/opentfraw)
[![PyPI](https://img.shields.io/pypi/v/opentfraw.svg)](https://pypi.org/project/opentfraw/)
[![docs.rs](https://img.shields.io/docsrs/opentfraw)](https://docs.rs/opentfraw)
[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](https://github.com/Sigilweaver/OpenTFRaw/blob/main/LICENSE)
[![Rust MSRV](https://img.shields.io/badge/rust-1.88%2B-orange.svg)](https://www.rust-lang.org)

> Part of the [OpenMassSpec](https://github.com/Sigilweaver/OpenMassSpec)
> stack for mass spectrometry raw-file access.

Rust and Python reader for Thermo Fisher `.raw` mass spectrometry files.
It is tested on format versions 63, 64 and 66, from LTQ ion traps through
Orbitrap Astral and TSQ Altis (see [Known issues and gaps](#known-issues-and-gaps)
and [CORPUS.md](CORPUS.md)).

Documentation: [sigilweaver.app/opentfraw/docs](https://sigilweaver.app/opentfraw/docs)

## Known issues and gaps

Measured on the public test corpus (291 files from 56 PRIDE datasets).

- **Scan metadata is wrong on 34 of 291 corpus files (12%).** On these
  files the scan event is misread: MS level, polarity, analyzer,
  activation and precursor fields can be wrong, for example MS2 scans
  reported as negative-polarity MS1 in a positive-mode run. Peak m/z and
  intensity values are not affected. Affected: 30 of 52 Orbitrap Fusion
  Lumos files (PXD007683, PXD025230, PXD037285, PXD064117, PXD067311,
  PXD070648), 2 of 2 Orbitrap Ascend files (PXD061791), 1 of 18 Orbitrap
  Eclipse files (PXD054530) and 1 LTQ file (PXD039587).
- **Precursor m/z reads as 0.5** on the MS2 scans of 4 Orbitrap Fusion
  Lumos files (PXD023217): `target_mz` and `selected_mz` are 0.5 while the
  scan filter and the reaction hold the real precursor (for example 652.9).
- **Activation methods:** CID, HCD, ETD and EThcD are identified. ETD and
  EThcD are read only from format version 66 files (no older ETD file is in
  the corpus). Other methods (UVPD, ECD, PQD and so on) decode as unknown,
  and mzML then has no dissociation-method term.
- **Analyzer is not reported** for Orbitrap Astral MS2 scans and for TSQ
  triple-quadrupole scans.
- **SRM data:** polarity is always reported as positive and isolation width
  as 0.7. Decoding of TSQ Vantage (format version 63) SRM files is
  unreliable: in PXD020246, some scans contain NaN values or peaks outside
  the scan's m/z range. 4 of its 35,098 scans have a flat-peak record that
  matches no known layout; they raise an error, and
  `to_mzml(..., skip_unreadable=True)` (Python) or the `to_mzml` example's
  `--skip-unreadable` flag writes the other scans and reports how many
  were left out.
- **Profile spectra** with an unrecognised calibration layout have NaN m/z
  values; mzML reports the count in `opentfraw.unconverted_profile_bins`.
- **m/z accuracy** follows the instrument's stored calibration. In FTMS
  MS1 scans of 223 corpus files, background ions with known masses
  (polysiloxanes, diisooctyl phthalate) are within 0.55 ppm of their
  theoretical m/z at the median and 4.1 ppm at the 95th percentile.
- **Tested files:** format versions 63, 64 and 66 only; positive polarity
  only in the corpus (two negative-mode Q Exactive Plus files back opt-in
  tests, see CORPUS.md); no GC-Orbitrap, LCQ-family, Orbitrap ID-X, Exactive,
  TSQ Quantis/Endura/Fortis or single-quadrupole files; no data from
  non-MS controllers (UV, PDA). Other versions and instruments may open but
  are untested.

## Install

**Prefer [`openmassspec-io`](https://github.com/Sigilweaver/OpenMassSpec)
with the `thermo` feature/extra** unless you need this parser standalone
(minimal dependencies, or building your own abstraction) - the umbrella
gives you format auto-detection, mzML conversion, and Arrow streaming
across all wired-in vendors for free:

```sh
cargo add openmassspec-io --features thermo
```

```sh
pip install openmassspec[thermo]
```

Standalone:

Rust:

```sh
cargo add opentfraw
```

Python:

```sh
pip install opentfraw
```

## Quickstart

Rust:

```rust
use std::{fs::File, io::BufReader};
use opentfraw::RawFileReader;

let raw = RawFileReader::open_path("sample.raw")?;
// Buffer the scan reads: an unbuffered File is about 11x slower.
let mut file = BufReader::new(File::open("sample.raw")?);
let info = &raw.run_header.sample_info;
for scan_num in info.first_scan_number..=info.last_scan_number {
    let peaks = raw.read_scan_peaks(&mut file, scan_num)?;
    println!("scan {scan_num}: {} peaks", peaks.len());
}
```

Python:

```python
import opentfraw

raw = opentfraw.RawFile("run.raw")
mz, intensity = raw.peaks(3)
print(raw.scan(3)["filter_string"])
raw.to_mzml("run.mzML")
```

See the [docs site](https://sigilweaver.app/opentfraw/docs) for the full
guide, format specification, and API reference.

## Used by

| Project | Description |
|---------|-------------|
| [Mascope](https://github.com/ultra-trace-systems/mascope) by [Ultra Trace](https://github.com/ultra-trace-systems) | Mass spectrometry data analysis platform using OpenTFRaw to ingest Thermo `.raw` files. |

## License

Apache-2.0. See [LICENSE](https://github.com/Sigilweaver/OpenTFRaw/blob/main/LICENSE).

The format specification was developed by binary analysis of public
mass-spectrometry datasets (PRIDE accessions). See
[CORPUS.md](https://github.com/Sigilweaver/OpenTFRaw/blob/main/CORPUS.md) and [ATTRIBUTION.md](https://github.com/Sigilweaver/OpenTFRaw/blob/main/ATTRIBUTION.md).
