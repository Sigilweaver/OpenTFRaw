//! Scan filter string builder.
//!
//! A scan filter is a single-line textual summary of a scan's acquisition
//! parameters. Downstream proteomics tools use it as a key for correlating
//! identifications back to source scans.
//!
//! ## Grammar
//!
//! ```text
//! <analyzer> <polarity> <scan_mode> <ionization> [sid=<eV>] [d] <scan_type>
//!   [lock] ms<n>  [<precursor>@<method><energy> ...]  [<low>-<high>]
//! ```
//!
//! Token order follows the filter line rendered by the `stringify` methods
//! of the Finnigan Perl module (Gene Selkov, release 0.0206 on CPAN,
//! <https://metacpan.org/dist/Finnigan>):
//!
//! - `lib/Finnigan/ScanEventPreamble.pm`, `stringify`: analyzer, polarity,
//!   scan mode, ionization, dependent flag `d`, scan type, `ms<n>`
//!   (<https://metacpan.org/release/SELKOVJR/Finnigan-0.0206/source/lib/Finnigan/ScanEventPreamble.pm#L558>)
//! - `lib/Finnigan/ScanEvent.pm`, `stringify`: preamble, then precursors,
//!   then the m/z range
//!   (<https://metacpan.org/release/SELKOVJR/Finnigan-0.0206/source/lib/Finnigan/ScanEvent.pm#L197>)
//! - `lib/Finnigan/Reaction.pm`, `stringify`: `<precursor>@<method><energy>`
//!   (<https://metacpan.org/release/SELKOVJR/Finnigan-0.0206/source/lib/Finnigan/Reaction.pm#L36>)
//! - `lib/Finnigan/FractionCollector.pm`, `stringify`: `[<low>-<high>]`
//!   (<https://metacpan.org/release/SELKOVJR/Finnigan-0.0206/source/lib/Finnigan/FractionCollector.pm#L30>)
//!
//! ## Project conventions
//!
//! The following are this project's conventions, not taken from the Finnigan
//! module:
//!
//! - Numeric precision: precursor m/z and the m/z range use 4 decimals,
//!   energies use 2 decimals. SRM filters (built in
//!   [`crate::RawFileReader::scan_filter`]) use 3 decimals for Q1 and the Q3
//!   windows.
//! - Activation code 4 renders as `hcd` on an FTMS analyzer and `cid`
//!   otherwise (see [`activation_str`]).
//! - An EThcD event (see [`crate::scan_event::ScanEvent::activation`])
//!   renders a single `<precursor>@etd@hcd<energy>` clause.
//! - An ETD event renders `@etd` followed by the stored reaction value. On
//!   the corpus ETD files (PRIDE PXD006767) that value equals the method's
//!   activation time in ms, not a collision energy.
//! - SRM filters always start with a fixed `+` polarity, because those files
//!   have no scan event to read polarity from.
//! - Multiple precursors are separated by a space.
//! - `sid=<eV>` is rendered after ionization when a positive source CID
//!   energy is known.
//! - `lock` is rendered between the scan type and `ms<n>` when the scan's
//!   trailer reports at least one matched lock mass (`Number of LM Found:`
//!   greater than zero). The position follows the example in a comment in
//!   `ScanEventPreamble.pm` `stringify` of the Finnigan module
//!   (`FTMS {1;1}  + p ESI Full lock ms [60.00-1200.00]`, same URL as
//!   above). A scan that reports zero matches renders no `lock`, including
//!   one that keeps the previous scan's nonzero correction; whether such a
//!   scan should render it is not settled (README, "Known issues and gaps").
//!   The configured count (`Number of Lock Masses:`) alone
//!   never renders `lock`.
//! - Missing values fall back to `MS` for the analyzer, `+` for polarity and
//!   `Full` for the scan type; an unknown activation code omits the
//!   `@<method>` clause.
//!
//! ## Examples
//!
//! - `FTMS + p NSI Full ms [350.0000-1500.0000]`
//! - `FTMS + p NSI Full lock ms [350.0000-1500.0000]`
//! - `FTMS + c NSI d Full ms2 645.8311@hcd28.00 [150.0000-2000.0000]`
//! - `ITMS + c NSI d Full ms2 520.2400@cid35.00 [135.0000-1060.0000]`
//! - `ITMS + c NSI d Full ms3 810.5000@cid35.00 265.2700@cid35.00 [100.0000-1000.0000]` (MS3)
use crate::scan_event::ScanEvent;
use crate::scan_index::ScanIndexEntry;
use crate::types::{Activation, Analyzer, MsPower, ScanType};

