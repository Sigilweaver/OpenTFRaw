//! Collision energy unit and source, and the scan mode of the exported
//! arrays, on the public CI fixtures (see `common/mod.rs`). Tests that need
//! the wider public corpus are ignored; run them with
//! `OPENTFRAW_CORPUS_DIR` set and `--include-ignored`.

mod common;

use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};

use openmassspec_core::{write_mzml, CollisionEnergyUnit as CoreUnit, SpectrumSource};
use opentfraw::mzml::{scan_metadata, OpenTfRawSource};
use opentfraw::{CollisionEnergySource, CollisionEnergyUnit, RawFileReader};

fn flatv66_fixture() -> Option<PathBuf> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../corpus");
    let path = root.join("PXD041995_TSQ_Altis_milla010603.raw");
    if path.exists() {
        return Some(path);
    }
    assert!(
        std::env::var("REQUIRE_CORPUS").as_deref() != Ok("1"),
        "REQUIRE_CORPUS=1 but PXD041995 milla010603.raw is missing"
    );
    None
}

fn mzml_of(raw: &RawFileReader, path: &Path) -> String {
    let mut source = BufReader::new(File::open(path).unwrap());
    let mut src = OpenTfRawSource::new(raw, &mut source, "fixture.raw", false);
    let mut out = Vec::new();
    write_mzml(&mut src, &mut out).expect("strict write of the fixture");
    String::from_utf8(out).unwrap()
}

/// LTQ FT Ultra: the scan event stores 35 and the embedded method says
/// `Normalized Coll. Energy: 35.0`, so every MS2 scan is 35 NCE.
#[test]
fn ion_trap_scan_event_energy_is_nce_by_the_method() {
    let Some(path) = common::fixture() else {
        eprintln!("skipping: no Thermo fixture available");
        return;
    };
    let raw = RawFileReader::open_path(&path).unwrap();
    assert_eq!(raw.scan_event_energy_unit, Some(CollisionEnergyUnit::Nce));
    let mut ms2 = 0;
    for idx in 0..raw.num_scans {
        let meta = scan_metadata(&raw, idx).unwrap();
        let Some(p) = meta.precursor else { continue };
        ms2 += 1;
        assert_eq!(p.collision_energy, Some(35.0));
        assert_eq!(p.collision_energy_unit, Some(CollisionEnergyUnit::Nce));
        assert_eq!(
            p.collision_energy_source,
            Some(CollisionEnergySource::ScanEvent)
        );
        assert!(p.collision_energy_steps.is_empty());
    }
    assert!(ms2 > 0);

    let mut source = BufReader::new(File::open(&path).unwrap());
    let mut src = OpenTfRawSource::new(&raw, &mut source, "fixture.raw", false);
    let record = src
        .iter_spectra()
        .map(|r| r.unwrap())
        .find(|r| r.precursor.is_some())
        .unwrap();
    let energies = &record.precursor.as_ref().unwrap().collision_energies;
    assert_eq!(energies.len(), 1);
    assert_eq!(energies[0].value, 35.0);
    assert_eq!(energies[0].unit, Some(CoreUnit::Nce));

    // The writer states the unit: MS:1000138 normalized collision energy.
    let mzml = mzml_of(&raw, &path);
    assert!(mzml.contains("MS:1000138"));
    assert!(!mzml.contains("MS:1000045"));
}

/// The default export holds the centroid peak list, also for scans whose
/// event declares profile mode, so the exported scan mode is centroid.
#[test]
fn default_export_of_a_profile_scan_is_centroid() {
    let Some(path) = common::fixture() else {
        eprintln!("skipping: no Thermo fixture available");
        return;
    };
    let raw = RawFileReader::open_path(&path).unwrap();
    let mut source = BufReader::new(File::open(&path).unwrap());
    let mut declared_profile = 0;
    {
        let mut src = OpenTfRawSource::new(&raw, &mut source, "fixture.raw", false);
        for (idx, record) in src.iter_spectra().enumerate() {
            let record = record.unwrap();
            assert_eq!(
                record.scan_mode,
                Some(openmassspec_core::ScanMode::Centroid)
            );
            let meta = scan_metadata(&raw, idx as u32).unwrap();
            if meta.scan_mode == Some(opentfraw::ScanMode::Profile) {
                declared_profile += 1;
            }
        }
    }
    assert!(declared_profile > 0, "fixture has profile-mode scans");
    let mut src = OpenTfRawSource::new(&raw, &mut source, "fixture.raw", true);
    let profile = src
        .iter_spectra()
        .filter_map(|r| r.ok())
        .filter(|r| r.scan_mode == Some(openmassspec_core::ScanMode::Profile))
        .count();
    assert_eq!(profile, declared_profile);
}

