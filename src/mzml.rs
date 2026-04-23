/// mzML export for Thermo RAW files.
///
/// Writes a valid mzML 1.1.0 document to any `Write` sink. Produces one
/// `<spectrum>` element per scan. Binary arrays (m/z and intensity) are
/// stored as little-endian raw bytes encoded with standard Base64 — no
/// additional compression is applied, keeping this module dependency-free.
///
/// # Usage
/// ```no_run
/// use opentfraw::{RawFileReader, mzml::write_mzml};
/// let raw = RawFileReader::open_path("run.raw").unwrap();
/// let mut out = std::fs::File::create("run.mzML").unwrap();
/// let mut src = std::fs::File::open("run.raw").unwrap();
/// write_mzml(&raw, &mut src, &mut out, "run.raw").unwrap();
/// ```
use std::io::{Read, Seek, Write};

use crate::error::Result;
use crate::scan_event::ScanEvent;
use crate::scan_index::ScanIndexEntry;
use crate::types::{Activation, MsPower, Polarity};
use crate::RawFileReader;

// ─── Base64 (RFC 4648 §4, no line wrapping) ─────────────────────────────────

const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

fn base64_encode(data: &[u8]) -> String {
    let n = data.len();
    let mut out = Vec::with_capacity(((n + 2) / 3) * 4);
    let mut i = 0;
    while i + 2 < n {
        let b = ((data[i] as u32) << 16) | ((data[i + 1] as u32) << 8) | (data[i + 2] as u32);
        out.push(B64[((b >> 18) & 0x3f) as usize]);
        out.push(B64[((b >> 12) & 0x3f) as usize]);
        out.push(B64[((b >> 6) & 0x3f) as usize]);
        out.push(B64[(b & 0x3f) as usize]);
        i += 3;
    }
    if n - i == 2 {
        let b = ((data[i] as u32) << 16) | ((data[i + 1] as u32) << 8);
        out.push(B64[((b >> 18) & 0x3f) as usize]);
        out.push(B64[((b >> 12) & 0x3f) as usize]);
        out.push(B64[((b >> 6) & 0x3f) as usize]);
        out.push(b'=');
    } else if n - i == 1 {
        let b = (data[i] as u32) << 16;
        out.push(B64[((b >> 18) & 0x3f) as usize]);
        out.push(B64[((b >> 12) & 0x3f) as usize]);
        out.push(b'=');
        out.push(b'=');
    }
    // SAFETY: all bytes written are 7-bit ASCII.
    unsafe { String::from_utf8_unchecked(out) }
}

// ─── Encode f64 and f32 arrays to base64 ────────────────────────────────────

fn encode_f64_array(vals: &[f64]) -> String {
    let bytes: Vec<u8> = vals.iter().flat_map(|v| v.to_le_bytes()).collect();
    base64_encode(&bytes)
}

fn encode_f32_array(vals: &[f32]) -> String {
    let bytes: Vec<u8> = vals.iter().flat_map(|v| v.to_le_bytes()).collect();
    base64_encode(&bytes)
}

// ─── XML helpers ─────────────────────────────────────────────────────────────

fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

// ─── Instrument model → PSI-MS CV accession ─────────────────────────────────

