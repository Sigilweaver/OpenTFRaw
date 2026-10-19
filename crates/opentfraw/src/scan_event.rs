use crate::error::Result;
use crate::reader::BinaryReader;
use std::io::{Read, Seek};

/// Precursor reaction info for MS2+ scans (32 bytes).
///
/// Read through the accessor methods; the stored layout may change as more
/// of the scan event is decoded.
#[derive(Debug)]
#[non_exhaustive]
pub struct Reaction {
    pub(crate) precursor_mz: f64,
    pub(crate) energy: f64,
}

impl Reaction {
    /// Precursor m/z of this reaction.
    pub fn precursor_mz(&self) -> f64 {
        self.precursor_mz
    }

    /// Activation energy of this reaction as stored in the scan event.
    pub fn energy(&self) -> f64 {
        self.energy
    }
}

/// M/z acquisition range.
#[derive(Debug)]
#[non_exhaustive]
pub struct FractionCollector {
    pub(crate) low_mz: f64,
    pub(crate) high_mz: f64,
}

impl FractionCollector {
    /// Lower bound of the range, in m/z.
    pub fn low_mz(&self) -> f64 {
        self.low_mz
    }

    /// Upper bound of the range, in m/z.
    pub fn high_mz(&self) -> f64 {
        self.high_mz
    }
}

/// Scan event preamble - byte array encoding scan parameters.
#[derive(Debug)]
#[non_exhaustive]
pub struct ScanEventPreamble {
    pub(crate) bytes: Vec<u8>,
}

/// Complete scan event.
///
/// Read through the accessor methods; the stored layout may change as more
/// of the scan event is decoded.
#[derive(Debug)]
#[non_exhaustive]
pub struct ScanEvent {
    pub(crate) preamble: ScanEventPreamble,
    pub(crate) reactions: Vec<Reaction>,
    pub(crate) fraction_collectors: Vec<FractionCollector>,
    pub(crate) coefficients: Vec<f64>,
}

impl ScanEvent {
    /// The fixed-size preamble with the scan's enumerated settings.
    pub fn preamble(&self) -> &ScanEventPreamble {
        &self.preamble
    }

    /// Precursor reactions, first stage first. Empty for MS1 scans.
    pub fn reactions(&self) -> &[Reaction] {
        &self.reactions
    }

    /// Acquisition m/z ranges.
    pub fn fraction_collectors(&self) -> &[FractionCollector] {
        &self.fraction_collectors
    }

    /// Frequency-to-m/z calibration coefficients (empty when the scan has
    /// none). See [`crate::scan_data::freq_to_mz`].
    pub fn coefficients(&self) -> &[f64] {
        &self.coefficients
    }

    /// Activation method of this scan.
    ///
    /// This is [`ScanEventPreamble::activation`], except that an ETD MS2
    /// event read out in the Orbitrap (FTMS) with two or more reactions of
    /// which exactly one has a precursor m/z is [`crate::Activation::EThcD`].
    /// The precursor-less reaction is the supplemental step. Every such
    /// event in the corpus comes from a file whose method text says "Is
    /// EThcD Active = True" (PRIDE PXD077396 and PXD070648; see
    /// [`ScanEventPreamble::activation`]). The Orbitrap Velos Pro ETD files
    /// in PXD006767 store one reaction per event and stay ETD.
    pub fn activation(&self) -> Option<crate::Activation> {
        let act = self.preamble.activation()?;
        if act == crate::Activation::ETD && self.has_supplemental_reaction() {
            return Some(crate::Activation::EThcD);
        }
        Some(act)
    }

    /// An FTMS MS2 event with two or more reactions of which exactly one has
    /// a precursor m/z.
    fn has_supplemental_reaction(&self) -> bool {
        let with_precursor = self
            .reactions
            .iter()
            .filter(|r| r.precursor_mz > 0.0)
            .count();
        self.preamble.ms_power() == Some(crate::MsPower::Ms2)
            && self.preamble.analyzer() == Some(crate::Analyzer::FTMS)
            && self.reactions.len() >= 2
            && with_precursor == 1
    }
}

