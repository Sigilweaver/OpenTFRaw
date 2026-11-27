//! Peak counts of TSQ Vantage (format version 63) flat-peak scans, settled
//! against the scan index.
//!
//! Original RAW file from a public study: PRIDE PXD020246
//! (`iota_sp_1-1.RAW`, TSQ Vantage). The data is downloaded separately and is
//! not redistributed in this repo. Point `OPENTFRAW_TSQ_VANTAGE_RAW` at it, or
//! place it at `corpus/PXD020246_TSQVantage.RAW`.

use openmassspec_core::SpectrumSource;
use opentfraw::mzml::{OpenTfRawSource, RECOVERED_PEAK_COUNT_KEY, UNVERIFIED_PEAK_COUNT_KEY};
use opentfraw::{iter_spectra, PeakCountCheck, RawFileReader};
use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;

fn fixture() -> Option<PathBuf> {
    let path = std::env::var_os("OPENTFRAW_TSQ_VANTAGE_RAW")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../corpus/PXD020246_TSQVantage.RAW")
        });
    if path.exists() {
        Some(path)
    } else {
        eprintln!(
            "skipping: PXD020246 TSQVantage.RAW not found at {}",
            path.display()
        );
        None
    }
}

/// PXD020246: 35,098 scans; the default peak count disagrees with the scan
/// index on 1,969 (1,856 with unsorted m/z, 55 non-finite, 4 undecodable, 54
/// ascending but wrong) and exactly one other count agrees on every one.
#[test]
#[ignore = "requires public PXD020246 TSQVantage.RAW fixture"]
fn pxd020246_recovers_1969_scans_and_leaves_none_unverified() {
    let Some(path) = fixture() else { return };
    let raw = RawFileReader::open_path(&path).expect("open raw");
    assert_eq!(raw.num_scans, 35_098);
    assert_eq!(raw.v63_transition_events_skipped, 0);

    // Through the source: the extra keys and the counters.
    let mut file = BufReader::new(File::open(&path).expect("reopen raw"));
    let mut source = OpenTfRawSource::new(&raw, &mut file, "PXD020246_TSQVantage.RAW", false);
    let (mut n, mut flagged) = (0usize, 0usize);
    for record in source.iter_spectra() {
        let record = record.expect("every scan decodes");
        n += 1;
        assert!(record.mz.windows(2).all(|w| w[0] <= w[1]), "m/z ascends");
        assert!(record.mz.iter().all(|m| m.is_finite()));
        assert!(!record.extra.contains_key(UNVERIFIED_PEAK_COUNT_KEY));
        if record.extra.contains_key(RECOVERED_PEAK_COUNT_KEY) {
            flagged += 1;
        }
    }
    assert_eq!(n, 35_098);
    assert_eq!(flagged, 1_969);
    assert_eq!(source.recovered_peak_count_scans(), 1_969);
    assert_eq!(source.unverified_peak_count_scans(), 0);

    // Through the free function: the flag is on the record.
    let mut file = BufReader::new(File::open(&path).expect("reopen raw"));
    let mut recovered = 0usize;
    for record in iter_spectra(&raw, &mut file, false) {
        match record.expect("every scan decodes").peak_count {
            PeakCountCheck::Agreed => {}
            PeakCountCheck::Recovered => recovered += 1,
            other => panic!("unexpected {other:?}"),
        }
    }
    assert_eq!(recovered, 1_969);
    // The reader counts decodes: both passes above read every scan.
    assert_eq!(raw.peak_counts_recovered(), 2 * 1_969);
    assert_eq!(raw.peak_counts_unverified(), 0);
}
