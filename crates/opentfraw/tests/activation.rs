//! Activation decoding on public PRIDE files (see the format docs, section
//! 22.5). Run with `OPENTFRAW_CORPUS_DIR` pointing at the corpus and
//! `--include-ignored`.

use opentfraw::{mzml::scan_metadata, Activation, RawFileReader};

struct Expected {
    file: &'static str,
    ms2: usize,
    activation: Activation,
    filter_clause: &'static str,
    collision_energy: bool,
}

fn check(dir: &str, want: &Expected) {
    let path = std::path::Path::new(dir).join(want.file);
    let raw = RawFileReader::open_path(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let mut ms2 = 0;
    for idx in 0..raw.num_scans {
        let meta = scan_metadata(&raw, idx).expect("scan metadata");
        if meta.ms_level == 1 {
            assert!(
                meta.precursor.is_none(),
                "{}: MS1 with a precursor",
                want.file
            );
            continue;
        }
        assert_eq!(meta.ms_level, 2, "{}: scan {}", want.file, meta.scan_number);
        ms2 += 1;
        let p = meta.precursor.as_ref().expect("MS2 precursor");
        assert_eq!(
            p.activation,
            Some(want.activation),
            "{}: scan {}",
            want.file,
            meta.scan_number
        );
        assert_eq!(
            p.collision_energy.is_some(),
            want.collision_energy,
            "{}: scan {}",
            want.file,
            meta.scan_number
        );
        let filter = meta.filter.as_deref().unwrap_or("");
        assert!(
            filter.contains(want.filter_clause),
            "{}: {filter}",
            want.file
        );
    }
    assert_eq!(ms2, want.ms2, "{}", want.file);
}

#[test]
#[ignore = "requires the public corpus; set OPENTFRAW_CORPUS_DIR"]
fn activation_on_public_files() {
    let dir = std::env::var("OPENTFRAW_CORPUS_DIR").expect("set OPENTFRAW_CORPUS_DIR");
    let cases = [
        // Orbitrap Velos Pro ETD: method "Activation Type: ETD", "Activation
        // Time: 130.000"; the reaction stores 130, which is not a collision
        // energy.
        Expected {
            file: "PXD006767_LTQ_Orbitrap_EFM4_AspN_ETD_22032016.raw",
            ms2: 3415,
            activation: Activation::ETD,
            filter_clause: "@etd130.00 ",
            collision_energy: false,
        },
        // Orbitrap Eclipse EThcD: method "Is EThcD Active = True". The
        // reaction value is charge-scaled (95.834 at z=2, 42.593 at z=3) and
        // the "HCD Energy:" parameter is blank, so no collision energy.
        Expected {
            file: "PXD077396_Orbitrap_Eclipse_EC-OT30-25-190-EThcD-01.raw",
            ms2: 11340,
            activation: Activation::EThcD,
            filter_clause: "@etd@hcd ",
            collision_energy: false,
        },
        // Orbitrap Velos Pro CID: method "Activation Type: CID".
        Expected {
            file: "PXD051336_LTQ_Orbitrap_Velos_Pro_171214_S_SPMM_03_01_45pto.raw",
            ms2: 7872,
            activation: Activation::CID,
            filter_clause: "@cid35.00 ",
            collision_energy: true,
        },
    ];
    for want in &cases {
        check(&dir, want);
    }
}
