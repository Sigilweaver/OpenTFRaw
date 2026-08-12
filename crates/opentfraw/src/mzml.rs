/// mzML export for Thermo RAW files.
///
/// Writes a valid mzML 1.1.0 document to any `Write` sink. Produces one
/// `<spectrum>` element per scan. Binary arrays (m/z and intensity) are
/// stored as little-endian raw bytes encoded with standard Base64 - no
/// additional compression is applied, keeping this module dependency-free.
///
/// # Usage
/// ```no_run
/// use opentfraw::{RawFileReader, mzml::write_mzml};
/// let raw = RawFileReader::open_path("run.raw").unwrap();
/// let mut out = std::fs::File::create("run.mzML").unwrap();
/// let mut src = std::fs::File::open("run.raw").unwrap();
/// write_mzml(&raw, &mut src, &mut out, "run.raw", false).unwrap();
/// ```
use std::io::{Read, Seek, Write};

use crate::error::Result;
use crate::scan_event::ScanEvent;
use crate::types::{Activation, MsPower, Polarity};
use crate::RawFileReader;

// -- Structured spectrum record (vendor-neutral, no mzML) --

/// Precursor metadata for an MS2+ spectrum.
///
/// Built from the per-scan parameter table and/or the scan-event reaction
/// list. `target_mz` is the isolation-window center; `selected_mz` is the
/// monoisotopic-resolved precursor (when available). `collision_energy` is
/// either an absolute eV value or, when `ce_is_nce == true`, a normalized
/// collision energy.
#[derive(Debug, Clone, Default)]
pub struct PrecursorInfo {
    pub target_mz: Option<f64>,
    pub selected_mz: Option<f64>,
    pub isolation_width: Option<f64>,
    pub charge: Option<i32>,
    pub collision_energy: Option<f64>,
    pub ce_is_nce: bool,
    pub master_scan_number: Option<u32>,
    pub activation: Option<Activation>,
    /// Analyzer used for the precursor scan; needed by mzML CV mapping to
    /// disambiguate CID vs beam-type CID on FTMS instruments.
    pub analyzer: Option<crate::Analyzer>,
}

/// One fully-decoded spectrum with all metadata needed to emit mzML or
/// populate an in-memory record set.
///
/// Returned by [`extract_spectrum`] / [`iter_spectra`]. The mzML writer in
/// this crate is implemented on top of these records; downstream crates that
/// want to ingest Thermo data into their own column store should use
/// `extract_spectrum` directly to avoid the XML-then-parse round trip.
#[derive(Debug, Clone)]
pub struct SpectrumRecord {
    pub index: usize,
    pub scan_number: u32,
    pub ms_level: u32,
    pub is_ms1: bool,
    /// Whether this MS2+ scan uses data-independent acquisition.
    pub is_dia: bool,
    /// Whether broadband isolation is enabled for this scan.
    pub is_wideband: bool,
    pub polarity: Option<Polarity>,
    /// Effective scan mode after `include_profile` resolution.
    pub scan_mode: Option<crate::ScanMode>,
    pub filter: Option<String>,
    /// Retention time in minutes.
    pub retention_time_min: f64,
    pub total_ion_current: f64,
    pub base_peak_mz: f64,
    pub base_peak_intensity: f64,
    pub low_mz: f64,
    pub high_mz: f64,
    pub ion_injection_time_ms: Option<f64>,
    pub faims_cv: Option<f64>,
    pub precursor: Option<PrecursorInfo>,
    pub mz: Vec<f64>,
    pub intensity: Vec<f32>,
}

