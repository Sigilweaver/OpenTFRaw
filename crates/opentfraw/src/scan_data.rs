use crate::error::{Error, Result};
use crate::reader::BinaryReader;
use std::io::{Read, Seek, SeekFrom};

/// A single centroid peak (m/z + abundance).
#[derive(Debug, Clone)]
pub struct Peak {
    pub mz: f64,
    pub abundance: f32,
}

/// A node in the per-scan noise-vs-m/z function carried by FT scans.
///
/// Thermo stores noise and baseline as a piecewise-linear function of m/z
/// (a few dozen nodes), not per peak. Per-peak noise/baseline are recovered
/// by interpolating this function at each peak's m/z (see
/// [`ScanDataPacket::noise_at`]).
#[derive(Debug, Clone, Copy)]
pub struct NoiseNode {
    pub mz: f32,
    pub noise: f32,
    pub baseline: f32,
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
    /// Per-peak resolution, aligned 1:1 with `peaks`. Empty when the scan
    /// carries no FT label data (e.g. ion-trap scans) or its layout did not
    /// match the expected encoding.
    pub resolutions: Vec<f32>,
    /// Nodes of the scan's noise-vs-m/z function. Empty when absent. Use
    /// [`Self::noise_at`] to evaluate per-peak noise/baseline.
    pub noise_nodes: Vec<NoiseNode>,
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

        let peaks = Self::read_peaks(r, &header)?;
        let (resolutions, noise_nodes) = Self::read_labels(r, &header, peaks.len())?;

