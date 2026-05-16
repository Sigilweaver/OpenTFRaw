# opentfraw -- Python bindings

Python bindings for the [OpenTFRaw](https://github.com/Sigilweaver/OpenTFRaw)
Rust crate. Reads Thermo Fisher `.raw` mass spectrometry files via a
NumPy-friendly API.

## Install (from source)

```bash
cd python
pip install maturin
maturin develop --release
```

## Quickstart

```python
import opentfraw

raw = opentfraw.RawFile("experiment.raw")
print(len(raw), raw.instrument_model)

# Per-scan metadata + peaks as numpy arrays
s = raw.scan(1)
print(s["ms_level"], s["retention_time"], s["filter_string"])
mz = s["mz"]              # numpy.float64[:]
intensity = s["intensity"] # numpy.float32[:]

# Convert to mzML
raw.to_mzml("experiment.mzML")
```

## API

- `RawFile(path)` — load a `.raw` file
- `.num_scans`, `.first_scan`, `.last_scan`, `.instrument_model`, `.path`
- `.peaks(scan_number) -> (mz, intensity)` — fast centroided arrays
- `.scan_filter(scan_number) -> str | None` — Thermo scan filter string
- `.scan(scan_number) -> dict` — full per-scan metadata + arrays
- `.iter_scans() -> list[dict]` — every scan as dicts
- `.to_mzml(out_path)` — write mzML 1.1.0

This is an early preview. Feedback welcome.