/// Extract a single spectrum's record from `raw` at scan-index `idx`
/// (zero-based offset from the first scan).
///
/// Returns `None` if the scan's peak arrays cannot be read (matches the
/// silent-skip behaviour of [`write_mzml`]). `include_profile` controls
/// whether profile-mode scans return the raw profile signal or the
/// centroided peak list, matching [`write_mzml`].
pub fn extract_spectrum<R: Read + Seek>(
    raw: &RawFileReader,
    source: &mut R,
    idx: u32,
    include_profile: bool,
) -> Option<SpectrumRecord> {
    if idx >= raw.num_scans {
        return None;
    }
    let first_scan = raw.run_header.sample_info.first_scan_number;
    let scan_number = first_scan + idx;
    let entry = &raw.scan_index[idx as usize];
    let event = raw.scan_events.get(idx as usize);
    let params = raw.scan_params(scan_number);

    let is_srm = raw.flat_peaks;
    let level = if is_srm {
        2
    } else {
        event
            .and_then(|e| e.preamble.ms_power())
            .map(ms_level)
            .unwrap_or(1)
    };
    let polarity = if is_srm {
        Some(Polarity::Positive)
    } else {
        event.and_then(|e| e.preamble.polarity())
    };
    let scan_mode = if is_srm {
        Some(crate::ScanMode::Centroid)
    } else {
        event.and_then(|e| e.preamble.scan_mode())
    };
    let filter = raw.scan_filter(scan_number);
    let is_ms1 = !is_srm && level == 1;
    let is_dia = event.is_some_and(|e| e.preamble.is_dia());
    let is_wideband = event.is_some_and(|e| e.preamble.is_wideband());
    let srm_q1 = if is_srm {
        raw.srm_q1_by_event.get(&entry.scan_event).copied()
    } else {
        None
    };
    let srm_ce = if is_srm {
        raw.srm_ce_by_event.get(&entry.scan_event).copied()
    } else {
        None
    };

    let (mz, intensity, effective_scan_mode) =
        resolve_scan_arrays(raw, source, scan_number, include_profile, event, scan_mode)?;

    let precursor = if !is_ms1 {
        let info = if let Some(q1) = srm_q1 {
            PrecursorInfo {
                target_mz: Some(q1),
                selected_mz: Some(q1),
                isolation_width: Some(0.7),
                charge: None,
                collision_energy: srm_ce,
                ce_is_nce: false,
                master_scan_number: None,
                activation: event.and_then(|e| e.preamble.activation()),
                analyzer: event.and_then(|e| e.preamble.analyzer()),
            }
        } else {
            let reaction = event.and_then(|e| e.reactions.first());
            let tm = params
                .as_ref()
                .and_then(|p| p.isolation_target_mz())
                .filter(|&mz| mz > 0.0)
                .or_else(|| {
                    params
                        .as_ref()
                        .and_then(|p| p.monoisotopic_mz())
                        .filter(|&mz| mz > 0.0)
                })
                .or_else(|| reaction.map(|r| r.precursor_mz).filter(|&mz| mz > 0.0));
            let sm = params
                .as_ref()
                .and_then(|p| p.monoisotopic_mz())
                .filter(|&mz| mz > 0.0)
                .or(tm);
            let iw = params.as_ref().and_then(|p| p.isolation_width_mz());
            let ch = params
                .as_ref()
                .and_then(|p| p.charge_state())
                .filter(|&z| z > 0);
            let ae_from_params = params
                .as_ref()
                .and_then(|p| p.activation_energy())
                .filter(|&e| e > 0.0);
            let ae_is_nce = ae_from_params.is_some()
                && params
                    .as_ref()
                    .map(|p| p.activation_energy_is_nce())
                    .unwrap_or(false);
            let ae = ae_from_params.or_else(|| reaction.map(|r| r.energy).filter(|&e| e > 0.0));
            let master = params
                .as_ref()
                .and_then(|p| p.master_scan_number())
                .filter(|&n| n > 0)
                .map(|n| n as u32);
            PrecursorInfo {
                target_mz: tm,
                selected_mz: sm,
                isolation_width: iw,
                charge: ch,
                collision_energy: ae,
                ce_is_nce: ae_is_nce,
                master_scan_number: master,
                activation: event.and_then(|e| e.preamble.activation()),
                analyzer: event.and_then(|e| e.preamble.analyzer()),
            }
        };
        Some(info)
    } else {
        None
    };

    let ion_injection_time_ms = params.as_ref().and_then(|p| p.ion_injection_time_ms());
    let faims_cv = params.as_ref().and_then(|p| p.faims_cv());

    Some(SpectrumRecord {
        index: idx as usize,
        scan_number,
        ms_level: level,
        is_ms1,
        is_dia,
        is_wideband,
        polarity,
        scan_mode: effective_scan_mode,
        filter,
        retention_time_min: entry.start_time,
        total_ion_current: entry.total_current,
        base_peak_mz: entry.base_mz,
        base_peak_intensity: entry.base_intensity,
        low_mz: entry.low_mz,
        high_mz: entry.high_mz,
        ion_injection_time_ms,
        faims_cv,
        precursor,
        mz,
        intensity,
    })
}

/// Iterate every scan in `raw` as a [`SpectrumRecord`].
///
/// Skipped scans (those for which the peak arrays cannot be decoded) are
/// dropped silently, matching [`write_mzml`]. The returned iterator borrows
/// both `raw` and `source` for its lifetime.
pub fn iter_spectra<'a, R: Read + Seek>(
    raw: &'a RawFileReader,
    source: &'a mut R,
    include_profile: bool,
) -> impl Iterator<Item = SpectrumRecord> + 'a {
    let n = raw.num_scans;
    let mut idx: u32 = 0;
    std::iter::from_fn(move || {
        while idx < n {
            let cur = idx;
            idx += 1;
            if let Some(rec) = extract_spectrum(raw, source, cur, include_profile) {
                return Some(rec);
            }
        }
        None
    })
}
// -- Helpers used by extract_spectrum --

