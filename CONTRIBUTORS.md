# Contributors

Thank you to everyone who has contributed to OpenTFRaw.

## Oskari Kausiala ([@oskarsari](https://github.com/oskarsari)) — [Karsa Oy](https://github.com/karsa-oy)

Contributed in v1.2.0:

- **Exploris scan-event fix** — decoded the frequency-to-m/z calibration
  coefficients and MS2 precursor m/z for Orbitrap Exploris files, which were
  previously missing due to a body-offset mismatch.
- **Per-peak FT label data** — decoded the descriptor / unknown / triplet
  streams that trail the centroid peak list, exposing per-peak resolution,
  noise, and baseline via `ScanDataPacket.resolutions`, `noise_nodes`, and
  `noise_at(mz)`, with a Python `centroid_labels()` accessor.
- **`RawFile.profile()`** — surfaced raw profile spectra to the Python
  bindings, converting frequency-domain bins to m/z via the scan event's
  calibration coefficients.
- **`RawFile.scan_parameters()`** — exposed per-scan trailer parameters as a
  `{label: value}` dict in the Python bindings.
- **`RawFile.created`** — exposed the file creation / acquisition start
  timestamp from the Xcalibur audit tag.
