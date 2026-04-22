use std::io::{Read, Seek, SeekFrom};

use crate::error::{Error, Result};
use crate::error_log::ErrorEntry;
use crate::generic_data::{GenericDataHeader, GenericRecord, GenericValue};
use crate::header::FileHeader;
use crate::raw_file_info::RawFileInfo;
use crate::run_header::RunHeader;
use crate::scan_data::{read_flat_peaks, read_scan_srm_v66, Peak, ScanDataPacket};
use crate::scan_event::ScanEvent;
use crate::scan_index::ScanIndexEntry;
use crate::seq_row::SeqRow;

/// Low-level binary reading helpers.
pub(crate) struct BinaryReader<R> {
    inner: R,
    pos: u64,
}

impl<R: Read + Seek> BinaryReader<R> {
    pub fn new(inner: R) -> Self {
        Self { inner, pos: 0 }
    }

    #[allow(dead_code)]
    pub(crate) fn position(&self) -> u64 {
        self.pos
    }

    pub fn seek_to(&mut self, offset: u64) -> Result<()> {
        self.inner.seek(SeekFrom::Start(offset))?;
        self.pos = offset;
        Ok(())
    }

    pub fn read_bytes(&mut self, n: usize) -> Result<Vec<u8>> {
        let mut buf = vec![0u8; n];
        self.inner.read_exact(&mut buf).map_err(|e| {
            if e.kind() == std::io::ErrorKind::UnexpectedEof {
                Error::UnexpectedEof {
                    offset: self.pos,
                    needed: n,
                }
            } else {
                Error::Io(e)
            }
        })?;
        self.pos += n as u64;
        Ok(buf)
    }

    pub fn read_bytes_into(&mut self, buf: &mut [u8]) -> Result<()> {
        let n = buf.len();
        self.inner.read_exact(buf).map_err(|e| {
            if e.kind() == std::io::ErrorKind::UnexpectedEof {
                Error::UnexpectedEof {
                    offset: self.pos,
                    needed: n,
                }
            } else {
                Error::Io(e)
            }
        })?;
        self.pos += n as u64;
        Ok(())
    }

    pub fn skip(&mut self, n: usize) -> Result<()> {
        self.inner.seek(SeekFrom::Current(n as i64))?;
        self.pos += n as u64;
        Ok(())
    }

    pub fn length(&mut self) -> Result<u64> {
        let cur = self.pos;
        let end = self.inner.seek(SeekFrom::End(0))?;
        self.inner.seek(SeekFrom::Start(cur))?;
        self.pos = cur;
        Ok(end)
    }

    pub fn read_u8(&mut self) -> Result<u8> {
        let mut buf = [0u8; 1];
        self.read_bytes_into(&mut buf)?;
        Ok(buf[0])
    }

    pub fn read_u16(&mut self) -> Result<u16> {
        let mut buf = [0u8; 2];
        self.read_bytes_into(&mut buf)?;
        Ok(u16::from_le_bytes(buf))
    }

    pub fn read_i16(&mut self) -> Result<i16> {
        let mut buf = [0u8; 2];
        self.read_bytes_into(&mut buf)?;
        Ok(i16::from_le_bytes(buf))
    }

    pub fn read_u32(&mut self) -> Result<u32> {
        let mut buf = [0u8; 4];
        self.read_bytes_into(&mut buf)?;
        Ok(u32::from_le_bytes(buf))
    }

    pub fn read_i32(&mut self) -> Result<i32> {
        let mut buf = [0u8; 4];
        self.read_bytes_into(&mut buf)?;
        Ok(i32::from_le_bytes(buf))
    }

    pub fn read_u64(&mut self) -> Result<u64> {
        let mut buf = [0u8; 8];
        self.read_bytes_into(&mut buf)?;
        Ok(u64::from_le_bytes(buf))
    }

    pub fn read_f32(&mut self) -> Result<f32> {
        let mut buf = [0u8; 4];
        self.read_bytes_into(&mut buf)?;
        Ok(f32::from_le_bytes(buf))
    }

    pub fn read_f64(&mut self) -> Result<f64> {
        let mut buf = [0u8; 8];
        self.read_bytes_into(&mut buf)?;
        Ok(f64::from_le_bytes(buf))
    }

