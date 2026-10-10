//! Conformance harness: every spectrum produced by `OpenTfRawSource`
//! must satisfy the invariants in `openmassspec-core`.
//!
//! Uses the shared corpus fixture (see `common/mod.rs`). Skips when it is
//! absent so a plain checkout stays green; CI downloads the fixture and sets
//! `REQUIRE_CORPUS=1`, which turns a missing fixture into a failure - see
//! Sigilweaver/OpenMassSpec#5.

mod common;

use std::fs::File;
use std::io::BufReader;

use openmassspec_core::conformance::assert_source_invariants;
use opentfraw::{mzml::OpenTfRawSource, RawFileReader};

#[test]
fn opentfraw_conformance() {
    let Some(path) = common::fixture() else {
        eprintln!("skipping: no Thermo fixture available");
        return;
    };
    let raw = RawFileReader::open_path(&path).expect("open raw");
    let mut source = BufReader::new(File::open(&path).expect("reopen raw"));
    let filename = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    let mut src = OpenTfRawSource::new(&raw, &mut source, filename, false);
    let n = assert_source_invariants(&mut src).expect("conformance");
    assert!(n > 0, "expected at least one spectrum from {filename}");
    eprintln!("opentfraw: {n} spectra passed conformance");
}

#[test]
fn run_metadata_omits_acquisition_paths_by_default() {
    use openmassspec_core::SpectrumSource;

    let Some(path) = common::fixture() else {
        eprintln!("skipping: no Thermo fixture available");
        return;
    };
    let raw = RawFileReader::open_path(&path).expect("open raw");
    let mut source = BufReader::new(File::open(&path).expect("reopen raw"));
    let filename = path.file_name().and_then(|n| n.to_str()).unwrap_or("");

    let meta = OpenTfRawSource::new(&raw, &mut source, filename, false).run_metadata();
    assert!(!meta.extra.contains_key("opentfraw.computer_name"));
    assert!(!meta.extra.contains_key("opentfraw.original_file_path"));
    for key in [
        "opentfraw.original_file_name",
        "opentfraw.instrument_method_file",
        "opentfraw.processing_method_file",
    ] {
        let v = &meta.extra[key];
        assert!(!v.contains(['\\', '/']), "{key} leaks a directory: {v}");
    }
    assert_eq!(
        meta.extra["opentfraw.original_file_name"],
        "20171113_Map_NS1_1to139_4deg_50uM_001.raw"
    );
    assert_eq!(meta.software_version, env!("CARGO_PKG_VERSION"));

    let full = OpenTfRawSource::new(&raw, &mut source, filename, false)
        .with_acquisition_paths(true)
        .run_metadata();
    assert_eq!(
        full.extra["opentfraw.computer_name"],
        raw.raw_file_info.computer_name
    );
    assert_eq!(
        full.extra["opentfraw.original_file_name"],
        raw.seq_row.file_name
    );
    assert!(full.extra.contains_key("opentfraw.original_file_path"));
}