/// Resolve the activation filter-string token for a given activation and
/// analyzer. Code 1 renders as "hcd". Code 4 renders as "hcd" on an FTMS
/// analyzer and "cid" on any other analyzer; this is a project convention.
/// ETD renders as "etd" and EThcD as "ethcd" (the filter builder writes
/// EThcD as `@etd@hcd`).
pub fn activation_str(analyzer: Option<Analyzer>, act: Activation) -> &'static str {
    match act {
        Activation::CID => match analyzer {
            Some(Analyzer::FTMS) => "hcd",
            _ => "cid",
        },
        other => other.as_str(),
    }
}

/// Whether a scan's trailer reports a found lock mass, which is the only
/// condition that renders the `lock` token. The matched count is read
/// strictly: a missing, mistyped or negative count, a configured count alone
/// and a nonzero lock-mass correction do not qualify.
pub(crate) fn lock_mass_matched(params: &crate::ScanParams<'_>) -> bool {
    params
        .number_of_matched_lock_masses()
        .is_some_and(|n| n > 0)
}

/// Build the scan filter string for a single scan event, in the grammar
/// described in the module documentation.
///
/// - `event` - the scan event record (provides analyzer, polarity, activation, etc.)
/// - `index_entry` - provides the authoritative m/z scan window
/// - `precursor_mz` - final-stage precursor m/z from scan_params `Monoisotopic M/Z:`
/// - `activation_energy` - primary activation energy from scan_params, as
///   returned by [`crate::ScanParams::activation_energy`] (NCE when
///   an NCE label is present, otherwise eV)
///
/// For MS2+ scans the function first attempts to build the full precursor chain
/// from `event.reactions` (populated for both pre-v66 and v66 files). If reactions
/// are empty, it falls back to the single `precursor_mz` / `activation_energy` pair.
#[cfg(test)]
pub(crate) fn build_filter(
    event: &ScanEvent,
    index_entry: &ScanIndexEntry,
    precursor_mz: Option<f64>,
    activation_energy: Option<f64>,
) -> String {
    build_filter_with_source_cid(
        event,
        index_entry,
        precursor_mz,
        activation_energy,
        None,
        false,
    )
}

