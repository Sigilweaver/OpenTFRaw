use crate::error::{Error, Result};
use crate::reader::BinaryReader;
use std::io::{Read, Seek, SeekFrom};

/// A single centroid peak (m/z + abundance).
#[derive(Debug, Clone)]
pub struct Peak {
    pub mz: f64,
    pub abundance: f32,
}

/// A contiguous chunk of profile signal data.
#[derive(Debug)]
pub struct ProfileChunk {
    pub first_bin: u32,
    pub signal: Vec<f32>,
    pub fudge: Option<f32>,
}

/// Profile spectrum data.
#[derive(Debug)]
pub struct Profile {
    pub first_value: f64,
    pub step: f64,
    pub peak_count: u32,
    pub nbins: u32,
    pub chunks: Vec<ProfileChunk>,
}

/// The header of a scan data packet (40 bytes).
#[derive(Debug)]
pub struct PacketHeader {
    pub profile_size: u32,
    pub peak_list_size: u32,
    pub layout: u32,
    pub descriptor_list_size: u32,
    pub unknown_stream_size: u32,
    pub triplet_stream_size: u32,
    pub low_mz: f32,
    pub high_mz: f32,
}

/// A complete scan data packet.
#[derive(Debug)]
pub struct ScanDataPacket {
    pub header: PacketHeader,
    pub profile: Option<Profile>,
    pub peaks: Vec<Peak>,
}

impl ScanDataPacket {
    pub(crate) fn read<R: Read + Seek>(r: &mut BinaryReader<R>) -> Result<Self> {
        let header = PacketHeader::read(r)?;

        // Profile data
        let profile = if header.profile_size > 0 {
            Some(Profile::read(r, header.layout)?)
        } else {
            None
        };

        // Centroid peak list
        // Layout bit 16 (0x10000) means m/z is f64 instead of f32
        let wide_mz = header.layout & 0x10000 != 0;
        let peaks = if header.peak_list_size > 0 {
            let count = r.read_u32()?;
            let mut peaks = Vec::with_capacity(count as usize);
            for _ in 0..count {
                let mz = if wide_mz {
                    r.read_f64()?
                } else {
                    r.read_f32()? as f64
                };
                let abundance = r.read_f32()?;
                peaks.push(Peak { mz, abundance });
            }
            peaks
        } else {
            Vec::new()
        };

        // Skip descriptor, unknown, and triplet streams
        if header.descriptor_list_size > 0 {
            r.skip((header.descriptor_list_size * 4) as usize)?;
        }
        if header.unknown_stream_size > 0 {
            r.skip((header.unknown_stream_size * 4) as usize)?;
        }
        if header.triplet_stream_size > 0 {
            r.skip((header.triplet_stream_size * 4) as usize)?;
        }

        Ok(Self {
            header,
            profile,
            peaks,
        })
    }

    /// Read only the centroided peak list, skipping the (potentially large)
    /// profile data. This is 2-10× faster than [`Self::read`] for high-
    /// resolution Orbitrap scans where profile_size can be tens of thousands
    /// of 4-byte words.
    pub(crate) fn read_peaks_only<R: Read + Seek>(r: &mut BinaryReader<R>) -> Result<Vec<Peak>> {
        let header = PacketHeader::read(r)?;

        // Skip profile data instead of decoding it.
        if header.profile_size > 0 {
            r.skip((header.profile_size as usize) * 4)?;
        }

        // Centroid peak list - same as full read path.
        let wide_mz = header.layout & 0x10000 != 0;
        let peaks = if header.peak_list_size > 0 {
            let count = r.read_u32()?;
            let mut peaks = Vec::with_capacity(count as usize);
            for _ in 0..count {
                let mz = if wide_mz {
                    r.read_f64()?
                } else {
                    r.read_f32()? as f64
                };
                let abundance = r.read_f32()?;
                peaks.push(Peak { mz, abundance });
            }
            peaks
        } else {
            Vec::new()
        };

        Ok(peaks)
    }
}

impl PacketHeader {
    fn read<R: Read + Seek>(r: &mut BinaryReader<R>) -> Result<Self> {
        let _unk1 = r.read_u32()?;
        let profile_size = r.read_u32()?;
        let peak_list_size = r.read_u32()?;
        let layout = r.read_u32()?;
        let descriptor_list_size = r.read_u32()?;
        let unknown_stream_size = r.read_u32()?;
        let triplet_stream_size = r.read_u32()?;
        let _unk2 = r.read_u32()?;
        let low_mz = r.read_f32()?;
        let high_mz = r.read_f32()?;

        Ok(Self {
            profile_size,
            peak_list_size,
            layout,
            descriptor_list_size,
            unknown_stream_size,
            triplet_stream_size,
            low_mz,
            high_mz,
        })
    }
}

