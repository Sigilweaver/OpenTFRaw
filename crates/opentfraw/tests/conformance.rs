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