fn ms_level(power: MsPower) -> u32 {
    match power {
        MsPower::Undefined => 1,
        MsPower::Ms1 => 1,
        MsPower::Ms2 => 2,
        MsPower::Ms3 => 3,
        MsPower::Ms4 => 4,
        MsPower::Ms5 => 5,
        MsPower::Ms6 => 6,
        MsPower::Ms7 => 7,
        MsPower::Ms8 => 8,
    }
}

/// Resolve the m/z and intensity arrays for a single scan.
///
/// When `include_profile=true` AND the scan packet contains profile data, the
/// profile signal is decoded and returned as the primary arrays (with
/// `effective_scan_mode = Some(ScanMode::Profile)`). Otherwise the centroid
/// peak list is used.
///
/// Returns `None` when the scan cannot be read (caller should skip it).
fn resolve_scan_arrays<R: Read + Seek>(
    raw: &RawFileReader,
    source: &mut R,
    scan_number: u32,
    include_profile: bool,
    event: Option<&ScanEvent>,
    nominal_scan_mode: Option<crate::ScanMode>,
) -> Option<(Vec<f64>, Vec<f32>, Option<crate::ScanMode>)> {
    if include_profile && !raw.flat_peaks {
        let packet = raw.read_scan(source, scan_number).ok()?;
        if let Some(profile) = packet.profile {
            let coeffs = event.map(|e| e.coefficients.as_slice()).unwrap_or(&[]);
            let pairs = profile.to_mz_intensity(coeffs);
            let mz: Vec<f64> = pairs
                .iter()
                .filter(|(m, _)| *m > 0.0)
                .map(|(m, _)| *m)
                .collect();
            let int: Vec<f32> = pairs
                .iter()
                .filter(|(m, _)| *m > 0.0)
                .map(|(_, i)| *i as f32)
                .collect();
            return Some((mz, int, Some(crate::ScanMode::Profile)));
        }
        let mz: Vec<f64> = packet.peaks.iter().map(|p| p.mz).collect();
        let int: Vec<f32> = packet.peaks.iter().map(|p| p.abundance).collect();
        return Some((mz, int, nominal_scan_mode));
    }
    let peaks = raw.read_peaks_only(source, scan_number).ok()?;
    let mz: Vec<f64> = peaks.iter().map(|p| p.mz).collect();
    let int: Vec<f32> = peaks.iter().map(|p| p.abundance).collect();
    Some((mz, int, nominal_scan_mode))
}

// -- Adapter / canonical writer wrappers --
//
// The mzML emission machinery itself lives in `openmassspec_core`. Here we
// define a `SpectrumSource` adapter that pulls Thermo scans through
// `extract_spectrum` and converts each opentfraw `SpectrumRecord` into the
// vendor-neutral `openmassspec_core::SpectrumRecord` the canonical writer
// consumes. This keeps opentfraw's public mzML API stable and byte-identical
// to the pre-migration output, while sharing the writer with the other
// vendors.

use openmassspec_core as msc;

const SOFTWARE_NAME: &str = "opentfraw";
// Pinned for byte-identical output across crate version bumps. The mzML
// `<software version=...>` is informational; downstream tools do not key off
// it. If you change this, also update the conformance baseline.
const SOFTWARE_VERSION: &str = "0.1.0";

/// PSI-MS CV term for the source file format (Thermo RAW).
fn source_file_format_cv() -> msc::CvTerm {
    msc::CvTerm::new("MS:1000563", "Thermo RAW format")
}

/// PSI-MS CV term for the native ID format used by Thermo.
fn native_id_format_cv() -> msc::CvTerm {
    msc::CvTerm::new("MS:1000768", "Thermo nativeID format")
}

