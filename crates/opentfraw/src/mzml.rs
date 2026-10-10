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
use crate::extra::{scan_extras, ExtraFields};
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
/// collision energy. When a scan stores both, the NCE value is reported (see
/// [`crate::ScanParams::activation_energy`]).
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

/// Per-scan metadata: every field of a [`SpectrumRecord`] except the peak
/// arrays, plus the scan-index fields that locate the scan in the method.
///
/// Returned by [`scan_metadata`]. Reading it touches no scan data packet, so
/// it is cheap enough to build for every scan in a file.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct ScanMetadata {
    pub index: usize,
    pub scan_number: u32,
    /// Scan event index, as stored in the scan index. Some files store
    /// `0xFFFF` here.
    pub scan_event: u16,
    /// Scan segment index, as stored in the scan index. Some files store
    /// `0xFFFF` here.
    pub scan_segment: u16,
    /// Size of the scan data packet in bytes, as stored in the scan index.
    pub data_size: u32,
    pub ms_level: u32,
    pub is_ms1: bool,
    /// Whether this MS2+ scan uses data-independent acquisition.
    pub is_dia: bool,
    /// Whether broadband isolation is enabled for this scan.
    pub is_wideband: bool,
    pub polarity: Option<Polarity>,
    /// Scan mode as recorded in the scan event.
    pub scan_mode: Option<crate::ScanMode>,
    /// Mass analyzer of this scan.
    pub analyzer: Option<crate::Analyzer>,
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
}

/// Extract the metadata of the scan at scan-index `idx` (zero-based offset
/// from the first scan), without reading its peak arrays.
///
/// This is the single place where per-scan fields are derived from the scan
/// index, scan event and trailer. [`extract_spectrum`], the mzML writer, the
/// `openmassspec_core` adapter and the Python bindings all build on it, so a
/// field wired here reaches every output. Returns `None` if `idx` is out of
/// range.
pub fn scan_metadata(raw: &RawFileReader, idx: u32) -> Option<ScanMetadata> {
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

    Some(ScanMetadata {
        index: idx as usize,
        scan_number,
        scan_event: entry.scan_event,
        scan_segment: entry.scan_segment,
        data_size: entry.data_size,
        ms_level: level,
        is_ms1,
        is_dia,
        is_wideband,
        polarity,
        scan_mode,
        analyzer: event.and_then(|e| e.preamble.analyzer()),
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
    })
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
    // `SpectrumRecord` has no field for the unconverted-bin count; the
    // `openmassspec_core` adapter reports it in `extra`.
    let (meta, mz, intensity, effective_scan_mode, _unconverted_mz_bins) =
        extract_parts(raw, source, idx, include_profile)?;
    Some(SpectrumRecord {
        index: meta.index,
        scan_number: meta.scan_number,
        ms_level: meta.ms_level,
        is_ms1: meta.is_ms1,
        is_dia: meta.is_dia,
        is_wideband: meta.is_wideband,
        polarity: meta.polarity,
        scan_mode: effective_scan_mode,
        filter: meta.filter,
        retention_time_min: meta.retention_time_min,
        total_ion_current: meta.total_ion_current,
        base_peak_mz: meta.base_peak_mz,
        base_peak_intensity: meta.base_peak_intensity,
        low_mz: meta.low_mz,
        high_mz: meta.high_mz,
        ion_injection_time_ms: meta.ion_injection_time_ms,
        faims_cv: meta.faims_cv,
        precursor: meta.precursor,
        mz,
        intensity,
    })
}

/// A scan's metadata, its peak arrays, the scan mode those arrays are in,
/// and the number of profile bins dropped because their m/z could not be
/// computed (unknown calibration layout).
type ScanParts = (
    ScanMetadata,
    Vec<f64>,
    Vec<f32>,
    Option<crate::ScanMode>,
    usize,
);