impl ScanEventPreamble {
    /// The raw preamble bytes. Their length depends on the file version.
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub(crate) fn size_for_version(version: u32) -> usize {
        match version {
            0..=8 => 41,
            57 | 60 => 80,
            62 => 120,
            63 | 64 => 128,
            _ => 136, // v66+
        }
    }

    /// Polarity: byte 4.
    pub fn polarity(&self) -> Option<crate::Polarity> {
        self.bytes
            .get(4)
            .and_then(|&b| crate::Polarity::from_byte(b))
    }

    /// Scan mode (centroid/profile): byte 5.
    pub fn scan_mode(&self) -> Option<crate::ScanMode> {
        self.bytes
            .get(5)
            .and_then(|&b| crate::ScanMode::from_byte(b))
    }

    /// MS power: byte 6.
    pub fn ms_power(&self) -> Option<crate::MsPower> {
        self.bytes
            .get(6)
            .and_then(|&b| crate::MsPower::from_byte(b))
    }

    /// Scan type: byte 7.
    pub fn scan_type(&self) -> Option<crate::ScanType> {
        self.bytes
            .get(7)
            .and_then(|&b| crate::ScanType::from_byte(b))
    }

    /// Dependent scan flag: byte 10.
    pub fn is_dependent(&self) -> bool {
        self.bytes.get(10).copied() == Some(1)
    }

    /// True if this scan is a Data-Independent Acquisition (DIA) MS2+ scan:
    /// ms_power >= 2 and the dependent flag is NOT set.  In DIA mode the
    /// instrument selects a wide isolation window and fragments all ions in
    /// that window together, without targeting a specific precursor.
    pub fn is_dia(&self) -> bool {
        let ms_power = self.bytes.get(6).copied().unwrap_or(0);
        ms_power >= 2 && !self.is_dependent()
    }

    /// Ionization mode: byte 11.
    pub fn ionization(&self) -> Option<crate::Ionization> {
        self.bytes
            .get(11)
            .and_then(|&b| crate::Ionization::from_byte(b))
    }

    /// Activation method from the preamble alone: byte 24, or
    /// [`crate::Activation::ETD`] when the electron-transfer byte is set.
    ///
    /// This cannot tell EThcD from ETD, which needs the event's reactions;
    /// use [`ScanEvent::activation`] for a scan's activation.
    ///
    /// Byte 120 of a 136-byte (version 66) preamble is 1 on electron-transfer
    /// MSn events. Corpus evidence (PRIDE public files):
    ///
    /// - PXD006767 `EFM4_AspN_ETD_22032016.raw`,
    ///   `EFM4_lysargiNase_ETD_22032016.raw` and
    ///   `EFM7_lysargiNase_ETD_22032016.raw` (Orbitrap Velos Pro): the
    ///   embedded instrument method says "Activation Type: ETD" for every
    ///   dependent scan, and byte 120 is 1 on all 10,836 MS2 events (3,415,
    ///   3,266 and 4,155) and 0 on every MS1 event. Byte 24 is 4 on both MS1
    ///   and MS2 events, so it does not carry the method here. The reaction
    ///   energy field holds 130, the method's "Activation Time: 130.000",
    ///   where CID files hold the normalized collision energy.
    /// - PXD077396 `EC-OT30-25-190-EThcD-01/03/09.raw` (Orbitrap Eclipse) and
    ///   PXD070648 `..._1019_a.raw` and `..._991_a.raw` (Orbitrap Fusion
    ///   Lumos): the method text says "Is EThcD Active = True" and byte 120
    ///   is 1 on all 59,034 MS2 events.
    /// - Velos-family and tribrid files whose method declares only CID or HCD
    ///   (for example PXD051336 Velos Pro, PXD000790 Elite, PXD032800
    ///   Eclipse, PXD006062 Fusion Lumos) have 0 on every event; Q Exactive,
    ///   Exploris and Astral files have 2 on every event.
    ///
    /// The flag is only read when byte 24 holds a named code (1 or 4): files
    /// whose scan events are misread also show 1 at byte 120, mostly with
    /// byte 24 zero or out of range (one such event in PXD039587, whose
    /// method declares only CID, still decodes as ETD). The format docs
    /// (section 22.5) list the counts. Version 63 and 64 preambles also have a byte
    /// 120, but no corpus file of those versions uses ETD, so it is not read
    /// there.
    pub fn activation(&self) -> Option<crate::Activation> {
        let base = self
            .bytes
            .get(24)
            .and_then(|&b| crate::Activation::from_byte(b))?;
        let is_msn = self.bytes.get(6).is_some_and(|&p| p >= 2);
        let named = matches!(base, crate::Activation::HCD | crate::Activation::CID);
        if self.bytes.len() == 136 && is_msn && named && self.bytes.get(120) == Some(&1) {
            return Some(crate::Activation::ETD);
        }
        Some(base)
    }