/// Resolve the instrument CV term for `raw` (lookup mirrors the historical
/// in-crate writer; expands as new Thermo models appear in the corpus).
fn instrument_cv(raw: &RawFileReader) -> msc::CvTerm {
    if let Some(model) = raw.instrument_model {
        let known: &[(&str, &str, &str)] = &[
            ("Orbitrap Astral", "MS:1003355", "Orbitrap Astral"),
            ("Orbitrap Ascend", "MS:1003028", "Orbitrap Ascend"),
            ("Orbitrap Eclipse", "MS:1003029", "Orbitrap Eclipse"),
            (
                "Orbitrap Fusion Lumos",
                "MS:1002732",
                "Orbitrap Fusion Lumos",
            ),
            ("Orbitrap Fusion", "MS:1002416", "Orbitrap Fusion"),
            (
                "Orbitrap Exploris 480",
                "MS:1003028",
                "Orbitrap Exploris 480",
            ),
            (
                "Orbitrap Exploris 240",
                "MS:1003098",
                "Orbitrap Exploris 240",
            ),
            (
                "Orbitrap Exploris 120",
                "MS:1003199",
                "Orbitrap Exploris 120",
            ),
            ("Q Exactive HF-X", "MS:1002877", "Q Exactive HF-X"),
            ("Q Exactive HF", "MS:1002523", "Q Exactive HF"),
            ("Q Exactive Plus", "MS:1002634", "Q Exactive Plus"),
            ("Q Exactive UHMR", "MS:1003245", "Q Exactive UHMR"),
            ("Q Exactive", "MS:1001911", "Q Exactive"),
            ("LTQ Orbitrap Velos Pro", "MS:1001742", "LTQ Orbitrap Velos"),
            ("LTQ Orbitrap Velos", "MS:1001742", "LTQ Orbitrap Velos"),
            ("LTQ Orbitrap Elite", "MS:1001910", "LTQ Orbitrap Elite"),
            ("LTQ Orbitrap XL", "MS:1000556", "LTQ Orbitrap XL"),
            ("LTQ Orbitrap", "MS:1000449", "LTQ Orbitrap"),
            ("LTQ Velos Pro", "MS:1001096", "LTQ Velos Pro"),
            ("LTQ Velos", "MS:1000855", "LTQ Velos"),
            ("LTQ XL", "MS:1000854", "LTQ XL"),
            ("LTQ FT", "MS:1000448", "LTQ FT"),
            ("LTQ", "MS:1000447", "LTQ"),
            ("TSQ Altis", "MS:1003108", "TSQ Altis"),
            ("TSQ Quantiva", "MS:1002498", "TSQ Quantiva"),
            ("TSQ Endura", "MS:1002497", "TSQ Endura"),
            ("TSQ Vantage", "MS:1001510", "TSQ Vantage"),
            ("LCQ Classic", "MS:1000443", "LCQ Classic"),
            ("LCQ Deca", "MS:1000446", "LCQ Deca"),
            ("LCQ Advantage", "MS:1000590", "LCQ Advantage"),
        ];
        for (prefix, acc, name) in known {
            if model.starts_with(prefix) {
                return msc::CvTerm::new(acc, *name);
            }
        }
    }
    msc::CvTerm::new("MS:1000483", "Thermo Fisher Scientific instrument model")
}

/// Acquisition start timestamp (RFC 3339), when the RAW file's info
/// preamble carries a valid date. See
/// [`crate::raw_file_info::RawFileInfoPreamble::acquisition_date_rfc3339`]
/// for the instrument-local-time-with-no-timezone caveat this carries.
fn start_timestamp(raw: &RawFileReader) -> Option<String> {
    raw.raw_file_info.preamble.acquisition_date_rfc3339()
}

fn convert_polarity(p: Option<Polarity>) -> Option<msc::Polarity> {
    p.map(|p| match p {
        Polarity::Negative => msc::Polarity::Negative,
        Polarity::Positive => msc::Polarity::Positive,
    })
}

fn convert_scan_mode(m: Option<crate::ScanMode>) -> Option<msc::ScanMode> {
    m.map(|m| match m {
        crate::ScanMode::Centroid => msc::ScanMode::Centroid,
        crate::ScanMode::Profile => msc::ScanMode::Profile,
    })
}

fn convert_analyzer(a: Option<crate::Analyzer>) -> Option<msc::Analyzer> {
    a.map(|a| match a {
        crate::Analyzer::ITMS => msc::Analyzer::ITMS,
        crate::Analyzer::TQMS => msc::Analyzer::TQMS,
        crate::Analyzer::SQMS => msc::Analyzer::SQMS,
        crate::Analyzer::TOFMS => msc::Analyzer::TOFMS,
        crate::Analyzer::FTMS => msc::Analyzer::FTMS,
        crate::Analyzer::Sector => msc::Analyzer::Sector,
    })
}

fn convert_activation(a: Option<Activation>) -> Option<msc::Activation> {
    a.map(|a| match a {
        Activation::HCD => msc::Activation::HCD,
        Activation::MPID => msc::Activation::MPID,
        Activation::ETD => msc::Activation::ETD,
        Activation::CID => msc::Activation::CID,
        Activation::ECD => msc::Activation::ECD,
        Activation::IRMPD => msc::Activation::IRMPD,
        Activation::PD => msc::Activation::PD,
        Activation::PQD => msc::Activation::PQD,
        Activation::UVPD => msc::Activation::UVPD,
        Activation::SID => msc::Activation::SID,
        Activation::EThcD => msc::Activation::EThcD,
    })
}

