//! Conformance harness: every spectrum produced by `OpenTfRawSource`
//! must satisfy the invariants in `openmassspec-core`.
//!
//! Looks for a small Thermo `.raw` fixture in a few candidate locations
//! (see `fixture()`) and skips silently when none is present, so a plain
//! checkout without any corpus stays green.
//!
//! In CI, `.github/workflows/ci.yml`'s `build` job downloads
//! `corpus/PXD054004_LTQ_FT_20171113_Map_NS1_1to139_4deg_50uM_001.raw`
//! (repo-root-relative) ahead of `cargo test`, so this test exercises a
//! real decode path there instead of skipping - see
//! Sigilweaver/OpenMassSpec#5.

use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;

use openmassspec_core::conformance::assert_source_invariants;
use opentfraw::{mzml::OpenTfRawSource, RawFileReader};

fn fixture() -> Option<PathBuf> {
    let candidates = [
        // CI / repo-root corpus dir (gitignored; populated by ci.yml or
        // scripts/fetch_corpus.py). This is the one CI actually uses.
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(
            "../../corpus/PXD054004_LTQ_FT_20171113_Map_NS1_1to139_4deg_50uM_001.raw",
        ),
        // Local dev setups that symlink a sibling SpecLance checkout's
        // corpus/ (see SpecLance's own fixtures/CORPUS.md conventions).
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../SpecLance/corpus/thermo/PXD068962_Q_Exactive_UHMR_insource-CID.raw"),
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../SpecLance/corpus/thermo/PXD054004_LTQ_FT_20171113_Map_NS1_1to139_4deg_50uM_001.raw"),
    ];
    candidates.into_iter().find(|p| p.exists())
}

#[test]
fn opentfraw_conformance() {
    let Some(path) = fixture() else {
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
