use crate::error::Result;
use crate::reader::BinaryReader;
use std::io::{Read, Seek};

/// Precursor reaction info for MS2+ scans (32 bytes).
#[derive(Debug)]
pub struct Reaction {
    pub precursor_mz: f64,
    pub unknown_double: f64,
    pub energy: f64,
    pub unknown_long1: u32,
    pub unknown_long2: u32,
}

/// M/z acquisition range.
#[derive(Debug)]
pub struct FractionCollector {
    pub low_mz: f64,
    pub high_mz: f64,
}

/// Scan event preamble — byte array encoding scan parameters.
#[derive(Debug)]
pub struct ScanEventPreamble {
    pub bytes: Vec<u8>,
}

/// Complete scan event.
#[derive(Debug)]
pub struct ScanEvent {
    pub preamble: ScanEventPreamble,
    pub reactions: Vec<Reaction>,
    pub fraction_collectors: Vec<FractionCollector>,
    pub coefficients: Vec<f64>,
}

impl ScanEventPreamble {
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

    /// Activation method: byte 24.
    pub fn activation(&self) -> Option<crate::Activation> {
        self.bytes
            .get(24)
            .and_then(|&b| crate::Activation::from_byte(b))
    }

    /// Wideband (broadband isolation) flag: byte 32.
    pub fn is_wideband(&self) -> bool {
        self.bytes.get(32).copied() == Some(1)
    }

    /// Analyzer type: byte 40.
    pub fn analyzer(&self) -> Option<crate::Analyzer> {
        self.bytes
            .get(40)
            .and_then(|&b| crate::Analyzer::from_byte(b))
    }

    /// Raw value of the activation byte (byte 24). Useful for diagnostics when
    /// `activation()` returns `None` (unrecognised code).
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
            let body_size = if is_primary { body_primary } else { body_dependent };
            Self::read_v66(r, preamble, body_size)
        } else {
            Self::read_pre_v66(r, preamble)
        }
    }

    /// V66 scan events have a fixed-size body (size determined by the caller
    /// from the stream's address-space: body_size = event_size - preamble_size).
    ///
    /// Empirically verified layout (for Q Exactive HF-X with 144-byte body):
    ///   body[0..4]:   u32 unknown_long[0] (always 1)
    ///   body[4..8]:   u32 flags (0 for MS1, 0xA0000000 for MS2)
    ///   body[8..64]:  opaque fields (precursor-related for MS2, range aux for MS1)
    ///   body[fc_off..fc_off+16]: FractionCollector (scan window) at body_size-64
    ///   body[np_off..np_off+4]:  u32 nparam at body_size-60
    ///   body[np_off+4..]:        f64[nparam] coefficients
    ///
    /// The FC and nparam are at fixed offsets from the END of the body,
    /// which is what allows this to generalize across different body sizes.
    fn read_v66<R: Read + Seek>(
        r: &mut BinaryReader<R>,
        preamble: ScanEventPreamble,
        body_size: usize,
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
        let candidates: &[usize] = &[64, 8, 128, body_size.saturating_sub(80)];
        let fraction_collectors = candidates
            .iter()
            .copied()
            .find_map(|off| {
                if off + 16 > body_size {
                    return None;
                }
                let low_mz =
                    f64::from_le_bytes(body[off..off + 8].try_into().unwrap());
                let high_mz =
                    f64::from_le_bytes(body[off + 8..off + 16].try_into().unwrap());
                // A valid scan window must be finite, monotonic, and within
                // physically realistic m/z bounds (instruments top out well
                // below 1e5 m/z). Accept lo == hi as well because some
                // SIM / tSIM scans use a single-point window.
                if low_mz.is_finite()
                    && high_mz.is_finite()
                    && low_mz >= 0.1
                    && low_mz <= high_mz
                    && high_mz <= 50_000.0
                {
                    Some(vec![FractionCollector { low_mz, high_mz }])
                } else {
                    None
                }
            })
            .unwrap_or_default();

        // nparam + coefficients live at a fixed offset from the end of the body
        // (body_size - 64). This is independent of the FC location and is
        // consistent across all v66 instruments in the corpus.
        let np_off = body_size.saturating_sub(64);
        let mut coefficients = Vec::new();
        if np_off + 4 <= body_size {
            let nparam_raw =
                u32::from_le_bytes(body[np_off..np_off + 4].try_into().unwrap()) as usize;
            // Cap nparam at the number of f64s that actually fit in the remaining body.
            // Without this cap, a garbage nparam (e.g. 0xFFFFFFFF from uninitialised
            // bytes) causes billions of loop iterations just to evaluate the guard.
            let max_nparam = (body_size.saturating_sub(np_off + 4)) / 8;
            let nparam = nparam_raw.min(max_nparam);
            for i in 0..nparam {
                let off = np_off + 4 + i * 8;
                coefficients
                    .push(f64::from_le_bytes(body[off..off + 8].try_into().unwrap()));
            }
        }

        // Parse precursor reactions from the v66 body for dependent scans and for
        // non-dependent MS2+ scans (DIA mode). In DIA, MS2 scans are not flagged as
        // dependent but still carry one or more isolation window reactions in the body.
        //
        // Condition: parse reactions when ms_power >= 2 OR the scan is flagged dependent.
        // (MS1 primary scans with ms_power <= 1 and dependent=false are skipped.)
        let is_ms2_plus = preamble.bytes.get(6).copied().unwrap_or(0) >= 2;
        let reactions = if (!is_ms2_plus && !preamble.is_dependent()) || body_size < 8 {
            Vec::new()
        } else {
            let np = u32::from_le_bytes(body[4..8].try_into().unwrap()) as usize;
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
                    let mz = f64::from_le_bytes(body[off..off + 8].try_into().unwrap());
                    let unk = f64::from_le_bytes(body[off + 8..off + 16].try_into().unwrap());
                    let energy = f64::from_le_bytes(body[off + 16..off + 24].try_into().unwrap());
                    let ul1 = u32::from_le_bytes(body[off + 24..off + 28].try_into().unwrap());
                    let ul2 = u32::from_le_bytes(body[off + 28..off + 32].try_into().unwrap());
                    // Accept only reactions with plausible m/z values (0 is valid for
                    // MS1 triggers; accept non-negative finite values).
                    if mz.is_finite() && mz >= 0.0 {
                        rxs.push(Reaction {
                            precursor_mz: mz,
                            unknown_double: unk,
                            energy,
                            unknown_long1: ul1,
                            unknown_long2: ul2,
                        });
                    }
                }
                rxs
            }
        };

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

impl Reaction {
    fn read<R: Read + Seek>(r: &mut BinaryReader<R>) -> Result<Self> {
        let precursor_mz = r.read_f64()?;
        let unknown_double = r.read_f64()?;
        let energy = r.read_f64()?;
        let unknown_long1 = r.read_u32()?;
        let unknown_long2 = r.read_u32()?;
        Ok(Self {
            precursor_mz,
            unknown_double,
            energy,
            unknown_long1,
            unknown_long2,
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