/// Thermo native ID string for `scan_number`.
fn native_id_for(scan_number: u32) -> String {
    format!("controllerType=0 controllerNumber=1 scan={scan_number}")
}

fn to_msc_record(rec: SpectrumRecord) -> msc::SpectrumRecord {
    let precursor = rec.precursor.map(|p| msc::PrecursorInfo {
        target_mz: p.target_mz,
        selected_mz: p.selected_mz,
        isolation_width: p.isolation_width,
        charge: p.charge,
        intensity: None,
        collision_energy: p.collision_energy,
        ce_is_nce: p.ce_is_nce,
        precursor_native_id: p.master_scan_number.map(native_id_for),
        activation: convert_activation(p.activation),
        analyzer: convert_analyzer(p.analyzer),
        ccs: None,
    });
    msc::SpectrumRecord {
        extra: ::std::collections::BTreeMap::new(),
        acquisition_event_id: None,
        index: rec.index,
        scan_number: rec.scan_number,
        native_id: native_id_for(rec.scan_number),
        ms_level: rec.ms_level,
        polarity: convert_polarity(rec.polarity),
        scan_mode: convert_scan_mode(rec.scan_mode),
        analyzer: None, // Per-spectrum analyzer is rarely useful here; the
        // precursor's analyzer is what matters for CID/HCD CV resolution.
        filter: rec.filter,
        retention_time_sec: rec.retention_time_min * 60.0,
        total_ion_current: Some(rec.total_ion_current),
        base_peak_mz: Some(rec.base_peak_mz),
        base_peak_intensity: Some(rec.base_peak_intensity),
        low_mz: Some(rec.low_mz),
        high_mz: Some(rec.high_mz),
        ion_injection_time_ms: rec.ion_injection_time_ms,
        inv_mobility: None,
        faims_cv: rec.faims_cv,
        precursor,
        mz: rec.mz,
        intensity: rec.intensity,
        inv_mobility_per_peak: None,
    }
}

// -- Chromatogram records (TIC / BPC / SRM) --
//
// These map already-decoded per-scan fields into vendor-neutral
// `openmassspec_core::ChromatogramRecord`s. No new binary parsing happens
// here: the TIC/BPC series come straight from the scan index
// (`total_current`, `base_intensity`), and SRM transitions are formed by
// grouping the same scans by their `scan_event` using the Q1/Q3 maps the
// reader already builds. Following the sibling Waters wiring
// (Sigilweaver/OpenWRaw#9), a trace is only emitted when it can be labeled
// with a real PSI-MS chromatogram-type term and its defining fields resolve;
// anything ambiguous is omitted rather than guessed.

/// Build the total-ion-current chromatogram from `(rt_min, total_current)`
/// pairs, or `None` when the file has no scans.
///
/// `ChromatogramRecord::time_sec` is seconds; the reader reports retention
/// time in minutes, hence the `* 60.0`.
fn tic_record(tic: &[(f64, f64)]) -> Option<msc::ChromatogramRecord> {
    if tic.is_empty() {
        return None;
    }
    Some(msc::ChromatogramRecord {
        index: 0,
        id: "TIC".to_string(),
        chromatogram_type: Some(msc::CvTerm::new(
            "MS:1000235",
            "total ion current chromatogram",
        )),
        precursor_mz: None,
        product_mz: None,
        time_sec: tic.iter().map(|&(rt, _)| (rt * 60.0) as f32).collect(),
        intensity: tic.iter().map(|&(_, i)| i as f32).collect(),
    })
}

/// Build the base-peak chromatogram from `(rt_min, base_intensity, base_mz)`
/// triples, or `None` when the file has no scans. `base_mz` is not carried on
/// a chromatogram record, so only the intensity trace is emitted.
fn bpc_record(bpc: &[(f64, f64, f64)]) -> Option<msc::ChromatogramRecord> {
    if bpc.is_empty() {
        return None;
    }
    Some(msc::ChromatogramRecord {
        index: 0,
        id: "BPC".to_string(),
        chromatogram_type: Some(msc::CvTerm::new("MS:1000628", "basepeak chromatogram")),
        precursor_mz: None,
        product_mz: None,
        time_sec: bpc.iter().map(|&(rt, _, _)| (rt * 60.0) as f32).collect(),
        intensity: bpc.iter().map(|&(_, bpi, _)| bpi as f32).collect(),
    })
}