/// Build a filter with an explicitly sourced energy. Zero does not establish
/// that source CID is off, so it is omitted rather than rendered as `!sid`.
///
/// `lock_mass_matched` is true when the scan's trailer reports at least one
/// matched lock mass (see [`crate::ScanParams::number_of_matched_lock_masses`]);
/// it renders the `lock` token between the scan type and `ms<n>`.
pub(crate) fn build_filter_with_source_cid(
    event: &ScanEvent,
    index_entry: &ScanIndexEntry,
    precursor_mz: Option<f64>,
    activation_energy: Option<f64>,
    source_cid_energy_ev: Option<f64>,
    lock_mass_matched: bool,
) -> String {
    let mut out = String::with_capacity(96);
    let p = &event.preamble;
    let analyzer = p.analyzer();

    // Analyzer (FTMS, ITMS, TQMS, ...)
    if let Some(a) = analyzer {
        out.push_str(a.as_str());
    } else {
        out.push_str("MS");
    }
    out.push(' ');

    // Polarity
    if let Some(pol) = p.polarity() {
        out.push(pol.symbol());
    } else {
        out.push('+');
    }
    out.push(' ');

    // Scan mode (c/p)
    if let Some(m) = p.scan_mode() {
        out.push(m.symbol());
        out.push(' ');
    }

    // Ionization
    if let Some(ion) = p.ionization() {
        out.push_str(ion.as_str());
        out.push(' ');
    }

    // Source CID follows the ionization tokens in the public filter grammar.
    if let Some(energy) = source_cid_energy_ev.filter(|e| e.is_finite() && *e > 0.0) {
        out.push_str(&format!("sid={energy:.2} "));
    }

    // Dependent-scan flag
    if p.is_dependent() {
        out.push_str("d ");
    }

    // Scan type
    match p.scan_type() {
        Some(ScanType::Full) => out.push_str("Full"),
        Some(ScanType::Zoom) => out.push('Z'),
        Some(ScanType::Sim) => out.push_str("SIM"),
        Some(ScanType::Srm) => out.push_str("SRM"),
        Some(ScanType::Crm) => out.push_str("CRM"),
        Some(ScanType::Q1) => out.push_str("Q1MS"),
        Some(ScanType::Q3) => out.push_str("Q3MS"),
        _ => out.push_str("Full"),
    }
    out.push(' ');

    // Lock-mass token, between the scan type and the MS level.
    if lock_mass_matched {
        out.push_str("lock ");
    }

    // ms<n> level
    let n = match p.ms_power() {
        Some(MsPower::Ms1) | Some(MsPower::Undefined) | None => 1,
        Some(MsPower::Ms2) => 2,
        Some(MsPower::Ms3) => 3,
        Some(MsPower::Ms4) => 4,
        Some(MsPower::Ms5) => 5,
        Some(MsPower::Ms6) => 6,
        Some(MsPower::Ms7) => 7,
        Some(MsPower::Ms8) => 8,
    };
    if n == 1 {
        out.push_str("ms");
    } else {
        out.push_str(&format!("ms{n}"));
    }

    // Precursor chain (only for MSn scans)
    if n >= 2 {
        // An unrecognised activation code gets no `@<method>` clause.
        let act = event
            .activation()
            .filter(|a| !matches!(a, Activation::Unknown(_)));
        let reactions = &event.reactions;

        if act == Some(Activation::EThcD) {
            // Eclipse / Fusion Lumos EThcD: one valid precursor, two-clause filter.
            // activation_energy from scan_params "HCD Energy:" = supplemental HCD NCE%.
            if let Some(rx) = reactions.iter().find(|r| r.precursor_mz > 0.0) {
                out.push(' ');
                out.push_str(&format!("{:.4}", rx.precursor_mz));
                out.push('@');
                out.push_str("etd");
                out.push('@');
                out.push_str("hcd");
                if let Some(e) = activation_energy {
                    out.push_str(&format!("{e:.2}"));
                }
            }
        } else if !reactions.is_empty() {
            // Full precursor chain from parsed reactions.
            // For MS3: reactions[0] = parent MS2 precursor, reactions[1] = MS3 precursor.
            // Each intermediate reaction uses its stored energy; the final reaction uses
            // activation_energy from scan_params when available (more accurate label).
            let last = reactions.len() - 1;
            for (i, rx) in reactions.iter().enumerate() {
                let mz = rx.precursor_mz;
                if mz <= 0.0 {
                    continue;
                }
                out.push(' ');
                out.push_str(&format!("{mz:.4}"));
                if let Some(a) = act {
                    out.push('@');
                    let is_last = i == last;
                    out.push_str(activation_str(analyzer, a));
                    let energy = if is_last {
                        activation_energy.or({
                            if rx.energy > 0.0 {
                                Some(rx.energy)
                            } else {
                                None
                            }
                        })
                    } else {
                        if rx.energy > 0.0 {
                            Some(rx.energy)
                        } else {
                            activation_energy
                        }
                    };
                    if let Some(e) = energy {
                        out.push_str(&format!("{e:.2}"));
                    }
                }
            }
        } else if let Some(mz) = precursor_mz {
            // Fallback: single precursor from scan_params.
            out.push(' ');
            out.push_str(&format!("{mz:.4}"));
            if let Some(a) = act {
                out.push('@');
                out.push_str(activation_str(analyzer, a));
                if let Some(e) = activation_energy {
                    out.push_str(&format!("{e:.2}"));
                }
            }
        }
    }

    // Scan range [low-high]
    if index_entry.low_mz.is_finite()
        && index_entry.high_mz.is_finite()
        && index_entry.low_mz > 0.0
        && index_entry.high_mz > index_entry.low_mz
    {
        out.push(' ');
        out.push_str(&format!(
            "[{:.4}-{:.4}]",
            index_entry.low_mz, index_entry.high_mz
        ));
    } else if let Some(fc) = event.fraction_collectors.first() {
        if fc.low_mz > 0.0 && fc.high_mz > fc.low_mz {
            out.push(' ');
            out.push_str(&format!("[{:.4}-{:.4}]", fc.low_mz, fc.high_mz));
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scan_event::{FractionCollector, Reaction, ScanEvent, ScanEventPreamble};
    use crate::scan_index::ScanIndexEntry;

    fn make_preamble(
        polarity: u8,
        scan_mode: u8,
        ms_power: u8,
        scan_type: u8,
    ) -> ScanEventPreamble {
        let mut bytes = vec![0u8; 136];
        bytes[4] = polarity;
        bytes[5] = scan_mode;
        bytes[6] = ms_power;
        bytes[7] = scan_type;
        bytes[11] = 5; // NSI
        bytes[40] = 4; // FTMS
        ScanEventPreamble { bytes }
    }

    fn make_index(low: f64, high: f64) -> ScanIndexEntry {
        ScanIndexEntry {
            index: 0,
            scan_event: 0,
            scan_segment: 0,
            data_size: 0,
            start_time: 0.0,
            total_current: 0.0,
            base_intensity: 0.0,
            base_mz: 0.0,
            low_mz: low,
            high_mz: high,
            offset: 0,
        }
    }

    #[test]
    fn lock_token_sits_between_scan_type_and_ms_level() {
        let ms1 = ScanEvent {
            preamble: make_preamble(1, 1, 1, 0),
            reactions: vec![],
            fraction_collectors: vec![],
            coefficients: vec![],
        };
        let idx = make_index(350.0, 1500.0);
        assert_eq!(
            build_filter_with_source_cid(&ms1, &idx, None, None, None, true),
            "FTMS + p NSI Full lock ms [350.0000-1500.0000]"
        );
        assert_eq!(
            build_filter_with_source_cid(&ms1, &idx, None, None, None, false),
            "FTMS + p NSI Full ms [350.0000-1500.0000]"
        );
        // After `sid=` and `d`, before `ms2`.
        let mut preamble = make_preamble(1, 1, 2, 0);
        preamble.bytes[10] = 1;
        let ms2 = ScanEvent {
            preamble,
            reactions: vec![],
            fraction_collectors: vec![],
            coefficients: vec![],
        };
        assert_eq!(
            build_filter_with_source_cid(&ms2, &idx, None, None, Some(20.0), true),
            "FTMS + p NSI sid=20.00 d Full lock ms2 [350.0000-1500.0000]"
        );
    }

    #[test]
    fn only_a_positive_matched_count_renders_lock() {
        use crate::generic_data::{GenericRecord, GenericValue};
        use crate::ScanParams;
        let rec = |fields: &[(&str, GenericValue)]| {
            GenericRecord::new(
                fields
                    .iter()
                    .map(|(k, v)| (k.to_string(), v.clone()))
                    .collect(),
            )
        };
        let matched = |n| ("Number of LM Found:", GenericValue::Int32(n));
        let configured = |n| ("Number of Lock Masses:", GenericValue::Int32(n));
        let correction = ("LM Correction (ppm):", GenericValue::Float64(1.5));
        let cases = [
            (rec(&[matched(1), configured(9), correction.clone()]), true),
            (rec(&[matched(2), configured(1)]), true),
            (rec(&[matched(1)]), true),
            // Zero matches that keep an earlier nonzero correction: undecided,
            // rendered without `lock`.
            (rec(&[matched(0), configured(1), correction.clone()]), false),
            (rec(&[matched(0), configured(0)]), false),
            // Configuration alone is not a found lock mass.
            (rec(&[configured(9)]), false),
            (rec(&[configured(9), correction.clone()]), false),
            (rec(&[matched(-1), configured(9)]), false),
            (
                rec(&[("Number of LM Found:", GenericValue::String("1".into()))]),
                false,
            ),
            (rec(&[]), false),
        ];
        for (record, want) in &cases {
            assert_eq!(lock_mass_matched(&ScanParams::new(record)), *want);
        }
    }

    #[test]
    fn source_cid_token_placement_and_unknown_values() {
        let ev = ScanEvent {
            preamble: make_preamble(0, 1, 1, 2),
            reactions: vec![],
            fraction_collectors: vec![],
            coefficients: vec![],
        };
        let idx = make_index(100.0, 200.0);
        assert_eq!(
            build_filter_with_source_cid(&ev, &idx, None, None, Some(20.0), false),
            "FTMS - p NSI sid=20.00 SIM ms [100.0000-200.0000]"
        );
        for energy in [
            None,
            Some(0.0),
            Some(-1.0),
            Some(f64::NAN),
            Some(f64::INFINITY),
        ] {
            assert_eq!(
                build_filter_with_source_cid(&ev, &idx, None, None, energy, false),
                build_filter(&ev, &idx, None, None)
            );
        }
    }

    #[test]
    fn ms1_full_profile() {
        let ev = ScanEvent {
            preamble: make_preamble(1, 1, 1, 0),
            reactions: vec![],
            fraction_collectors: vec![FractionCollector {
                low_mz: 350.0,
                high_mz: 1500.0,
            }],
            coefficients: vec![],
        };
        let idx = make_index(350.0, 1500.0);
        let s = build_filter(&ev, &idx, None, None);
        assert_eq!(s, "FTMS + p NSI Full ms [350.0000-1500.0000]");
    }

    #[test]
    fn ms2_centroid_hcd_scan_params() {
        let mut pre = make_preamble(1, 0, 2, 0);
        pre.bytes[10] = 1; // dependent
        pre.bytes[24] = 1; // HCD (code 1)
        let ev = ScanEvent {
            preamble: pre,
            reactions: vec![],
            fraction_collectors: vec![FractionCollector {
                low_mz: 150.0,
                high_mz: 2000.0,
            }],
            coefficients: vec![],
        };
        let idx = make_index(150.0, 2000.0);
        let s = build_filter(&ev, &idx, Some(645.8311), Some(28.0));
        assert_eq!(
            s,
            "FTMS + c NSI d Full ms2 645.8311@hcd28.00 [150.0000-2000.0000]"
        );
    }

    #[test]
    fn ms2_reactions_chain() {
        let mut pre = make_preamble(1, 0, 2, 0);
        pre.bytes[10] = 1; // dependent
        pre.bytes[40] = 0; // ITMS
        pre.bytes[24] = 4; // CID
        let rxn = Reaction {
            precursor_mz: 440.254,
            energy: 35.0,
        };
        let ev = ScanEvent {
            preamble: pre,
            reactions: vec![rxn],
            fraction_collectors: vec![FractionCollector {
                low_mz: 116.0,
                high_mz: 892.0,
            }],
            coefficients: vec![],
        };
        let idx = make_index(116.0, 892.0);
        let s = build_filter(&ev, &idx, Some(440.254), Some(35.0));
        // ITMS + CID code 4 → "cid"
        assert_eq!(
            s,
            "ITMS + c NSI d Full ms2 440.2540@cid35.00 [116.0000-892.0000]"
        );
    }

    #[test]
    fn ms3_chain() {
        let mut pre = make_preamble(1, 0, 3, 0);
        pre.bytes[10] = 1; // dependent
        pre.bytes[40] = 0; // ITMS
        pre.bytes[24] = 4; // CID
        let ev = ScanEvent {
            preamble: pre,
            reactions: vec![
                Reaction {
                    precursor_mz: 810.50,
                    energy: 35.0,
                },
                Reaction {
                    precursor_mz: 265.27,
                    energy: 35.0,
                },
            ],
            fraction_collectors: vec![FractionCollector {
                low_mz: 100.0,
                high_mz: 1000.0,
            }],
            coefficients: vec![],
        };
        let idx = make_index(100.0, 1000.0);
        let s = build_filter(&ev, &idx, Some(265.27), Some(35.0));
        assert_eq!(
            s,
            "ITMS + c NSI d Full ms3 810.5000@cid35.00 265.2700@cid35.00 [100.0000-1000.0000]"
        );
    }

    #[test]
    fn unknown_activation_omits_method_clause() {
        let mut pre = make_preamble(1, 0, 2, 0);
        pre.bytes[10] = 1; // dependent
        pre.bytes[40] = 4; // FTMS
        pre.bytes[24] = 12; // no established meaning
        let ev = ScanEvent {
            preamble: pre,
            reactions: vec![],
            fraction_collectors: vec![FractionCollector {
                low_mz: 150.0,
                high_mz: 2000.0,
            }],
            coefficients: vec![],
        };
        let idx = make_index(150.0, 2000.0);
        assert_eq!(ev.preamble.activation(), Some(Activation::Unknown(12)));
        let s = build_filter(&ev, &idx, Some(649.12), Some(35.0));
        assert_eq!(s, "FTMS + c NSI d Full ms2 649.1200 [150.0000-2000.0000]");
    }
    /// Preamble of a PXD006767 Orbitrap Velos Pro ETD MS2 event: ITMS,
    /// byte 24 = 4, byte 120 = 1.
    fn etd_preamble(len: usize) -> ScanEventPreamble {
        let mut pre = make_preamble(1, 0, 2, 0);
        pre.bytes.resize(len, 0);
        pre.bytes[10] = 1; // dependent
        pre.bytes[40] = 0; // ITMS
        pre.bytes[24] = 4;
        pre.bytes[120] = 1;
        pre
    }

    #[test]
    fn etd_flag_renders_etd_clause() {
        let ev = ScanEvent {
            preamble: etd_preamble(136),
            reactions: vec![Reaction {
                precursor_mz: 467.1021,
                energy: 130.0,
            }],
            fraction_collectors: vec![],
            coefficients: vec![],
        };
        assert_eq!(ev.preamble.activation(), Some(Activation::ETD));
        assert_eq!(ev.activation(), Some(Activation::ETD));
        let idx = make_index(50.0, 1415.0);
        assert_eq!(
            build_filter(&ev, &idx, Some(467.1021), None),
            "ITMS + c NSI d Full ms2 467.1021@etd130.00 [50.0000-1415.0000]"
        );
    }

    #[test]
    fn etd_flag_needs_v66_msn_and_a_named_code() {
        // Version 63/64 preamble: byte 120 is not read.
        assert_eq!(etd_preamble(128).activation(), Some(Activation::CID));
        // MS1 event.
        let mut pre = etd_preamble(136);
        pre.bytes[6] = 1;
        assert_eq!(pre.activation(), Some(Activation::CID));
        // Byte 24 zero or unnamed, as on misread events.
        let mut pre = etd_preamble(136);
        pre.bytes[24] = 0;
        assert_eq!(pre.activation(), None);
        pre.bytes[24] = 11;
        assert_eq!(pre.activation(), Some(Activation::Unknown(11)));
        // Byte 120 = 2 (Q Exactive / Exploris / Astral) is not ETD.
        let mut pre = etd_preamble(136);
        pre.bytes[120] = 2;
        assert_eq!(pre.activation(), Some(Activation::CID));
    }

    #[test]
    fn ethcd_event_renders_two_clause_filter() {
        let mut pre = etd_preamble(136);
        pre.bytes[40] = 4; // FTMS
        let ev = ScanEvent {
            preamble: pre,
            reactions: vec![
                Reaction {
                    precursor_mz: 507.8661,
                    energy: 95.83,
                },
                Reaction {
                    precursor_mz: 0.0,
                    energy: 0.0,
                },
            ],
            fraction_collectors: vec![],
            coefficients: vec![],
        };
        assert_eq!(ev.preamble.activation(), Some(Activation::ETD));
        assert_eq!(ev.activation(), Some(Activation::EThcD));
        let idx = make_index(110.0, 1026.0);
        assert_eq!(
            build_filter(&ev, &idx, Some(507.8661), Some(25.0)),
            "FTMS + c NSI d Full ms2 507.8661@etd@hcd25.00 [110.0000-1026.0000]"
        );
    }

    #[test]
    fn two_reactions_without_etd_flag_are_not_ethcd() {
        let mut pre = make_preamble(1, 0, 2, 0);
        pre.bytes[10] = 1;
        pre.bytes[24] = 4;
        let ev = ScanEvent {
            preamble: pre,
            reactions: vec![
                Reaction {
                    precursor_mz: 507.8661,
                    energy: 30.0,
                },
                Reaction {
                    precursor_mz: 0.0,
                    energy: 0.0,
                },
            ],
            fraction_collectors: vec![],
            coefficients: vec![],
        };
        assert_eq!(ev.activation(), Some(Activation::CID));
        let idx = make_index(110.0, 1026.0);
        assert_eq!(
            build_filter(&ev, &idx, Some(507.8661), Some(30.0)),
            "FTMS + c NSI d Full ms2 507.8661@hcd30.00 [110.0000-1026.0000]"
        );
    }
}