impl Profile {
    fn read<R: Read + Seek>(r: &mut BinaryReader<R>, layout: u32) -> Result<Self> {
        let first_value = r.read_f64()?;
        let step = r.read_f64()?;
        let peak_count = r.read_u32()?;
        let nbins = r.read_u32()?;

        let has_fudge = layout & 0xFF != 0;
        let mut chunks = Vec::with_capacity(peak_count as usize);
        for _ in 0..peak_count {
            let first_bin = r.read_u32()?;
            let chunk_nbins = r.read_u32()?;
            let fudge = if has_fudge { Some(r.read_f32()?) } else { None };
            let mut signal = Vec::with_capacity(chunk_nbins as usize);
            for _ in 0..chunk_nbins {
                signal.push(r.read_f32()?);
            }
            chunks.push(ProfileChunk {
                first_bin,
                signal,
                fudge,
            });
        }

        Ok(Self {
            first_value,
            step,
            peak_count,
            nbins,
            chunks,
        })
    }
}

impl Profile {
    /// Convert profile bins to (mz, intensity) pairs using the conversion coefficients.
    pub fn to_mz_intensity(&self, coefficients: &[f64]) -> Vec<(f64, f64)> {
        let mut result = Vec::with_capacity(self.nbins as usize);
        for chunk in &self.chunks {
            for (i, &intensity) in chunk.signal.iter().enumerate() {
                let bin_global = chunk.first_bin as f64 + i as f64;
                let freq = self.first_value + bin_global * self.step;
                let freq_adj = if let Some(fudge) = chunk.fudge {
                    freq + fudge as f64
                } else {
                    freq
                };
                let mz = freq_to_mz(freq_adj, coefficients);
                result.push((mz, intensity as f64));
            }
        }
        result
    }
}

/// Convert frequency to m/z using the conversion coefficients from the scan event.
///
/// The coefficient array includes metadata prefix values:
/// - nparam=4 (LTQ-FT/ICR): [unknown, A, B, C] → Mz = A + B/f + C/f²
/// - nparam=5 (Orbitrap v66): [unk0, unk1, A, B, C] → Mz = A + B/f² + C/f⁴
/// - nparam=7 (Orbitrap): [unknown, I, A, B, C, D, E] → Mz = A + B/f² + C/f⁴
pub fn freq_to_mz(freq: f64, coefficients: &[f64]) -> f64 {
    if freq == 0.0 {
        return 0.0;
    }
    match coefficients.len() {
        0 => freq, // No conversion (already m/z domain, e.g. ITMS)
        4 => {
            // LTQ-FT / ICR: Mz = A + B/f + C/f²
            let (a, b, c) = (coefficients[1], coefficients[2], coefficients[3]);
            a + b / freq + c / (freq * freq)
        }
        5 => {
            // Orbitrap v66: Mz = A + B/f² + C/f⁴
            let (a, b, c) = (coefficients[2], coefficients[3], coefficients[4]);
            let f2 = freq * freq;
            a + b / f2 + c / (f2 * f2)
        }
        7 => {
            // Orbitrap: Mz = A + B/f² + C/f⁴
            let (a, b, c) = (coefficients[2], coefficients[3], coefficients[4]);
            let f2 = freq * freq;
            a + b / f2 + c / (f2 * f2)
        }
        _ => freq,
    }
}

/// Read a flat-peak scan (TSQ/SRM format).
///
/// In this format, the scan data stream at `data_addr` contains variable-length records.
/// Each scan index entry's `offset` field holds the **cumulative end byte offset** within
/// the data stream. Peaks are stored as contiguous (f32 mz, f32 intensity) pairs at the
/// end of each record, followed by `peak_count` flag bytes (1 byte per peak).
///
/// `peak_count` is typically `data_size - 1`.
pub fn read_flat_peaks<R: Read + Seek>(
    source: &mut R,
    data_addr: u64,
    cum_end: u64,
    data_size: u32,
) -> Result<Vec<Peak>> {
    if data_size <= 1 {
        return Ok(Vec::new());
    }

    // Try peak_count = data_size - 1 first, then data_size - 2 as fallback.
    // Each peak occupies 9 bytes total: 8 bytes (f32 mz + f32 int) + 1 flag byte.
    // The peaks section is at the end of the record.
    for subtract in [1u32, 2] {
        if data_size <= subtract {
            continue;
        }
        let peak_count = (data_size - subtract) as usize;
        let peak_section_bytes = peak_count as u64 * 9;
        if peak_section_bytes > cum_end {
            continue;
        }
        let peaks_start = data_addr + cum_end - peak_section_bytes;
        source.seek(SeekFrom::Start(peaks_start))?;
        let mut r = BinaryReader::new(&mut *source);

        let mut peaks = Vec::with_capacity(peak_count);
        for _ in 0..peak_count {
            let mz = r.read_f32()? as f64;
            let abundance = r.read_f32()?;
            peaks.push(Peak { mz, abundance });
        }

        // Validate: first peak mz should be a plausible mass value (or zero for empty transitions)
        let looks_valid = if let Some(first) = peaks.first() {
            first.mz == 0.0 || (first.mz > 10.0 && first.mz < 10_000.0)
        } else {
            true
        };

        if looks_valid {
            return Ok(peaks);
        }
    }

    // Neither worked; return empty
    Err(Error::UnexpectedEof {
        offset: data_addr + cum_end,
        needed: 0,
    })
}