/// Group SRM/flat-peak scans into one chromatogram per transition.
///
/// `scans` is `(scan_event, rt_min, total_current)` in scan (time) order.
/// Each `scan_event` is one Q1->Q3 transition; its points are the summed
/// product-ion current (`total_current`) already decoded per scan, so no
/// re-summing of peaks is needed. A transition is emitted only when its Q1
/// precursor is known (`q1_by_event`); the product m/z is filled in only when
/// exactly one Q3 window resolves unambiguously, and left unset otherwise
/// rather than guessed. Events are visited in sorted order for deterministic
/// output.
///
/// Two transitions can legitimately share the same Q1/Q3 (e.g. a scheduled
/// method re-monitoring the same pair across separate retention-time
/// windows, each its own `scan_event`), which would otherwise collide on the
/// same `Q1={..} Q3={..}` id - `mzML`'s chromatogram `id` is a plain
/// `xsd:string`, not `xsd:ID`, so nothing upstream would catch a duplicate,
/// but the indexed-mzML writer keys its offset index by `id`
/// (`openmassspec_core::mzml::write_indexed_mzml`), so a collision there
/// would make one of the two transitions unreachable by id-based lookup.
/// Disambiguate any repeat by appending its `scan_event`.
fn srm_chromatograms(
    scans: &[(u16, f64, f64)],
    q1_by_event: &std::collections::HashMap<u16, f64>,
    q3_windows: &std::collections::HashMap<u16, Vec<(f32, f32)>>,
) -> Vec<msc::ChromatogramRecord> {
    let mut points_by_event: std::collections::BTreeMap<u16, Vec<(f32, f32)>> =
        std::collections::BTreeMap::new();
    for &(event, rt_min, current) in scans {
        points_by_event
            .entry(event)
            .or_default()
            .push(((rt_min * 60.0) as f32, current as f32));
    }

    let mut out = Vec::new();
    let mut seen_ids: std::collections::HashSet<String> = std::collections::HashSet::new();
    for (event, points) in points_by_event {
        let Some(&q1) = q1_by_event.get(&event) else {
            continue;
        };
        let product_mz = match q3_windows.get(&event) {
            Some(windows) if windows.len() == 1 => {
                let (lo, hi) = windows[0];
                Some(((lo + hi) / 2.0) as f64)
            }
            _ => None,
        };
        let base_id = match product_mz {
            Some(q3) => format!("SRM Q1={q1:.4} Q3={q3:.4}"),
            None => format!("SRM Q1={q1:.4}"),
        };
        let id = if seen_ids.insert(base_id.clone()) {
            base_id
        } else {
            format!("{base_id} event={event}")
        };
        out.push(msc::ChromatogramRecord {
            index: 0,
            id,
            chromatogram_type: Some(msc::CvTerm::new(
                "MS:1000627",
                "selected reaction monitoring chromatogram",
            )),
            precursor_mz: Some(q1),
            product_mz,
            time_sec: points.iter().map(|&(t, _)| t).collect(),
            intensity: points.iter().map(|&(_, i)| i).collect(),
        });
    }
    out
}

/// Assemble every chromatogram trace `raw` can supply: always a TIC and BPC
/// (when the file has scans), plus one SRM chromatogram per transition for
/// flat-peak SRM files. Records are indexed sequentially in emission order.
fn build_chromatograms(raw: &RawFileReader) -> Vec<msc::ChromatogramRecord> {
    let mut out = Vec::new();
    out.extend(tic_record(&raw.tic_chromatogram()));
    out.extend(bpc_record(&raw.bpc_chromatogram()));

    if raw.flat_peaks && !raw.srm_q1_by_event.is_empty() {
        let scans: Vec<(u16, f64, f64)> = raw
            .scan_index
            .iter()
            .map(|e| (e.scan_event, e.start_time, e.total_current))
            .collect();
        out.extend(srm_chromatograms(
            &scans,
            &raw.srm_q1_by_event,
            &raw.srm_q3_windows,
        ));
    }

    for (i, rec) in out.iter_mut().enumerate() {
        rec.index = i;
    }
    out
}

/// `SpectrumSource` adapter over a Thermo RAW reader.
///
/// Use this when you want to feed Thermo data into any
/// `openmassspec_core`-shaped consumer (the canonical mzML writer, a column
/// store ingester, future Arrow bridge, ...). For the common case of "I just
/// want mzML out", call [`write_mzml`] or [`write_indexed_mzml`] directly.
pub struct OpenTfRawSource<'a, R: Read + Seek> {
    raw: &'a RawFileReader,
    source: &'a mut R,
    raw_filename: &'a str,
    include_profile: bool,
}

