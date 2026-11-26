# Migrating to opentfraw 4.0

4.0 adopts `openmassspec-core` 4.0. `CHANGELOG.md` lists every change. Core's
own changes are in
[MIGRATING-4.0.md](https://github.com/Sigilweaver/OpenMassSpecCore/blob/main/MIGRATING-4.0.md)
in OpenMassSpecCore; this page covers what callers of this crate edit.

## Python

- Python 3.10 or newer is required.
- `RawFile.iter_records()` and `run_info()` return Core 4 records. In a
  record's `precursor`:

  ```python
  # before (3.1.0), one MS2 record of an LTQ FT Ultra file
  rec["precursor"]["collision_energy"]   # 35.0
  rec["precursor"]["ce_is_nce"]          # False (the unit was assumed eV)
  rec["low_mz"], rec["high_mz"]          # 145.0, 590.0 (the acquisition range)

  # after (4.0.0)
  rec["precursor"]["collision_energies"]  # [{"value": 35.0, "unit": "nce"}]
  rec["low_mz"], rec["high_mz"]           # 185.475..., 543.246... (observed)
  rec["scan_windows"]                     # [{"lower": 145.0, "upper": 590.0}]
  ```

  A stepped energy lists every step. `unit` is `"nce"`, `"ev"` or `None` when
  the file does not state it. `analyzer` is `"tof"` where it was `"tofms"`.
  A scan with no peaks has `base_peak_mz` and `base_peak_intensity` of `None`
  (it had 0.0) and a `warnings` entry of kind `opentfraw.empty_spectrum`.
- `RawFile.scan()` and `iter_scans()` keep `collision_energy` and
  `collision_energy_is_nce` and add `collision_energy_unit`,
  `collision_energy_source` and `collision_energy_steps`.
- `to_mzml` raises `OSError` for a scan whose collision energy has no stated
  unit, whose m/z values are not ascending, or whose precursor names a scan
  that is not in the output. Pass `allow=["unknown_collision_energy_unit"]`
  (or `"dangling_precursor_ref"`) to write the first and last with a warning;
  unsorted m/z has no `allow`. The README lists which files this affects.
- `to_mzml` emits a `UserWarning` for every warning code it wrote, including
  `opentfraw.empty_spectrum` for scans with no peaks.

## Rust

```toml
[dependencies]
opentfraw = "4"
openmassspec-core = "4"   # only if you use it directly
```

- `OpenTfRawSource`'s `SpectrumSource` impl, `mzml::write_mzml` and
  `mzml::write_indexed_mzml` need `R: Read + Seek + Send`. A reader that is
  not `Send` has to be wrapped or replaced, for example by `File` or
  `BufReader<File>`.
- `PrecursorInfo` has `collision_energy_unit: Option<CollisionEnergyUnit>`,
  `collision_energy_source` and `collision_energy_steps`. Read the unit from
  `collision_energy_unit` instead of `ce_is_nce`, which is true only for NCE
  and false for both eV and an unknown unit.
- On the Core side, a record has `precursor.collision_energies` (a list with
  a unit each) in place of `collision_energy` and `ce_is_nce`, and
  `retention_time_sec` is an `Option`.
- To write spectra that Core's strict checks refuse, build a `Leniency` and
  pass it to `WriteOptions::with_leniency`; the `to_mzml` example does this
  with `--allow <check>`.
