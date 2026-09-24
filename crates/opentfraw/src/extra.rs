//! Reader-specific per-scan values for `openmassspec_core::SpectrumRecord::extra`.
//!
//! Every value OpenTFRaw decodes for a scan that has no first-class field in
//! `openmassspec_core` is registered here under a stable `opentfraw.*` key
//! (the `<reader>.<field>` convention `openmassspec_core` asks for). The keys
//! are normalized across instrument families: `opentfraw.resolution` is read
//! from `"Orbitrap Resolution:"` or `"FT Resolution:"`, whichever the file
//! carries. Values are rendered as strings; a field the scan does not carry is
//! omitted.
//!
//! Each entry becomes a `<userParam>` on the spectrum in mzML, so callers that
//! want leaner output can pick fields with [`ExtraFields`].

use std::collections::BTreeMap;

use crate::mzml::ScanMetadata;
use crate::reader::{ScanParams, StatusLogEntry};
use crate::RawFileReader;

/// Which `opentfraw.*` extra fields to emit. Keys are the full names returned
/// by [`extra_field_keys`]; keys not in that list are ignored.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum ExtraFields {
    /// Every registered field (the default).
    #[default]
    All,
    /// No extra fields.
    None,
    /// Only the listed fields.
    Only(Vec<String>),
    /// Every registered field except the listed ones.
    Except(Vec<String>),
}

impl ExtraFields {
    /// Whether the field `key` is selected.
    pub fn includes(&self, key: &str) -> bool {
        match self {
            Self::All => true,
            Self::None => false,
            Self::Only(keys) => keys.iter().any(|k| k == key),
            Self::Except(keys) => !keys.iter().any(|k| k == key),
        }
    }
}

/// The values an extra field is read from.
struct Scan<'a> {
    meta: &'a ScanMetadata,
    params: Option<ScanParams<'a>>,
    status: Option<StatusLogEntry<'a>>,
}