    pub fn read_i8(&mut self) -> Result<i8> {
        let mut buf = [0u8; 1];
        self.read_bytes_into(&mut buf)?;
        Ok(buf[0] as i8)
    }

    /// Read a fixed-width UTF-16-LE string of `byte_len` bytes, stripping null padding.
    pub fn read_utf16_fixed(&mut self, byte_len: usize) -> Result<String> {
        let pos = self.pos;
        let raw = self.read_bytes(byte_len)?;
        if byte_len % 2 != 0 {
            return Err(Error::InvalidUtf16(pos));
        }
        let units: Vec<u16> = raw
            .chunks_exact(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .collect();
        // Find null terminator
        let end = units.iter().position(|&u| u == 0).unwrap_or(units.len());
        String::from_utf16(&units[..end]).map_err(|_| Error::InvalidUtf16(pos))
    }

    /// Read a PascalStringWin32: UInt32 char count, then that many UTF-16-LE code units.
    pub fn read_pascal_string(&mut self) -> Result<String> {
        let pos = self.pos;
        let char_count = self.read_u32()? as usize;
        if char_count == 0 {
            return Ok(String::new());
        }
        let byte_len = char_count.checked_mul(2).ok_or(Error::InvalidUtf16(pos))?;
        let raw = self.read_bytes(byte_len)?;
        let units: Vec<u16> = raw
            .chunks_exact(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .collect();
        // Strip trailing nulls
        let end = units.iter().position(|&u| u == 0).unwrap_or(units.len());
        String::from_utf16(&units[..end]).map_err(|_| Error::InvalidUtf16(pos))
    }

    /// Read a Windows FILETIME and return Unix timestamp as f64 seconds.
    pub fn read_windows_filetime(&mut self) -> Result<f64> {
        let ft = self.read_u64()?;
        if ft == 0 {
            return Ok(0.0);
        }
        Ok((ft as f64 / 10_000_000.0) - 11_644_473_600.0)
    }
}

/// A parsed Thermo Fisher RAW file.
pub struct RawFileReader {
    pub header: FileHeader,
    pub seq_row: SeqRow,
    pub raw_file_info: RawFileInfo,
    pub run_header: RunHeader,
    pub scan_index: Vec<ScanIndexEntry>,
    pub scan_events: Vec<ScanEvent>,
    pub scan_parameters_header: GenericDataHeader,
    pub scan_parameters: Vec<GenericRecord>,
    pub error_log: Vec<ErrorEntry>,
    // Instrument log uses same structure
    pub inst_log_header: GenericDataHeader,
    pub inst_log: Vec<GenericRecord>,
    /// Raw file version from the header.
    pub version: u32,
    /// Number of scans.
    pub num_scans: u32,
    /// Data stream base address (for computing absolute scan offsets).
    pub data_addr: u64,
    /// True if scan data uses flat-peak format (TSQ/SRM) instead of PacketHeader.
    pub flat_peaks: bool,
    /// Detected scan-data encoding (the format used by [`Self::read_scan_peaks`]).
    pub scan_format: crate::scan_format::ScanDataFormat,
    /// Detected device family (informational).
    pub device_family: crate::device::DeviceFamily,
    /// Canonical instrument model name if one was detected in the file's
    /// metadata region (e.g. `"Orbitrap Fusion Lumos"`). `None` means only
    /// the coarse family could be inferred.
    pub instrument_model: Option<&'static str>,
}

impl RawFileReader {
    /// Open and parse a RAW file from a reader.
    pub fn open<R: Read + Seek>(source: R) -> Result<Self> {
        let mut r = BinaryReader::new(source);

        // 1. FileHeader
        let header = FileHeader::read(&mut r)?;
        let version = header.version;

        // 2. SeqRow
        let seq_row = SeqRow::read(&mut r, version)?;

        // 3. ASInfo (read and discard preamble + string)
        let _as_preamble = r.read_bytes(24)?; // ASInfoPreamble: 24 bytes
        let _as_text = r.read_pascal_string()?;

        // 4. RawFileInfo
        let raw_file_info = RawFileInfo::read(&mut r, version)?;

        // 5. Extract addresses
        let data_addr = raw_file_info.preamble.data_addr;

        // 6. Select the MS controller RunHeader.
        // Multi-controller files (e.g. UV + MS) have one RunHeader per controller.
        // The MS controller has ntrailer > 0 (v64+) or first_scan <= last_scan with
        // nsegs > 0 (v63 and earlier). We iterate all addresses and pick the best.
        let run_header = {
            let addrs = &raw_file_info.preamble.run_header_addrs;
            let mut chosen = None;
            for &addr in addrs {
                if addr == 0 {
                    continue;
                }
                r.seek_to(addr)?;
                let rh = RunHeader::read(&mut r, version)?;
                // Heuristic for identifying the MS controller:
                // 1. For v64+: ntrailer > 0 (scan events present) — catches most instruments.
                // 2. For all versions: RunHeader.data_addr == preamble.data_addr — the MS
                //    controller's scan data begins at the same address the preamble declares.
                //    This catches TSQ/triple-quad instruments where ntrailer=0 (no scan events).
                // 3. Pre-v64 fallback: valid scan range with nsegs > 0.
                let is_ms = if version >= 64 {
                    rh.ntrailer > 0 || rh.data_addr == data_addr
                } else {
                    rh.sample_info.last_scan_number >= rh.sample_info.first_scan_number
                        && rh.nsegs > 0
                };
                if is_ms {
                    chosen = Some(rh);
                    break;
                }
            }
            // Fall back to first address if no MS controller found
            match chosen {
                Some(rh) => rh,
                None => {
                    r.seek_to(addrs[0])?;
                    RunHeader::read(&mut r, version)?
                }
            }
        };

        let first_scan = run_header.sample_info.first_scan_number;
        let last_scan = run_header.sample_info.last_scan_number;

        let num_scans = if last_scan >= first_scan {
            last_scan - first_scan + 1
        } else {
            0
        };

        // 7. Scan index
        r.seek_to(run_header.scan_index_addr)?;
        let mut scan_index = Vec::with_capacity(num_scans as usize);
        for _ in 0..num_scans {
            scan_index.push(ScanIndexEntry::read(&mut r, version)?);
        }

        // 8. Scan event trailer
        r.seek_to(run_header.scan_trailer_addr)?;
        let n_events = if version >= 64 {
            // v64+: first u32 is a preamble (not count); use ntrailer from RunHeader
            let _preamble = r.read_u32()?;
            run_header.ntrailer
        } else {
            r.read_u32()?
        };
        let mut scan_events = Vec::with_capacity(n_events as usize);
        for _ in 0..n_events {
            scan_events.push(ScanEvent::read(&mut r, version)?);
        }

        // 9. Error log — in v64+ the error log is followed by the
        //    GenericDataHeader for scan parameters (after some opaque padding),
        //    so we read it first.
        let n_errors = run_header.sample_info.error_log_length;
        let error_log = if n_errors > 0 {
            r.seek_to(run_header.error_log_addr)?;
            if version >= 64 {
                let _preamble = r.read_u32()?;
            }
            let mut log = Vec::with_capacity(n_errors as usize);
            for _ in 0..n_errors {
                log.push(ErrorEntry::read(&mut r)?);
            }
            log
        } else {
            Vec::new()
        };

        // 10. Scan parameters (trailer extra) — GenericData format in v64+.
        //     The schema (GDH) is written between the error-log region and
        //     the scan-trailer stream; the records are written at
        //     `scan_params_addr` (tail of file) with NO stream preamble —
        //     records begin directly at scan_params_addr. Any bytes after
        //     the last record are trailing padding and can be ignored.
        //     We scan forward from the error-log address to locate the
        //     schema — the intervening padding varies by instrument.
        let (scan_parameters_header, scan_parameters) = if version >= 64 {
            r.seek_to(run_header.error_log_addr)?;
            let scan_distance =
                run_header.scan_trailer_addr.saturating_sub(run_header.error_log_addr);
            // Estimate per-record size from the tail of the file using integer
            // division. Any remainder bytes are trailing data, not a preamble.
            let file_size = r.length()?;
            let tail = file_size.saturating_sub(run_header.scan_params_addr);
            let expected_record_size = if num_scans > 0 && tail > 0 {
                let per_scan = tail / num_scans as u64;
                if per_scan >= 4 { Some(per_scan as usize) } else { None }
            } else {
                None
            };
            match GenericDataHeader::find_forward(
                &mut r,
                scan_distance,
                expected_record_size,
            )? {
                Some(hdr) => {
                    // Records start directly at scan_params_addr — no stream preamble.
                    r.seek_to(run_header.scan_params_addr)?;
                    let mut params = Vec::with_capacity(num_scans as usize);
                    for _ in 0..num_scans {
                        params.push(GenericRecord::read(&mut r, &hdr)?);
                    }
                    (hdr, params)
                }
                None => (GenericDataHeader { fields: Vec::new() }, Vec::new()),
            }
        } else {
            (GenericDataHeader { fields: Vec::new() }, Vec::new())
        };

        // 11. Instrument log — GenericData format in v64+
        let (inst_log_header, inst_log) = if version >= 64 {
            r.seek_to(run_header.inst_log_addr)?;
            match GenericDataHeader::try_read(&mut r)? {
                Some(hdr) => {
                    let n_inst = run_header.sample_info.inst_log_length;
                    let mut log = Vec::with_capacity(n_inst as usize);
                    for _ in 0..n_inst {
                        log.push(GenericRecord::read(&mut r, &hdr)?);
                    }
                    (hdr, log)
                }
                None => (GenericDataHeader { fields: Vec::new() }, Vec::new()),
            }
        } else {
            (GenericDataHeader { fields: Vec::new() }, Vec::new())
        };

        // Detect flat-peak (TSQ/SRM) format.
        // Reliable indicator: ntrailer == 0 means no scan event trailer was written, which
        // is the case for all TSQ/triple-quad SRM instruments.
        // Fallback: first scan data_size < 100 (catches edge cases with tiny SRM windows).
        // In the flat format, data_size is the number of MRM peaks, not bytes.
        let flat_peaks = run_header.ntrailer == 0
            || scan_index
                .first()
                .map(|e| e.data_size < 100)
                .unwrap_or(false);

        // Classify scan format and device family.
        let scan_format = crate::scan_format::ScanDataFormat::detect(version, flat_peaks);
        let first_analyzer = scan_events.first().and_then(|e| e.preamble.analyzer());

        // Scan a prefix of the file for the canonical instrument model string.
        // Thermo embeds the model as a UTF-16LE string in the metadata region
        // preceding the scan data — almost always within the first ~16 KB.
        // Cap at 64 KB to give plenty of headroom without scanning large files.
        let scan_window_cap = 64 * 1024u64;
        let window_len = scan_window_cap.min(data_addr);
        let metadata_window = if window_len > 0 {
            r.seek_to(0)?;
            r.read_bytes(window_len as usize).unwrap_or_default()
        } else {
            Vec::new()
        };
        let detected = crate::device::DeviceFamily::detect_instrument(
            &metadata_window,
            &header.audit_start.tag2,
            &seq_row.inst_method,
            first_analyzer,
        );
        let device_family = detected.family;
        let instrument_model = detected.model;

        Ok(Self {
            header,
            seq_row,
            raw_file_info,
            run_header,
            scan_index,
            scan_events,
            scan_parameters_header,
            scan_parameters,
            error_log,
            inst_log_header,
            inst_log,
            version,
            num_scans,
            data_addr,
            flat_peaks,
            scan_format,
            device_family,
            instrument_model,
        })
    }

    /// Open a RAW file from a path.
    pub fn open_path(path: impl AsRef<std::path::Path>) -> Result<Self> {
        let file = std::fs::File::open(path)?;
        let reader = std::io::BufReader::new(file);
        Self::open(reader)
    }

    /// Read a single scan data packet (PacketHeader format).
    pub fn read_scan<R: Read + Seek>(
        &self,
        source: &mut R,
        scan_number: u32,
    ) -> Result<ScanDataPacket> {
        let idx = (scan_number - self.run_header.sample_info.first_scan_number) as usize;
        if idx >= self.scan_index.len() {
            return Err(Error::AddressOutOfRange(scan_number as u64));
        }
        let entry = &self.scan_index[idx];
        let abs_offset = self.data_addr + entry.offset;
        source.seek(SeekFrom::Start(abs_offset))?;
        let mut r = BinaryReader::new(source);
        ScanDataPacket::read(&mut r)
    }

    /// Read a single scan as flat peaks (TSQ/SRM format).
    ///
    /// In this format, `entry.offset` is the cumulative end byte offset within
    /// the data stream. Peaks are (f32, f32) pairs at the end of each record.
    pub fn read_scan_flat<R: Read + Seek>(
        &self,
        source: &mut R,
        scan_number: u32,
    ) -> Result<Vec<Peak>> {
        let idx = (scan_number - self.run_header.sample_info.first_scan_number) as usize;
        if idx >= self.scan_index.len() {
            return Err(Error::AddressOutOfRange(scan_number as u64));
        }
        let entry = &self.scan_index[idx];
        read_flat_peaks(source, self.data_addr, entry.offset, entry.data_size)
    }

    /// Read a single scan in v66 SRM format (TSQ Quantiva / TSQ Altis).
    ///
    /// `entry.offset` is the START byte offset within the data stream.
    /// The record is fixed-size (`entry.data_size` bytes) and contains:
    ///   n_peaks (u32), header, m/z window table, then peak triplets.
    pub fn read_scan_srm_v66<R: Read + Seek>(
        &self,
        source: &mut R,
        scan_number: u32,
    ) -> Result<Vec<Peak>> {
        let idx = (scan_number - self.run_header.sample_info.first_scan_number) as usize;
        if idx >= self.scan_index.len() {
            return Err(Error::AddressOutOfRange(scan_number as u64));
        }
        let entry = &self.scan_index[idx];
        read_scan_srm_v66(source, self.data_addr, entry.offset, entry.data_size)
    }

    /// Read a single scan's peaks using whichever decoder matches this file's
    /// scan-data format.
    ///
    /// This is the recommended high-level entry point. It dispatches on
    /// [`Self::scan_format`] so callers do not have to know whether a file is
    /// a TSQ SRM run (flat peaks) or an Orbitrap/ion-trap acquisition
    /// (PacketHeader records).
    ///
    /// The returned `Vec<Peak>` contains centroided peaks regardless of the
    /// underlying format. For PacketHeader files that also contain a profile
    /// signal, use [`Self::read_scan`] to access both.
    pub fn read_scan_peaks<R: Read + Seek>(
        &self,
        source: &mut R,
        scan_number: u32,
    ) -> Result<Vec<Peak>> {
        use crate::scan_format::ScanDataFormat;
        match self.scan_format {
            ScanDataFormat::PacketHeader => {
                let pkt = self.read_scan(source, scan_number)?;
                Ok(pkt.peaks)
            }
            ScanDataFormat::FlatV63 => self.read_scan_flat(source, scan_number),
            ScanDataFormat::FlatV66 => self.read_scan_srm_v66(source, scan_number),
        }
    }

    /// Return the scan-parameter record for a given 1-based scan number.
    ///
    /// Returns `None` if the file has no scan-parameter stream or if
    /// `scan_number` is outside the valid scan range.
    pub fn scan_parameters(&self, scan_number: u32) -> Option<&GenericRecord> {
        let first = self.run_header.sample_info.first_scan_number;
        let idx = scan_number.checked_sub(first)? as usize;
        self.scan_parameters.get(idx)
    }

    /// Return a typed view of the scan-parameter record for a given scan.
    ///
    /// This wraps [`Self::scan_parameters`] in a [`ScanParams`] accessor that
    /// provides named, type-safe fields and handles label-name variations
    /// across instrument families.
    pub fn scan_params(&self, scan_number: u32) -> Option<ScanParams<'_>> {
        self.scan_parameters(scan_number).map(ScanParams)
    }
}

// ─── High-level typed accessor for scan parameters ──────────────────────────

/// Typed accessor for a scan's extra parameters (`ScanParams` stream).
///
/// The underlying [`GenericRecord`] stores named fields whose labels vary
/// slightly across Thermo instrument families. This wrapper normalises the
/// most common labels so callers do not need to hard-code instrument-specific
/// strings.
///
/// # Example
/// ```no_run
/// use opentfraw::RawFileReader;
/// let raw = RawFileReader::open_path("experiment.raw").unwrap();
/// if let Some(p) = raw.scan_params(1) {
///     println!("Injection time: {:?} ms", p.ion_injection_time_ms());
///     println!("Charge state:   {:?}", p.charge_state());
/// }
/// ```
pub struct ScanParams<'a>(pub &'a GenericRecord);

