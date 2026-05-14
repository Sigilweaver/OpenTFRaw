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
/// write_mzml(&raw, &mut src, &mut out, "run.raw", false).unwrap();
/// ```
use std::io::{Read, Seek, Write};

use crate::error::Result;
use crate::scan_event::ScanEvent;
use crate::scan_index::ScanIndexEntry;
use crate::types::{Activation, MsPower, Polarity};
use crate::RawFileReader;

// ─── Byte-counting Write wrapper ─────────────────────────────────────────────

struct CountingWriter<'a, W: Write> {
    inner: &'a mut W,
    pos: u64,
    sha1: Sha1,
    hashing: bool,
}

impl<'a, W: Write> CountingWriter<'a, W> {
    fn new(inner: &'a mut W) -> Self {
        Self {
            inner,
            pos: 0,
            sha1: Sha1::new(),
            hashing: true,
        }
    }
}

impl<W: Write> Write for CountingWriter<'_, W> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let n = self.inner.write(buf)?;
        self.pos += n as u64;
        if self.hashing {
            self.sha1.update(&buf[..n]);
        }
        Ok(n)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.inner.flush()
    }
}

// ─── Minimal SHA-1 (RFC 3174) ─────────────────────────────────────────────────

struct Sha1 {
    state: [u32; 5],
    count: u64,
    buf: [u8; 64],
    buf_len: usize,
}

impl Sha1 {
    fn new() -> Self {
        Self {
            state: [0x67452301, 0xEFCDAB89, 0x98BADCFE, 0x10325476, 0xC3D2E1F0],
            count: 0,
            buf: [0u8; 64],
            buf_len: 0,
        }
    }

    fn update(&mut self, data: &[u8]) {
        let mut off = 0;
        while off < data.len() {
            let space = 64 - self.buf_len;
            let take = space.min(data.len() - off);
            self.buf[self.buf_len..self.buf_len + take].copy_from_slice(&data[off..off + take]);
            self.buf_len += take;
            self.count += take as u64;
            off += take;
            if self.buf_len == 64 {
                self.compress();
                self.buf_len = 0;
            }
        }
    }

    fn compress(&mut self) {
        let mut w = [0u32; 80];
        for i in 0..16 {
            w[i] = u32::from_be_bytes(self.buf[i * 4..i * 4 + 4].try_into().unwrap());
        }
        for i in 16..80 {
            w[i] = (w[i - 3] ^ w[i - 8] ^ w[i - 14] ^ w[i - 16]).rotate_left(1);
        }
        let [mut a, mut b, mut c, mut d, mut e] = self.state;
        for i in 0..80 {
            let (f, k) = match i {
                0..=19 => ((b & c) | (!b & d), 0x5A827999u32),
                20..=39 => (b ^ c ^ d, 0x6ED9EBA1),
                40..=59 => ((b & c) | (b & d) | (c & d), 0x8F1BBCDC),
                _ => (b ^ c ^ d, 0xCA62C1D6),
            };
            let temp = a
                .rotate_left(5)
                .wrapping_add(f)
                .wrapping_add(e)
                .wrapping_add(k)
                .wrapping_add(w[i]);
            e = d;
            d = c;
            c = b.rotate_left(30);
            b = a;
            a = temp;
        }
        self.state[0] = self.state[0].wrapping_add(a);
        self.state[1] = self.state[1].wrapping_add(b);
        self.state[2] = self.state[2].wrapping_add(c);
        self.state[3] = self.state[3].wrapping_add(d);
        self.state[4] = self.state[4].wrapping_add(e);
    }

    fn finalize(mut self) -> [u8; 20] {
        let bit_count = self.count * 8;
        self.update(&[0x80]);
        while self.buf_len != 56 {
            self.update(&[0u8]);
        }
        self.update(&bit_count.to_be_bytes());
        let mut digest = [0u8; 20];
        for (i, &word) in self.state.iter().enumerate() {
            digest[i * 4..i * 4 + 4].copy_from_slice(&word.to_be_bytes());
        }
        digest
    }
}

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

