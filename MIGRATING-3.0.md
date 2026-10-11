# Migrating to opentfraw 3.0

3.0 is the last planned breaking release of the 3.x line. Public enums and
structs are `#[non_exhaustive]`, so later 3.x releases can add variants,
fields and accessors without breaking your code. `CHANGELOG.md` lists every
change; this page covers what callers usually need to edit.

Python users: the `RawFile` API is unchanged except that decode failures
raise instead of being skipped (step 2).

## 1. Dependencies

```toml
[dependencies]
opentfraw = "3"
openmassspec-core = "3"   # only if you use it directly
```

## 2. Unreadable scans are errors

Nothing skips an unreadable scan any more.

`extract_spectrum` returns `Result`:

```rust
// before
if let Some(rec) = opentfraw::extract_spectrum(&raw, &mut src, idx, false) { ... }

// after
let rec = opentfraw::extract_spectrum(&raw, &mut src, idx, false)?;
```

`iter_spectra` yields one `Result` per scan:

```rust
for rec in opentfraw::iter_spectra(&raw, &mut src, false) {
    let rec = rec?;
    // ...
}
```

`OpenTfRawSource` follows the `openmassspec_core` 3.0 `SpectrumSource`
trait: `iter_spectra` and `iter_chromatograms` yield
`openmassspec_core::Result` items.

`write_mzml` and `write_indexed_mzml` return
`Result<openmassspec_core::WriteSummary>` and stop at the first unreadable
scan. If you used the value as `()`, ignore it or read
`summary.spectra_written`. To write the readable scans and count the rest:

```rust
use openmassspec_core::{write_mzml_with_options, RecordErrorPolicy, WriteOptions};

let mut src = opentfraw::mzml::OpenTfRawSource::new(&raw, &mut file, "run.raw", false);
let opts = WriteOptions::new().with_record_error_policy(RecordErrorPolicy::Skip);
let summary = write_mzml_with_options(&mut src, &mut out, &opts)?;
if summary.skipped() > 0 {
    eprintln!("{} scans could not be decoded", summary.skipped());
}
```

`RawFileReader::instrument_method_text` returns `Result<Option<String>>`:
add a `?`.

## 3. Errors

`opentfraw::Error` is `#[non_exhaustive]`; add a `_ =>` arm to matches. It
has new variants: `Core` (an `openmassspec_core::Error`, for example from
the mzML writer), `ScanIndexOutOfRange`, and `UnrecognizedScanRecord`. `?` converts an
`opentfraw::Error` into `openmassspec_core::Error`.

## 4. Matching enums

Every public enum (`Analyzer`, `Polarity`, `ScanMode`, `MsPower`,
`ScanType`, `Ionization`, `Activation`, `GenericType`, `GenericValue`,
`DeviceFamily`, `ScanDataFormat`, `ControllerType`, `ExtraFields`) needs a
wildcard arm when matched outside this crate. Decide what an unknown value
means for your code rather than defaulting silently.

## 5. Building structs

Struct literals of this crate's types no longer compile outside it. Fields
stay public for reading. The types you may need to build have constructors:

```rust
// before
let rec = GenericRecord { values };
let p = ScanParams(&rec);

// after
let rec = GenericRecord::new(values);
let p = ScanParams::new(&rec);
```

`StatusLogEntry::new` works the same way. `PrecursorInfo` keeps `Default`.

The scan-event types are read through methods instead of fields:

```rust
// before
let coeffs = &event.coefficients;
for rx in &event.reactions { println!("{} {}", rx.precursor_mz, rx.energy); }

// after
let coeffs = event.coefficients();
for rx in event.reactions() { println!("{} {}", rx.precursor_mz(), rx.energy()); }
```

The same applies to `event.preamble()`, `event.fraction_collectors()`,
`FractionCollector::low_mz()` / `high_mz()` and `ScanEventPreamble::bytes()`.

## 6. Removed items

| Removed | Use instead |
| --- | --- |
| `scan_data::read_flat_peaks`, `scan_data::read_scan_srm_v66` | `RawFileReader::read_scan_flat`, `read_scan_srm_v66`, or `read_scan_peaks` |
| `scan_data::read_scan_srm_v66_windows`, `search_v63_transition` | `RawFileReader::srm_q3_windows`, `srm_q1_by_event`, `srm_ce_by_event` |
| `scan_filter::build_filter` | `RawFileReader::scan_filter` |
| `DeviceFamily::detect*`, `DetectedInstrument` | `RawFileReader::device_family`, `instrument_model` |
| `AuditTag::unknown_long`, `RunHeader::unk_addr`, `Reaction::unknown_*` | none; these held undecoded bytes |
| `Activation` variants other than `HCD` and `CID`; `ControllerType::{Analog, Adc, Pda, Uv}` | `Activation::Unknown(code)`; `ControllerType::Other` (see `CHANGELOG.md`) |
