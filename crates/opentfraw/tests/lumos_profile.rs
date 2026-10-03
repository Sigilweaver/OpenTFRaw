use opentfraw::RawFileReader;
use std::{fs::File, io::BufReader};

// Original RAW from the deliberately published PRIDE study PXD031322.
// The data is downloaded separately and is not redistributed in this repo.
#[test]
#[ignore = "requires public PXD031322 OFL001513-YLL-GPF-15K-1.raw fixture"]
fn public_lumos_ms1_profile_calibration_matches_scan_trailers() {
    let path = std::env::var("OPENTFRAW_LUMOS_PROFILE_RAW").expect("set public fixture path");
    let raw = RawFileReader::open_path(&path).unwrap();
    assert_eq!(raw.num_scans, 152328);
    let mut source = BufReader::new(File::open(&path).unwrap());
    let mut primary_count = 0;
    for (index, event) in raw.scan_events.iter().enumerate() {
        if event.preamble.ms_power() != Some(opentfraw::MsPower::Ms1) {
            continue;
        }
        let scan = raw.run_header.sample_info.first_scan_number + index as u32;
        let params = raw.scan_params(scan).unwrap();
        assert_eq!(event.coefficients.len(), 7, "scan {scan}");
        assert_eq!(Some(event.coefficients[2]), params.conversion_parameter_a());
        assert_eq!(Some(event.coefficients[3]), params.conversion_parameter_b());
        assert_eq!(Some(event.coefficients[4]), params.conversion_parameter_c());
        let profile = raw.read_scan(&mut source, scan).unwrap().profile.unwrap();
        let points = profile.to_mz_intensity(&event.coefficients);
        assert!(!points.is_empty(), "scan {scan}");
        for (mz, intensity) in points {
            assert!(
                mz.is_finite() && (349.0..=1001.0).contains(&mz),
                "scan {scan}: {mz}"
            );
            assert!(intensity.is_finite() && intensity >= 0.0);
        }
        primary_count += 1;
    }
    assert_eq!(primary_count, 2308);
}
