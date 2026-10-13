//! FlatV66 triple-quadrupole (TSQ) SRM transition decoding, precursor
//! disambiguation, and acquisition parameter extraction.
//!
//! Original RAW files from public studies:
//! - PRIDE PXD041995: `PXD041995_TSQ_Altis_milla010603.raw`
//! - MetaboLights MTBLS6991: `MTBLS6991_mtab_saltysar11_3_011023_40.raw`
//!
//! The data is downloaded separately and is not redistributed in this repo.

use opentfraw::extra::scan_extras;
use opentfraw::{scan_metadata, ExtraFields, RawFileReader};
use std::path::PathBuf;

const MILLA_FIXTURE_NAME: &str = "PXD041995_TSQ_Altis_milla010603.raw";

fn corpus_path(filename: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../corpus")
        .join(filename)
}

// Public PRIDE PXD041995 milla010603.raw.
// Verifies FlatV66 precursor disambiguation when multiple scan events share
// product ion channels (e.g. event 6 and event 31).
#[test]
#[ignore = "requires public PXD041995 milla010603.raw fixture"]
fn public_flatv66_shared_product_precursor_disambiguation() {
    let raw = RawFileReader::open_path(corpus_path(MILLA_FIXTURE_NAME)).unwrap();

    // Event 6 is VVVPAK(+2) with true precursor 306.707.
    // Prior to disambiguation, shared product 513.34 caused it to report 357.213.
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
        // Prior to disambiguation, shared product 587.351 caused it to report 434.905.
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

// Public PRIDE PXD041995 milla010603.raw.
// Verifies extraction of SRM collision energy, dwell time, and isolation window
// width, as well as omission of opentfraw.srm_collision_energies when CEs are uniform.
#[test]
#[ignore = "requires public PXD041995 milla010603.raw fixture"]
fn public_flatv66_srm_metadata_parsing() {
    let raw = RawFileReader::open_path(corpus_path(MILLA_FIXTURE_NAME)).unwrap();

    // Event 6: VVVPAK(+2), Q1=306.707, CE=12.1, dwell=100.0, iso_width=0.7
    assert!(
        (raw.srm_ce_by_event.get(&6).copied().unwrap_or_default() - 12.1).abs() < 0.1,
        "Event 6 CE in map expected ~12.1"
    );
    assert_eq!(
        raw.srm_dwell_by_event.get(&6).copied(),
        Some(100.0),
        "Event 6 dwell time expected 100.0 ms"
    );
    assert_eq!(
        raw.srm_iso_width_by_event.get(&6).copied(),
        Some(0.7),
        "Event 6 isolation width expected 0.7"
    );

    // Event 31: AAEILGR(+2), Q1=365.216, CE=13.9, dwell=100.0, iso_width=0.7
    assert!(
        (raw.srm_ce_by_event.get(&31).copied().unwrap_or_default() - 13.9).abs() < 0.1,
        "Event 31 CE in map expected ~13.9"
    );

    // Verify through scan_metadata and scan_extras
    for idx in 0..raw.num_scans.min(10000) {
        let entry = &raw.scan_index[idx as usize];
        if entry.scan_event == 6 {
            let meta = scan_metadata(&raw, idx).expect("scan metadata");
            let prec = meta.precursor.as_ref().expect("precursor");
            assert!(
                (prec.collision_energy.unwrap_or_default() - 12.1).abs() < 0.1,
                "Event 6 PrecursorInfo CE expected ~12.1"
            );
            assert_eq!(
                prec.isolation_width,
                Some(0.7),
                "Event 6 PrecursorInfo isolation_width expected 0.7"
            );

            let extras = scan_extras(&raw, &meta, &ExtraFields::All);
            assert_eq!(
                extras.get("opentfraw.dwell_time_ms").map(|s| s.as_str()),
                Some("100"),
                "Event 6 dwell time extra field expected '100'"
            );
            // All transitions for event 6 share CE=12.1, so multiple collision energies should not be emitted
            assert!(
                !extras.contains_key("opentfraw.srm_collision_energies"),
                "Event 6 with uniform CEs should not emit srm_collision_energies"
            );
            break;
        }
    }
}

// Public MetaboLights MTBLS6991 mtab_saltysar11_3_011023_40.raw.
// Verifies disparate collision energies across transitions for the same precursor
// (glycine 12.09 vs 5.43), RF lens voltage, dwell time, and embedded instrument method extraction.
#[test]
#[ignore = "requires public MTBLS6991 mtab_saltysar11_3_011023_40.raw fixture"]
fn public_flatv66_published_ebi_metadata_and_method() {
    let path = std::env::var("OPENTFRAW_FLATV66_MULTIPLE_CE_RAW")
        .map(PathBuf::from)
        .unwrap_or_else(|_| corpus_path("MTBLS6991_mtab_saltysar11_3_011023_40.raw"));
    let raw = RawFileReader::open_path(&path).unwrap();

    // Event 0 (glycine confirm): Q1=76.0, Q3=29.968, CE=12.09, RF=30.0
    // Event 2 (putrescine confirm): Q1=89.0, Q3=29.968, CE=23.48, RF=30.0
    // These two scan events share product ion 29.968, demonstrating product ion disambiguation.
    assert!((raw.srm_q1_by_event.get(&0).copied().unwrap_or_default() - 76.0).abs() < 0.1);
    assert!((raw.srm_ce_by_event.get(&0).copied().unwrap_or_default() - 12.09).abs() < 0.1);
    assert_eq!(raw.srm_rf_lens_by_event.get(&0).copied(), Some(30.0));
    assert_eq!(raw.srm_dwell_by_event.get(&0).copied(), Some(100.0));
    assert_eq!(raw.srm_iso_width_by_event.get(&0).copied(), Some(0.7));

    assert!((raw.srm_q1_by_event.get(&2).copied().unwrap_or_default() - 89.0).abs() < 0.1);
    assert!((raw.srm_ce_by_event.get(&2).copied().unwrap_or_default() - 23.48).abs() < 0.1);

    // Event 6 (choline confirm): Q1=104.0, CE=21.90, RF=44.0
    assert_eq!(raw.srm_rf_lens_by_event.get(&6).copied(), Some(44.0));
    assert!((raw.srm_ce_by_event.get(&6).copied().unwrap_or_default() - 21.90).abs() < 0.1);

    // Verify through scan_metadata and scan_extras
    let mut tested_event_0 = false;
    let mut tested_event_6 = false;
    for idx in 0..raw.num_scans.min(1000) {
        let entry = &raw.scan_index[idx as usize];
        if entry.scan_event == 0 && !tested_event_0 {
            let meta = scan_metadata(&raw, idx).expect("scan metadata");
            let prec = meta.precursor.as_ref().expect("precursor");
            assert!((prec.collision_energy.unwrap_or_default() - 12.09).abs() < 0.1);
            let extras = scan_extras(&raw, &meta, &ExtraFields::All);
            assert_eq!(
                extras
                    .get("opentfraw.srm_collision_energies")
                    .map(|s| s.as_str()),
                Some("12.09 5.43"),
                "Event 0 (glycine) should report both quant and confirm collision energies"
            );
            tested_event_0 = true;
        } else if entry.scan_event == 6 && !tested_event_6 {
            let meta = scan_metadata(&raw, idx).expect("scan metadata");
            let prec = meta.precursor.as_ref().expect("precursor");
            assert!((prec.collision_energy.unwrap_or_default() - 21.90).abs() < 0.1);
            assert_eq!(prec.isolation_width, Some(0.7));

            let extras = scan_extras(&raw, &meta, &ExtraFields::All);
            assert_eq!(
                extras.get("opentfraw.s_lens_rf_level").map(|s| s.as_str()),
                Some("44")
            );
            assert_eq!(
                extras.get("opentfraw.dwell_time_ms").map(|s| s.as_str()),
                Some("100")
            );
            tested_event_6 = true;
        }
        if tested_event_0 && tested_event_6 {
            break;
        }
    }
    assert!(tested_event_0, "Tested event 0");
    assert!(tested_event_6, "Tested event 6");

    // Verify embedded instrument method text extraction
    let mut file_for_method = std::fs::File::open(&path).expect("reopen file for method");
    let method_text = raw
        .instrument_method_text(&mut file_for_method)
        .expect("extract method text")
        .expect("method text should be present");
    assert!(
        method_text.contains("TSQ Altis Plus Method Summary"),
        "Method text should contain instrument method summary header"
    );
    assert!(
        method_text.contains("glycine"),
        "Method text should contain compound definitions"
    );
    assert!(
        method_text.contains("putrescine"),
        "Method text should contain compound definitions"
    );
}