type Getter = fn(&Scan<'_>) -> Option<String>;

fn param<T: ToString>(
    scan: &Scan<'_>,
    get: impl Fn(&ScanParams<'_>) -> Option<T>,
) -> Option<String> {
    scan.params.as_ref().and_then(get).map(|v| v.to_string())
}

fn status<T: ToString>(
    scan: &Scan<'_>,
    get: impl Fn(&StatusLogEntry<'_>) -> Option<T>,
) -> Option<String> {
    scan.status.as_ref().and_then(get).map(|v| v.to_string())
}

fn sps_masses(p: &ScanParams<'_>) -> Option<String> {
    let masses: Vec<String> = (0..)
        .map_while(|channel| p.sps_mass(channel))
        .filter(|&m| m > 0.0)
        .map(|m| m.to_string())
        .collect();
    (!masses.is_empty()).then(|| masses.join(" "))
}

fn possible_charge_states(p: &ScanParams<'_>) -> Option<String> {
    let charges = p.possible_charge_states()?;
    Some(
        charges
            .iter()
            .map(u32::to_string)
            .collect::<Vec<_>>()
            .join(" "),
    )
}

/// Every registered extra field: its key and how it is read. Fields that
/// `openmassspec_core::SpectrumRecord` already carries (injection time,
/// charge, precursor m/z, isolation window, collision energy, FAIMS CV,
/// master scan, scan event) are not repeated here.
const EXTRA_FIELDS: &[(&str, Getter)] = &[
    // Scan event and scan index.
    ("opentfraw.is_dia", |s| Some(s.meta.is_dia.to_string())),
    ("opentfraw.is_wideband", |s| {
        Some(s.meta.is_wideband.to_string())
    }),
    ("opentfraw.scan_segment", |s| {
        (s.meta.scan_segment != u16::MAX).then(|| s.meta.scan_segment.to_string())
    }),
    ("opentfraw.data_size", |s| {
        Some(s.meta.data_size.to_string())
    }),
    // Trailer (scan parameters).
    ("opentfraw.resolution", |s| {
        param(s, |p| p.orbitrap_resolution())
    }),
    ("opentfraw.micro_scan_count", |s| {
        param(s, |p| p.micro_scan_count())
    }),
    ("opentfraw.agc_enabled", |s| param(s, |p| p.agc_enabled())),
    ("opentfraw.agc_target", |s| param(s, |p| p.agc_target())),
    ("opentfraw.agc_fill", |s| param(s, |p| p.agc_fill())),
    ("opentfraw.max_ion_time_ms", |s| {
        param(s, |p| p.max_ion_time_ms())
    }),
    ("opentfraw.elapsed_scan_time_s", |s| {
        param(s, |p| p.elapsed_scan_time_s())
    }),
    ("opentfraw.lock_mass_correction_ppm", |s| {
        param(s, |p| p.lock_mass_correction_ppm())
    }),
    ("opentfraw.number_of_lock_masses", |s| {
        param(s, |p| p.number_of_lock_masses())
    }),
    ("opentfraw.supplemental_activation_energy", |s| {
        param(s, |p| p.supplemental_activation_energy())
    }),
    ("opentfraw.hcd_energy", |s| {
        param(s, |p| p.hcd_energy().map(str::to_owned))
    }),
    ("opentfraw.possible_charge_states", |s| {
        s.params.as_ref().and_then(possible_charge_states)
    }),
    ("opentfraw.isotopic_fit_error", |s| {
        param(s, |p| p.isotopic_fit_error())
    }),
    ("opentfraw.sps_masses", |s| {
        s.params.as_ref().and_then(sps_masses)
    }),
    ("opentfraw.faims_voltage_on", |s| {
        param(s, |p| p.faims_voltage_on())
    }),
    ("opentfraw.s_lens_rf_level", |s| {
        param(s, |p| p.s_lens_rf_level())
    }),
    ("opentfraw.analyzer_temperature", |s| {
        param(s, |p| p.analyzer_temperature())
    }),
    ("opentfraw.ps_injection_time_ms", |s| {
        param(s, |p| p.ps_injection_time_ms())
    }),
    ("opentfraw.reagent_ion_injection_time_ms", |s| {
        param(s, |p| p.reagent_ion_injection_time_ms())
    }),
    ("opentfraw.reagent_ion_agc", |s| {
        param(s, |p| p.reagent_ion_agc())
    }),
    ("opentfraw.source_cid_energy_ev", |s| {
        param(s, |p| p.source_cid_energy_ev())
    }),
    ("opentfraw.dynamic_rt_shift_min", |s| {
        param(s, |p| p.dynamic_rt_shift_min())
    }),
    ("opentfraw.conversion_parameter_a", |s| {
        param(s, |p| p.conversion_parameter_a())
    }),
    ("opentfraw.conversion_parameter_b", |s| {
        param(s, |p| p.conversion_parameter_b())
    }),
    ("opentfraw.conversion_parameter_c", |s| {
        param(s, |p| p.conversion_parameter_c())
    }),
    ("opentfraw.raw_ovft", |s| param(s, |p| p.raw_ovft())),
    ("opentfraw.scan_description", |s| {
        param(s, |p| p.scan_description().map(str::to_owned))
    }),
    ("opentfraw.multi_inject_info", |s| {
        param(s, |p| p.multi_inject_info().map(str::to_owned))
    }),
    // Instrument status log.
    ("opentfraw.status.spray_voltage", |s| {
        status(s, |l| l.spray_voltage())
    }),
    ("opentfraw.status.capillary_temperature", |s| {
        status(s, |l| l.capillary_temperature())
    }),
    ("opentfraw.status.ion_injection_time_ms", |s| {
        status(s, |l| l.ion_injection_time_ms())
    }),
    ("opentfraw.status.resolution", |s| {
        status(s, |l| l.ft_resolution())
    }),
    ("opentfraw.status.faims_cv", |s| status(s, |l| l.faims_cv())),
    ("opentfraw.status.s_lens_rf_level", |s| {
        status(s, |l| l.s_lens_rf_level())
    }),
    ("opentfraw.status.analyzer_temperature", |s| {
        status(s, |l| l.analyzer_temperature())
    }),
    ("opentfraw.status.lock_mass_correction_ppm", |s| {
        status(s, |l| l.lock_mass_correction_ppm())
    }),
    ("opentfraw.status.number_of_lock_masses", |s| {
        status(s, |l| l.number_of_lock_masses())
    }),
];

/// Every registered `opentfraw.*` key, in emission order.
pub fn extra_field_keys() -> impl Iterator<Item = &'static str> {
    EXTRA_FIELDS.iter().map(|(key, _)| *key)
}

/// The selected extra fields for one scan, keyed by their `opentfraw.*`
/// name. Fields the scan does not carry are left out.
pub fn scan_extras(
    raw: &RawFileReader,
    meta: &ScanMetadata,
    fields: &ExtraFields,
) -> BTreeMap<String, String> {
    if *fields == ExtraFields::None {
        return BTreeMap::new();
    }
    let scan = Scan {
        meta,
        params: raw.scan_params(meta.scan_number),
        status: raw.status_log_entry(meta.scan_number),
    };
    EXTRA_FIELDS
        .iter()
        .filter(|(key, _)| fields.includes(key))
        .filter_map(|(key, get)| get(&scan).map(|value| (key.to_string(), value)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_are_unique_and_namespaced() {
        let mut keys: Vec<&str> = EXTRA_FIELDS.iter().map(|(k, _)| *k).collect();
        assert!(keys.iter().all(|k| k.starts_with("opentfraw.")));
        keys.sort_unstable();
        keys.dedup();
        assert_eq!(keys.len(), EXTRA_FIELDS.len());
    }

    #[test]
    fn selection() {
        let key = "opentfraw.resolution";
        assert!(ExtraFields::All.includes(key));
        assert!(!ExtraFields::None.includes(key));
        assert!(ExtraFields::Only(vec![key.into()]).includes(key));
        assert!(!ExtraFields::Only(vec!["opentfraw.agc_target".into()]).includes(key));
        assert!(!ExtraFields::Except(vec![key.into()]).includes(key));
        assert!(ExtraFields::Except(vec!["opentfraw.agc_target".into()]).includes(key));
    }
}
