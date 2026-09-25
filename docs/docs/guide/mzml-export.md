---
sidebar_position: 4
---

# mzML export

The `to_mzml` example writes a minimal HUPO mzML 1.1 document covering
the scans in a RAW file. It is also exposed as a method on the Python
`RawFile`:

```python
import opentfraw

raw = opentfraw.RawFile("run.raw")
raw.to_mzml("run.mzML")
```

The output is intentionally minimal: spectrum index, scan headers,
m/z + intensity arrays (base64 + zlib), and the filter string. It
validates against the PSI-MS mzML 1.1 schema and is suitable as a
bridge into existing mzML-based pipelines. It does not aim to populate
every optional controlled-vocabulary annotation the schema permits; it
covers the core scan and peak data plus the vendor metadata OpenTFRaw
decodes directly from the binary.

Each spectrum also carries the scan's acquisition event id and its
`opentfraw.*` extra values (resolution, AGC, lock mass, instrument-status
readings and so on; `opentfraw.extra_field_keys()` lists them) as
`<userParam>` elements, and references an `instrumentConfiguration` for
its mass analyzer. The extras add roughly 10-14% to the file on
small-spectrum data. To choose them:

```python
raw.to_mzml("run.mzML", extra_fields=[])  # none
raw.to_mzml("run.mzML", extra_fields=["opentfraw.resolution"])  # only these
raw.to_mzml("run.mzML", exclude_extra_fields=["opentfraw.sps_masses"])  # all but these
```

From Rust, pass `ExtraFields` to `OpenTfRawSource::with_extra_fields`.

OpenTFRaw's goal is direct, open access to the RAW binary and a
faithful open-standard mzML rendering of it.