fn instrument_cv(raw: &RawFileReader) -> (&'static str, &'static str) {
    // Returns (accession, name). Prefer the specific model name when available.
    if let Some(model) = raw.instrument_model {
        // Map known Thermo model names to PSI-MS CV terms where possible.
        // Accessions from psi-ms.obo (instrument model branch MS:1000031).
        let known: &[(&str, &str, &str)] = &[
            ("Orbitrap Astral",          "MS:1003355", "Orbitrap Astral"),
            ("Orbitrap Ascend",          "MS:1003028", "Orbitrap Ascend"),
            ("Orbitrap Eclipse",         "MS:1003029", "Orbitrap Eclipse"),
            ("Orbitrap Fusion Lumos",    "MS:1002732", "Orbitrap Fusion Lumos"),
            ("Orbitrap Fusion",          "MS:1002416", "Orbitrap Fusion"),
            ("Orbitrap Exploris 480",    "MS:1003028", "Orbitrap Exploris 480"),
            ("Orbitrap Exploris 240",    "MS:1003098", "Orbitrap Exploris 240"),
            ("Orbitrap Exploris 120",    "MS:1003199", "Orbitrap Exploris 120"),
            ("Q Exactive HF-X",          "MS:1002877", "Q Exactive HF-X"),
            ("Q Exactive HF",            "MS:1002523", "Q Exactive HF"),
            ("Q Exactive Plus",          "MS:1002634", "Q Exactive Plus"),
            ("Q Exactive UHMR",          "MS:1003245", "Q Exactive UHMR"),
            ("Q Exactive",               "MS:1001911", "Q Exactive"),
            ("LTQ Orbitrap Velos Pro",   "MS:1001742", "LTQ Orbitrap Velos"),
            ("LTQ Orbitrap Velos",       "MS:1001742", "LTQ Orbitrap Velos"),
            ("LTQ Orbitrap Elite",       "MS:1001910", "LTQ Orbitrap Elite"),
            ("LTQ Orbitrap XL",          "MS:1000556", "LTQ Orbitrap XL"),
            ("LTQ Orbitrap",             "MS:1000449", "LTQ Orbitrap"),
            ("LTQ Velos Pro",            "MS:1001096", "LTQ Velos Pro"),
            ("LTQ Velos",                "MS:1000855", "LTQ Velos"),
            ("LTQ XL",                   "MS:1000854", "LTQ XL"),
            ("LTQ FT",                   "MS:1000448", "LTQ FT"),
            ("LTQ",                      "MS:1000447", "LTQ"),
            ("TSQ Altis",                "MS:1003108", "TSQ Altis"),
            ("TSQ Quantiva",             "MS:1002498", "TSQ Quantiva"),
            ("TSQ Endura",               "MS:1002497", "TSQ Endura"),
            ("TSQ Vantage",              "MS:1001510", "TSQ Vantage"),
            ("LCQ Classic",              "MS:1000443", "LCQ Classic"),
            ("LCQ Deca",                 "MS:1000446", "LCQ Deca"),
            ("LCQ Advantage",            "MS:1000590", "LCQ Advantage"),
        ];
        for (prefix, acc, name) in known {
            if model.starts_with(prefix) {
                return (acc, name);
            }
        }
    }
    ("MS:1000483", "Thermo Fisher Scientific instrument model")
}

// ─── MS level from MsPower ───────────────────────────────────────────────────

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

// ─── Activation method → CV accession ────────────────────────────────────────

fn activation_cv(act: Activation) -> (&'static str, &'static str) {
    match act {
        Activation::HCD => ("MS:1000422", "beam-type collision-induced dissociation"),
        Activation::CID => ("MS:1000133", "collision-induced dissociation"),
    }
}

// ─── Main entry point ─────────────────────────────────────────────────────────

