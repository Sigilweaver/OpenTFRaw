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
m/z + intensity arrays (base64 + zlib), and the filter string. It is
suitable as a bridge into existing mzML-based pipelines but does not
attempt to reproduce every controlled-vocabulary annotation that
`ProteoWizard msconvert` would emit.

For richer mzML, run `msconvert` on a Windows host with the
`ThermoRawFileParser` tools; OpenTFRaw is designed to give you direct
access to the binary, not to be a drop-in replacement for that
toolchain.