    /// Wideband flag: byte 32, `true` only for 1. The Finnigan Perl module
    /// (Gene Selkov, `Finnigan::ScanEventPreamble`) names this byte
    /// "wideband" with 0 = off, 1 = on, 2 = undefined.
    pub fn is_wideband(&self) -> bool {
        self.bytes.get(32).copied() == Some(1)
    }

    /// Analyzer type: byte 40.
    pub fn analyzer(&self) -> Option<crate::Analyzer> {
        self.bytes
            .get(40)
            .and_then(|&b| crate::Analyzer::from_byte(b))
    }

    /// Raw value of byte 24, for diagnostics. [`Self::activation`] decodes it
    /// together with the electron-transfer byte (byte 120).
    pub fn activation_byte(&self) -> u8 {
        self.bytes.get(24).copied().unwrap_or(0)
    }
}

impl ScanEvent {
    /// Read one scan event.
    ///
    /// For v66 files, `body_primary` is the body size for primary (MS1) scans
    /// and `body_dependent` is the body size for dependent (MS2+) scans.
    /// For uniform-event files these two values are identical.
    /// Pass `(0, 0)` for pre-v66 files (body size is self-describing).
    pub(crate) fn read<R: Read + Seek>(
        r: &mut BinaryReader<R>,
        version: u32,
        body_primary: usize,
        body_dependent: usize,
    ) -> Result<Self> {
        let preamble_size = ScanEventPreamble::size_for_version(version);
        let preamble_bytes = r.read_bytes(preamble_size)?;
        let preamble = ScanEventPreamble {
            bytes: preamble_bytes,
        };

        if version >= 66 {
            // Select body size: primary (MS1) vs dependent (MS2+).
            // Primary = ms_power <= Ms1 AND not dependent.
            let is_primary = preamble.bytes.get(6).copied().unwrap_or(0) <= 1
                && preamble.bytes.get(10).copied() != Some(1);
            let body_size = if is_primary {
                body_primary
            } else {
                body_dependent
            };
            // Tribrid instruments (Eclipse, Fusion Lumos) use variable-length events
            // detected when body_primary != body_dependent.  Dependent events on
            // these instruments use a different body layout than QExactive/Exploris.
            let is_tribrid_dep = body_primary != body_dependent && !is_primary;
            Self::read_v66(r, preamble, body_size, is_tribrid_dep)
        } else {
            Self::read_pre_v66(r, preamble)
        }
    }