impl<'a> ScanParams<'a> {
    /// Return the raw `GenericRecord` for direct field access.
    #[inline]
    pub fn record(&self) -> &GenericRecord {
        self.0
    }

    /// Ion injection / fill time in milliseconds.
    ///
    /// Label varies: `"Ion Injection Time (ms):"` (Orbitrap family) vs
    /// `"Ion Inject Time (ms):"` (older LTQ variants).
    pub fn ion_injection_time_ms(&self) -> Option<f64> {
        // Try canonical label first; fall back to legacy label.
        self.0.get_f64("Ion Injection Time (ms):")
            .or_else(|| self.0.get_f64("Ion Inject Time (ms):"))
    }

    /// Precursor charge state (0 = unknown / MS1 scan).
    pub fn charge_state(&self) -> Option<i32> {
        self.0.get_i32("Charge State:")
            // Some LCQ files use UInt8 for charge state.
            .or_else(|| {
                self.0.get("Charge State:").and_then(|v| match v {
                    GenericValue::UInt8(n) => Some(*n as i32),
                    _ => None,
                })
            })
    }

    /// Monoisotopic precursor m/z (0 = not determined).
    pub fn monoisotopic_mz(&self) -> Option<f64> {
        self.0.get_f64("Monoisotopic M/Z:")
    }