impl<'a, R: Read + Seek> OpenTfRawSource<'a, R> {
    pub fn new(
        raw: &'a RawFileReader,
        source: &'a mut R,
        raw_filename: &'a str,
        include_profile: bool,
    ) -> Self {
        Self {
            raw,
            source,
            raw_filename,
            include_profile,
        }
    }
}

impl<'a, R: Read + Seek> msc::SpectrumSource for OpenTfRawSource<'a, R> {
    fn run_metadata(&self) -> msc::RunMetadata {
        msc::RunMetadata {
            extra: ::std::collections::BTreeMap::new(),
            source_file_name: self.raw_filename.to_string(),
            source_file_format: source_file_format_cv(),
            native_id_format: native_id_format_cv(),
            instrument: instrument_cv(self.raw),
            instrument_serial_number: None,
            software_name: SOFTWARE_NAME.into(),
            software_version: SOFTWARE_VERSION.into(),
            acquisition_software_name: None,
            acquisition_software_version: None,
            start_timestamp: start_timestamp(self.raw),
            mobility_array_kind: None,
            analyzers: Vec::new(),
        }
    }

    fn iter_spectra<'s>(&'s mut self) -> Box<dyn Iterator<Item = msc::SpectrumRecord> + 's> {
        let n = self.raw.num_scans;
        let raw = self.raw;
        let source = &mut *self.source;
        let include_profile = self.include_profile;
        let mut idx: u32 = 0;
        Box::new(std::iter::from_fn(move || {
            while idx < n {
                let cur = idx;
                idx += 1;
                if let Some(rec) = extract_spectrum(raw, source, cur, include_profile) {
                    return Some(to_msc_record(rec));
                }
            }
            None
        }))
    }

    fn spectrum_count_hint(&self) -> Option<usize> {
        Some(self.raw.num_scans as usize)
    }

    fn iter_chromatograms<'s>(
        &'s mut self,
    ) -> Box<dyn Iterator<Item = msc::ChromatogramRecord> + 's> {
        Box::new(build_chromatograms(self.raw).into_iter())
    }
}

// -- Public mzML entry points (unchanged signatures) --

/// Write the contents of `raw` as mzML 1.1.0 to `out`.
///
/// * `source` - an open handle to the original `.raw` file (needed to read
///   scan data packets).
/// * `raw_filename` - the file name used for the `<sourceFile>` element.
/// * `include_profile` - when `true`, profile-mode scans export the raw
///   profile signal instead of the centroid peak list.
///
/// All spectra are written; no filtering is applied. Scans for which peak
/// data cannot be read are skipped silently.
pub fn write_mzml<R, W>(
    raw: &RawFileReader,
    source: &mut R,
    out: &mut W,
    raw_filename: &str,
    include_profile: bool,
) -> Result<()>
where
    R: Read + Seek,
    W: Write,
{
    let mut src = OpenTfRawSource::new(raw, source, raw_filename, include_profile);
    msc::write_mzml(&mut src, out)?;
    Ok(())
}