        Ok(Self {
            header,
            profile,
            peaks,
            resolutions,
            noise_nodes,
        })
    }

    /// Read peaks and FT label data while skipping the (potentially large)
    /// profile signal. This is the fast path when only centroids and their
    /// labels (resolution / noise / baseline) are needed.
    pub(crate) fn read_skip_profile<R: Read + Seek>(r: &mut BinaryReader<R>) -> Result<Self> {
        let header = PacketHeader::read(r)?;

        if header.profile_size > 0 {
            r.skip((header.profile_size as usize) * 4)?;
        }

        let peaks = Self::read_peaks(r, &header)?;
        let (resolutions, noise_nodes) = Self::read_labels(r, &header, peaks.len())?;

        Ok(Self {
            header,
            profile: None,
            peaks,
            resolutions,
            noise_nodes,
        })
    }

    /// Read only the centroided peak list, skipping the profile data and the
    /// trailing label streams. This is 2-10× faster than [`Self::read`] for
    /// high-resolution Orbitrap scans where profile_size can be tens of
    /// thousands of 4-byte words.
    pub(crate) fn read_peaks_only<R: Read + Seek>(r: &mut BinaryReader<R>) -> Result<Vec<Peak>> {
        let header = PacketHeader::read(r)?;

        // Skip profile data instead of decoding it.
        if header.profile_size > 0 {
            r.skip((header.profile_size as usize) * 4)?;
        }

        Self::read_peaks(r, &header)
    }

    /// Decode the centroid peak list. Assumes the cursor sits at the start of
    /// the peak list (i.e. profile data has already been read or skipped).
    fn read_peaks<R: Read + Seek>(
        r: &mut BinaryReader<R>,
        header: &PacketHeader,
    ) -> Result<Vec<Peak>> {
        // Layout bit 16 (0x10000) means m/z is f64 instead of f32.
        let wide_mz = header.layout & 0x10000 != 0;
        if header.peak_list_size == 0 {
            return Ok(Vec::new());
        }
        let count = r.read_u32()?;
        let item_size: u64 = if wide_mz { 12 } else { 8 };
        r.check_count(count as u64, item_size)?;
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
        Ok(peaks)
    }

    /// Decode the per-peak label streams that follow the centroid peak list:
    /// descriptor (peak index, skipped), unknown (per-peak resolution), and
    /// triplet (noise-vs-m/z function). Assumes the cursor sits immediately
    /// after the peak list.
    fn read_labels<R: Read + Seek>(
        r: &mut BinaryReader<R>,
        header: &PacketHeader,
        n_peaks: usize,
    ) -> Result<(Vec<f32>, Vec<NoiseNode>)> {
        // Descriptor stream: one u32 per peak (a peak index), not label data.
        //
        // Cast to usize *before* multiplying: `descriptor_list_size` is an
        // untrusted u32 read straight from the file, and multiplying by 4 in
        // u32 arithmetic first can overflow (and panic, in a debug/fuzz
        // build) for values above ~1.07 billion. Widening first is safe
        // because `usize` is 64-bit on every platform this crate targets.
        if header.descriptor_list_size > 0 {
            r.skip(header.descriptor_list_size as usize * 4)?;
        }

        // Unknown stream: for FT scans this is `[count_word, resolution...]`
        // with one f32 resolution per centroid peak. Decode it only when the
        // size matches that layout; otherwise skip it so other encodings are
        // left untouched.
        let resolutions = if n_peaks > 0 && header.unknown_stream_size as usize == n_peaks + 1 {
            let _count = r.read_u32()?;
            r.check_count(n_peaks as u64, 4)?;
            let mut res = Vec::with_capacity(n_peaks);
            for _ in 0..n_peaks {
                res.push(r.read_f32()?);
            }
            res
        } else {
            // Same overflow hazard as the descriptor stream above: widen
            // before multiplying.
            if header.unknown_stream_size > 0 {
                r.skip(header.unknown_stream_size as usize * 4)?;
            }
            Vec::new()
        };

        // Triplet stream: the noise-vs-m/z function as (m/z, noise, baseline)
        // f32 nodes. Per-peak noise/baseline come from interpolating this at
        // the peak m/z (see `noise_at`).
        let node_count = header.triplet_stream_size / 3;
        r.check_count(node_count as u64, 12)?;
        let mut noise_nodes = Vec::with_capacity(node_count as usize);
        for _ in 0..node_count {
            let mz = r.read_f32()?;
            let noise = r.read_f32()?;
            let baseline = r.read_f32()?;
            noise_nodes.push(NoiseNode {
                mz,
                noise,
                baseline,
            });
        }
        // Skip any trailing words if the stream size is not a multiple of 3.
        // The remainder here is always < 3 (by construction of `node_count`
        // as a floor division), so widening before multiplying isn't
        // strictly required for overflow safety, but it matches the pattern
        // used everywhere else in this function for consistency.
        let consumed = node_count * 3;
        if header.triplet_stream_size > consumed {
            r.skip((header.triplet_stream_size - consumed) as usize * 4)?;
        }

        Ok((resolutions, noise_nodes))
    }

    /// Linearly interpolate `(noise, baseline)` at `mz` from the scan's
    /// noise-vs-m/z function. Returns `None` when the scan carries no noise
    /// nodes. Queries outside the node range clamp to the nearest endpoint.
    pub fn noise_at(&self, mz: f64) -> Option<(f32, f32)> {
        let nodes = &self.noise_nodes;
        if nodes.is_empty() {
            return None;
        }
        let x = mz as f32;
        if x <= nodes[0].mz {
            return Some((nodes[0].noise, nodes[0].baseline));
        }
        for w in nodes.windows(2) {
            let (lo, hi) = (&w[0], &w[1]);
            if x <= hi.mz {
                let span = hi.mz - lo.mz;
                let f = if span > 0.0 { (x - lo.mz) / span } else { 0.0 };
                let noise = lo.noise + f * (hi.noise - lo.noise);
                let baseline = lo.baseline + f * (hi.baseline - lo.baseline);
                return Some((noise, baseline));
            }
        }
        let last = nodes[nodes.len() - 1];
        Some((last.noise, last.baseline))
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
        // Minimum on-disk size of one chunk: first_bin(4) + chunk_nbins(4).
        r.check_count(peak_count as u64, 8)?;
        let mut chunks = Vec::with_capacity(peak_count as usize);
        for _ in 0..peak_count {
            let first_bin = r.read_u32()?;
            let chunk_nbins = r.read_u32()?;
            let fudge = if has_fudge { Some(r.read_f32()?) } else { None };
            r.check_count(chunk_nbins as u64, 4)?;
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
        // Size the allocation from the actually-parsed (and therefore already
        // bounded) chunk signal lengths, not from `self.nbins`: that field is
        // an untrusted file-provided value not otherwise validated against
        // the real signal data, so using it directly here would reopen the
        // same unbounded-allocation issue the read-path checks close off.
        let cap: usize = self.chunks.iter().map(|c| c.signal.len()).sum();
        let mut result = Vec::with_capacity(cap);
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
        // `data_addr` and `cum_end` are both untrusted u64 values read
        // straight from the file (a RawFileInfo address and a ScanIndexEntry
        // offset respectively); saturate instead of panicking on overflow
        // for a corrupt/crafted combination of the two.
        let peaks_start = data_addr
            .saturating_add(cum_end)
            .saturating_sub(peak_section_bytes);
        source.seek(SeekFrom::Start(peaks_start))?;
        let mut r = BinaryReader::new(&mut *source);

        r.check_count(peak_count as u64, 8)?;
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
        offset: data_addr.saturating_add(cum_end),
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
    // Both are untrusted u64 values read from the file; saturate
    // instead of panicking on overflow for a corrupt combination.
    let abs_start = data_addr.saturating_add(start_offset);
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
    r.check_count(n_peaks as u64, 12)?;
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
    // Both are untrusted u64 values read from the file; saturate
    // instead of panicking on overflow for a corrupt combination.
    let abs_start = data_addr.saturating_add(start_offset);
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
    r.check_count(n_peaks as u64, 8)?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn header_with(
        descriptor_list_size: u32,
        unknown_stream_size: u32,
        triplet_stream_size: u32,
    ) -> PacketHeader {
        PacketHeader {
            profile_size: 0,
            peak_list_size: 0,
            layout: 0,
            descriptor_list_size,
            unknown_stream_size,
            triplet_stream_size,
            low_mz: 0.0,
            high_mz: 0.0,
        }
    }

    /// Regression test for a fuzzer-found crash: `descriptor_list_size` and
    /// `unknown_stream_size` are untrusted u32 values read straight from the
    /// file. Multiplying by 4 in u32 arithmetic (rather than widening first)
    /// overflows - and panics under debug assertions - for values at or
    /// above `u32::MAX / 4 + 1`. `read_labels` must reject or skip past
    /// these cleanly instead of panicking.
    #[test]
    fn huge_descriptor_list_size_does_not_overflow() {
        let header = header_with(u32::MAX / 4 + 1, 0, 0);
        let mut r = BinaryReader::new(Cursor::new(vec![0u8; 8]));
        // Skipping past EOF is allowed (only a subsequent read would fail);
        // the point of this test is that it must not panic.
        let _ = ScanDataPacket::read_labels(&mut r, &header, 0);
    }

    #[test]
    fn huge_unknown_stream_size_does_not_overflow() {
        // n_peaks = 0 so the `unknown_stream_size == n_peaks + 1` decode
        // branch is not taken and the plain skip path (the one that
        // crashed) runs instead.
        let header = header_with(0, u32::MAX / 4 + 1, 0);
        let mut r = BinaryReader::new(Cursor::new(vec![0u8; 8]));
        let _ = ScanDataPacket::read_labels(&mut r, &header, 0);
    }

    #[test]
    fn read_labels_with_no_streams_returns_empty() {
        let header = header_with(0, 0, 0);
        let mut r = BinaryReader::new(Cursor::new(Vec::new()));
        let (resolutions, noise_nodes) = ScanDataPacket::read_labels(&mut r, &header, 0).unwrap();
        assert!(resolutions.is_empty());
        assert!(noise_nodes.is_empty());
    }

    #[test]
    fn read_labels_decodes_resolutions_when_size_matches() {
        // unknown_stream_size == n_peaks + 1 selects the resolution-decoding
        // branch: a leading count word followed by one f32 per peak.
        let header = header_with(0, 3, 0);
        let mut bytes = 2u32.to_le_bytes().to_vec(); // count word
        bytes.extend_from_slice(&1.5f32.to_le_bytes());
        bytes.extend_from_slice(&2.5f32.to_le_bytes());
        let mut r = BinaryReader::new(Cursor::new(bytes));
        let (resolutions, _) = ScanDataPacket::read_labels(&mut r, &header, 2).unwrap();
        assert_eq!(resolutions, vec![1.5, 2.5]);
    }

    #[test]
    fn read_labels_decodes_noise_nodes() {
        let header = header_with(0, 0, 3); // one (mz, noise, baseline) node
        let mut bytes = 100.0f32.to_le_bytes().to_vec();
        bytes.extend_from_slice(&5.0f32.to_le_bytes());
        bytes.extend_from_slice(&1.0f32.to_le_bytes());
        let mut r = BinaryReader::new(Cursor::new(bytes));
        let (_, noise_nodes) = ScanDataPacket::read_labels(&mut r, &header, 0).unwrap();
        assert_eq!(noise_nodes.len(), 1);
        assert_eq!(noise_nodes[0].mz, 100.0);
        assert_eq!(noise_nodes[0].noise, 5.0);
        assert_eq!(noise_nodes[0].baseline, 1.0);
    }

    #[test]
    fn read_peaks_rejects_implausible_count_before_allocating() {
        // peak_list_size must be nonzero or read_peaks short-circuits to an
        // empty Vec before ever reading a count.
        let mut header = header_with(0, 0, 0);
        header.peak_list_size = 1;
        let mut bytes = u32::MAX.to_le_bytes().to_vec(); // declared peak count
        bytes.extend_from_slice(&[0u8; 4]); // far short of what's needed
        let mut r = BinaryReader::new(Cursor::new(bytes));
        let err = ScanDataPacket::read_peaks(&mut r, &header).unwrap_err();
        assert!(matches!(err, Error::AllocationTooLarge { .. }));
    }

    #[test]
    fn read_peaks_empty_when_peak_list_size_zero() {
        let header = header_with(0, 0, 0);
        let mut r = BinaryReader::new(Cursor::new(Vec::new()));
        assert_eq!(
            ScanDataPacket::read_peaks(&mut r, &header).unwrap().len(),
            0
        );
    }

    #[test]
    fn read_flat_peaks_decodes_valid_record() {
        // data_addr=0, cum_end = end offset of this scan's record. One peak
        // (mz=500.0, abundance=10.0) plus one trailing flag byte, so
        // data_size = peak_count(1) + 1 = 2.
        let mut bytes = vec![0u8; 100]; // leading padding
        let peak_start = bytes.len();
        bytes.extend_from_slice(&500.0f32.to_le_bytes());
        bytes.extend_from_slice(&10.0f32.to_le_bytes());
        bytes.push(0); // flag byte accounted for by `data_size - 1`
        let cum_end = (bytes.len()) as u64;
        let mut cursor = Cursor::new(bytes);
        let peaks = read_flat_peaks(&mut cursor, 0, cum_end, 2).unwrap();
        assert_eq!(peaks.len(), 1);
        assert_eq!(peaks[0].mz, 500.0);
        assert_eq!(peaks[0].abundance, 10.0);
        let _ = peak_start; // silence unused-var warning if layout changes
    }

    #[test]
    fn read_flat_peaks_rejects_implausible_peak_count() {
        // data_size implies a peak_count that could never fit in the file.
        let bytes = vec![0u8; 16];
        let mut cursor = Cursor::new(bytes);
        let err = read_flat_peaks(&mut cursor, 0, 16, u32::MAX).unwrap_err();
        // Either our new allocation guard or the pre-existing
        // "peak_section_bytes > cum_end" guard is an acceptable rejection
        // reason; a panic or successful huge allocation is not.
        match err {
            Error::AllocationTooLarge { .. } | Error::UnexpectedEof { .. } => {}
            other => panic!("unexpected error variant: {other:?}"),
        }
    }

    #[test]
    fn read_scan_srm_v66_decodes_valid_record() {
        // n_peaks=1, 28 bytes of skipped header, one (lo,hi) window pair,
        // then one (channel, mz, intensity) peak record.
        let mut bytes = 1u32.to_le_bytes().to_vec();
        bytes.extend_from_slice(&[0u8; 28]);
        bytes.extend_from_slice(&100.0f32.to_le_bytes()); // window lo
        bytes.extend_from_slice(&110.0f32.to_le_bytes()); // window hi
        bytes.extend_from_slice(&7u32.to_le_bytes()); // channel idx
        bytes.extend_from_slice(&105.0f32.to_le_bytes()); // mz
        bytes.extend_from_slice(&42.0f32.to_le_bytes()); // intensity
        let mut cursor = Cursor::new(bytes);
        let peaks = read_scan_srm_v66(&mut cursor, 0, 0, 0).unwrap();
        assert_eq!(peaks.len(), 1);
        assert_eq!(peaks[0].mz, 105.0);
        assert_eq!(peaks[0].abundance, 42.0);
    }

    #[test]
    fn read_scan_srm_v66_zero_peaks_is_empty() {
        let bytes = 0u32.to_le_bytes().to_vec();
        let mut cursor = Cursor::new(bytes);
        assert!(read_scan_srm_v66(&mut cursor, 0, 0, 0).unwrap().is_empty());
    }

    #[test]
    fn read_scan_srm_v66_windows_decodes_pairs() {
        let mut bytes = 2u32.to_le_bytes().to_vec();
        bytes.extend_from_slice(&[0u8; 28]);
        bytes.extend_from_slice(&1.0f32.to_le_bytes());
        bytes.extend_from_slice(&2.0f32.to_le_bytes());
        bytes.extend_from_slice(&3.0f32.to_le_bytes());
        bytes.extend_from_slice(&4.0f32.to_le_bytes());
        let mut cursor = Cursor::new(bytes);
        let windows = read_scan_srm_v66_windows(&mut cursor, 0, 0).unwrap();
        assert_eq!(windows, vec![(1.0, 2.0), (3.0, 4.0)]);
    }

    #[test]
    fn search_v63_transition_finds_matching_record() {
        // 72-byte-per-channel table: [flag][unk][Q1][Q3_center][Q3_width][dwell][CE]...
        let mut data = vec![0u8; 8]; // leading offset so `j - 8` is in range
        data.extend_from_slice(&1.0f64.to_le_bytes()); // flag
        data.extend_from_slice(&0.0f64.to_le_bytes()); // unknown
        data.extend_from_slice(&500.0f64.to_le_bytes()); // Q1
        data.extend_from_slice(&300.0f64.to_le_bytes()); // Q3 center
        data.extend_from_slice(&1.0f64.to_le_bytes()); // Q3 width
        data.extend_from_slice(&0.02f64.to_le_bytes()); // dwell
        data.extend_from_slice(&25.0f64.to_le_bytes()); // CE
                                                        // Trailing padding: the scan only considers positions `j` with
                                                        // `j < data.len() - 32`, so there must be slack past the CE field
                                                        // for the Q3-center position (at offset 32) to be in range.
        data.extend_from_slice(&[0u8; 16]);
        let (q1, q3w, ce) = search_v63_transition(&data, 300.0).unwrap();
        assert_eq!(q1, 500.0);
        assert_eq!(q3w, 1.0);
        assert_eq!(ce, 25.0);
    }

    #[test]
    fn search_v63_transition_no_match_returns_none() {
        let data = vec![0u8; 128];
        assert!(search_v63_transition(&data, 300.0).is_none());
    }

    #[test]
    fn to_mz_intensity_capacity_uses_actual_signal_length_not_untrusted_nbins() {
        // `nbins` deliberately does not match the real chunk data below; the
        // conversion must not use it to size an allocation (see the fix in
        // `Profile::to_mz_intensity`), and the result must reflect the real
        // (small) amount of signal data actually present.
        let profile = Profile {
            first_value: 100.0,
            step: 0.01,
            peak_count: 1,
            nbins: u32::MAX,
            chunks: vec![ProfileChunk {
                first_bin: 0,
                signal: vec![1.0, 2.0, 3.0],
                fudge: None,
            }],
        };
        let result = profile.to_mz_intensity(&[]);
        assert_eq!(result.len(), 3);
    }
}