/// Write the contents of `raw` as mzML 1.1.0 to `out`.
///
/// * `source` — an open handle to the original `.raw` file (needed to read
///   scan data packets).
/// * `raw_filename` — the file name used for the `<sourceFile>` element.
///   Typically `Path::file_name()`.
///
/// All spectra are written; no filtering is applied. Scans for which peak
/// data cannot be read are skipped silently.
pub fn write_mzml<R, W>(
    raw: &RawFileReader,
    source: &mut R,
    out: &mut W,
    raw_filename: &str,
) -> Result<()>
where
    R: Read + Seek,
    W: Write,
{
    let first_scan = raw.run_header.sample_info.first_scan_number;
    let n_spectra = raw.num_scans as usize;

    let (inst_acc, inst_name) = instrument_cv(raw);

    // ── XML declaration + root ─────────────────────────────────────────────
    writeln!(out, r#"<?xml version="1.0" encoding="utf-8"?>"#)?;
    writeln!(out, r#"<mzML xmlns="http://psi.hupo.org/ms/mzml""#)?;
    writeln!(out, r#"      xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance""#)?;
    writeln!(out, r#"      xsi:schemaLocation="http://psi.hupo.org/ms/mzml http://psidev.info/files/ms/mzML/xsd/mzML1.1.2_idx.xsd""#)?;
    writeln!(out, r#"      version="1.1.0">"#)?;

    // ── cvList ─────────────────────────────────────────────────────────────
    writeln!(out, r#"  <cvList count="2">"#)?;
    writeln!(out, r#"    <cv id="MS" fullName="Proteomics Standards Initiative Mass Spectrometry Ontology" version="4.1.100" URI="https://raw.githubusercontent.com/HUPO-PSI/psi-ms-CV/master/psi-ms.obo"/>"#)?;
    writeln!(out, r#"    <cv id="UO" fullName="Unit Ontology" version="09:04:2014" URI="https://raw.githubusercontent.com/bio-ontology-research-group/unit-ontology/master/unit.obo"/>"#)?;
    writeln!(out, r#"  </cvList>"#)?;

    // ── fileDescription ────────────────────────────────────────────────────
    writeln!(out, r#"  <fileDescription>"#)?;
    writeln!(out, r#"    <fileContent>"#)?;
    writeln!(out, r#"      <cvParam cvRef="MS" accession="MS:1000579" name="MS1 spectrum" value=""/>"#)?;
    writeln!(out, r#"      <cvParam cvRef="MS" accession="MS:1000580" name="MSn spectrum" value=""/>"#)?;
    writeln!(out, r#"    </fileContent>"#)?;
    writeln!(out, r#"    <sourceFileList count="1">"#)?;
    writeln!(
        out,
        r#"      <sourceFile id="sf1" name="{}" location="">"#,
        escape(raw_filename)
    )?;
    writeln!(out, r#"        <cvParam cvRef="MS" accession="MS:1000563" name="Thermo RAW format" value=""/>"#)?;
    writeln!(out, r#"        <cvParam cvRef="MS" accession="MS:1000768" name="Thermo nativeID format" value=""/>"#)?;
    writeln!(out, r#"      </sourceFile>"#)?;
    writeln!(out, r#"    </sourceFileList>"#)?;
    writeln!(out, r#"  </fileDescription>"#)?;

    // ── softwareList ───────────────────────────────────────────────────────
    writeln!(out, r#"  <softwareList count="1">"#)?;
    writeln!(out, r#"    <software id="opentfraw" version="0.1.0">"#)?;
    writeln!(out, r#"      <cvParam cvRef="MS" accession="MS:1000799" name="custom unreleased software tool" value="opentfraw"/>"#)?;
    writeln!(out, r#"    </software>"#)?;
    writeln!(out, r#"  </softwareList>"#)?;

    // ── instrumentConfigurationList ────────────────────────────────────────
    writeln!(out, r#"  <instrumentConfigurationList count="1">"#)?;
    writeln!(out, r#"    <instrumentConfiguration id="IC1">"#)?;
    writeln!(
        out,
        r#"      <cvParam cvRef="MS" accession="{}" name="{}" value=""/>"#,
        inst_acc, escape(inst_name)
    )?;
    writeln!(out, r#"    </instrumentConfiguration>"#)?;
    writeln!(out, r#"  </instrumentConfigurationList>"#)?;

    // ── dataProcessingList ─────────────────────────────────────────────────
    writeln!(out, r#"  <dataProcessingList count="1">"#)?;
    writeln!(out, r#"    <dataProcessing id="dp1">"#)?;
    writeln!(out, r#"      <processingMethod order="0" softwareRef="opentfraw">"#)?;
    writeln!(out, r#"        <cvParam cvRef="MS" accession="MS:1000544" name="Conversion to mzML" value=""/>"#)?;
    writeln!(out, r#"      </processingMethod>"#)?;
    writeln!(out, r#"    </dataProcessing>"#)?;
    writeln!(out, r#"  </dataProcessingList>"#)?;

    // ── run ────────────────────────────────────────────────────────────────
    writeln!(
        out,
        r#"  <run id="{}" defaultInstrumentConfigurationRef="IC1" defaultSourceFileRef="sf1">"#,
        escape(raw_filename)
    )?;
    writeln!(
        out,
        r#"    <spectrumList count="{}" defaultDataProcessingRef="dp1">"#,
        n_spectra
    )?;

    // ── spectra ────────────────────────────────────────────────────────────
    for idx in 0..raw.num_scans {
        let scan_number = first_scan + idx;
        let entry = &raw.scan_index[idx as usize];
        let event_idx = (entry.scan_event as u32).saturating_sub(1) as usize;
        let event = raw.scan_events.get(event_idx);
        let params = raw.scan_params(scan_number);

        // Read peaks — skip profile data for speed, skip scan on error.
        let peaks = match raw.read_peaks_only(source, scan_number) {
            Ok(p) => p,
            Err(_) => continue,
        };

        let level = event
            .and_then(|e| e.preamble.ms_power())
            .map(ms_level)
            .unwrap_or(1);
        let polarity = event.and_then(|e| e.preamble.polarity());
        let scan_mode = event.and_then(|e| e.preamble.scan_mode());
        let filter = raw.scan_filter(scan_number);
        let is_ms1 = level == 1;

        let mz_vals: Vec<f64> = peaks.iter().map(|p| p.mz as f64).collect();
        let int_vals: Vec<f32> = peaks.iter().map(|p| p.abundance).collect();
        let n_peaks = mz_vals.len();

        write_spectrum(
            out,
            idx as usize,
            scan_number,
            level,
            polarity,
            scan_mode,
            filter.as_deref(),
            is_ms1,
            entry,
            event,
            params,
            &mz_vals,
            &int_vals,
            n_peaks,
        )?;
    }

    writeln!(out, r#"    </spectrumList>"#)?;
    writeln!(out, r#"  </run>"#)?;
    writeln!(out, r#"</mzML>"#)?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn write_spectrum<W: Write>(
    out: &mut W,
    index: usize,
    scan_number: u32,
    level: u32,
    polarity: Option<Polarity>,
    scan_mode: Option<crate::ScanMode>,
    filter: Option<&str>,
    is_ms1: bool,
    entry: &ScanIndexEntry,
    event: Option<&ScanEvent>,
    params: Option<crate::ScanParams<'_>>,
    mz_vals: &[f64],
    int_vals: &[f32],
    n_peaks: usize,
) -> Result<()> {
    let spectrum_type_acc = if is_ms1 {
        ("MS:1000579", "MS1 spectrum")
    } else {
        ("MS:1000580", "MSn spectrum")
    };

    writeln!(
        out,
        r#"      <spectrum id="scan={scan}" index="{idx}" defaultArrayLength="{n}">"#,
        scan = scan_number,
        idx = index,
        n = n_peaks
    )?;
    writeln!(
        out,
        r#"        <cvParam cvRef="MS" accession="MS:1000511" name="ms level" value="{level}"/>"#
    )?;
    writeln!(
        out,
        r#"        <cvParam cvRef="MS" accession="{}" name="{}" value=""/>"#,
        spectrum_type_acc.0, spectrum_type_acc.1
    )?;

    // Centroid vs profile. Many downstream tools (e.g. MSFragger) refuse to
    // process spectra missing this tag, so we emit it unconditionally, falling
    // back to "profile" (the MS1 default on Orbitrap) when the preamble byte
    // is missing.
    match scan_mode {
        Some(crate::ScanMode::Centroid) => writeln!(
            out,
            r#"        <cvParam cvRef="MS" accession="MS:1000127" name="centroid spectrum" value=""/>"#
        )?,
        _ => writeln!(
            out,
            r#"        <cvParam cvRef="MS" accession="MS:1000128" name="profile spectrum" value=""/>"#
        )?,
    }

    // Polarity
    match polarity {
        Some(Polarity::Positive) => writeln!(
            out,
            r#"        <cvParam cvRef="MS" accession="MS:1000130" name="positive scan" value=""/>"#
        )?,
        Some(Polarity::Negative) => writeln!(
            out,
            r#"        <cvParam cvRef="MS" accession="MS:1000129" name="negative scan" value=""/>"#
        )?,
        _ => {}
    }

    // Scan-level statistics
    writeln!(
        out,
        r#"        <cvParam cvRef="MS" accession="MS:1000285" name="total ion current" value="{:.6}"/>"#,
        entry.total_current
    )?;
    writeln!(
        out,
        r#"        <cvParam cvRef="MS" accession="MS:1000504" name="base peak m/z" value="{:.6}"/>"#,
        entry.base_mz
    )?;
    writeln!(
        out,
        r#"        <cvParam cvRef="MS" accession="MS:1000505" name="base peak intensity" value="{:.6}"/>"#,
        entry.base_intensity
    )?;
    writeln!(
        out,
        r#"        <cvParam cvRef="MS" accession="MS:1000528" name="lowest observed m/z" value="{:.6}"/>"#,
        entry.low_mz
    )?;
    writeln!(
        out,
        r#"        <cvParam cvRef="MS" accession="MS:1000527" name="highest observed m/z" value="{:.6}"/>"#,
        entry.high_mz
    )?;

    // Scan list (retention time)
    writeln!(out, r#"        <scanList count="1">"#)?;
    writeln!(
        out,
        r#"          <cvParam cvRef="MS" accession="MS:1000795" name="no combination" value=""/>"#
    )?;
    writeln!(out, r#"          <scan>"#)?;

    // Thermo scan filter string — crucial for downstream tools that key off
    // the filter (MSFragger, MaxQuant, pyteomics, Skyline, ...).
    if let Some(f) = filter {
        if !f.is_empty() {
            writeln!(
                out,
                r#"            <cvParam cvRef="MS" accession="MS:1000512" name="filter string" value="{}"/>"#,
                escape(f)
            )?;
        }
    }

    writeln!(
        out,
        r#"            <cvParam cvRef="MS" accession="MS:1000016" name="scan start time" value="{:.6}" unitCvRef="UO" unitAccession="UO:0000031" unitName="minute"/>"#,
        entry.start_time
    )?;

    // Ion injection time from params
    if let Some(ref p) = params {
        if let Some(it) = p.ion_injection_time_ms() {
            writeln!(
                out,
                r#"            <cvParam cvRef="MS" accession="MS:1000927" name="ion injection time" value="{:.6}" unitCvRef="UO" unitAccession="UO:0000028" unitName="millisecond"/>"#,
                it
            )?;
        }
    }

    writeln!(out, r#"            <scanWindowList count="1">"#)?;
    writeln!(out, r#"              <scanWindow>"#)?;
    writeln!(
        out,
        r#"                <cvParam cvRef="MS" accession="MS:1000501" name="scan window lower limit" value="{:.6}" unitCvRef="MS" unitAccession="MS:1000040" unitName="m/z"/>"#,
        entry.low_mz
    )?;
    writeln!(
        out,
        r#"                <cvParam cvRef="MS" accession="MS:1000500" name="scan window upper limit" value="{:.6}" unitCvRef="MS" unitAccession="MS:1000040" unitName="m/z"/>"#,
        entry.high_mz
    )?;
    writeln!(out, r#"              </scanWindow>"#)?;
    writeln!(out, r#"            </scanWindowList>"#)?;
    writeln!(out, r#"          </scan>"#)?;
    writeln!(out, r#"        </scanList>"#)?;

    // Precursor info for MS2+
    if !is_ms1 {
        if let Some(ev) = event {
            if !ev.reactions.is_empty() {
                writeln!(out, r#"        <precursorList count="1">"#)?;
                let rx = &ev.reactions[0];

                // Resolve the precursor scan number from ScanParams if available
                let master_ref = params
                    .as_ref()
                    .and_then(|p| p.master_scan_number())
                    .filter(|&n| n > 0)
                    .map(|n| format!("scan={n}"));
                if let Some(ref mref) = master_ref {
                    writeln!(out, r#"          <precursor spectrumRef="{mref}">"#)?;
                } else {
                    writeln!(out, r#"          <precursor>"#)?;
                }

                // Isolation window
                writeln!(out, r#"            <isolationWindow>"#)?;
                writeln!(
                    out,
                    r#"              <cvParam cvRef="MS" accession="MS:1000827" name="isolation window target m/z" value="{:.6}" unitCvRef="MS" unitAccession="MS:1000040" unitName="m/z"/>"#,
                    rx.precursor_mz
                )?;
                writeln!(out, r#"            </isolationWindow>"#)?;

                // Selected ion
                writeln!(out, r#"            <selectedIonList count="1">"#)?;
                writeln!(out, r#"              <selectedIon>"#)?;

                // Prefer monoisotopic m/z from params if non-zero
                let sel_mz = params
                    .as_ref()
                    .and_then(|p| p.monoisotopic_mz())
                    .filter(|&mz| mz > 0.0)
                    .unwrap_or(rx.precursor_mz);
                writeln!(
                    out,
                    r#"                <cvParam cvRef="MS" accession="MS:1000744" name="selected ion m/z" value="{:.6}" unitCvRef="MS" unitAccession="MS:1000040" unitName="m/z"/>"#,
                    sel_mz
                )?;

                // Charge state
                if let Some(ref p) = params {
                    if let Some(z) = p.charge_state().filter(|&z| z > 0) {
                        writeln!(
                            out,
                            r#"                <cvParam cvRef="MS" accession="MS:1000041" name="charge state" value="{z}"/>"#
                        )?;
                    }
                }
                writeln!(out, r#"              </selectedIon>"#)?;
                writeln!(out, r#"            </selectedIonList>"#)?;

                // Activation
                writeln!(out, r#"            <activation>"#)?;
                if let Some(act) = ev.preamble.activation() {
                    let (acc, name) = activation_cv(act);
                    writeln!(
                        out,
                        r#"              <cvParam cvRef="MS" accession="{acc}" name="{name}" value=""/>"#
                    )?;
                } else {
                    // Unknown activation — use generic CID as fallback
                    writeln!(
                        out,
                        r#"              <cvParam cvRef="MS" accession="MS:1000133" name="collision-induced dissociation" value=""/>"#
                    )?;
                }
                if rx.energy > 0.0 {
                    writeln!(
                        out,
                        r#"              <cvParam cvRef="MS" accession="MS:1000045" name="collision energy" value="{:.2}" unitCvRef="UO" unitAccession="UO:0000266" unitName="electronvolt"/>"#,
                        rx.energy
                    )?;
                }
                writeln!(out, r#"            </activation>"#)?;

                writeln!(out, r#"          </precursor>"#)?;
                writeln!(out, r#"        </precursorList>"#)?;
            }
        }
    }

    // Binary arrays
    let n_arrays: usize = if n_peaks > 0 { 2 } else { 0 };
    if n_arrays > 0 {
        let mz_b64 = encode_f64_array(mz_vals);
        let int_b64 = encode_f32_array(int_vals);

        writeln!(
            out,
            r#"        <binaryDataArrayList count="{n_arrays}">"#
        )?;

        // m/z array — f64, no compression
        writeln!(
            out,
            r#"          <binaryDataArray encodedLength="{}">"#,
            mz_b64.len()
        )?;
        writeln!(out, r#"            <cvParam cvRef="MS" accession="MS:1000514" name="m/z array" value=""/>"#)?;
        writeln!(out, r#"            <cvParam cvRef="MS" accession="MS:1000523" name="64-bit float" value=""/>"#)?;
        writeln!(out, r#"            <cvParam cvRef="MS" accession="MS:1000576" name="no compression" value=""/>"#)?;
        writeln!(out, r#"            <binary>{mz_b64}</binary>"#)?;
        writeln!(out, r#"          </binaryDataArray>"#)?;

        // intensity array — f32, no compression
        writeln!(
            out,
            r#"          <binaryDataArray encodedLength="{}">"#,
            int_b64.len()
        )?;
        writeln!(out, r#"            <cvParam cvRef="MS" accession="MS:1000515" name="intensity array" value=""/>"#)?;
        writeln!(out, r#"            <cvParam cvRef="MS" accession="MS:1000521" name="32-bit float" value=""/>"#)?;
        writeln!(out, r#"            <cvParam cvRef="MS" accession="MS:1000576" name="no compression" value=""/>"#)?;
        writeln!(out, r#"            <binary>{int_b64}</binary>"#)?;
        writeln!(out, r#"          </binaryDataArray>"#)?;

        writeln!(out, r#"        </binaryDataArrayList>"#)?;
    }

    writeln!(out, r#"      </spectrum>"#)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_rfc_vectors() {
        assert_eq!(base64_encode(b""), "");
        assert_eq!(base64_encode(b"f"), "Zg==");
        assert_eq!(base64_encode(b"fo"), "Zm8=");
        assert_eq!(base64_encode(b"foo"), "Zm9v");
        assert_eq!(base64_encode(b"foobar"), "Zm9vYmFy");
        assert_eq!(base64_encode(b"Man"), "TWFu");
    }
}