    /// V66 scan events have a fixed-size body (size determined by the caller
    /// from the stream's address-space: body_size = event_size - preamble_size).
    ///
    /// Two body layouts are in use across instrument families:
    ///
    /// **QExactive / Exploris / uniform-event files** (body_primary == body_dependent):
    ///   body[0..4]:   u32 unknown_long[0] (always 1)
    ///   body[4..8]:   u32 flags (0 for MS1, 0xA0000000 for MS2)
    ///   body[8..64]:  opaque fields (precursor-related for MS2, range aux for MS1)
    ///   body[fc_off..fc_off+16]: FractionCollector (scan window) at body_size-64
    ///
    /// **Tribrid dependent events** (Eclipse, Fusion Lumos; is_tribrid_dep=true):
    ///   body[0..4]:   u32 n_reactions  (0 for MS1, 1 for HCD, 2 for EThcD)
    ///   body[4..]:    n_reactions * 32-byte Reaction records
    ///   body[body_size-88..body_size-72]: FractionCollector (scan window)
    ///
    /// Calibration follows the FractionCollector in uniform events and
    /// 96-byte primary bodies; tribrid dependent bodies use body_size-64.
    fn read_v66<R: Read + Seek>(
        r: &mut BinaryReader<R>,
        preamble: ScanEventPreamble,
        body_size: usize,
        is_tribrid_dep: bool,
    ) -> Result<Self> {
        let body = r.read_bytes(body_size)?;

        // FractionCollector (scan window) location varies by instrument family:
        //   - Q Exactive / Exploris / Astral (body_size ≥ 136): offset 64
        //   - Orbitrap Elite / Fusion / Fusion Lumos / Velos Pro (body_size=96):
        //     MS1 → offset 8, MS2 → offset 64
        //   - Orbitrap Ascend (body_size=152) MS2 → offset 128
        //   - LTQ ion-trap only files (body_size < 96): offset 8
        // Empirically verified across a 24-file multi-instrument corpus.
        //
        // Strategy: try a small list of candidate offsets in priority order
        // and accept the first that yields a plausible m/z window. This is
        // robust across the observed zoo of body layouts.
        // Tribrid dependent events (Eclipse, Fusion Lumos) store the FractionCollector
        // at body[body_size-88] = body[120] for a 208-byte body.  All other v66
        // instruments use one of the legacy candidate offsets.
        let tribrid_fc = body_size.saturating_sub(88);
        let tribrid_candidates;
        let legacy_candidates;
        let candidates: &[usize] = if is_tribrid_dep {
            tribrid_candidates = [tribrid_fc, 8usize, 64, 128];
            &tribrid_candidates
        } else {
            legacy_candidates = [64usize, 8, 128, body_size.saturating_sub(80)];
            &legacy_candidates
        };
        let fc_match = candidates
            .iter()
            .copied()
            .find_map(|off| Some((off, plausible_window(&body, off)?)));
        let (fc_offset, fraction_collectors) = match fc_match {
            Some((off, fc)) => (Some(off), vec![fc]),
            None => (None, Vec::new()),
        };

        // The nparam + coefficients block immediately follows the scan-window
        // FractionCollector. On Q Exactive / Exploris / Astral (FC at offset 64)
        // that is offset 80; the legacy `body_size - 64` only coincides with it
        // when body_size == 144 (e.g. Q Exactive), so Exploris (body_size 136)
        // came back with no coefficients and its profile m/z was mis-converted.
        // In the 96-byte primary layout (e.g. Fusion Lumos), FC is at offset 8
        // and nparam is at 24, not body_size - 64 = 32. Limit this additional
        // case to the observed body size: short ion-trap and tribrid dependent
        // bodies have different tails and must keep their existing offsets.
        let np_off = match fc_offset {
            Some(64) => 80,
            Some(8)
                if body_size == 96
                    && preamble.bytes.get(6).copied().unwrap_or(0) <= 1
                    && !preamble.is_dependent() =>
            {
                24
            }
            _ => body_size.saturating_sub(64),
        };
        let coefficients = read_coefficients(&body, np_off)?;

        // Parse precursor reactions from the v66 body for dependent scans and for
        // non-dependent MS2+ scans (DIA mode). In DIA, MS2 scans are not flagged as
        // dependent but still carry one or more isolation window reactions in the body.
        //
        // Condition: parse reactions when ms_power >= 2 OR the scan is flagged dependent.
        // (MS1 primary scans with ms_power <= 1 and dependent=false are skipped.)
        let is_ms2_plus = preamble.bytes.get(6).copied().unwrap_or(0) >= 2;
        let mut reactions = if (!is_ms2_plus && !preamble.is_dependent()) || body_size < 8 {
            Vec::new()
        } else if is_tribrid_dep {
            // Tribrid dependent events (Eclipse, Fusion Lumos): n_reactions is stored at
            // body[0..4] and each 32-byte Reaction record begins at body[4].
            // The second reaction (if any) is typically a zero-mz supplemental step.
            let np = if body_size >= 4 {
                crate::bytes::read_u32_le(&body, 0)? as usize
            } else {
                0
            };
            let rxn_start = 4usize;
            let max_np = body_size.saturating_sub(rxn_start + 32) / 32;
            if np == 0 || np > max_np.max(1) {
                Vec::new()
            } else {
                let mut rxs = Vec::with_capacity(np);
                for i in 0..np {
                    let off = rxn_start + i * 32;
                    if off + 32 > body_size {
                        break;
                    }
                    let mz = crate::bytes::read_f64_le(&body, off)?;
                    let energy = crate::bytes::read_f64_le(&body, off + 16)?;
                    if mz.is_finite() && mz >= 0.0 {
                        rxs.push(Reaction {
                            precursor_mz: mz,
                            energy,
                        });
                    }
                }
                rxs
            }
        } else {
            let np = crate::bytes::read_u32_le(&body, 4)? as usize;
            // Sanity check: np must fit within the body minus minimum fixed overhead.
            // Each reaction is 32 bytes; require at least 32 bytes of post-reaction
            // data (FC=16, nparam=4, minimum tail) for the body to be plausible.
            let max_np = body_size.saturating_sub(8 + 32) / 32;
            if np == 0 || np > max_np.max(1) {
                Vec::new()
            } else {
                let mut rxs = Vec::with_capacity(np);
                for i in 0..np {
                    let off = 8 + i * 32;
                    if off + 32 > body_size {
                        break;
                    }
                    let mz = crate::bytes::read_f64_le(&body, off)?;
                    let energy = crate::bytes::read_f64_le(&body, off + 16)?;
                    // Accept only reactions with plausible m/z values (0 is valid for
                    // MS1 triggers; accept non-negative finite values).
                    if mz.is_finite() && mz >= 0.0 {
                        rxs.push(Reaction {
                            precursor_mz: mz,
                            energy,
                        });
                    }
                }
                rxs
            }
        };

        // Exploris v66 dependent scans store the reaction record starting at
        // body offset 4 (precursor m/z as an f64 at body[4..12]), with no
        // leading count word, so the offset-8 parse above finds nothing. Fall
        // back to that layout when an MS2+/dependent, non-tribrid scan yielded
        // no reaction.
        if reactions.is_empty()
            && (is_ms2_plus || preamble.is_dependent())
            && !is_tribrid_dep
            && body_size >= 36
        {
            let mz = crate::bytes::read_f64_le(&body, 4)?;
            if mz.is_finite() && mz > 0.0 && mz < 50_000.0 {
                reactions.push(Reaction {
                    precursor_mz: mz,
                    energy: crate::bytes::read_f64_le(&body, 20)?,
                });
            }
        }

        Ok(Self {
            preamble,
            reactions,
            fraction_collectors,
            coefficients,
        })
    }