/// Write the contents of `raw` as an indexed mzML 1.1.0 document.
///
/// Indexed mzML adds a `<indexList>` element after all spectra with the byte
/// offset of each `<spectrum>` element, enabling random-access parsing by
/// tools such as pyteomics and pymzml without a full file scan. The
/// `<fileChecksum>` element contains the SHA-1 hash of the file content up
/// to and including `</indexList>`.
pub fn write_indexed_mzml<R, W>(
    raw: &RawFileReader,
    source: &mut R,
    out: &mut W,
    raw_filename: &str,
    include_profile: bool,
) -> Result<()>
where
    R: Read + Seek,
    W: Write,
{
    let mut src = OpenTfRawSource::new(raw, source, raw_filename, include_profile);
    msc::write_indexed_mzml(&mut src, out)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn tic_record_maps_pairs_and_converts_minutes_to_seconds() {
        // (rt_min, total_current)
        let tic = [(0.0, 100.0), (0.5, 250.0), (1.0, 175.0)];
        let rec = tic_record(&tic).expect("non-empty TIC");
        assert_eq!(rec.id, "TIC");
        let cv = rec.chromatogram_type.as_ref().unwrap();
        assert_eq!(cv.accession, "MS:1000235");
        assert_eq!(cv.name, "total ion current chromatogram");
        assert!(rec.precursor_mz.is_none());
        assert!(rec.product_mz.is_none());
        assert_eq!(rec.time_sec, vec![0.0, 30.0, 60.0]); // rt_min * 60
        assert_eq!(rec.intensity, vec![100.0, 250.0, 175.0]);
    }

    #[test]
    fn bpc_record_uses_base_intensity_and_drops_base_mz() {
        // (rt_min, base_intensity, base_mz)
        let bpc = [(0.0, 90.0, 500.0), (0.5, 220.0, 501.0)];
        let rec = bpc_record(&bpc).expect("non-empty BPC");
        assert_eq!(rec.id, "BPC");
        let cv = rec.chromatogram_type.as_ref().unwrap();
        assert_eq!(cv.accession, "MS:1000628");
        assert_eq!(cv.name, "basepeak chromatogram");
        assert_eq!(rec.time_sec, vec![0.0, 30.0]);
        assert_eq!(rec.intensity, vec![90.0, 220.0]); // base_mz not carried
    }

    #[test]
    fn tic_and_bpc_records_are_none_when_no_scans() {
        assert!(tic_record(&[]).is_none());
        assert!(bpc_record(&[]).is_none());
    }

    #[test]
    fn srm_groups_scans_by_event_and_sets_precursor_and_product() {
        // Two transitions interleaved in scan order, one scan_event each.
        let scans = [
            (1u16, 0.0f64, 10.0f64),
            (2u16, 0.0, 20.0),
            (1, 0.5, 12.0),
            (2, 0.5, 22.0),
        ];
        let mut q1 = HashMap::new();
        q1.insert(1u16, 500.0);
        q1.insert(2u16, 600.0);
        let mut q3 = HashMap::new();
        q3.insert(1u16, vec![(100.0f32, 101.0f32)]); // single window -> product m/z
        q3.insert(2u16, vec![(200.0f32, 201.0f32)]);

        let recs = srm_chromatograms(&scans, &q1, &q3);
        assert_eq!(recs.len(), 2);

        // BTreeMap order: event 1 first.
        let r1 = &recs[0];
        assert_eq!(r1.precursor_mz, Some(500.0));
        assert_eq!(r1.product_mz, Some(100.5)); // (100 + 101) / 2
        assert_eq!(r1.id, "SRM Q1=500.0000 Q3=100.5000");
        let cv = r1.chromatogram_type.as_ref().unwrap();
        assert_eq!(cv.accession, "MS:1000627");
        assert_eq!(cv.name, "selected reaction monitoring chromatogram");
        assert_eq!(r1.time_sec, vec![0.0, 30.0]);
        assert_eq!(r1.intensity, vec![10.0, 12.0]);

        let r2 = &recs[1];
        assert_eq!(r2.precursor_mz, Some(600.0));
        assert_eq!(r2.product_mz, Some(200.5));
        assert_eq!(r2.intensity, vec![20.0, 22.0]);
    }

    #[test]
    fn srm_skips_events_without_a_known_q1_and_omits_ambiguous_product() {
        let scans = [
            (1u16, 0.0f64, 10.0f64), // no Q1 -> skipped
            (2u16, 0.0, 20.0),       // Q1 known, two Q3 windows -> product omitted
        ];
        let mut q1 = HashMap::new();
        q1.insert(2u16, 600.0);
        let mut q3 = HashMap::new();
        q3.insert(2u16, vec![(200.0f32, 201.0f32), (300.0f32, 301.0f32)]);

        let recs = srm_chromatograms(&scans, &q1, &q3);
        assert_eq!(recs.len(), 1, "event without a Q1 must be skipped");
        assert_eq!(recs[0].precursor_mz, Some(600.0));
        assert!(
            recs[0].product_mz.is_none(),
            "ambiguous multi-window Q3 must not be guessed"
        );
        assert_eq!(recs[0].id, "SRM Q1=600.0000");
    }

    #[test]
    fn srm_disambiguates_ids_when_two_events_share_the_same_q1_and_q3() {
        // Scheduled method re-monitoring the same transition in two separate
        // time windows: same Q1/Q3, different scan_event.
        let scans = [(1u16, 0.0f64, 10.0f64), (2u16, 5.0, 15.0)];
        let mut q1 = HashMap::new();
        q1.insert(1u16, 500.0);
        q1.insert(2u16, 500.0);
        let mut q3 = HashMap::new();
        q3.insert(1u16, vec![(100.0f32, 101.0f32)]);
        q3.insert(2u16, vec![(100.0f32, 101.0f32)]);

        let recs = srm_chromatograms(&scans, &q1, &q3);
        assert_eq!(recs.len(), 2);
        assert_eq!(recs[0].id, "SRM Q1=500.0000 Q3=100.5000");
        assert_eq!(
            recs[1].id, "SRM Q1=500.0000 Q3=100.5000 event=2",
            "second event with an identical Q1/Q3 must get a disambiguated id"
        );
        assert_ne!(
            recs[0].id, recs[1].id,
            "ids feed the indexed-mzML offset index and must stay unique"
        );
    }
}
