use opentfraw::generic_data::{GenericRecord, GenericValue};
use opentfraw::{
    extra::scan_extras, scan_metadata, ExtraFields, RawFileReader, ScanParams, StatusLogEntry,
    StatusLogRecord,
};

fn record(matched: Option<GenericValue>, configured: Option<GenericValue>) -> GenericRecord {
    let mut values = Vec::new();
    if let Some(v) = matched {
        values.push(("Number of LM Found:".into(), v));
    }
    if let Some(v) = configured {
        values.push(("Number of Lock Masses:".into(), v));
    }
    GenericRecord { values }
}

fn assert_counts(
    r: &GenericRecord,
    matched: Option<i32>,
    configured: Option<i32>,
    legacy: Option<i32>,
) {
    let p = ScanParams(r);
    assert_eq!(p.number_of_matched_lock_masses(), matched);
    assert_eq!(p.number_of_configured_lock_masses(), configured);
    assert_eq!(p.number_of_lock_masses(), legacy);
    assert_eq!(p.number_of_lm_found(), legacy);
    let s = StatusLogEntry(r);
    assert_eq!(s.number_of_matched_lock_masses(), matched);
    assert_eq!(s.number_of_configured_lock_masses(), configured);
    assert_eq!(s.number_of_lock_masses(), legacy);
}

#[test]
fn configured_only_does_not_become_matched() {
    assert_counts(
        &record(None, Some(GenericValue::Int32(9))),
        None,
        Some(9),
        Some(9),
    );
}

#[test]
fn matched_only_does_not_become_configured() {
    assert_counts(
        &record(Some(GenericValue::Int16(1)), None),
        Some(1),
        None,
        Some(1),
    );
}

#[test]
fn conflicting_counts_and_zero_matches_stay_distinct() {
    assert_counts(
        &record(Some(GenericValue::Int32(1)), Some(GenericValue::Int32(9))),
        Some(1),
        Some(9),
        Some(1),
    );
    assert_counts(
        &record(Some(GenericValue::Int8(0)), Some(GenericValue::Int32(9))),
        Some(0),
        Some(9),
        Some(0),
    );
    assert_counts(
        &record(Some(GenericValue::Int32(0)), Some(GenericValue::Int32(0))),
        Some(0),
        Some(0),
        Some(0),
    );
}

#[test]
fn missing_and_mistyped_counts_stay_unknown() {
    assert_counts(&record(None, None), None, None, None);
    assert_counts(
        &record(
            Some(GenericValue::String("1".into())),
            Some(GenericValue::Int32(9)),
        ),
        None,
        Some(9),
        Some(9),
    );
    assert_counts(
        &record(
            Some(GenericValue::Int32(1)),
            Some(GenericValue::Float64(9.0)),
        ),
        Some(1),
        None,
        Some(1),
    );
    assert_counts(
        &record(
            Some(GenericValue::Bool(true)),
            Some(GenericValue::String("9".into())),
        ),
        None,
        None,
        None,
    );
}

#[test]
fn negative_counts_do_not_establish_matches_or_configuration() {
    assert_counts(
        &record(Some(GenericValue::Int32(-1)), Some(GenericValue::Int32(9))),
        None,
        Some(9),
        Some(-1),
    );
    assert_counts(
        &record(Some(GenericValue::Int32(1)), Some(GenericValue::Int32(-1))),
        Some(1),
        None,
        Some(1),
    );
}