fn extract_parts<R: Read + Seek>(
    raw: &RawFileReader,
    source: &mut R,
    idx: u32,
    include_profile: bool,
) -> Option<ScanParts> {
    let meta = scan_metadata(raw, idx)?;
    let event = raw.scan_events.get(idx as usize);
    let (mz, intensity, effective_scan_mode, unconverted) = resolve_scan_arrays(
        raw,
        source,
        meta.scan_number,
        include_profile,
        event,
        meta.scan_mode,
    )?;
    Some((meta, mz, intensity, effective_scan_mode, unconverted))
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

/// m/z, intensity, effective scan mode, and unconverted profile-bin count.
type ScanArrays = (Vec<f64>, Vec<f32>, Option<crate::ScanMode>, usize);

/// Resolve the m/z and intensity arrays for a single scan.
///
/// When `include_profile=true` AND the scan packet contains profile data, the
/// profile signal is decoded and returned as the primary arrays (with
/// `effective_scan_mode = Some(ScanMode::Profile)`). Otherwise the centroid
/// peak list is used.
///
/// The last element counts profile bins dropped because their m/z is `NaN`
/// (unknown calibration layout, see `scan_data::freq_to_mz`).
///
/// Returns `None` when the scan cannot be read (caller should skip it).
fn resolve_scan_arrays<R: Read + Seek>(
    raw: &RawFileReader,
    source: &mut R,
    scan_number: u32,
    include_profile: bool,
    event: Option<&ScanEvent>,
    nominal_scan_mode: Option<crate::ScanMode>,
) -> Option<ScanArrays> {
    if include_profile && !raw.flat_peaks {
        let packet = raw.read_scan(source, scan_number).ok()?;
        if let Some(profile) = packet.profile {
            let coeffs = event.map(|e| e.coefficients.as_slice()).unwrap_or(&[]);
            let pairs = profile.to_mz_intensity(coeffs);
            // `m > 0.0` also drops NaN m/z (unknown calibration layout), so
            // unconverted bins never reach mzML. They are counted instead.
            let unconverted = pairs.iter().filter(|(m, _)| m.is_nan()).count();
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
            return Some((mz, int, Some(crate::ScanMode::Profile), unconverted));
        }
        let mz: Vec<f64> = packet.peaks.iter().map(|p| p.mz).collect();
        let int: Vec<f32> = packet.peaks.iter().map(|p| p.abundance).collect();
        return Some((mz, int, nominal_scan_mode, 0));
    }
    let peaks = raw.read_peaks_only(source, scan_number).ok()?;
    let mz: Vec<f64> = peaks.iter().map(|p| p.mz).collect();
    let int: Vec<f32> = peaks.iter().map(|p| p.abundance).collect();
    Some((mz, int, nominal_scan_mode, 0))
}

// -- Adapter / canonical writer wrappers --
//
// The mzML emission machinery itself lives in `openmassspec_core`. Here we
// define a `SpectrumSource` adapter that pulls Thermo scans through
// `extract_parts` and converts each into the vendor-neutral
// `openmassspec_core::SpectrumRecord` the canonical writer consumes, so the
// writer is shared with the other vendors.

use openmassspec_core as msc;

const SOFTWARE_NAME: &str = "opentfraw";
/// Spectrum `extra` key holding the number of profile bins dropped because
/// the scan's calibration layout is unknown and their m/z is `NaN`. Written
/// regardless of [`ExtraFields`], since it reports lost data.
const UNCONVERTED_PROFILE_BINS_KEY: &str = "opentfraw.unconverted_profile_bins";
// Written to mzML `<software version=...>` so converted files record the
// opentfraw release that produced them.
const SOFTWARE_VERSION: &str = env!("CARGO_PKG_VERSION");

/// PSI-MS CV term for the source file format (Thermo RAW).
fn source_file_format_cv() -> msc::CvTerm {
    msc::CvTerm::new("MS:1000563", "Thermo RAW format")
}

/// PSI-MS CV term for the native ID format used by Thermo.
fn native_id_format_cv() -> msc::CvTerm {
    msc::CvTerm::new("MS:1000768", "Thermo nativeID format")
}

/// PSI-MS generic parent term used when a detected model has no specific
/// instrument-model term (the model name then goes into the
/// `opentfraw.instrument_model` run userParam).
const GENERIC_THERMO_INSTRUMENT: (&str, &str) =
    ("MS:1000483", "Thermo Fisher Scientific instrument model");

/// Detected model name (exactly as produced by `device::MODEL_REGISTRY`) to
/// PSI-MS instrument-model term. Checked against psi-ms.obo by
/// `tests::instrument_table_matches_psi_ms_obo`. Registry names with no
/// specific PSI-MS term are deliberately absent and fall back to
/// [`GENERIC_THERMO_INSTRUMENT`].
const THERMO_INSTRUMENT_MODELS: &[(&str, &str, &str)] = &[
    ("Orbitrap Astral", "MS:1003378", "Orbitrap Astral"),
    ("Orbitrap Ascend", "MS:1003356", "Orbitrap Ascend"),
    (
        "Orbitrap Fusion Lumos",
        "MS:1002732",
        "Orbitrap Fusion Lumos",
    ),
    ("Orbitrap Eclipse", "MS:1003029", "Orbitrap Eclipse"),
    ("Orbitrap Fusion", "MS:1002416", "Orbitrap Fusion"),
    (
        "Orbitrap Exploris 480",
        "MS:1003028",
        "Orbitrap Exploris 480",
    ),
    (
        "Orbitrap Exploris 240",
        "MS:1003094",
        "Orbitrap Exploris 240",
    ),
    (
        "Orbitrap Exploris 120",
        "MS:1003095",
        "Orbitrap Exploris 120",
    ),
    (
        "Orbitrap Exploris GC 240",
        "MS:1003423",
        "Orbitrap Exploris GC 240",
    ),
    ("Q Exactive HF-X", "MS:1002877", "Q Exactive HF-X"),
    ("Q Exactive UHMR", "MS:1003245", "Q Exactive UHMR"),
    ("Q Exactive Plus", "MS:1002634", "Q Exactive Plus"),
    ("Q Exactive HF", "MS:1002523", "Q Exactive HF"),
    ("Q Exactive GC", "MS:1003395", "Q Exactive GC Orbitrap"),
    ("Q Exactive Focus", "MS:1002993", "Q Exactive Focus"),
    ("Q Exactive", "MS:1001911", "Q Exactive"),
    ("LTQ Orbitrap Velos Pro", "MS:1003096", "Orbitrap Velos Pro"),
    (
        "LTQ Orbitrap Velos ETD",
        "MS:1003499",
        "LTQ Orbitrap Velos/ETD",
    ),
    ("LTQ Orbitrap Velos", "MS:1001742", "LTQ Orbitrap Velos"),
    ("LTQ Orbitrap Elite", "MS:1001910", "Orbitrap Elite"),
    (
        "LTQ Orbitrap Discovery",
        "MS:1000555",
        "LTQ Orbitrap Discovery",
    ),
    ("LTQ Orbitrap XL ETD", "MS:1000639", "LTQ Orbitrap XL ETD"),
    ("LTQ Orbitrap XL", "MS:1000556", "LTQ Orbitrap XL"),
    ("LTQ Orbitrap", "MS:1000449", "LTQ Orbitrap"),
    ("Orbitrap Elite", "MS:1001910", "Orbitrap Elite"),
    ("Orbitrap Velos Pro", "MS:1003096", "Orbitrap Velos Pro"),
    ("Orbitrap Velos", "MS:1001742", "LTQ Orbitrap Velos"),
    ("Orbitrap Discovery", "MS:1000555", "LTQ Orbitrap Discovery"),
    ("Orbitrap XL", "MS:1000556", "LTQ Orbitrap XL"),
    ("LTQ FT Ultra", "MS:1000557", "LTQ FT Ultra"),
    ("LTQ FT", "MS:1000448", "LTQ FT"),
    ("LTQ Velos Pro", "MS:1003495", "Velos Pro"),
    ("LTQ Velos ETD", "MS:1000856", "LTQ Velos/ETD"),
    ("LTQ Velos", "MS:1000855", "LTQ Velos"),
    ("LTQ XL ETD", "MS:1000638", "LTQ XL ETD"),
    ("LTQ XL", "MS:1000854", "LTQ XL"),
    ("LTQ", "MS:1000447", "LTQ"),
    ("LCQ Fleet", "MS:1000578", "LCQ Fleet"),
    ("LCQ Advantage", "MS:1000167", "LCQ Advantage"),
    ("LCQ Deca XP Plus", "MS:1000169", "LCQ Deca XP Plus"),
    ("LCQ Deca", "MS:1000554", "LCQ Deca"),
    ("LCQ Classic", "MS:1000168", "LCQ Classic"),
    ("TSQ Quantiva", "MS:1002418", "TSQ Quantiva"),
    ("TSQ Quantum Ultra AM", "MS:1000743", "TSQ Quantum Ultra AM"),
    ("TSQ Quantum Ultra", "MS:1000751", "TSQ Quantum Ultra"),
    ("TSQ Quantum Access", "MS:1000644", "TSQ Quantum Access"),
    ("TSQ Quantum", "MS:1000199", "TSQ Quantum"),
    ("TSQ Vantage", "MS:1001510", "TSQ Vantage"),
    ("TSQ Endura", "MS:1002419", "TSQ Endura"),
    ("TSQ Altis Plus", "MS:1003292", "TSQ Altis Plus"),
    ("TSQ Altis", "MS:1002874", "TSQ Altis"),
    ("TSQ 8000 Evo", "MS:1002525", "TSQ 8000 Evo"),
    ("TSQ 9000", "MS:1002876", "TSQ 9000"),
];

/// Look up the specific PSI-MS term for a detected model name.
fn instrument_term(model: &str) -> Option<(&'static str, &'static str)> {
    THERMO_INSTRUMENT_MODELS
        .iter()
        .find(|(m, _, _)| *m == model)
        .map(|(_, acc, name)| (*acc, *name))
}

/// Resolve the instrument CV term for `raw`. Unknown or undetected models get
/// the generic Thermo Fisher Scientific term.
fn instrument_cv(raw: &RawFileReader) -> msc::CvTerm {
    let (acc, name) = raw
        .instrument_model
        .and_then(instrument_term)
        .unwrap_or(GENERIC_THERMO_INSTRUMENT);
    msc::CvTerm::new(acc, name)
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

/// `Unknown` codes map to `None`: the shared model has no "unknown" method.
fn convert_activation(a: Option<Activation>) -> Option<msc::Activation> {
    a.and_then(|a| match a {
        Activation::HCD => Some(msc::Activation::HCD),
        Activation::CID => Some(msc::Activation::CID),
        Activation::Unknown(_) => None,
    })
}

/// Thermo native ID string for `scan_number`.
fn native_id_for(scan_number: u32) -> String {
    format!("controllerType=0 controllerNumber=1 scan={scan_number}")
}

fn to_msc_record(
    parts: ScanParts,
    extra: ::std::collections::BTreeMap<String, String>,
) -> msc::SpectrumRecord {
    let (meta, mz, intensity, scan_mode, _) = parts;
    let precursor = meta.precursor.map(|p| msc::PrecursorInfo {
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
        extra,
        // 0xFFFF is a "no event" sentinel some files store.
        acquisition_event_id: (meta.scan_event != u16::MAX).then_some(u32::from(meta.scan_event)),
        index: meta.index,
        scan_number: meta.scan_number,
        native_id: native_id_for(meta.scan_number),
        ms_level: meta.ms_level,
        polarity: convert_polarity(meta.polarity),
        scan_mode: convert_scan_mode(scan_mode),
        analyzer: convert_analyzer(meta.analyzer),
        filter: meta.filter,
        retention_time_sec: meta.retention_time_min * 60.0,
        total_ion_current: Some(meta.total_ion_current),
        base_peak_mz: Some(meta.base_peak_mz),
        base_peak_intensity: Some(meta.base_peak_intensity),
        low_mz: Some(meta.low_mz),
        high_mz: Some(meta.high_mz),
        ion_injection_time_ms: meta.ion_injection_time_ms,
        inv_mobility: None,
        faims_cv: meta.faims_cv,
        precursor,
        mz,
        intensity,
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
    extra_fields: ExtraFields,
    acquisition_paths: bool,
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
            extra_fields: ExtraFields::All,
            acquisition_paths: false,
        }
    }

    /// Include acquisition-workstation details in the run metadata: the
    /// acquisition computer name (`opentfraw.computer_name`), the original
    /// directory (`opentfraw.original_file_path`), and full original paths in
    /// `opentfraw.original_file_name` and the method-file entries.
    ///
    /// Off by default: those values identify the source machine and its
    /// directory layout (often including user or project names), so by
    /// default only file names are written. The same values stay available
    /// on [`RawFileReader`] (`raw_file_info.computer_name`, `seq_row`).
    pub fn with_acquisition_paths(mut self, include: bool) -> Self {
        self.acquisition_paths = include;
        self
    }

    /// Choose which `opentfraw.*` values go into each spectrum's `extra`
    /// map (and so into mzML `<userParam>`s). Defaults to
    /// [`ExtraFields::All`]; see [`crate::extra::extra_field_keys`] for the keys.
    pub fn with_extra_fields(mut self, fields: ExtraFields) -> Self {
        self.extra_fields = fields;
        self
    }
}

impl<R: Read + Seek> OpenTfRawSource<'_, R> {
    /// A stored acquisition path, reduced to its file name unless
    /// [`Self::with_acquisition_paths`] is on.
    fn path_value(&self, path: &str) -> String {
        if self.acquisition_paths {
            path.to_string()
        } else {
            file_name_only(path).to_string()
        }
    }
}

/// Final component of a Windows or POSIX path (both separators accepted,
/// since RAW files store the acquisition PC's Windows paths).
fn file_name_only(path: &str) -> &str {
    path.rsplit(['\\', '/']).next().unwrap_or(path)
}

/// Distinct analyzers across the file's scan events, in first-seen order.
fn run_analyzers(raw: &RawFileReader) -> Vec<msc::Analyzer> {
    let mut out = Vec::new();
    for event in &raw.scan_events {
        if let Some(a) = convert_analyzer(event.preamble.analyzer()) {
            if !out.contains(&a) {
                out.push(a);
            }
        }
    }
    out
}

impl<'a, R: Read + Seek> msc::SpectrumSource for OpenTfRawSource<'a, R> {
    fn run_metadata(&self) -> msc::RunMetadata {
        let mut extra = ::std::collections::BTreeMap::new();
        extra.insert("opentfraw.raw_version".into(), self.raw.version.to_string());
        extra.insert(
            "opentfraw.scan_format".into(),
            format!("{:?}", self.raw.scan_format),
        );
        extra.insert(
            "opentfraw.device_family".into(),
            format!("{:?}", self.raw.device_family),
        );
        extra.insert(
            "opentfraw.controller_count".into(),
            self.raw.raw_file_info.preamble.controller_count.to_string(),
        );
        if self.acquisition_paths {
            extra.insert(
                "opentfraw.computer_name".into(),
                self.raw.raw_file_info.computer_name.clone(),
            );
        }
        if let Some(e) = self.raw.status_log_error() {
            extra.insert("opentfraw.status_log_error".into(), e.to_string());
        }
        if let Some(model) = self.raw.instrument_model {
            if instrument_term(model).is_none() {
                extra.insert("opentfraw.instrument_model".into(), model.to_string());
            }
        }
        let row = &self.raw.seq_row;
        extra.insert("opentfraw.sample_id".into(), row.id.clone());
        extra.insert("opentfraw.sample_comment".into(), row.comment.clone());
        extra.insert("opentfraw.sample_vial".into(), row.vial.clone());
        extra.insert(
            "opentfraw.injection_row_number".into(),
            row.injection.row_number.to_string(),
        );
        extra.insert(
            "opentfraw.injection_vial".into(),
            row.injection.vial.clone(),
        );
        extra.insert(
            "opentfraw.injection_volume".into(),
            row.injection.injection_volume.to_string(),
        );
        extra.insert(
            "opentfraw.sample_weight".into(),
            row.injection.sample_weight.to_string(),
        );
        extra.insert(
            "opentfraw.sample_volume".into(),
            row.injection.sample_volume.to_string(),
        );
        extra.insert(
            "opentfraw.istd_amount".into(),
            row.injection.istd_amount.to_string(),
        );
        extra.insert(
            "opentfraw.dilution_factor".into(),
            row.injection.dilution_factor.to_string(),
        );
        extra.insert(
            "opentfraw.instrument_method_file".into(),
            self.path_value(&row.inst_method),
        );
        extra.insert(
            "opentfraw.processing_method_file".into(),
            self.path_value(&row.proc_method),
        );
        extra.insert(
            "opentfraw.original_file_name".into(),
            self.path_value(&row.file_name),
        );
        if self.acquisition_paths {
            extra.insert("opentfraw.original_file_path".into(), row.path.clone());
        }
        for (i, (heading, value)) in self
            .raw
            .raw_file_info
            .label_headings
            .iter()
            .zip(&row.user_labels)
            .enumerate()
        {
            extra.insert(format!("opentfraw.user_label.{i}.heading"), heading.clone());
            extra.insert(format!("opentfraw.user_label.{i}.value"), value.clone());
        }
        msc::RunMetadata {
            extra,
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
            analyzers: run_analyzers(self.raw),
        }
    }

    fn iter_spectra<'s>(&'s mut self) -> Box<dyn Iterator<Item = msc::SpectrumRecord> + 's> {
        let n = self.raw.num_scans;
        let raw = self.raw;
        let source = &mut *self.source;
        let include_profile = self.include_profile;
        let extra_fields = &self.extra_fields;
        let mut idx: u32 = 0;
        Box::new(std::iter::from_fn(move || {
            while idx < n {
                let cur = idx;
                idx += 1;
                if let Some(parts) = extract_parts(raw, source, cur, include_profile) {
                    let mut extra = scan_extras(raw, &parts.0, extra_fields);
                    if parts.4 > 0 {
                        extra.insert(UNCONVERTED_PROFILE_BINS_KEY.into(), parts.4.to_string());
                    }
                    return Some(to_msc_record(parts, extra));
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

// -- Public mzML entry points --

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

    /// Every PSI-MS term under MS:1000483 "Thermo Fisher Scientific
    /// instrument model" (including the parent itself), as (accession, name).
    /// Extracted from psi-ms.obo data-version 4.1.249 (non-obsolete terms
    /// only). Regenerate when the table needs a term added after that release.
    const PSI_MS_THERMO_INSTRUMENT_TERMS: &[(&str, &str)] = &[
        ("MS:1000125", "Thermo Finnigan instrument model"),
        ("MS:1000153", "DELTA plusAdvantage"),
        ("MS:1000154", "DELTAplusXP"),
        ("MS:1000167", "LCQ Advantage"),
        ("MS:1000168", "LCQ Classic"),
        ("MS:1000169", "LCQ Deca XP Plus"),
        ("MS:1000172", "MAT253"),
        ("MS:1000173", "MAT900XP"),
        ("MS:1000174", "MAT900XP Trap"),
        ("MS:1000175", "MAT95XP"),
        ("MS:1000176", "MAT95XP Trap"),
        ("MS:1000179", "neptune"),
        ("MS:1000185", "PolarisQ"),
        ("MS:1000193", "Surveyor MSQ"),
        ("MS:1000196", "TEMPUS TOF"),
        ("MS:1000197", "TRACE DSQ"),
        ("MS:1000198", "TRITON"),
        ("MS:1000199", "TSQ Quantum"),
        ("MS:1000447", "LTQ"),
        ("MS:1000448", "LTQ FT"),
        ("MS:1000449", "LTQ Orbitrap"),
        ("MS:1000450", "LXQ"),
        ("MS:1000483", "Thermo Fisher Scientific instrument model"),
        ("MS:1000492", "Thermo Electron instrument model"),
        ("MS:1000493", "Finnigan MAT instrument model"),
        ("MS:1000494", "Thermo Scientific instrument model"),
        ("MS:1000554", "LCQ Deca"),
        ("MS:1000555", "LTQ Orbitrap Discovery"),
        ("MS:1000556", "LTQ Orbitrap XL"),
        ("MS:1000557", "LTQ FT Ultra"),
        ("MS:1000558", "GC Quantum"),
        ("MS:1000578", "LCQ Fleet"),
        ("MS:1000622", "Surveyor PDA"),
        ("MS:1000623", "Accela PDA"),
        ("MS:1000634", "DSQ"),
        ("MS:1000635", "ITQ 700"),
        ("MS:1000636", "ITQ 900"),
        ("MS:1000637", "ITQ 1100"),
        ("MS:1000638", "LTQ XL ETD"),
        ("MS:1000639", "LTQ Orbitrap XL ETD"),
        ("MS:1000640", "DFS"),
        ("MS:1000641", "DSQ II"),
        ("MS:1000642", "MALDI LTQ XL"),
        ("MS:1000643", "MALDI LTQ Orbitrap"),
        ("MS:1000644", "TSQ Quantum Access"),
        ("MS:1000645", "Element XR"),
        ("MS:1000646", "Element 2"),
        ("MS:1000647", "Element GD"),
        ("MS:1000648", "GC IsoLink"),
        ("MS:1000649", "Exactive"),
        ("MS:1000743", "TSQ Quantum Ultra AM"),
        ("MS:1000748", "SSQ 7000"),
        ("MS:1000749", "TSQ 7000"),
        ("MS:1000750", "TSQ"),
        ("MS:1000751", "TSQ Quantum Ultra"),
        ("MS:1000854", "LTQ XL"),
        ("MS:1000855", "LTQ Velos"),
        ("MS:1000856", "LTQ Velos/ETD"),
        ("MS:1001510", "TSQ Vantage"),
        ("MS:1001742", "LTQ Orbitrap Velos"),
        ("MS:1001908", "ISQ"),
        ("MS:1001909", "Velos Plus"),
        ("MS:1001910", "Orbitrap Elite"),
        ("MS:1001911", "Q Exactive"),
        ("MS:1002416", "Orbitrap Fusion"),
        ("MS:1002417", "Orbitrap Fusion ETD"),
        ("MS:1002418", "TSQ Quantiva"),
        ("MS:1002419", "TSQ Endura"),
        ("MS:1002523", "Q Exactive HF"),
        ("MS:1002525", "TSQ 8000 Evo"),
        ("MS:1002526", "Exactive Plus"),
        ("MS:1002634", "Q Exactive Plus"),
        ("MS:1002732", "Orbitrap Fusion Lumos"),
        ("MS:1002835", "LTQ Orbitrap Classic"),
        ("MS:1002874", "TSQ Altis"),
        ("MS:1002875", "TSQ Quantis"),
        ("MS:1002876", "TSQ 9000"),
        ("MS:1002877", "Q Exactive HF-X"),
        ("MS:1002992", "Orbitrap Exploris GC-MS"),
        ("MS:1002993", "Q Exactive Focus"),
        ("MS:1002994", "Orbitrap Excedion Pro"),
        ("MS:1003028", "Orbitrap Exploris 480"),
        ("MS:1003029", "Orbitrap Eclipse"),
        ("MS:1003094", "Orbitrap Exploris 240"),
        ("MS:1003095", "Orbitrap Exploris 120"),
        ("MS:1003096", "Orbitrap Velos Pro"),
        ("MS:1003112", "Orbitrap ID-X"),
        ("MS:1003245", "Q Exactive UHMR"),
        ("MS:1003292", "TSQ Altis Plus"),
        ("MS:1003356", "Orbitrap Ascend"),
        ("MS:1003378", "Orbitrap Astral"),
        ("MS:1003395", "Q Exactive GC Orbitrap"),
        ("MS:1003409", "Stellar"),
        ("MS:1003411", "Orbitrap IQ-X"),
        ("MS:1003423", "Orbitrap Exploris GC 240"),
        ("MS:1003442", "Orbitrap Astral Zoom"),
        ("MS:1003449", "ISQ 7000"),
        ("MS:1003495", "Velos Pro"),
        ("MS:1003496", "MALDI LTQ Orbitrap XL"),
        ("MS:1003497", "MALDI LTQ Orbitrap Discovery"),
        ("MS:1003498", "TSQ Quantum Access MAX"),
        ("MS:1003499", "LTQ Orbitrap Velos/ETD"),
        ("MS:1003500", "ISQ LT"),
        ("MS:1003501", "ITQ"),
        ("MS:1003502", "TSQ Quantum XLS"),
        ("MS:1003503", "TSQ 8000"),
        ("MS:1003504", "DeltaPlus IRMS"),
        ("MS:1003554", "ThermoQuest Voyager"),
        ("MS:1003800", "TSQ Certis"),
    ];

    fn obo_name(accession: &str) -> Option<&'static str> {
        PSI_MS_THERMO_INSTRUMENT_TERMS
            .iter()
            .find(|(acc, _)| *acc == accession)
            .map(|(_, name)| *name)
    }

    #[test]
    fn instrument_table_matches_psi_ms_obo() {
        let mut bad = Vec::new();
        for (model, acc, name) in THERMO_INSTRUMENT_MODELS {
            match obo_name(acc) {
                Some(n) if n == *name => {}
                Some(n) => bad.push(format!("{model}: {acc} is {n:?}, table says {name:?}")),
                None => bad.push(format!(
                    "{model}: {acc} is not a Thermo instrument model term"
                )),
            }
        }
        let (acc, name) = GENERIC_THERMO_INSTRUMENT;
        if obo_name(acc) != Some(name) {
            bad.push(format!("generic term {acc} {name:?} not in OBO list"));
        }
        assert!(
            bad.is_empty(),
            "instrument table errors:\n{}",
            bad.join("\n")
        );
    }

    #[test]
    fn instrument_table_has_unique_models_from_the_registry() {
        let registry: Vec<&str> = crate::device::MODEL_REGISTRY
            .iter()
            .map(|(name, _)| *name)
            .collect();
        let mut seen = std::collections::HashSet::new();
        for (model, _, _) in THERMO_INSTRUMENT_MODELS {
            assert!(seen.insert(*model), "duplicate table row for {model}");
            assert!(
                registry.contains(model),
                "{model} is never produced by device::MODEL_REGISTRY"
            );
        }
    }

    #[test]
    fn unmatched_models_fall_back_to_generic_term() {
        // Registry names with no specific PSI-MS term.
        for model in [
            "Orbitrap Exploris MX",
            "Orbitrap Exploris",
            "LCQ Deca XP",
            "LCQ DUO",
            "LCQ",
            "TSQ Quantum Discovery",
            "TSQ",
        ] {
            assert_eq!(instrument_term(model), None, "{model}");
        }
        assert_eq!(
            instrument_term("Orbitrap Astral"),
            Some(("MS:1003378", "Orbitrap Astral"))
        );
    }

    #[test]
    fn file_name_only_strips_windows_and_posix_dirs() {
        assert_eq!(
            file_name_only(r"D:\DATA2021\Some User\run-1.raw"),
            "run-1.raw"
        );
        assert_eq!(file_name_only("F:/Methods/x/method.meth"), "method.meth");
        assert_eq!(file_name_only("plain.raw"), "plain.raw");
        assert_eq!(file_name_only(""), "");
    }

    #[test]
    fn software_version_tracks_crate_version() {
        assert_eq!(SOFTWARE_VERSION, env!("CARGO_PKG_VERSION"));
    }
}
