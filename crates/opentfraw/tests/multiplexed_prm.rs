use opentfraw::{scan_metadata, RawFileReader};

// Public PRIDE PXD021080
// Aug1_A057_0-2fmol_CSP-NP_12fmolHeavy_Mucus_iRT_PRM_7min_r1.raw.
// A Q Exactive HF-X PRM acquisition in which most MS2 scans isolate two
// precursors. Their scan-event records hold two 56-byte reactions, so the
// stream mixes 280- and 336-byte records. Each scan's trailer names its
// injections in `Multiple Injection:`, one `h` per precursor, plus `mpx` on
// the multiplexed ones.
#[test]
#[ignore = "requires public PXD021080 Q Exactive HF-X multiplexed PRM fixture"]
fn public_multiplexed_prm_events() {
    let path = std::env::var("OPENTFRAW_MULTIPLEXED_PRM_RAW").expect("set fixture path");
    let raw = RawFileReader::open_path(path).unwrap();
    assert_eq!(raw.num_scans, 4142);
    let first = raw.run_header.sample_info.first_scan_number;
    let mut by_precursors = [0u32; 3];
    for n in first..=raw.run_header.sample_info.last_scan_number {
        let event = &raw.scan_events[(n - first) as usize];
        let precursors = event.reactions().len();
        let injections = raw
            .scan_params(n)
            .unwrap()
            .record()
            .get_string("Multiple Injection:")
            .unwrap()
            .split_whitespace()
            .filter(|token| *token == "h")
            .count();
        assert_eq!(precursors, injections, "scan {n}");
        by_precursors[precursors] += 1;

        let meta = scan_metadata(&raw, n - first).unwrap();
        assert_eq!(meta.ms_level, 2, "scan {n}");
        let filter = raw.scan_filter(n).unwrap();
        assert!(filter.starts_with("FTMS + p NSI Full ms2 "), "{filter}");
        assert_eq!(filter.matches("@hcd28.00").count(), precursors, "{filter}");
    }
    assert_eq!(by_precursors, [0, 1061, 3081]);
}
