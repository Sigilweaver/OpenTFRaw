//! Scan windows and observed m/z range of the Core records, on the public CI
//! fixtures (see `common/mod.rs`).

mod common;

use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;

use openmassspec_core::SpectrumSource;
use opentfraw::mzml::{scan_metadata, OpenTfRawSource};
use opentfraw::RawFileReader;

/// LTQ FT Ultra: a scan's window is the acquisition range of its filter
/// string, and its observed m/z range is that of its peaks.
#[test]
fn scan_window_is_the_acquisition_range_not_the_observed_range() {
    let Some(path) = common::fixture() else {
        eprintln!("skipping: no Thermo fixture available");
        return;
    };
    let raw = RawFileReader::open_path(&path).unwrap();
    let mut source = BufReader::new(File::open(&path).unwrap());
    let mut src = OpenTfRawSource::new(&raw, &mut source, "fixture.raw", false);
    let first = src.iter_spectra().next().unwrap().unwrap();
    assert_eq!(
        first.filter.as_deref(),
        Some("FTMS + p ESI Full ms [300.0000-2000.0000]")
    );
    assert_eq!(first.scan_windows.len(), 1);
    assert_eq!(first.scan_windows[0].lower, 300.0);
    assert_eq!(first.scan_windows[0].upper, 2000.0);
    assert_eq!(first.low_mz, first.mz.first().copied());
    assert_eq!(first.high_mz, first.mz.last().copied());
    assert!(first.low_mz.unwrap() > 300.0);
}

/// TSQ Altis SRM: the windows are the product-ion windows of the filter
/// string, to its three decimals.
#[test]
fn srm_scan_windows_are_the_product_ion_windows() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../corpus/PXD041995_TSQ_Altis_milla010603.raw");
    if !path.exists() {
        assert!(
            std::env::var("REQUIRE_CORPUS").as_deref() != Ok("1"),
            "REQUIRE_CORPUS=1 but PXD041995 milla010603.raw is missing"
        );
        eprintln!("skipping: no FlatV66 fixture available");
        return;
    }
    let raw = RawFileReader::open_path(&path).unwrap();
    for idx in (0..raw.num_scans).step_by(97) {
        let meta = scan_metadata(&raw, idx).unwrap();
        let filter = meta.filter.as_deref().unwrap();
        let listed: Vec<(f64, f64)> = filter
            .split('[')
            .nth(1)
            .map(|w| {
                w.trim_end_matches(']')
                    .split(", ")
                    .map(|pair| {
                        let (lo, hi) = pair.split_once('-').unwrap();
                        (lo.parse().unwrap(), hi.parse().unwrap())
                    })
                    .collect()
            })
            .unwrap_or_default();
        assert_eq!(meta.scan_windows.len(), listed.len(), "{filter}");
        for (window, shown) in meta.scan_windows.iter().zip(&listed) {
            // The filter string shows three decimals.
            assert!((window.0 - shown.0).abs() <= 1e-3, "{filter}");
            assert!((window.1 - shown.1).abs() <= 1e-3, "{filter}");
        }
    }
}