fn activation_cv(
    act: Activation,
    analyzer: Option<crate::Analyzer>,
) -> (&'static str, &'static str) {
    match act {
        Activation::HCD => ("MS:1000422", "beam-type collision-induced dissociation"),
        Activation::ETD | Activation::EThcD => ("MS:1000598", "electron transfer dissociation"),
        // On FTMS instruments (Orbitrap, FT-ICR) byte=4 is beam-type CID (HCD),
        // not ion-trap CID. Mirror the same logic as the scan-filter builder.
        Activation::CID => match analyzer {
            Some(crate::Analyzer::FTMS) => {
                ("MS:1000422", "beam-type collision-induced dissociation")
            }
            _ => ("MS:1000133", "collision-induced dissociation"),
        },
        Activation::MPID => (
            "MS:1002481",
            "supplemental beam-type collision-induced dissociation",
        ),
        Activation::ECD => ("MS:1000250", "electron capture dissociation"),
        Activation::IRMPD => ("MS:1000262", "infrared multiphoton dissociation"),
        Activation::PD => ("MS:1001880", "in-source collision-induced dissociation"),
        Activation::PQD => ("MS:1000599", "pulsed q dissociation"),
        Activation::UVPD => ("MS:1003246", "ultraviolet photodissociation"),
        Activation::SID => ("MS:1000422", "beam-type collision-induced dissociation"),
    }
}

// ─── Helpers ──────────────────────────────────────────────────────────────────

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
            // Filter out zero or negative m/z bins (can appear at edges of chunks).
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
        // No profile data in packet — fall through to centroid.
        let mz: Vec<f64> = packet.peaks.iter().map(|p| p.mz).collect();
        let int: Vec<f32> = packet.peaks.iter().map(|p| p.abundance).collect();
        return Some((mz, int, nominal_scan_mode));
    }
    // Default path: fast centroid-only read.
    let peaks = raw.read_peaks_only(source, scan_number).ok()?;
    let mz: Vec<f64> = peaks.iter().map(|p| p.mz as f64).collect();
    let int: Vec<f32> = peaks.iter().map(|p| p.abundance).collect();
    Some((mz, int, nominal_scan_mode))
}

// ─── Main entry point ─────────────────────────────────────────────────────────