    /// Read a v66 event whose body starts with its reaction count, as on
    /// Q Exactive and Exploris instruments: a u32 count, that many 56-byte
    /// reactions (precursor m/z at +0, activation energy at +16), a u32, the
    /// acquisition window, the coefficient count and the coefficients. This is
    /// the pre-v66 order with a longer reaction record.
    ///
    /// `body_size` comes from the stream walk, which sized the record by the
    /// same count.
    pub(crate) fn read_counted_v66<R: Read + Seek>(
        r: &mut BinaryReader<R>,
        body_size: usize,
    ) -> Result<Self> {
        const REACTION: usize = 56;
        let preamble = ScanEventPreamble {
            bytes: r.read_bytes(ScanEventPreamble::size_for_version(66))?,
        };
        let body = r.read_bytes(body_size)?;
        let n_reactions = crate::bytes::read_u32_le(&body, 0)? as usize;
        // Every event stores at least one reaction; a primary MS1 event's is
        // not a precursor.
        let is_primary =
            preamble.bytes.get(6).copied().unwrap_or(0) <= 1 && !preamble.is_dependent();
        let mut reactions = Vec::new();
        if !is_primary {
            for i in 0..n_reactions {
                let off = 4 + i * REACTION;
                let mz = crate::bytes::read_f64_le(&body, off)?;
                if mz.is_finite() && mz >= 0.0 {
                    reactions.push(Reaction {
                        precursor_mz: mz,
                        energy: crate::bytes::read_f64_le(&body, off + 16)?,
                    });
                }
            }
        }
        let fc_off = n_reactions.saturating_mul(REACTION).saturating_add(8);
        let fraction_collectors = plausible_window(&body, fc_off).into_iter().collect();
        let coefficients = read_coefficients(&body, fc_off.saturating_add(16))?;
        Ok(Self {
            preamble,
            reactions,
            fraction_collectors,
            coefficients,
        })
    }