/// TSQ Altis SRM: the transition table's collision energy is eV.
#[test]
fn srm_transition_energy_is_ev() {
    let Some(path) = flatv66_fixture() else {
        eprintln!("skipping: no FlatV66 fixture available");
        return;
    };
    let raw = RawFileReader::open_path(&path).unwrap();
    let mut with_energy = 0;
    for idx in (0..raw.num_scans).step_by(97) {
        let meta = scan_metadata(&raw, idx).unwrap();
        let p = meta.precursor.expect("SRM scans carry a precursor");
        if p.collision_energy.is_some() {
            with_energy += 1;
            assert_eq!(p.collision_energy_unit, Some(CollisionEnergyUnit::Ev));
            assert_eq!(
                p.collision_energy_source,
                Some(CollisionEnergySource::SrmTransition)
            );
        }
    }
    assert!(with_energy > 0);
}

fn corpus(file: &str) -> PathBuf {
    let dir = std::env::var("OPENTFRAW_CORPUS_DIR").expect("set OPENTFRAW_CORPUS_DIR");
    Path::new(&dir).join(file)
}

/// Q Exactive HF scans with a stepped NCE label (`"22.50,25.00,27.50"`,
/// PXD005573) carry every step and their mean; scans with a single NCE label
/// carry that value.
#[test]
#[ignore = "requires the public PXD005573 Fig2HeLa file"]
fn stepped_nce_label_carries_every_step() {
    let path = corpus("PXD005573_Q_Exactive_HF_Fig2HeLa-0-5h_MHRM_R01_T0.raw");
    let raw = RawFileReader::open_path(&path).unwrap();
    let mut stepped = 0;
    for idx in 0..raw.num_scans {
        let Some(p) = scan_metadata(&raw, idx).unwrap().precursor else {
            continue;
        };
        if p.collision_energy_steps.is_empty() {
            continue;
        }
        stepped += 1;
        assert_eq!(p.collision_energy_steps, vec![22.5, 25.0, 27.5]);
        assert_eq!(p.collision_energy, Some(25.0));
        assert_eq!(p.collision_energy_unit, Some(CollisionEnergyUnit::Nce));
        assert_eq!(
            p.collision_energy_source,
            Some(CollisionEnergySource::TrailerNce)
        );
    }
    assert!(stepped > 0);
}

/// Q Exactive Plus: a numeric `HCD Energy eV:` label and no NCE label.
#[test]
#[ignore = "requires the public PXD006767 Q Exactive Plus HCD file"]
fn ev_label_is_ev() {
    let path = corpus("PXD006767_LTQ_Orbitrap_EFM4_AspN_HCD_16032016.raw");
    let raw = RawFileReader::open_path(&path).unwrap();
    let mut n = 0;
    for idx in 0..raw.num_scans {
        let Some(p) = scan_metadata(&raw, idx).unwrap().precursor else {
            continue;
        };
        n += 1;
        assert_eq!(p.collision_energy_unit, Some(CollisionEnergyUnit::Ev));
        assert_eq!(
            p.collision_energy_source,
            Some(CollisionEnergySource::TrailerEv)
        );
    }
    assert!(n > 0);
}

/// PXD068962 in-source CID: the source energy comes with the unit in its
/// key and the method as its source; the MS2 collision energy has its own
/// unit.
#[test]
#[ignore = "requires the public PXD068962 insource-CID.raw file"]
fn source_cid_file_carries_both_energies_with_units() {
    use opentfraw::{extra::scan_extras, ExtraFields};
    let path = corpus("PXD068962_Q_Exactive_UHMR_insource-CID.raw");
    let raw = RawFileReader::open_path(&path).unwrap();
    let mut with_ce = 0;
    for idx in 0..raw.num_scans {
        let meta = scan_metadata(&raw, idx).unwrap();
        let extra = scan_extras(&raw, &meta, &ExtraFields::All);
        assert_eq!(extra["opentfraw.source_cid_energy_ev"], "200");
        if let Some(p) = &meta.precursor {
            if p.collision_energy.is_some() {
                with_ce += 1;
                assert!(p.collision_energy_unit.is_some());
                assert!(extra.contains_key("opentfraw.collision_energy_source"));
            }
        }
    }
    eprintln!("{with_ce} scans with a collision energy");
}