/// Write the contents of `raw` as mzML 1.1.0 to `out`.
///
/// * `source` — an open handle to the original `.raw` file (needed to read
///   scan data packets).
/// * `raw_filename` — the file name used for the `<sourceFile>` element.
///   Typically `Path::file_name()`.
/// * `include_profile` — when `true`, profile-mode scans (Orbitrap MS1,
///   etc.) export the raw profile signal instead of the centroid peak list.
///   Centroid-mode scans are unaffected.
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
    let first_scan = raw.run_header.sample_info.first_scan_number;
    let n_spectra = raw.num_scans as usize;

    let (inst_acc, inst_name) = instrument_cv(raw);

    // ── XML declaration + root ─────────────────────────────────────────────
    writeln!(out, r#"<?xml version="1.0" encoding="utf-8"?>"#)?;
    writeln!(out, r#"<mzML xmlns="http://psi.hupo.org/ms/mzml""#)?;
    writeln!(
        out,
        r#"      xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance""#
    )?;
    writeln!(
        out,
        r#"      xsi:schemaLocation="http://psi.hupo.org/ms/mzml http://psidev.info/files/ms/mzML/xsd/mzML1.1.2_idx.xsd""#
    )?;
    writeln!(out, r#"      version="1.1.0">"#)?;

    // ── cvList ─────────────────────────────────────────────────────────────
    writeln!(out, r#"  <cvList count="2">"#)?;
    writeln!(
        out,
        r#"    <cv id="MS" fullName="Proteomics Standards Initiative Mass Spectrometry Ontology" version="4.1.100" URI="https://raw.githubusercontent.com/HUPO-PSI/psi-ms-CV/master/psi-ms.obo"/>"#
    )?;
    writeln!(
        out,
        r#"    <cv id="UO" fullName="Unit Ontology" version="09:04:2014" URI="https://raw.githubusercontent.com/bio-ontology-research-group/unit-ontology/master/unit.obo"/>"#
    )?;
    writeln!(out, r#"  </cvList>"#)?;

    // ── fileDescription ────────────────────────────────────────────────────
    writeln!(out, r#"  <fileDescription>"#)?;
    writeln!(out, r#"    <fileContent>"#)?;
    writeln!(
        out,
        r#"      <cvParam cvRef="MS" accession="MS:1000579" name="MS1 spectrum" value=""/>"#
    )?;
    writeln!(
        out,
        r#"      <cvParam cvRef="MS" accession="MS:1000580" name="MSn spectrum" value=""/>"#
    )?;
    writeln!(out, r#"    </fileContent>"#)?;
    writeln!(out, r#"    <sourceFileList count="1">"#)?;
    writeln!(
        out,
        r#"      <sourceFile id="sf1" name="{}" location="">"#,
        escape(raw_filename)
    )?;
    writeln!(
        out,
        r#"        <cvParam cvRef="MS" accession="MS:1000563" name="Thermo RAW format" value=""/>"#
    )?;
    writeln!(
        out,
        r#"        <cvParam cvRef="MS" accession="MS:1000768" name="Thermo nativeID format" value=""/>"#
    )?;
    writeln!(out, r#"      </sourceFile>"#)?;
    writeln!(out, r#"    </sourceFileList>"#)?;
    writeln!(out, r#"  </fileDescription>"#)?;

    // ── softwareList ───────────────────────────────────────────────────────
    writeln!(out, r#"  <softwareList count="1">"#)?;
    writeln!(out, r#"    <software id="opentfraw" version="0.1.0">"#)?;
    writeln!(
        out,
        r#"      <cvParam cvRef="MS" accession="MS:1000799" name="custom unreleased software tool" value="opentfraw"/>"#
    )?;
    writeln!(out, r#"    </software>"#)?;
    writeln!(out, r#"  </softwareList>"#)?;

    // ── instrumentConfigurationList ────────────────────────────────────────
    writeln!(out, r#"  <instrumentConfigurationList count="1">"#)?;
    writeln!(out, r#"    <instrumentConfiguration id="IC1">"#)?;
    writeln!(
        out,
        r#"      <cvParam cvRef="MS" accession="{}" name="{}" value=""/>"#,
        inst_acc,
        escape(inst_name)
    )?;
    writeln!(out, r#"    </instrumentConfiguration>"#)?;
    writeln!(out, r#"  </instrumentConfigurationList>"#)?;

    // ── dataProcessingList ─────────────────────────────────────────────────
    writeln!(out, r#"  <dataProcessingList count="1">"#)?;
    writeln!(out, r#"    <dataProcessing id="dp1">"#)?;
    writeln!(
        out,
        r#"      <processingMethod order="0" softwareRef="opentfraw">"#
    )?;
    writeln!(
        out,
        r#"        <cvParam cvRef="MS" accession="MS:1000544" name="Conversion to mzML" value=""/>"#
    )?;
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
        // `scan_events` is one-entry-per-scan, parallel to `scan_index`.
        // The `entry.scan_event` field is a scan-event-class identifier, not
        // an index into this array.
        let event = raw.scan_events.get(idx as usize);
        let params = raw.scan_params(scan_number);

        // SRM (flat-peak) files have no scan events; derive ms_level and scan mode directly.
        let is_srm = raw.flat_peaks;
        let level = if is_srm {
            2
        } else {
            event
                .and_then(|e| e.preamble.ms_power())
                .map(ms_level)
                .unwrap_or(1)
        };
        // SRM files have no scan events; default to positive polarity (the vast majority of SRM
        // experiments are positive-mode). Other formats read polarity from the scan event preamble.
        let polarity = if is_srm {
            Some(crate::types::Polarity::Positive)
        } else {
            event.and_then(|e| e.preamble.polarity())
        };
        // SRM peaks are always centroid; fall back to scan event preamble for other formats.
        let scan_mode = if is_srm {
            Some(crate::ScanMode::Centroid)
        } else {
            event.and_then(|e| e.preamble.scan_mode())
        };
        let filter = raw.scan_filter(scan_number);
        let is_ms1 = !is_srm && level == 1;
        // Q1 precursor mass for SRM scans (looked up by scan event class).
        let srm_q1 = if is_srm {
            raw.srm_q1_by_event.get(&entry.scan_event).copied()
        } else {
            None
        };
        // Collision energy for v63 SRM scans (from transition table; empty for v66).
        let srm_ce = if is_srm {
            raw.srm_ce_by_event.get(&entry.scan_event).copied()
        } else {
            None
        };

        let (mz_vals, int_vals, effective_scan_mode) = match resolve_scan_arrays(
            raw, source, scan_number, include_profile, event, scan_mode,
        ) {
            Some(arrays) => arrays,
            None => continue,
        };
        let n_peaks = mz_vals.len();

        write_spectrum(
            out,
            idx as usize,
            scan_number,
            level,
            polarity,
            effective_scan_mode,
            filter.as_deref(),
            is_ms1,
            entry,
            event,
            params,
            srm_q1,
            srm_ce,
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

/// Write the contents of `raw` as an indexed mzML 1.1.0 document.
///
/// Indexed mzML adds a `<indexList>` element after all spectra with the byte
/// offset of each `<spectrum>` element, enabling random-access parsing by
/// tools such as pyteomics and pymzml without a full file scan.
///
/// The `<fileChecksum>` element contains the SHA-1 hash of the file content
/// up to and including `</indexList>`, computed on the fly.
///
/// Pass `include_profile=true` to export raw profile signal for profile-mode
/// scans instead of centroid peaks.
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
    let first_scan = raw.run_header.sample_info.first_scan_number;
    let n_spectra = raw.num_scans as usize;
    let (inst_acc, inst_name) = instrument_cv(raw);

    // CountingWriter tracks byte offsets and feeds bytes into SHA-1 while hashing=true.
    let mut cw = CountingWriter::new(out);

    // ── XML declaration + root (indexedmzML wrapper) ───────────────────────
    writeln!(cw, r#"<?xml version="1.0" encoding="utf-8"?>"#)?;
    writeln!(cw, r#"<indexedmzML xmlns="http://psi.hupo.org/ms/mzml""#)?;
    writeln!(
        cw,
        r#"             xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance""#
    )?;
    writeln!(
        cw,
        r#"             xsi:schemaLocation="http://psi.hupo.org/ms/mzml http://psidev.info/files/ms/mzML/xsd/mzML1.1.2_idx.xsd">"#
    )?;
    writeln!(cw, r#"  <mzML xmlns="http://psi.hupo.org/ms/mzml" version="1.1.0">"#)?;

    // ── cvList ─────────────────────────────────────────────────────────────
    writeln!(cw, r#"  <cvList count="2">"#)?;
    writeln!(
        cw,
        r#"    <cv id="MS" fullName="Proteomics Standards Initiative Mass Spectrometry Ontology" version="4.1.100" URI="https://raw.githubusercontent.com/HUPO-PSI/psi-ms-CV/master/psi-ms.obo"/>"#
    )?;
    writeln!(
        cw,
        r#"    <cv id="UO" fullName="Unit Ontology" version="09:04:2014" URI="https://raw.githubusercontent.com/bio-ontology-research-group/unit-ontology/master/unit.obo"/>"#
    )?;
    writeln!(cw, r#"  </cvList>"#)?;

    // ── fileDescription ────────────────────────────────────────────────────
    writeln!(cw, r#"  <fileDescription>"#)?;
    writeln!(cw, r#"    <fileContent>"#)?;
    writeln!(
        cw,
        r#"      <cvParam cvRef="MS" accession="MS:1000579" name="MS1 spectrum" value=""/>"#
    )?;
    writeln!(
        cw,
        r#"      <cvParam cvRef="MS" accession="MS:1000580" name="MSn spectrum" value=""/>"#
    )?;
    writeln!(cw, r#"    </fileContent>"#)?;
    writeln!(cw, r#"    <sourceFileList count="1">"#)?;
    writeln!(
        cw,
        r#"      <sourceFile id="sf1" name="{}" location="">"#,
        escape(raw_filename)
    )?;
    writeln!(
        cw,
        r#"        <cvParam cvRef="MS" accession="MS:1000563" name="Thermo RAW format" value=""/>"#
    )?;
    writeln!(
        cw,
        r#"        <cvParam cvRef="MS" accession="MS:1000768" name="Thermo nativeID format" value=""/>"#
    )?;
    writeln!(cw, r#"      </sourceFile>"#)?;
    writeln!(cw, r#"    </sourceFileList>"#)?;
    writeln!(cw, r#"  </fileDescription>"#)?;

    // ── softwareList ───────────────────────────────────────────────────────
    writeln!(cw, r#"  <softwareList count="1">"#)?;
    writeln!(cw, r#"    <software id="opentfraw" version="0.1.0">"#)?;
    writeln!(
        cw,
        r#"      <cvParam cvRef="MS" accession="MS:1000799" name="custom unreleased software tool" value="opentfraw"/>"#
    )?;
    writeln!(cw, r#"    </software>"#)?;
    writeln!(cw, r#"  </softwareList>"#)?;

    // ── instrumentConfigurationList ────────────────────────────────────────
    writeln!(cw, r#"  <instrumentConfigurationList count="1">"#)?;
    writeln!(cw, r#"    <instrumentConfiguration id="IC1">"#)?;
    writeln!(
        cw,
        r#"      <cvParam cvRef="MS" accession="{}" name="{}" value=""/>"#,
        inst_acc,
        escape(inst_name)
    )?;
    writeln!(cw, r#"    </instrumentConfiguration>"#)?;
    writeln!(cw, r#"  </instrumentConfigurationList>"#)?;

    // ── dataProcessingList ─────────────────────────────────────────────────
    writeln!(cw, r#"  <dataProcessingList count="1">"#)?;
    writeln!(cw, r#"    <dataProcessing id="dp1">"#)?;
    writeln!(
        cw,
        r#"      <processingMethod order="0" softwareRef="opentfraw">"#
    )?;
    writeln!(
        cw,
        r#"        <cvParam cvRef="MS" accession="MS:1000544" name="Conversion to mzML" value=""/>"#
    )?;
    writeln!(cw, r#"      </processingMethod>"#)?;
    writeln!(cw, r#"    </dataProcessing>"#)?;
    writeln!(cw, r#"  </dataProcessingList>"#)?;

    // ── run ────────────────────────────────────────────────────────────────
    writeln!(
        cw,
        r#"  <run id="{}" defaultInstrumentConfigurationRef="IC1" defaultSourceFileRef="sf1">"#,
        escape(raw_filename)
    )?;
    writeln!(
        cw,
        r#"    <spectrumList count="{}" defaultDataProcessingRef="dp1">"#,
        n_spectra
    )?;

    // ── spectra (record byte offset before each <spectrum>) ────────────────
    let mut spectrum_offsets: Vec<(u32, u64)> = Vec::with_capacity(n_spectra);
    for idx in 0..raw.num_scans {
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
            Some(crate::types::Polarity::Positive)
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

        let (mz_vals, int_vals, effective_scan_mode) = match resolve_scan_arrays(
            raw, source, scan_number, include_profile, event, scan_mode,
        ) {
            Some(arrays) => arrays,
            None => continue,
        };
        let n_peaks = mz_vals.len();

        // Record byte offset of this <spectrum> element before writing it.
        spectrum_offsets.push((scan_number, cw.pos));

        write_spectrum(
            &mut cw,
            idx as usize,
            scan_number,
            level,
            polarity,
            effective_scan_mode,
            filter.as_deref(),
            is_ms1,
            entry,
            event,
            params,
            srm_q1,
            srm_ce,
            &mz_vals,
            &int_vals,
            n_peaks,
        )?;
    }

    writeln!(cw, r#"    </spectrumList>"#)?;
    writeln!(cw, r#"  </run>"#)?;
    writeln!(cw, r#"  </mzML>"#)?;

    // ── indexList ──────────────────────────────────────────────────────────
    let index_list_offset = cw.pos;
    writeln!(cw, r#"  <indexList count="1">"#)?;
    writeln!(cw, r#"    <index name="spectrum">"#)?;
    for (scan_number, offset) in &spectrum_offsets {
        writeln!(
            cw,
            r#"      <offset idRef="controllerType=0 controllerNumber=1 scan={}">{}</offset>"#,
            scan_number,
            offset
        )?;
    }
    writeln!(cw, r#"    </index>"#)?;
    writeln!(cw, r#"  </indexList>"#)?;

    // Stop hashing; compute SHA-1 of everything through </indexList>.
    cw.hashing = false;
    let finished_sha1 = std::mem::replace(&mut cw.sha1, Sha1::new());
    let digest = finished_sha1.finalize();
    let hex: String = digest.iter().map(|b| format!("{:02x}", b)).collect();

    writeln!(
        cw,
        r#"  <indexListOffset>{}</indexListOffset>"#,
        index_list_offset
    )?;
    writeln!(cw, r#"  <fileChecksum>{}</fileChecksum>"#, hex)?;
    writeln!(cw, r#"</indexedmzML>"#)?;
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
    srm_q1: Option<f64>,
    srm_ce: Option<f64>,
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
        r#"      <spectrum id="controllerType=0 controllerNumber=1 scan={scan}" index="{idx}" defaultArrayLength="{n}">"#,
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
    //
    // For SRM (flat-peak) scans: Q1 is stored in the method transition table
    // and retrieved via srm_q1. The isolation window is ±0.35 Da (TSQ default).
    //
    // For other MS2+ scans: in v66 RAW files, scan-event reactions are empty and
    // the precursor info lives entirely in the per-scan parameter table. Older
    // formats populate `event.reactions[0]`. We emit a <precursor> block whenever
    // we have enough data from either source.
    if !is_ms1 {
        // For SRM scans, Q1 is the precursor; isolation window is ±0.35 Da.
        // For other MS2+: resolve from params or event.reactions.
        let (target_mz, sel_mz, iso_width, charge, act_energy, act_energy_is_nce) = if srm_q1.is_some() {
            (srm_q1, srm_q1, Some(0.7f64), None::<i32>, srm_ce, false)
        } else {
            let reaction = event.and_then(|e| e.reactions.first());
            // Build target_mz from the best available source. Each source is
            // filtered to exclude 0.0 (absent/unknown) before trying the next:
            //   1. isolation_target_mz() — "MS2 Isolation Offset:" or "Target M/Z:" in scan params
            //   2. monoisotopic_mz()     — "Monoisotopic M/Z:" in scan params (v66 DDA)
            //   3. reaction.precursor_mz — scan-event body (tribrid / older formats)
            // Applying filter() per-source ensures a 0.0 from source 1 does NOT
            // prevent source 2 from being tried (the bug that caused isolation
            // window target to be absent for Q Exactive HF DDA MS2 scans).
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
            let ae = ae_from_params
                .or_else(|| reaction.map(|r| r.energy).filter(|&e| e > 0.0));
            (tm, sm, iw, ch, ae, ae_is_nce)
        };

        // Always emit a <precursorList> for MSn spectra; mzML requires it.
        // For DIA scans the precursor m/z is 0/unknown, but the activation
        // method and (when available) isolation window are still recorded.
        writeln!(out, r#"        <precursorList count="1">"#)?;

        // Link to the precursor (MS1) scan when known.
        let master_ref = params
            .as_ref()
            .and_then(|p| p.master_scan_number())
            .filter(|&n| n > 0)
            .map(|n| format!("controllerType=0 controllerNumber=1 scan={n}"));
        if let Some(ref mref) = master_ref {
            writeln!(out, r#"          <precursor spectrumRef="{mref}">"#)?;
        } else {
            writeln!(out, r#"          <precursor>"#)?;
        }

        // Isolation window (center + lower/upper offsets). Omit entirely
        // when no window information is available (e.g. all-ions DIA).
        if target_mz.is_some() || iso_width.is_some() {
            writeln!(out, r#"            <isolationWindow>"#)?;
            if let Some(mz) = target_mz {
                writeln!(
                    out,
                    r#"              <cvParam cvRef="MS" accession="MS:1000827" name="isolation window target m/z" value="{:.6}" unitCvRef="MS" unitAccession="MS:1000040" unitName="m/z"/>"#,
                    mz
                )?;
            }
            if let Some(w) = iso_width {
                // Thermo reports a total isolation width; mzML splits it into
                // symmetric lower/upper offsets around the target.
                let half = w / 2.0;
                writeln!(
                    out,
                    r#"              <cvParam cvRef="MS" accession="MS:1000828" name="isolation window lower offset" value="{:.6}" unitCvRef="MS" unitAccession="MS:1000040" unitName="m/z"/>"#,
                    half
                )?;
                writeln!(
                    out,
                    r#"              <cvParam cvRef="MS" accession="MS:1000829" name="isolation window upper offset" value="{:.6}" unitCvRef="MS" unitAccession="MS:1000040" unitName="m/z"/>"#,
                    half
                )?;
            }
            writeln!(out, r#"            </isolationWindow>"#)?;
        }

        // Selected ion.
        if let Some(mz) = sel_mz {
            writeln!(out, r#"            <selectedIonList count="1">"#)?;
            writeln!(out, r#"              <selectedIon>"#)?;
            writeln!(
                out,
                r#"                <cvParam cvRef="MS" accession="MS:1000744" name="selected ion m/z" value="{:.6}" unitCvRef="MS" unitAccession="MS:1000040" unitName="m/z"/>"#,
                mz
            )?;
            if let Some(z) = charge {
                writeln!(
                    out,
                    r#"                <cvParam cvRef="MS" accession="MS:1000041" name="charge state" value="{z}"/>"#
                )?;
            }
            writeln!(out, r#"              </selectedIon>"#)?;
            writeln!(out, r#"            </selectedIonList>"#)?;
        }

        // Activation.
        writeln!(out, r#"            <activation>"#)?;
        if let Some(act) = event.and_then(|e| e.preamble.activation()) {
            let analyzer = event.and_then(|e| e.preamble.analyzer());
            let (acc, name) = activation_cv(act, analyzer);
            writeln!(
                out,
                r#"              <cvParam cvRef="MS" accession="{acc}" name="{name}" value=""/>"#
            )?;
        } else {
            writeln!(
                out,
                r#"              <cvParam cvRef="MS" accession="MS:1000133" name="collision-induced dissociation" value=""/>"#
            )?;
        }
        if let Some(e) = act_energy {
            if act_energy_is_nce {
                writeln!(
                    out,
                    r#"              <cvParam cvRef="MS" accession="MS:1002013" name="normalized collision energy" value="{:.2}"/>"#,
                    e
                )?;
            } else {
                writeln!(
                    out,
                    r#"              <cvParam cvRef="MS" accession="MS:1000045" name="collision energy" value="{:.2}" unitCvRef="UO" unitAccession="UO:0000266" unitName="electronvolt"/>"#,
                    e
                )?;
            }
        }
        writeln!(out, r#"            </activation>"#)?;

        writeln!(out, r#"          </precursor>"#)?;
        writeln!(out, r#"        </precursorList>"#)?;
    }

    // Binary arrays
    let n_arrays: usize = if n_peaks > 0 { 2 } else { 0 };
    if n_arrays > 0 {
        let mz_b64 = encode_f64_array(mz_vals);
        let int_b64 = encode_f32_array(int_vals);

        writeln!(out, r#"        <binaryDataArrayList count="{n_arrays}">"#)?;

        // m/z array — f64, no compression
        writeln!(
            out,
            r#"          <binaryDataArray encodedLength="{}">"#,
            mz_b64.len()
        )?;
        writeln!(
            out,
            r#"            <cvParam cvRef="MS" accession="MS:1000514" name="m/z array" value=""/>"#
        )?;
        writeln!(
            out,
            r#"            <cvParam cvRef="MS" accession="MS:1000523" name="64-bit float" value=""/>"#
        )?;
        writeln!(
            out,
            r#"            <cvParam cvRef="MS" accession="MS:1000576" name="no compression" value=""/>"#
        )?;
        writeln!(out, r#"            <binary>{mz_b64}</binary>"#)?;
        writeln!(out, r#"          </binaryDataArray>"#)?;

        // intensity array — f32, no compression
        writeln!(
            out,
            r#"          <binaryDataArray encodedLength="{}">"#,
            int_b64.len()
        )?;
        writeln!(
            out,
            r#"            <cvParam cvRef="MS" accession="MS:1000515" name="intensity array" value=""/>"#
        )?;
        writeln!(
            out,
            r#"            <cvParam cvRef="MS" accession="MS:1000521" name="32-bit float" value=""/>"#
        )?;
        writeln!(
            out,
            r#"            <cvParam cvRef="MS" accession="MS:1000576" name="no compression" value=""/>"#
        )?;
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