    /// Number of micro-scans averaged into this scan.
    pub fn micro_scan_count(&self) -> Option<i32> {
        self.0.get_i32("Micro Scan Count:")
    }

    /// Orbitrap / FT resolving power (e.g. 60000, 120000).
    pub fn ft_resolution(&self) -> Option<i32> {
        self.0.get_i32("Orbitrap Resolution:")
            .or_else(|| self.0.get_i32("FT Resolution:"))
    }

    /// HCD collision energy string (e.g. `"27%"` or `"28.00"`).
    ///
    /// The label and unit differ between firmware generations; this returns
    /// whatever string is present (eV or percent).
    pub fn hcd_energy(&self) -> Option<&str> {
        self.0.get_string("HCD Energy:")
    }

    /// Scan number of the master (MS1) scan that triggered this dependent scan.
    /// Returns `None` or `Some(-1)` if this is not a dependent scan.
    pub fn master_scan_number(&self) -> Option<i32> {
        self.0.get_i32("Master Scan Number:")
            .or_else(|| self.0.get_i32("Master Index:"))
    }

    /// Number of lock masses found / matched.
    pub fn number_of_lm_found(&self) -> Option<i32> {
        self.0.get_i32("Number of LM Found:")
            .or_else(|| self.0.get_i32("Number of Lock Masses:"))
    }

    /// Lock-mass m/z correction applied (ppm).
    pub fn lm_correction_ppm(&self) -> Option<f64> {
        self.0.get_f64("LM Correction (ppm):")
            .or_else(|| self.0.get_f64("LM m/z-Correction (ppm):"))
    }

    /// AGC target fill value (ion count).
    pub fn agc_target(&self) -> Option<i32> {
        self.0.get_i32("AGC Target:")
    }

    /// Whether automated gain control (AGC) was active.
    pub fn agc_enabled(&self) -> Option<bool> {
        match self.0.get("AGC:")? {
            GenericValue::Bool(b) => Some(*b),
            GenericValue::String(s) => Some(s.to_ascii_lowercase().contains("on")),
            _ => None,
        }
    }

    /// Elapsed scan time in seconds (Orbitrap instruments only).
    pub fn elapsed_scan_time_s(&self) -> Option<f64> {
        self.0.get_f64("Elapsed Scan Time (sec):")
    }

    /// Maximum allowed ion injection time in milliseconds.
    pub fn max_ion_time_ms(&self) -> Option<f64> {
        self.0.get_f64("Max. Ion Time (ms):")
    }
}
