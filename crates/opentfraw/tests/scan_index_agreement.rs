//! Cross-check decoded centroids against the scan index on the CI fixture.
//!
//! For every scan, the most intense decoded peak must match the index's
//! base-peak m/z and intensity, and no decoded peak may fall outside the
//! index's scan window. The scan index and the peak stream are written
//! independently by the instrument software, so agreement here is a strong
//! end-to-end check of peak decoding. Profile m/z values, when present, must
//! be finite (an unknown calibration must not leak through as m/z).
//!
//! Uses the shared corpus fixture (see `common/mod.rs`); skips when it is
//! absent unless `REQUIRE_CORPUS=1`.

mod common;

use std::fs::File;
use std::io::BufReader;

use opentfraw::RawFileReader;

/// Relative tolerance for the base-peak intensity (index stores f64, peaks
/// store f32).
const INTENSITY_REL_TOL: f64 = 1e-5;
/// Absolute m/z tolerance for the base peak. The index base m/z can differ
/// from the stored centroid by ~1e-4 on LTQ FT data.
const MZ_TOL: f64 = 1e-3;
/// Absolute m/z slack on the scan-window bounds. Orbitrap centroids at the
/// window edge can sit a few thousandths below the nominal low m/z (seen on
/// 6 of 152,328 Fusion Lumos scans in PXD031322).
const WINDOW_TOL: f64 = 0.01;

#[test]
fn decoded_peaks_agree_with_scan_index() {
    let Some(path) = common::fixture() else {
        eprintln!("skipping: no Thermo fixture available");
        return;
    };
    let raw = RawFileReader::open_path(&path).expect("open raw");
    let mut source = BufReader::new(File::open(&path).expect("reopen raw"));
    let first = raw.run_header.sample_info.first_scan_number;

    let mut checked = 0usize;
    let mut with_peaks = 0usize;
    let mut failures = Vec::new();
    for (i, entry) in raw.scan_index.iter().enumerate() {
        let scan = first + i as u32;
        let packet = raw.read_scan(&mut source, scan).expect("read scan");
        let peaks = &packet.peaks;
        checked += 1;

        if let (Some(profile), Some(event)) = (&packet.profile, raw.scan_events.get(i)) {
            if let Some((mz, _)) = profile
                .to_mz_intensity(&event.coefficients)
                .into_iter()
                .find(|(mz, _)| !mz.is_finite())
            {
                failures.push(format!("scan {scan}: non-finite profile m/z {mz}"));
            }
        }

        if peaks.is_empty() {
            if entry.base_intensity > 0.0 {
                failures.push(format!(
                    "scan {scan}: no peaks but index base intensity {}",
                    entry.base_intensity
                ));
            }
            continue;
        }
        with_peaks += 1;

        let max_ab = peaks.iter().map(|p| p.abundance).fold(f32::MIN, f32::max) as f64;
        let rel = (max_ab - entry.base_intensity).abs() / entry.base_intensity.abs().max(1.0);
        if rel > INTENSITY_REL_TOL {
            failures.push(format!(
                "scan {scan}: base intensity decoded {max_ab} vs index {}",
                entry.base_intensity
            ));
        }
        // Ties on intensity are possible; any tied peak may be the base peak.
        let base_mz_ok = peaks
            .iter()
            .filter(|p| p.abundance as f64 == max_ab)
            .any(|p| (p.mz - entry.base_mz).abs() <= MZ_TOL);
        if !base_mz_ok {
            failures.push(format!(
                "scan {scan}: base m/z index {} not among most intense decoded peaks",
                entry.base_mz
            ));
        }

        for p in peaks {
            if !(p.mz >= entry.low_mz - WINDOW_TOL && p.mz <= entry.high_mz + WINDOW_TOL) {
                failures.push(format!(
                    "scan {scan}: peak m/z {} outside window [{}, {}]",
                    p.mz, entry.low_mz, entry.high_mz
                ));
                break;
            }
        }
    }

    eprintln!(
        "scan index agreement: {checked} scans checked, {with_peaks} with peaks, {} failures",
        failures.len()
    );
    assert!(checked > 0, "fixture has no scans");
    assert!(with_peaks > 0, "fixture has no decoded peaks");
    assert!(
        failures.is_empty(),
        "{} disagreements, first few:\n{}",
        failures.len(),
        failures
            .iter()
            .take(20)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    );
}
