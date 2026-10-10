//! Instrument status-log decoding on public PRIDE files.
//!
//! `status_log_on_ci_fixture` uses the shared CI fixture (see `common/mod.rs`)
//! and skips when it is absent unless `REQUIRE_CORPUS=1`.
//! `status_log_across_file_versions` needs a local copy of the public corpus
//! (CORPUS.md): set `OPENTFRAW_CORPUS_DIR` to its directory and run with
//! `--ignored`.

mod common;

use opentfraw::RawFileReader;

/// What one file's status log must decode to.
struct Expected {
    file: &'static str,
    version: u32,
    fields: usize,
    records: usize,
    /// Records on the acquisition's time axis.
    time_axis: std::ops::Range<usize>,
    /// Scans with a status-log record at or before their start time.
    scans_with_status: usize,
    /// A Float32/Float64 label present in every record, and the physical
    /// range all its values must fall in.
    label: &'static str,
    range: (f64, f64),
}

fn check(raw: &RawFileReader, want: &Expected) {
    let name = want.file;
    assert_eq!(raw.status_log_error(), None, "{name}");
    assert_eq!(raw.version, want.version, "{name}");
    assert_eq!(raw.inst_log_header.fields.len(), want.fields, "{name}");
    assert_eq!(raw.inst_log.len(), want.records, "{name}");
    assert_eq!(
        raw.inst_log.len(),
        raw.run_header.sample_info.inst_log_length as usize,
        "{name}"
    );
    assert_eq!(raw.inst_log_time_axis, want.time_axis, "{name}");

    let axis = &raw.inst_log[want.time_axis.clone()];
    assert!(
        axis.windows(2).all(|w| w[0].time <= w[1].time),
        "{name}: time axis is not monotonic"
    );
    // The time axis spans the acquisition: it starts no later than one
    // minute after the first scan and ends no earlier than one minute
    // before the last.
    let first_rt = raw.scan_index.first().unwrap().start_time;
    let last_rt = raw.scan_index.last().unwrap().start_time;
    assert!(f64::from(axis[0].time) <= first_rt + 1.0, "{name}");
    assert!(
        f64::from(axis.last().unwrap().time) >= last_rt - 1.0,
        "{name}"
    );

    for rec in &raw.inst_log {
        let v = rec
            .record
            .get_f64(want.label)
            .unwrap_or_else(|| panic!("{name}: no {:?} at {}", want.label, rec.time));
        assert!(
            (want.range.0..=want.range.1).contains(&v),
            "{name}: {:?} = {v} at {}",
            want.label,
            rec.time
        );
    }

    let first = raw.run_header.sample_info.first_scan_number;
    let last = raw.run_header.sample_info.last_scan_number;
    let mut with_status = 0;
    for scan in first..=last {
        if let Some(rec) = raw.status_log_record(scan) {
            with_status += 1;
            let rt = raw.scan_index[(scan - first) as usize].start_time as f32;
            assert!(rec.time <= rt, "{name}: scan {scan} sees a later record");
            assert!(raw.status_log_entry(scan).is_some());
        }
    }
    assert_eq!(with_status, want.scans_with_status, "{name}");
}

#[test]
fn status_log_on_ci_fixture() {
    let Some(path) = common::fixture() else {
        eprintln!("skipping: no Thermo fixture available");
        return;
    };
    let raw = RawFileReader::open_path(&path).expect("open raw");
    check(
        &raw,
        &Expected {
            file: common::CORPUS_FIXTURE_NAME,
            version: 64,
            fields: 136,
            records: 571,
            time_axis: 0..571,
            scans_with_status: 1052,
            label: "Capillary Temp (C):",
            range: (270.0, 280.0),
        },
    );
}

#[test]
#[ignore = "requires the public corpus; set OPENTFRAW_CORPUS_DIR"]
fn status_log_across_file_versions() {
    let dir = std::env::var("OPENTFRAW_CORPUS_DIR").expect("set OPENTFRAW_CORPUS_DIR");
    let cases = [
        // v63 TSQ: ends with one record at time zero, off the time axis.
        Expected {
            file: "PXD020246_TSQVantage.RAW",
            version: 63,
            fields: 63,
            records: 2289,
            time_axis: 0..2288,
            scans_with_status: 35074,
            label: "Capillary Temperature",
            range: (260.0, 280.0),
        },
        // v63 ion trap / Orbitrap hybrid.
        Expected {
            file: "PXD069348_LTQOrbitrap.RAW",
            version: 63,
            fields: 158,
            records: 4806,
            time_axis: 0..4806,
            scans_with_status: 5269,
            label: "FT Analyzer Temp. (\u{b0}C):",
            range: (20.0, 35.0),
        },
        // v66 Q-Orbitrap: Float64 values.
        Expected {
            file: "PXD068962_Q_Exactive_UHMR_insource-CID.raw",
            version: 66,
            fields: 54,
            records: 305,
            time_axis: 0..305,
            scans_with_status: 3047,
            label: "High Vacuum Sensor (mbar)",
            range: (1e-10, 1e-8),
        },
        // v66 triple quadrupole, labels with a `{unit}` suffix.
        Expected {
            file: "PXD069101_TSQ_Altis_milla0255_01.raw",
            version: 66,
            fields: 133,
            records: 1109,
            time_axis: 0..1109,
            scans_with_status: 3606,
            label: "Ion Transfer Tube Temp {\u{b0}C}",
            range: (300.0, 350.0),
        },
        // v66 Tribrid: a block on an earlier clock precedes the acquisition.
        Expected {
            file: "PXD037285_Orbitrap_Fusion_Lumos_01819_A02_P019006_S00_U03_R1_TMT10.raw",
            version: 66,
            fields: 219,
            records: 3087,
            time_axis: 386..3087,
            scans_with_status: 38130,
            label: "Analyzer Temp",
            range: (20.0, 35.0),
        },
    ];
    for want in &cases {
        let path = std::path::Path::new(&dir).join(want.file);
        let raw =
            RawFileReader::open_path(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        check(&raw, want);
    }
}
