/// Scan filter string builder — reproduces Thermo's canonical scan filter
/// syntax for interoperability with downstream tools.
///
/// A Thermo scan filter is a single-line textual summary of a scan's
/// acquisition parameters. It is consumed by virtually every proteomics
/// tool (Proteome Discoverer, MSFragger, MaxQuant, DIA-NN, Skyline,
/// pyteomics, ...) and is the natural key for correlating peptide
/// identifications back to source scans.
///
/// ## Grammar
///
/// ```text
/// <analyzer> <polarity> <scan_mode> <ionization> [<dependent>] <scan_type>
///   ms<n>  [<precursor>@<method><energy> ...]  [<range>]
/// ```
///
/// ## Examples (verified against Thermo output)
///
/// - `FTMS + p NSI Full ms [350.0000-1500.0000]`
/// - `FTMS + c NSI d Full ms2 645.8311@hcd28.00 [150.0000-2000.0000]`
/// - `ITMS + c NSI d Full ms2 520.2400@cid35.00 [135.0000-1060.0000]`
/// - `FTMS + c NSI d Full ms2 649.1234@etd35.00@hcd28.00 [150.0000-2000.0000]` (EThcD)
/// - `ITMS + c NSI d Full ms3 810.50@cid35.00 265.27@cid35.00 [100.0000-1000.0000]` (MS3)
use crate::scan_event::ScanEvent;
use crate::scan_index::ScanIndexEntry;
use crate::types::{Activation, Analyzer, MsPower, ScanType};

/// Resolve the activation filter-string token for a given activation code and
/// analyzer. On FTMS instruments, both `CID` (code 4) and `HCD` (code 1) render
/// as "hcd" because they are all beam-type collisions; on ITMS, code 4 renders
/// as "cid".
pub fn activation_str(analyzer: Option<Analyzer>, act: Activation) -> &'static str {
    match act {
        Activation::CID => match analyzer {
            Some(Analyzer::FTMS) => "hcd",
            _ => "cid",
        },
        other => other.as_str(),
    }
}

/// Build the canonical Thermo scan filter string for a single scan event.
///
/// - `event` — the scan event record (provides analyzer, polarity, activation, etc.)
/// - `index_entry` — provides the authoritative m/z scan window
/// - `precursor_mz` — final-stage precursor m/z from scan_params `Monoisotopic M/Z:`
/// - `activation_energy` — primary activation energy (eV or NCE %) from scan_params
/// - `supplemental_energy` — supplemental HCD energy for EThcD scans; `None` for all other types
///
/// For MS2+ scans the function first attempts to build the full precursor chain
/// from `event.reactions` (populated for both pre-v66 and v66 files). If reactions
/// are empty, it falls back to the single `precursor_mz` / `activation_energy` pair.
pub fn build_filter(
    event: &ScanEvent,
    index_entry: &ScanIndexEntry,
    precursor_mz: Option<f64>,
    activation_energy: Option<f64>,
    supplemental_energy: Option<f64>,
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

    // Dependent-scan flag
    if p.is_dependent() {
        out.push_str("d ");
    }

    // Scan type
    match p.scan_type() {
        Some(ScanType::Full) => out.push_str("Full"),
        Some(ScanType::Zoom) => out.push_str("Z"),
        Some(ScanType::Sim) => out.push_str("SIM"),
        Some(ScanType::Srm) => out.push_str("SRM"),
        Some(ScanType::Crm) => out.push_str("CRM"),
        Some(ScanType::Q1) => out.push_str("Q1MS"),
        Some(ScanType::Q3) => out.push_str("Q3MS"),
        _ => out.push_str("Full"),
    }
    out.push(' ');

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
        let act = p.activation();
        let reactions = &event.reactions;

        // Detect EThcD from tribrid instruments (Eclipse, Fusion Lumos).
        // These encode EThcD as CID/HCD activation (byte 24 = 4 or 1) with
        // n_reactions = 2 in the body, where only the first reaction has a
        // valid non-zero precursor m/z.  The preamble activation byte is
        // never EThcD (12) for these instruments.
        let n_valid_precursors = reactions
            .iter()
            .filter(|r| r.precursor_mz > 0.0)
            .count();
        let is_tribrid_ethcd = n == 2
            && reactions.len() >= 2
            && n_valid_precursors == 1
            && matches!(analyzer, Some(Analyzer::FTMS))
            && matches!(
                act,
                Some(Activation::CID) | Some(Activation::HCD)
            );

        if is_tribrid_ethcd {
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
                    let is_ethcd = a == Activation::EThcD;

                    // EThcD: final precursor gets two clauses (@etd<e>@hcd<se>).
                    if is_last && is_ethcd {
                        out.push_str("etd");
                        if let Some(e) = activation_energy {
                            out.push_str(&format!("{e:.2}"));
                        }
                        out.push('@');
                        out.push_str("hcd");
                        if let Some(se) = supplemental_energy {
                            out.push_str(&format!("{se:.2}"));
                        }
                    } else {
                        let astr = activation_str(analyzer, a);
                        out.push_str(astr);
                        let energy = if is_last {
                            activation_energy.or_else(|| {
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
            }
        } else if let Some(mz) = precursor_mz {
            // Fallback: single precursor from scan_params.
            out.push(' ');
            out.push_str(&format!("{mz:.4}"));
            if let Some(a) = act {
                out.push('@');
                if a == Activation::EThcD {
                    out.push_str("etd");
                    if let Some(e) = activation_energy {
                        out.push_str(&format!("{e:.2}"));
                    }
                    out.push('@');
                    out.push_str("hcd");
                    if let Some(se) = supplemental_energy {
                        out.push_str(&format!("{se:.2}"));
                    }
                } else {
                    out.push_str(activation_str(analyzer, a));
                    if let Some(e) = activation_energy {
                        out.push_str(&format!("{e:.2}"));
                    }
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
        let s = build_filter(&ev, &idx, None, None, None);
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
        let s = build_filter(&ev, &idx, Some(645.8311), Some(28.0), None);
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
            unknown_double: 1.0,
            energy: 35.0,
            unknown_long1: 0,
            unknown_long2: 0,
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
        let s = build_filter(&ev, &idx, Some(440.254), Some(35.0), None);
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
                    unknown_double: 1.0,
                    energy: 35.0,
                    unknown_long1: 0,
                    unknown_long2: 0,
                },
                Reaction {
                    precursor_mz: 265.27,
                    unknown_double: 1.0,
                    energy: 35.0,
                    unknown_long1: 0,
                    unknown_long2: 0,
                },
            ],
            fraction_collectors: vec![FractionCollector {
                low_mz: 100.0,
                high_mz: 1000.0,
            }],
            coefficients: vec![],
        };
        let idx = make_index(100.0, 1000.0);
        let s = build_filter(&ev, &idx, Some(265.27), Some(35.0), None);
        assert_eq!(
            s,
            "ITMS + c NSI d Full ms3 810.5000@cid35.00 265.2700@cid35.00 [100.0000-1000.0000]"
        );
    }

    #[test]
    fn ethcd_filter() {
        let mut pre = make_preamble(1, 0, 2, 0);
        pre.bytes[10] = 1; // dependent
        pre.bytes[40] = 4; // FTMS
        pre.bytes[24] = 12; // EThcD
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
        let s = build_filter(&ev, &idx, Some(649.12), Some(35.0), Some(28.0));
        assert_eq!(
            s,
            "FTMS + c NSI d Full ms2 649.1200@etd35.00@hcd28.00 [150.0000-2000.0000]"
        );
    }
}