    fn read_pre_v66<R: Read + Seek>(
        r: &mut BinaryReader<R>,
        preamble: ScanEventPreamble,
    ) -> Result<Self> {
        let np = r.read_u32()?;
        let mut reactions = Vec::new();
        for _ in 0..np {
            reactions.push(Reaction::read(r)?);
        }
        let _unk1 = r.read_u32()?;
        let fc = FractionCollector::read(r)?;
        let nparam = r.read_u32()?;
        r.check_count(nparam as u64, 8)?;
        let mut coefficients = Vec::with_capacity(nparam as usize);
        for _ in 0..nparam {
            coefficients.push(r.read_f64()?);
        }
        let _unk2 = r.read_u32()?;
        let _unk3 = r.read_u32()?;

        Ok(Self {
            preamble,
            reactions,
            fraction_collectors: vec![fc],
            coefficients,
        })
    }
}

// The acquisition window at `off` of a v66 body, if it is plausible: finite,
// monotonic, and within physically realistic m/z bounds (instruments top out
// well below 1e5 m/z). lo == hi is accepted because some SIM / tSIM scans use
// a single-point window.
fn plausible_window(body: &[u8], off: usize) -> Option<FractionCollector> {
    if off.saturating_add(16) > body.len() {
        return None;
    }
    let low_mz = crate::bytes::read_f64_le(body, off).ok()?;
    let high_mz = crate::bytes::read_f64_le(body, off + 8).ok()?;
    (low_mz.is_finite()
        && high_mz.is_finite()
        && low_mz >= 0.1
        && low_mz <= high_mz
        && high_mz <= 50_000.0)
        .then_some(FractionCollector { low_mz, high_mz })
}

// The coefficient count at `np_off` of a v66 body and the coefficients after it.
fn read_coefficients(body: &[u8], np_off: usize) -> Result<Vec<f64>> {
    let mut coefficients = Vec::new();
    if np_off.saturating_add(4) <= body.len() {
        let nparam_raw = crate::bytes::read_u32_le(body, np_off)? as usize;
        // Cap nparam at the number of f64s that actually fit in the remaining body.
        // Without this cap, a garbage nparam (e.g. 0xFFFFFFFF from uninitialised
        // bytes) causes billions of loop iterations just to evaluate the guard.
        let max_nparam = (body.len().saturating_sub(np_off + 4)) / 8;
        let nparam = nparam_raw.min(max_nparam);
        for i in 0..nparam {
            let off = np_off + 4 + i * 8;
            coefficients.push(crate::bytes::read_f64_le(body, off)?);
        }
    }
    Ok(coefficients)
}

impl Reaction {
    fn read<R: Read + Seek>(r: &mut BinaryReader<R>) -> Result<Self> {
        // Layout: precursor m/z (f64), an undecoded f64, energy (f64), two
        // undecoded u32s.
        let precursor_mz = r.read_f64()?;
        r.skip(8)?;
        let energy = r.read_f64()?;
        r.skip(8)?;
        Ok(Self {
            precursor_mz,
            energy,
        })
    }
}

