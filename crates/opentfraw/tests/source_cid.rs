use opentfraw::generic_data::{GenericRecord, GenericValue};
use opentfraw::{extra::scan_extras, scan_metadata, ExtraFields, RawFileReader};

// Public GlycoPOST GPST000122.0 NGlycans_Serum_Fetuin_File1.raw.
// This is a negative-polarity Q Exactive Plus acquisition with two explicitly
// linked method events. Its source-CID energy is absent from every trailer.
#[test]
#[ignore = "requires public GPST000122 Q Exactive Plus source-CID fixture"]
fn public_negative_plus_source_cid_method_fallback() {
    let path = std::env::var("OPENTFRAW_SOURCE_CID_PLUS_RAW").expect("set fixture path");
    let raw = RawFileReader::open_path(&path).unwrap();
    assert_eq!(raw.instrument_model, Some("Q Exactive Plus"));
    assert_eq!(raw.num_scans, 6674);
    let mut source = std::fs::File::open(path).unwrap();
    let method = raw.instrument_method_text(&mut source).unwrap();
    assert_eq!(
        method
            .matches("<Fragmentation_Source unit=\"eV\" FullInternalName=\"decimal?\">70</Fragmentation_Source>")
            .count(),
        2
    );
    let first = raw.run_header.sample_info.first_scan_number;
    let mut ms1 = 0;
    let mut ms2 = 0;
    for n in first..=raw.run_header.sample_info.last_scan_number {
        assert_eq!(raw.scan_params(n).unwrap().source_cid_energy_ev(), None);
        assert_eq!(raw.source_cid_energy_ev(n), Some(70.0));
        assert_eq!(raw.source_cid_energy_source(n), Some("instrument_method"));
        let filter = raw.scan_filter(n).unwrap();
        assert!(filter.starts_with("FTMS - p NSI sid=70.00 "));
        let meta = scan_metadata(&raw, n - first).unwrap();
        match meta.ms_level {
            1 => ms1 += 1,
            2 => ms2 += 1,
            level => panic!("unexpected MS level {level}"),
        }
        let extra = scan_extras(&raw, &meta, &ExtraFields::All);
        assert_eq!(extra["opentfraw.source_cid_energy_ev"], "70");
        assert_eq!(
            extra["opentfraw.source_cid_energy_source"],
            "instrument_method"
        );
    }
    assert_eq!((ms1, ms2), (419, 6255));
}

// Public PRIDE PXD068962 insource-CID.raw, already used by OpenMassSpec CI.
// Run explicitly with OPENTFRAW_SOURCE_CID_RAW set to this public acquisition.
#[test]
#[ignore = "requires public PXD068962 insource-CID.raw fixture"]
fn public_source_cid_method_and_trailer_precedence() {
    let path = std::env::var("OPENTFRAW_SOURCE_CID_RAW").expect("set fixture path");
    let mut raw = RawFileReader::open_path(path).unwrap();
    assert_eq!(raw.num_scans, 3047);
    let first = raw.run_header.sample_info.first_scan_number;
    for n in first..=raw.run_header.sample_info.last_scan_number {
        assert_eq!(raw.scan_params(n).unwrap().source_cid_energy_ev(), None);
        assert_eq!(raw.source_cid_energy_ev(n), Some(200.0));
        assert_eq!(raw.source_cid_energy_source(n), Some("instrument_method"));
        assert!(raw
            .scan_filter(n)
            .unwrap()
            .contains(" NSI sid=200.00 Full "));
    }
    let meta = scan_metadata(&raw, first).unwrap();
    let extra = scan_extras(&raw, &meta, &ExtraFields::All);
    assert_eq!(extra["opentfraw.source_cid_energy_ev"], "200");
    assert_eq!(
        extra["opentfraw.source_cid_energy_source"],
        "instrument_method"
    );
    assert_eq!(raw.source_cid_energy_ev(first - 1), None);
    let original = raw.scan_parameters[0].values.clone();
    for (value, expected) in [
        (GenericValue::Float64(35.0), Some(35.0)),
        (GenericValue::Float32(0.0), Some(0.0)),
        (GenericValue::Float64(f64::NAN), None),
        (GenericValue::String("20".into()), None),
    ] {
        raw.scan_parameters[0].values = original.clone();
        raw.scan_parameters[0]
            .values
            .push(("Source CID eV:".into(), value));
        assert_eq!(raw.source_cid_energy_ev(first), expected);
        assert_eq!(
            raw.source_cid_energy_source(first),
            expected.map(|_| "trailer")
        );
    }
    for ids in [
        vec![],
        vec![("Scan Segment:", GenericValue::Int32(1))],
        vec![
            ("Scan Segment:", GenericValue::Int32(1)),
            ("Scan Event:", GenericValue::String("1".into())),
        ],
        vec![
            ("Scan Segment:", GenericValue::Int32(1)),
            ("Scan Event:", GenericValue::Int32(2)),
        ],
    ] {
        raw.scan_parameters[0] = GenericRecord {
            values: ids.into_iter().map(|(k, v)| (k.to_owned(), v)).collect(),
        };
        assert_eq!(raw.source_cid_energy_ev(first), None);
    }
}