// The fixtures are intentionally published original acquisitions. Paths are
// supplied by the caller; neither acquisition data nor vendor output is stored
// in this repository. Exact sources and hashes are documented in CORPUS.md.
#[test]
#[ignore = "requires public MTBLS5657, PXD071477 and PXD064947 lock-mass fixtures"]
fn public_lock_mass_counts_and_inherited_correction() {
    for (variable, instrument, expected_scans, expected_matches, expected_ms1_zero) in [
        (
            "OPENTFRAW_LOCK_MASS_PLUS_RAW",
            "Q Exactive Plus",
            138,
            138,
            0,
        ),
        (
            "OPENTFRAW_LOCK_MASS_HFX_RAW",
            "Q Exactive HF-X",
            27832,
            3559,
            209,
        ),
        (
            "OPENTFRAW_LOCK_MASS_EXPLORIS_RAW",
            "Orbitrap Exploris 480",
            51254,
            16033,
            161,
        ),
    ] {
        let path = std::env::var(variable).unwrap_or_else(|_| panic!("set {variable}"));
        let mut raw = RawFileReader::open_path(path).unwrap();
        assert_eq!(raw.instrument_model, Some(instrument));
        assert_eq!(raw.num_scans, expected_scans);
        let first = raw.run_header.sample_info.first_scan_number;
        let mut positive = 0;
        let mut ms1_zero = 0;
        let mut previous_correction = None;
        for n in first..=raw.run_header.sample_info.last_scan_number {
            let params = raw.scan_params(n).unwrap();
            let matched = params.number_of_matched_lock_masses().unwrap();
            let configured = params.number_of_configured_lock_masses().unwrap();
            let correction = params.lock_mass_correction_ppm().unwrap();
            assert_eq!(
                configured,
                if instrument == "Q Exactive Plus" {
                    9
                } else {
                    1
                }
            );
            assert!(correction.is_finite());
            assert_ne!(correction, 0.0);
            if matched > 0 {
                positive += 1;
                previous_correction = Some(correction);
            } else {
                assert_eq!(Some(correction), previous_correction);
            }
            let meta = scan_metadata(&raw, n - first).unwrap();
            let extra = scan_extras(&raw, &meta, &ExtraFields::All);
            assert_eq!(
                extra["opentfraw.number_of_matched_lock_masses"],
                matched.to_string()
            );
            assert_eq!(
                extra["opentfraw.number_of_configured_lock_masses"],
                configured.to_string()
            );
            assert_eq!(
                extra["opentfraw.number_of_lock_masses"],
                matched.to_string()
            );
            if matched == 0 && meta.ms_level == 1 {
                ms1_zero += 1;
                assert_eq!(
                    raw.scan_parameters(n).unwrap().get_i32("Scan Segment:"),
                    Some(1)
                );
                assert_eq!(
                    raw.scan_parameters(n).unwrap().get_i32("Scan Event:"),
                    Some(1)
                );
            }
        }
        assert_eq!(positive, expected_matches);
        assert_eq!(ms1_zero, expected_ms1_zero);

        // A configured-only record must not leak configuration into the strict
        // matched extra. Exercise the registered trailer and status-log paths,
        // while retaining the established compatibility key.
        for (matched, configured, want_m, want_c) in [
            (None, Some(GenericValue::Int32(9)), None, Some("9")),
            (
                Some(GenericValue::Int32(0)),
                Some(GenericValue::Int32(9)),
                Some("0"),
                Some("9"),
            ),
            (Some(GenericValue::Int32(1)), None, Some("1"), None),
            (None, None, None, None),
        ] {
            let rec = record(matched, configured);
            raw.scan_parameters[0] = GenericRecord {
                values: rec.values.clone(),
            };
            // A single record written at time zero is in effect for scan 0.
            raw.inst_log = vec![StatusLogRecord {
                time: 0.0,
                record: rec,
            }];
            raw.inst_log_time_axis = 0..1;
            raw.status_log_error = None;
            let meta = scan_metadata(&raw, 0).unwrap();
            let extras = scan_extras(&raw, &meta, &ExtraFields::All);
            for prefix in ["opentfraw.", "opentfraw.status."] {
                assert_eq!(
                    extras
                        .get(&format!("{prefix}number_of_matched_lock_masses"))
                        .map(String::as_str),
                    want_m
                );
                assert_eq!(
                    extras
                        .get(&format!("{prefix}number_of_configured_lock_masses"))
                        .map(String::as_str),
                    want_c
                );
                assert_eq!(
                    extras
                        .get(&format!("{prefix}number_of_lock_masses"))
                        .map(String::as_str),
                    want_m.or(want_c)
                );
            }
        }
    }
}
