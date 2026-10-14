//! Integration test verifying FlatV66 transition disambiguation
//! when multiple scan events share product ion m/z channels.

use opentfraw::{scan_metadata, RawFileReader};
use std::path::PathBuf;

fn find_flatv66_fixture() -> Option<PathBuf> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let candidates = [
        root.join("corpus/milla010603.raw"),
        root.join("corpus/PXD041995_TSQ_Altis_milla010603.raw"),
        root.join("milla010603.raw"),
    ];
    candidates.into_iter().find(|p| p.exists())
}

#[test]
fn test_flatv66_shared_product_precursor_disambiguation() {
    let Some(path) = find_flatv66_fixture() else {
        assert!(
            std::env::var("REQUIRE_CORPUS").as_deref() != Ok("1"),
            "REQUIRE_CORPUS=1 but public PXD041995 milla010603.raw is missing"
        );
        eprintln!("Skipping FlatV66 corpus test: public PXD041995 milla010603.raw is missing");
        return;
    };

    let file = std::fs::File::open(&path).expect("open file");
    let raw = RawFileReader::open(file).expect("open raw file");

    // Event 6 is VVVPAK(+2) with true precursor 306.707.
    // Prior to the fix, shared product 513.34 caused it to report 357.213.
    let mut found_event_6 = false;
    let mut found_event_31 = false;

    for idx in 0..raw.num_scans.min(10000) {
        let entry = &raw.scan_index[idx as usize];
        if entry.scan_event == 6 && !found_event_6 {
            let meta = scan_metadata(&raw, idx).expect("scan metadata");
            let prec = meta
                .precursor
                .and_then(|p| p.selected_mz)
                .unwrap_or_default();
            assert!(
                (prec - 306.707).abs() < 0.05,
                "Event 6 precursor expected ~306.707, got {prec}"
            );
            found_event_6 = true;
        }

        // Event 31 is AAEILGR(+2) with true precursor 365.216.
        // Prior to the fix, shared product 587.351 caused it to report 434.905.
        if entry.scan_event == 31 && !found_event_31 {
            let meta = scan_metadata(&raw, idx).expect("scan metadata");
            let prec = meta
                .precursor
                .and_then(|p| p.selected_mz)
                .unwrap_or_default();
            assert!(
                (prec - 365.216).abs() < 0.05,
                "Event 31 precursor expected ~365.216, got {prec}"
            );
            found_event_31 = true;
        }

        if found_event_6 && found_event_31 {
            break;
        }
    }

    assert!(
        found_event_6,
        "Did not encounter scan event 6 in test scans"
    );
    assert!(
        found_event_31,
        "Did not encounter scan event 31 in test scans"
    );
}