impl FractionCollector {
    fn read<R: Read + Seek>(r: &mut BinaryReader<R>) -> Result<Self> {
        let low_mz = r.read_f64()?;
        let high_mz = r.read_f64()?;
        Ok(Self { low_mz, high_mz })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scan_data::{Profile, ProfileChunk};
    use std::io::Cursor;

    fn read_body(body_size: usize, fc_offset: usize, np_offset: usize, tribrid: bool) -> ScanEvent {
        let mut body = vec![0u8; body_size];
        body[fc_offset..fc_offset + 8].copy_from_slice(&350.0_f64.to_le_bytes());
        body[fc_offset + 8..fc_offset + 16].copy_from_slice(&1000.0_f64.to_le_bytes());
        // Calibration recorded in the public PXD031322 Fusion Lumos file.
        let coefficients = [
            0.0,
            0.0,
            0.0,
            211_782_331.2992454,
            -270_234_696.3002101,
            0.0,
            0.0,
        ];
        let count = if body_size == 136 { 5 } else { 7 };
        body[np_offset..np_offset + 4].copy_from_slice(&(count as u32).to_le_bytes());
        for (i, value) in coefficients[..count].iter().enumerate() {
            let off = np_offset + 4 + 8 * i;
            body[off..off + 8].copy_from_slice(&f64::to_le_bytes(*value));
        }
        let mut reader = BinaryReader::new(Cursor::new(body));
        let mut bytes = vec![0; 136];
        bytes[6] = if tribrid { 2 } else { 1 };
        ScanEvent::read_v66(&mut reader, ScanEventPreamble { bytes }, body_size, tribrid).unwrap()
    }

    #[test]
    fn lumos_primary_profile_uses_calibration_after_offset8_window() {
        let event = read_body(96, 8, 24, false);
        assert_eq!(event.coefficients.len(), 7);
        let profile = Profile {
            first_value: 755.4335627526089,
            step: 0.0,
            peak_count: 1,
            nbins: 1,
            chunks: vec![ProfileChunk {
                first_bin: 0,
                signal: vec![42.0],
                fudge: Some(0.0001),
            }],
        };
        let (mz, intensity) = profile.to_mz_intensity(&event.coefficients)[0];
        assert!(
            (mz - 371.1045838664921).abs() < 1e-9,
            "uncalibrated profile mass: {mz}"
        );
        assert_eq!(intensity, 42.0);
    }

    #[test]
    fn uniform_offset64_calibration_preserved() {
        for body_size in [136, 144] {
            let event = read_body(body_size, 64, 80, false);
            assert_eq!(
                event.coefficients.len(),
                if body_size == 136 { 5 } else { 7 }
            );
            assert_eq!(event.coefficients[3], 211_782_331.2992454);
        }
    }

    #[test]
    fn tribrid_dependent_calibration_keeps_eight_byte_gap() {
        let event = read_body(208, 120, 144, true);
        assert_eq!(event.coefficients.len(), 7);
        assert_eq!(event.coefficients[3], 211_782_331.2992454);
    }

    #[test]
    fn short_uncalibrated_ion_trap_body_stays_uncalibrated() {
        let mut body = vec![0u8; 32];
        body[8..16].copy_from_slice(&350.0_f64.to_le_bytes());
        body[16..24].copy_from_slice(&1000.0_f64.to_le_bytes());
        let mut reader = BinaryReader::new(Cursor::new(body));
        let event = ScanEvent::read_v66(
            &mut reader,
            ScanEventPreamble {
                bytes: vec![0; 136],
            },
            32,
            false,
        )
        .unwrap();
        assert!(event.coefficients.is_empty());
    }

    #[test]
    fn lumos_coefficient_count_is_bounded_by_body() {
        let mut body = vec![0u8; 96];
        body[8..16].copy_from_slice(&350.0_f64.to_le_bytes());
        body[16..24].copy_from_slice(&1000.0_f64.to_le_bytes());
        body[24..28].copy_from_slice(&u32::MAX.to_le_bytes());
        let mut reader = BinaryReader::new(Cursor::new(body));
        let event = ScanEvent::read_v66(
            &mut reader,
            ScanEventPreamble {
                bytes: vec![0; 136],
            },
            96,
            false,
        )
        .unwrap();
        assert_eq!(event.coefficients.len(), 8);
    }
}
