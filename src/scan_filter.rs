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
///   [<activation>] ms<n>  [<precursor>@<method><energy>]  [<range>]
/// ```
///
/// ## Examples (verified against Thermo output)
///
/// - `FTMS + p NSI Full ms [350.0000-1500.0000]`
/// - `FTMS + c NSI d Full ms2 645.8311@hcd28.00 [150.0000-2000.0000]`
/// - `ITMS + c NSI sid=35.00 d Full ms2 520.2400@cid35.00 [135.0000-1060.0000]`
///
/// ## Usage
///
/// ```no_run
/// use opentfraw::RawFileReader;
///
/// let raw = RawFileReader::open_path("run.raw").unwrap();
/// for scan in 1..=raw.num_scans {
///     println!("{}: {}", scan, raw.scan_filter(scan).unwrap_or_default());
/// }
/// ```
use crate::scan_event::ScanEvent;
use crate::scan_index::ScanIndexEntry;
use crate::types::{Activation, MsPower, ScanType};

/// Build the canonical Thermo scan filter string for a single scan event.
///
/// `event` is the scan event record; `index_entry` provides the m/z scan
/// window (FractionCollector is also available but the index copy is
/// authoritative on Thermo). `precursor_mz` and `activation_energy` should
/// be looked up from the per-scan parameter table when the scan is an MSn
/// (ms_power ≥ 2) — pass `None` for MS1.
pub fn build_filter(
    event: &ScanEvent,
    index_entry: &ScanIndexEntry,
    precursor_mz: Option<f64>,
    activation_energy: Option<f64>,
) -> String {
    let mut out = String::with_capacity(96);
    let p = &event.preamble;

    // Analyzer (FTMS, ITMS, TQMS, ...)
    if let Some(a) = p.analyzer() {
        out.push_str(a.as_str());
    } else {
        out.push_str("MS");
    }
    out.push(' ');

    // Polarity + space
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

    // Dependent-scan flag: precedes the scan-type token
    if p.is_dependent() {
        out.push_str("d ");
    }

    // Scan type (Full/Zoom/SIM/SRM/CRM/Q1/Q3)
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

    // ms<n> level token
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

    // Precursor@activation<energy> (only for MSn)
    if n >= 2 {
        if let Some(mz) = precursor_mz {
            out.push(' ');
            out.push_str(&format!("{mz:.4}"));
            if let Some(act) = p.activation() {
                out.push('@');
                out.push_str(act.as_str());
                if let Some(e) = activation_energy {
                    out.push_str(&format!("{e:.2}"));
                }
            }
        }
    }

    // Scan range [low-high] (always present, from index entry)
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

/// Additional suppressed fields (e.g. `sid=35.00`, `@hcd28.00`) could be
/// added by callers on top of this string. This function produces the
/// minimum-viable filter sufficient for scan identification.
#[allow(unused_imports)]
#[cfg(test)]
mod tests {
    use super::*;
    use crate::scan_event::{FractionCollector, ScanEvent, ScanEventPreamble};
    use crate::scan_index::ScanIndexEntry;

    fn make_preamble(polarity: u8, scan_mode: u8, ms_power: u8, scan_type: u8) -> ScanEventPreamble {
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
            fraction_collectors: vec![FractionCollector { low_mz: 350.0, high_mz: 1500.0 }],
            coefficients: vec![],
        };
        let idx = make_index(350.0, 1500.0);
        let s = build_filter(&ev, &idx, None, None);
        assert_eq!(s, "FTMS + p NSI Full ms [350.0000-1500.0000]");
    }

    #[test]
    fn ms2_centroid_dependent() {
        let mut pre = make_preamble(1, 0, 2, 0);
        pre.bytes[10] = 1; // dependent
        pre.bytes[24] = 1; // HCD
        let ev = ScanEvent {
            preamble: pre,
            reactions: vec![],
            fraction_collectors: vec![FractionCollector { low_mz: 150.0, high_mz: 2000.0 }],
            coefficients: vec![],
        };
        let idx = make_index(150.0, 2000.0);
        let s = build_filter(&ev, &idx, Some(645.8311), Some(28.0));
        assert_eq!(
            s,
            "FTMS + c NSI d Full ms2 645.8311@hcd28.00 [150.0000-2000.0000]"
        );
    }
}
