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

If a scan's data cannot be decoded, the export fails with that error
instead of leaving the scan out. To write the readable scans instead:

```python
skipped = raw.to_mzml("run.mzML", skip_unreadable=True)
```

This emits a `UserWarning` with the count and returns it. From Rust, an
`OpenTfRawSource` written with `openmassspec_core::write_mzml_with_options`
and `RecordErrorPolicy::Skip` does the same and reports the count in the
returned `WriteSummary`.

## Strict checks

The writer refuses a spectrum whose collision energy has no stated unit,
whose precursor references a spectrum that is not in the output, or whose m/z
values are not ascending. The error names the spectrum. The `allow` keyword
(Python) or `--allow <check>` flag (`to_mzml` example) writes the first two
anyway, marks each with a warning userParam and reports how many were marked:

```python
raw.to_mzml("run.mzML", allow=["unknown_collision_energy_unit"])
```

The checks are `unknown_collision_energy_unit`, `dangling_precursor_ref` and
`unknown_scan_mode` (OpenTFRaw always states a scan mode, so the last never
fires). Unsorted m/z has no check; `skip_unreadable=True` leaves such a scan
out and counts it. From Rust, build a `Leniency` and pass it to
`WriteOptions::with_leniency`.

A scan with no peaks is written with no base peak and the warning
`empty_spectrum`; the writer reports their number like any other warning.
A scan whose trailer states a charge but whose precursor has no m/z carries the
charge in the `opentfraw.precursor_charge` userParam and the warning
`opentfraw.charge_without_selected_mz`; mzML has no place for a charge without
a selected ion. The scan window of a scan is the acquisition range of its filter
string; the observed range is that of its peaks.

Collision energy carries the unit the file states. A trailer label
(`HCD Energy:` for normalized energy, `HCD Energy eV:` for electronvolts)
states its own unit; the energy stored in the scan event has none, and takes
the unit the embedded instrument method states (see the README, Known
issues and gaps). A stepped energy writes every step. Each spectrum's
`opentfraw.collision_energy_source` userParam says which of `trailer_nce`,
`trailer_ev`, `scan_event` or `srm_transition` the value came from. The
in-source CID energy is not a collision energy of the precursor: it stays in
`opentfraw.source_cid_energy_ev` (electronvolts) with its provenance in
`opentfraw.source_cid_energy_source`.

OpenTFRaw's goal is direct, open access to the RAW binary and a
faithful open-standard mzML rendering of it.
