//! Regression coverage for issue #44: DIA isolation-window center m/z on
//! Orbitrap Exploris 480 (PXD035500) and Fusion Lumos (PXD031322) DIA files.
//!
//! At the time issue #44 was filed, `CORPUS.md` documented this as an open
//! problem: no known m/z field in the DIA scan-event body. That note predates
//! PR #4 (fix(scan-event): decode Exploris coefficients + MS2 precursor),
//! which added a fallback that reads the precursor/isolation double directly
//! from `body[4..12]` whenever the primary reaction parse comes back empty on
//! an MS2+ event. That fallback fires for DDA and DIA alike (both are
//! MS2+/non-primary events), so it already recovers the DIA isolation center
//! on Exploris. For tribrid Fusion Lumos, DIA events go through the ordinary
//! tribrid reaction-record path (`n_reactions` at body[0], records from
//! body[4]) and already carry the isolation center as `reactions[0].precursor_mz`,
//! the same as DDA precursors do.
//!
//! Per this project's clean-room policy, correctness here is argued only
//! from a self-consistency invariant within the file itself, not from any
//! vendor tool: the same 32-byte reaction record that yields `precursor_mz`
//! also yields `unknown_double` at body offset +8, decoded by the existing,
//! already-reviewed reaction-parsing code (no new decoding added here). That
//! field is checked against `ScanParams::isolation_width_mz()`, an
//! independently-decoded value read from the self-describing trailer-extra
//! record (field label `"MS2 Isolation Width:"`, taken verbatim from ASCII
//! text embedded in the RAW file's own `GenericDataHeader` schema - not
//! looked up in any external reference). The two are decoded via entirely
//! separate code paths (scan-event body vs. trailer-extra table) and match
//! exactly across every DIA scan in both corpus files, which is strong
//! evidence the reaction record parsed at this body offset for DIA scans is
//! the real isolation-window record and not a misaligned read: `unknown_double`
//! is self-evidently the window width, and `precursor_mz` in the same record
//! is the window center.
//!
//! Looks for fixtures under `../../../ProLance/corpus/thermo/` (or the
//! current `SpecLance` naming - same convention as `tests/conformance.rs`).
//! Skips silently when absent so CI without the corpus is happy.

use std::path::PathBuf;

use opentfraw::RawFileReader;

fn corpus_file(name: &str) -> Option<PathBuf> {
    // Same sibling-checkout convention as `tests/conformance.rs`: current
    // SpecLance naming first, falling back to the pre-rename ProLance
    // checkout layout some local dev setups still have on disk.
    [
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../SpecLance/corpus/thermo"),
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../ProLance/corpus/thermo"),
    ]
    .into_iter()
    .map(|dir| dir.join(name))
    .find(|p| p.exists())
}

/// For every DIA MS2+ scan event, check:
///  - exactly one reaction (isolation window) is recovered
///  - its precursor_mz (window center) is finite and physically plausible
///  - its unknown_double (window width, same record) matches the
///    independently-decoded trailer `MS2 Isolation Width:` field
///
/// Returns the number of DIA scans checked.
fn assert_all_dia_scans_self_consistent(
    raw: &RawFileReader,
    center_low: f64,
    center_high: f64,
) -> usize {
    let mut n_dia = 0;
    let first_scan = raw.run_header.sample_info.first_scan_number;
    for (i, evt) in raw.scan_events.iter().enumerate() {
        if !evt.preamble.is_dia() {
            continue;
        }
        n_dia += 1;
        let scan_num = first_scan + i as u32;

        assert_eq!(
            evt.reactions.len(),
            1,
            "scan {scan_num}: DIA scan event should have exactly one reaction (isolation window)"
        );
        let rxn = &evt.reactions[0];
        assert!(
            rxn.precursor_mz.is_finite() && rxn.precursor_mz > center_low && rxn.precursor_mz < center_high,
            "scan {scan_num}: isolation center m/z {} out of plausible range ({center_low}, {center_high})",
            rxn.precursor_mz
        );

        let trailer_width = raw
            .scan_params(scan_num)
            .and_then(|p| p.isolation_width_mz())
            .unwrap_or_else(|| panic!("scan {scan_num}: no trailer MS2 Isolation Width"));
        assert!(
            (rxn.unknown_double - trailer_width).abs() < 1e-3,
            "scan {scan_num}: reaction-record width {} does not match trailer isolation width {trailer_width}",
            rxn.unknown_double
        );
    }
    n_dia
}

#[test]
fn exploris480_dia_isolation_mz_self_consistent() {
    let Some(path) = corpus_file("PXD035500_Orbitrap_Exploris_480_RN_SGLab_210301_DN_vDIA_01.raw")
    else {
        eprintln!("skipping: PXD035500 Exploris 480 DIA fixture not available");
        return;
    };
    let raw = RawFileReader::open_path(&path).expect("open raw");

    // Window centers can sit outside the declared [200,1200] scan range near
    // the edges of a window (center +/- half-width can cross the range), so
    // this is a loose sanity bound, not a precision check - the width
    // cross-check against the trailer is what does the real validation.
    let n_dia = assert_all_dia_scans_self_consistent(&raw, 100.0, 3000.0);
    assert!(
        n_dia > 80_000,
        "expected tens of thousands of DIA scans, got {n_dia}"
    );
}

#[test]
fn fusion_lumos_dia_isolation_mz_self_consistent() {
    let Some(path) = corpus_file("PXD031322_Orbitrap_Fusion_Lumos_OFL001513-YLL-GPF-15K-1.raw")
    else {
        eprintln!("skipping: PXD031322 Fusion Lumos DIA fixture not available");
        return;
    };
    let raw = RawFileReader::open_path(&path).expect("open raw");

    let n_dia = assert_all_dia_scans_self_consistent(&raw, 100.0, 3000.0);
    assert!(
        n_dia > 70_000,
        "expected tens of thousands of DIA scans, got {n_dia}"
    );
}
