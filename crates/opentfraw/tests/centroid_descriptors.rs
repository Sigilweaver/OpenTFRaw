//! The per-centroid descriptor stream, checked against what else the file
//! stores: the peak list, the profile chunks and the scan trailer.
//!
//! The first test uses the shared corpus fixture (see `common/mod.rs`) and
//! skips when it is absent unless `REQUIRE_CORPUS=1`. The second is opt-in
//! and takes published acquisitions through environment variables; their
//! sources and hashes are in CORPUS.md.

mod common;

use std::fs::File;
use std::io::BufReader;

use opentfraw::scan_data::freq_to_mz;
use opentfraw::{Analyzer, RawFileReader};

/// Mask of the peak position held in the low 20 bits of a descriptor word.
const POSITION_MASK: u32 = 0xF_FFFF;
/// Bit set on the centroid of a lock mass found in the scan.
const LOCK_MASS_BIT: u32 = 1 << 21;

#[test]
fn ft_descriptors_index_the_peaks_and_their_profile_chunks() {
    let Some(path) = common::fixture() else {
        eprintln!("skipping: no Thermo fixture available");
        return;
    };
    let raw = RawFileReader::open_path(&path).expect("open raw");
    let mut source = BufReader::new(File::open(&path).expect("reopen raw"));
    let first = raw.run_header.sample_info.first_scan_number;

    let (mut ft_scans, mut other_scans, mut centroids) = (0usize, 0usize, 0usize);
    for (i, event) in raw.scan_events.iter().enumerate() {
        let scan = first + i as u32;
        let packet = raw.read_scan(&mut source, scan).expect("read scan");
        if packet.peaks.is_empty() {
            continue;
        }
        if event.preamble().analyzer() != Some(Analyzer::FTMS) {
            // Ion-trap scans store no descriptor stream.
            assert_eq!(packet.header.descriptor_list_size, 0, "scan {scan}");
            assert!(packet.descriptors.is_empty(), "scan {scan}");
            other_scans += 1;
            continue;
        }
        ft_scans += 1;

        // One word per centroid, and the labels-only read returns the same.
        assert_eq!(packet.descriptors.len(), packet.peaks.len(), "scan {scan}");
        let labels = raw
            .read_scan_labels(&mut source, scan)
            .expect("read labels");
        assert_eq!(labels.descriptors, packet.descriptors, "scan {scan}");

        // The low 20 bits count the peaks, and the profile has one chunk per
        // centroid in the same order.
        let profile = packet.profile.as_ref().expect("FT scan stores a profile");
        assert_eq!(profile.chunks.len(), packet.peaks.len(), "scan {scan}");
        let coefficients = event.coefficients();
        for (k, (word, peak)) in packet.descriptors.iter().zip(&packet.peaks).enumerate() {
            assert_eq!((word & POSITION_MASK) as usize, k, "scan {scan}");

            // The centroid lies in its chunk's m/z range, or within a bin of
            // the chunk's first or last point.
            let chunk = &profile.chunks[k];
            let mz_at = |bin: f64| {
                let freq = profile.first_value + (chunk.first_bin as f64 + bin) * profile.step;
                freq_to_mz(freq, coefficients) + chunk.fudge.unwrap_or(0.0) as f64
            };
            let last = (chunk.signal.len() - 1) as f64;
            let (a, b) = (mz_at(0.0), mz_at(last));
            let (lo, hi) = (a.min(b), a.max(b));
            let bin = (mz_at(1.0) - mz_at(0.0)).abs();
            assert!(
                peak.mz >= lo - bin && peak.mz <= hi + bin,
                "scan {scan} peak {k}: m/z {} outside chunk {lo}..{hi}",
                peak.mz
            );
            centroids += 1;
        }
    }
    assert!(ft_scans > 0 && other_scans > 0 && centroids > 0);
}

// Published original acquisitions, supplied by the caller. Neither
// acquisition data nor vendor output is stored in this repository.
#[test]
#[ignore = "requires a public MTBLS5657, PXD071477 or PXD064947 lock-mass fixture"]
fn public_lock_mass_centroids_carry_bit_21() {
    // (variable, instrument, scans, scans locked on lock mass #1, #2, #3)
    let fixtures = [
        (
            "OPENTFRAW_LOCK_MASS_PLUS_RAW",
            "Q Exactive Plus",
            138,
            [0, 3, 135],
        ),
        (
            "OPENTFRAW_LOCK_MASS_HFX_RAW",
            "Q Exactive HF-X",
            27832,
            [3559, 0, 0],
        ),
        (
            "OPENTFRAW_LOCK_MASS_EXPLORIS_RAW",
            "Orbitrap Exploris 480",
            51254,
            [16033, 0, 0],
        ),
    ];
    let mut files = 0;
    for (variable, instrument, expected_scans, expected_by_label) in fixtures {
        let Ok(path) = std::env::var(variable) else {
            continue;
        };
        files += 1;
        let raw = RawFileReader::open_path(&path).unwrap();
        let mut source = BufReader::new(File::open(&path).unwrap());
        assert_eq!(raw.instrument_model, Some(instrument));
        assert_eq!(raw.num_scans, expected_scans);

        let first = raw.run_header.sample_info.first_scan_number;
        let mut by_label = [0usize; 3];
        for scan in first..=raw.run_header.sample_info.last_scan_number {
            let found = raw
                .scan_params(scan)
                .and_then(|p| p.number_of_matched_lock_masses())
                .unwrap_or_else(|| panic!("scan {scan}: no lock-mass count"));
            let record = raw.scan_parameters(scan).unwrap();
            let lock_masses = [1, 2, 3].map(|n| {
                record
                    .get_f64(&format!("Lock Mass #{n} (m/z):"))
                    .unwrap_or_else(|| panic!("scan {scan}: no lock mass #{n}"))
            });

            let packet = raw.read_scan_labels(&mut source, scan).unwrap();
            assert_eq!(packet.descriptors.len(), packet.peaks.len());
            let flagged: Vec<usize> = (0..packet.peaks.len())
                .filter(|&k| packet.descriptors[k] & LOCK_MASS_BIT != 0)
                .collect();
            // As many centroids flagged as lock masses found: one or none.
            assert_eq!(flagged.len(), found as usize, "{instrument} scan {scan}");
            assert!(flagged.len() <= 1, "{instrument} scan {scan}");

            // Centroid m/z is stored as f32 in these files, and the flagged
            // one is the f32 nearest a lock mass of the trailer.
            assert_eq!(packet.header.layout & 0x10000, 0);
            for k in flagged {
                let mz = packet.peaks[k].mz as f32;
                let label = lock_masses
                    .iter()
                    .position(|&m| m as f32 == mz)
                    .unwrap_or_else(|| panic!("scan {scan}: {mz} is none of {lock_masses:?}"));
                by_label[label] += 1;
            }
        }
        assert_eq!(by_label, expected_by_label, "{instrument}");
    }
    assert!(
        files > 0,
        "set at least one OPENTFRAW_LOCK_MASS_*_RAW variable"
    );
}