/// Read a flat-peak scan in v66 (TSQ Quantiva / TSQ Altis) SRM format.
///
/// In this format, the scan data stream contains fixed-size records.
/// Each scan index entry's `offset` field holds the **start byte offset** within
/// the stream (not cumulative end), and `record_size` is the number of bytes per record.
///
/// Record layout:
/// - bytes 0-3: u32 `n_peaks` (number of active SRM transitions in this window)
/// - bytes 4-31: other header fields (skipped)
/// - bytes 32..32+n_peaks*8: m/z window table, one (lo_mz: f32, hi_mz: f32) pair per peak
/// - bytes 32+n_peaks*8..: peak data, one (channel_idx: u32, mz: f32, intensity: f32) per peak
pub fn read_scan_srm_v66<R: Read + Seek>(
    source: &mut R,
    data_addr: u64,
    start_offset: u64,
    _record_size: u32,
) -> Result<Vec<Peak>> {
    let abs_start = data_addr + start_offset;
    source.seek(SeekFrom::Start(abs_start))?;
    let mut r = BinaryReader::new(source);

    // n_peaks at byte 0
    let n_peaks = r.read_u32()? as usize;
    if n_peaks == 0 {
        return Ok(Vec::new());
    }

    // Skip remaining header: bytes 4-31 (28 bytes)
    r.skip(28)?;

    // Skip m/z window table: n_peaks × 8 bytes (lo_mz f32 + hi_mz f32 per channel)
    r.skip(n_peaks * 8)?;

    // Read peak records: (u32 channel_idx, f32 mz, f32 intensity) × n_peaks
    let mut peaks = Vec::with_capacity(n_peaks);
    for _ in 0..n_peaks {
        let _channel = r.read_u32()?;
        let mz = r.read_f32()? as f64;
        let abundance = r.read_f32()?;
        peaks.push(Peak { mz, abundance });
    }

    Ok(peaks)
}

/// Read the Q3 isolation window table from an SRM v66 scan record.
///
/// Returns one `(lo_mz, hi_mz)` pair per active transition channel,
/// in channel order.
pub fn read_scan_srm_v66_windows<R: Read + Seek>(
    source: &mut R,
    data_addr: u64,
    start_offset: u64,
) -> Result<Vec<(f32, f32)>> {
    let abs_start = data_addr + start_offset;
    source.seek(SeekFrom::Start(abs_start))?;
    let mut r = BinaryReader::new(source);

    // n_peaks at byte 0
    let n_peaks = r.read_u32()? as usize;
    if n_peaks == 0 {
        return Ok(Vec::new());
    }

    // Skip remaining header: bytes 4-31 (28 bytes)
    r.skip(28)?;

    // Read m/z window table: n_peaks × 8 bytes (lo_mz f32, hi_mz f32)
    let mut windows = Vec::with_capacity(n_peaks);
    for _ in 0..n_peaks {
        let lo = r.read_f32()?;
        let hi = r.read_f32()?;
        windows.push((lo, hi));
    }

    Ok(windows)
}

/// Search the pre-data method/transition table for a v63 SRM transition record
/// matching the given Q3 center mass.
///
/// v63 (TSQ Quantum/Vantage) transition table layout: 72 bytes per channel record.
/// Relevant fields (all f64 little-endian):
///   - [+ 0] active-channel flag (1.0 for first channel of each precursor)
///   - [+ 8] unknown
///   - [+16] Q1 precursor mass (m/z)   ← returned
///   - [+24] Q3 center mass (m/z)      ← anchor for search
///   - [+32] Q3 window width (Da)      ← returned
///   - [+40] dwell time (s)
///   - [+48] collision energy (eV)     ← returned
///
/// Returns `(Q1, Q3_width, CE_eV)` if a plausible match is found.
pub fn search_v63_transition(data: &[u8], q3_center_target: f64) -> Option<(f64, f64, f64)> {
    let end = data.len().saturating_sub(32);
    for j in 8..end {
        if j + 8 > data.len() {
            break;
        }
        let v = f64::from_le_bytes(data[j..j + 8].try_into().ok()?);
        if (v - q3_center_target).abs() > 0.002 {
            continue;
        }
        // Candidate Q3_center at position j. Q1 is 8 bytes before.
        let q1 = f64::from_le_bytes(data[j - 8..j].try_into().ok()?);
        if !q1.is_finite() || !(50.0..=3000.0).contains(&q1) {
            continue;
        }
        // Q3_width is 8 bytes after Q3_center.
        if j + 16 > data.len() {
            continue;
        }
        let q3w = f64::from_le_bytes(data[j + 8..j + 16].try_into().ok()?);
        if !q3w.is_finite() || !(0.01..=10.0).contains(&q3w) {
            continue;
        }
        // CE is 24 bytes after Q3_center.
        if j + 32 > data.len() {
            continue;
        }
        let ce = f64::from_le_bytes(data[j + 24..j + 32].try_into().ok()?);
        if !ce.is_finite() || !(0.1..=300.0).contains(&ce) {
            continue;
        }
        return Some((q1, q3w, ce));
    }
    None
}
