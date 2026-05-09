use std::collections::HashMap;
use std::io::{Read, Seek, SeekFrom};

use crate::error::{Error, Result};
use crate::error_log::ErrorEntry;
use crate::generic_data::{GenericDataHeader, GenericRecord, GenericValue};
use crate::header::FileHeader;
use crate::raw_file_info::RawFileInfo;
use crate::run_header::RunHeader;
use crate::scan_data::{read_flat_peaks, read_scan_srm_v66, Peak, ScanDataPacket};
use crate::scan_event::{ScanEvent, ScanEventPreamble};
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
    /// For SRM (flat-peak) files: maps scan_event index → Q1 precursor mass (m/z).
    ///
    /// Populated at open time by scanning the method/transition table stored
    /// in the pre-scan-data header region. Empty for non-SRM instruments.
    pub srm_q1_by_event: HashMap<u16, f64>,
}

// ─── Multi-controller metadata ───────────────────────────────────────────────

/// Controller type codes as used in Thermo RAW files.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControllerType {
    Ms,
    Analog,
    Adc,
    Pda,
    Uv,
    Other,
}

impl ControllerType {
    fn from_nsegs_ntrailer(ntrailer: u32, nsegs: u32) -> Self {
        // Heuristic: MS controller always has ntrailer > 0 (v64+) or nsegs > 0.
        // Non-MS controllers (UV, analog, PDA) have ntrailer == 0 and nsegs == 1.
        // We can't reliably distinguish between non-MS types without parsing
        // the InstID/method block, so we fall back to Other for those.
        if ntrailer > 0 || nsegs > 1 {
            Self::Ms
        } else {
            Self::Other
        }
    }
}

/// Minimal metadata about one controller in a multi-controller RAW file.
#[derive(Debug, Clone)]
pub struct ControllerInfo {
    /// Zero-based controller index (position in `run_header_addrs`).
    pub index: usize,
    /// File offset to this controller's RunHeader.
    pub run_header_addr: u64,
    /// Whether this controller is the primary MS controller.
    pub is_ms_controller: bool,
    /// Inferred controller type.
    pub controller_type: ControllerType,
    /// First scan number.
    pub first_scan: u32,
    /// Last scan number.
    pub last_scan: u32,
    /// Acquisition start time (minutes).
    pub start_time: f64,
    /// Acquisition end time (minutes).
    pub end_time: f64,
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
        // For v66, compute per-event body sizes from the stream's address range.
        // The scan event stream spans [scan_trailer_addr+4 .. scan_params_addr).
        // Each event = preamble (136 bytes) + body.
        //
        // Simple instruments (Q Exactive, Exploris): all events are identical in
        // size so stream_bytes divides evenly by n_events.
        //
        // Tribrid instruments (Eclipse, Fusion Lumos): primary (MS1) scans and
        // dependent (MS2+) scans have different body layouts:
        //   Primary event:   232 bytes total (preamble 136 + body 96)
        //   Dependent event: 344 bytes total (preamble 136 + body 208)
        // Confirmed empirically across Orbitrap Eclipse (EThcD) and Fusion Lumos
        // (DIA, MS3) files.
        let preamble_size = ScanEventPreamble::size_for_version(version);
        let (v66_body_primary, v66_body_dependent): (usize, usize) =
            if version >= 66 && n_events > 0 {
                let stream_bytes = run_header
                    .scan_params_addr
                    .saturating_sub(run_header.scan_trailer_addr)
                    .saturating_sub(4);
                let remainder = stream_bytes % n_events as u64;
                if remainder == 0 {
                    // Uniform event size (Q Exactive, Exploris, etc.)
                    let body = (stream_bytes / n_events as u64) as usize;
                    let body = body.saturating_sub(preamble_size);
                    (body, body)
                } else {
                    // Variable-length events: tribrid Orbitrap instruments.
                    // Known sizes: primary=232, dependent=344 (body 96 and 208).
                    const PRIMARY_EVENT: u64 = 232;
                    const DEPENDENT_EVENT: u64 = 344;
                    let gap = DEPENDENT_EVENT - PRIMARY_EVENT;
                    let n = n_events as u64;
                    // n_primary * PRIMARY_EVENT + n_dependent * DEPENDENT_EVENT = stream_bytes
                    // n_primary + n_dependent = n
                    // => n_primary = (n * DEPENDENT_EVENT - stream_bytes) / gap
                    let n_primary_numerator = n
                        .saturating_mul(DEPENDENT_EVENT)
                        .saturating_sub(stream_bytes);
                    if n_primary_numerator % gap == 0 {
                        let n_primary = n_primary_numerator / gap;
                        let n_dependent = n.saturating_sub(n_primary);
                        let total_check = n_primary * PRIMARY_EVENT + n_dependent * DEPENDENT_EVENT;
                        if total_check == stream_bytes {
                            // Verified: use the tribrid sizes.
                            (
                                (PRIMARY_EVENT as usize).saturating_sub(preamble_size),
                                (DEPENDENT_EVENT as usize).saturating_sub(preamble_size),
                            )
                        } else {
                            // Fallback: use floor-average uniform body
                            let body = ((stream_bytes / n) as usize).saturating_sub(preamble_size);
                            (body, body)
                        }
                    } else {
                        // Fallback: use floor-average uniform body
                        let body = ((stream_bytes / n) as usize).saturating_sub(preamble_size);
                        (body, body)
                    }
                }
            } else {
                (0, 0)
            };
        let mut scan_events = Vec::with_capacity(n_events as usize);
        for _ in 0..n_events {
            scan_events.push(ScanEvent::read(
                &mut r,
                version,
                v66_body_primary,
                v66_body_dependent,
            )?);
        }

        // 9. Error log
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
            // Ensure reader is positioned at error_log_addr even when empty.
            r.seek_to(run_header.error_log_addr)?;
            Vec::new()
        };
        // The GDH for scan parameters immediately follows the error-log entries.
        // Do NOT seek back to error_log_addr — doing so would cause find_forward
        // to scan over the scan_index (which may sit between error_log and
        // scan_trailer in some file layouts), creating a CPU-spinning O(n) search
        // through megabytes of binary scan data.
        let after_error_log = r.position();

        // 10. Scan parameters (trailer extra) — GenericData format in v64+.
        //     The schema (GDH) is written just after the error-log entries;
        //     the records are written at `scan_params_addr` (tail of file)
        //     with NO stream preamble — records begin directly at
        //     scan_params_addr. Any bytes after the last record are trailing
        //     padding and can be ignored.
        let (scan_parameters_header, scan_parameters) = if version >= 64 {
            // Search from after the error log entries up to scan_trailer.
            // This skips any scan_index data that may sit in between.
            let scan_distance = run_header.scan_trailer_addr.saturating_sub(after_error_log);
            // Estimate per-record size from the tail of the file using integer
            // division. Any remainder bytes are trailing data, not a preamble.
            let file_size = r.length()?;
            let tail = file_size.saturating_sub(run_header.scan_params_addr);
            let expected_record_size = if num_scans > 0 && tail > 0 {
                let per_scan = tail / num_scans as u64;
                if per_scan >= 4 {
                    Some(per_scan as usize)
                } else {
                    None
                }
            } else {
                None
            };
            match GenericDataHeader::find_forward(&mut r, scan_distance, expected_record_size)? {
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

        // For SRM (flat-peak) files, read the entire pre-scan-data region so that
        // we can extract Q1 values from the method/transition table stored there.
        // For other instruments, read only 64 KB for instrument model detection.
        let scan_window_cap = if flat_peaks { data_addr } else { 64 * 1024u64 };
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

        // For SRM files: extract Q1 masses from the method/transition section.
        //
        // In Thermo TSQ RAW files, the SRM transition list is embedded in a
        // method structure stored in the pre-scan-data header region (bytes 0 to
        // data_addr). Each transition record stores (Q1: f64, Q3_lo: f64, Q3_hi: f64)
        // consecutively. We locate records by searching for Q3_lo values that match
        // the per-scan-event Q3 window lower bounds stored in the scan index.
        //
        // The scan_index low_mz field holds the minimum Q3 lower bound for a given
        // scan event (derived from f32 scan data, so it may differ from the f64 method
        // value by up to ~0.002 Da). We use a 0.002 Da tolerance for matching.
        let srm_q1_by_event = if flat_peaks && metadata_window.len() >= 24 {
            // Collect the first observed Q3 lower bound for each unique scan event.
            let mut event_q3_lo: HashMap<u16, f64> = HashMap::new();
            for entry in &scan_index {
                if entry.low_mz > 50.0 && entry.low_mz < 2000.0 {
                    event_q3_lo.entry(entry.scan_event).or_insert(entry.low_mz);
                }
            }

            let data = &metadata_window;
            let mut q1_map: HashMap<u16, f64> = HashMap::new();

            'outer: for (&event, &q3_lo_target) in &event_q3_lo {
                // Scan byte-by-byte for a f64 that approximates q3_lo_target.
                // The transition record layout is: [Q1: f64][Q3_lo: f64][Q3_hi: f64]...
                // so Q1 is at offset -8 and Q3_hi is at offset +8 from Q3_lo.
                let end = data.len().saturating_sub(16);
                for i in 8..end {
                    // SAFETY: bounds checked above; arrays are exactly 8 bytes.
                    let v = f64::from_le_bytes(data[i..i + 8].try_into().unwrap());
                    if (v - q3_lo_target).abs() < 0.002 {
                        let hi = f64::from_le_bytes(data[i + 8..i + 16].try_into().unwrap());
                        // Q3_hi must be slightly larger than Q3_lo (window < 0.1 Da).
                        if hi > v && (hi - v) < 0.1 {
                            let q1 = f64::from_le_bytes(data[i - 8..i].try_into().unwrap());
                            // Q1 must be a plausible precursor mass.
                            if q1 > 50.0 && q1 < 3000.0 {
                                q1_map.insert(event, q1);
                                continue 'outer;
                            }
                        }
                    }
                }
            }
            q1_map
        } else {
            HashMap::new()
        };

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
            srm_q1_by_event,
        })
    }

    /// Open a RAW file from a path.
    pub fn open_path(path: impl AsRef<std::path::Path>) -> Result<Self> {
        let file = std::fs::File::open(path)?;
        let reader = std::io::BufReader::new(file);
        Self::open(reader)
    }

    /// Enumerate all controllers in this RAW file.
    ///
    /// Multi-detector acquisition systems write one [`RunHeader`] per
    /// controller (MS, UV, PDA, Analog). This method parses all controller
    /// headers and returns a `Vec<ControllerInfo>` with basic metadata for
    /// each. The primary MS controller can be identified via
    /// [`ControllerInfo::is_ms_controller`].
    ///
    /// For single-controller files (the common case), this returns a
    /// one-element vec with the MS controller.
    pub fn controllers<R: Read + Seek>(&self, source: &mut R) -> Result<Vec<ControllerInfo>> {
        let mut r = BinaryReader::new(source);
        let addrs = &self.raw_file_info.preamble.run_header_addrs;
        let mut infos = Vec::with_capacity(addrs.len());
        for (i, &addr) in addrs.iter().enumerate() {
            if addr == 0 {
                continue;
            }
            r.seek_to(addr)?;
            let rh = RunHeader::read(&mut r, self.version)?;
            let is_ms = if self.version >= 64 {
                rh.ntrailer > 0 || rh.data_addr == self.data_addr
            } else {
                rh.nsegs > 0
            };
            let ct = if is_ms {
                ControllerType::Ms
            } else {
                ControllerType::from_nsegs_ntrailer(rh.ntrailer, rh.nsegs)
            };
            infos.push(ControllerInfo {
                index: i,
                run_header_addr: addr,
                is_ms_controller: is_ms,
                controller_type: ct,
                first_scan: rh.sample_info.first_scan_number,
                last_scan: rh.sample_info.last_scan_number,
                start_time: rh.sample_info.start_time,
                end_time: rh.sample_info.end_time,
            });
        }
        Ok(infos)
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

    /// Read centroided peaks only, skipping profile data.
    ///
    /// For PacketHeader files (Orbitrap / ion-trap), this skips the large
    /// profile-data section, making it 2–10× faster than
    /// [`Self::read_scan_peaks`] when only centroided m/z and intensity values
    /// are needed (e.g. mzML export, peak area queries).
    ///
    /// For TSQ/SRM files this is identical to [`Self::read_scan_peaks`].
    pub fn read_peaks_only<R: Read + Seek>(
        &self,
        source: &mut R,
        scan_number: u32,
    ) -> Result<Vec<Peak>> {
        use crate::scan_format::ScanDataFormat;
        match self.scan_format {
            ScanDataFormat::PacketHeader => {
                let idx = (scan_number - self.run_header.sample_info.first_scan_number) as usize;
                if idx >= self.scan_index.len() {
                    return Err(Error::AddressOutOfRange(scan_number as u64));
                }
                let entry = &self.scan_index[idx];
                let abs_offset = self.data_addr + entry.offset;
                source.seek(SeekFrom::Start(abs_offset))?;
                let mut r = BinaryReader::new(source);
                ScanDataPacket::read_peaks_only(&mut r)
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

    /// Return the raw instrument-log record for a given scan number, or
    /// `None` if the scan is out of range or no instrument log was found.
    ///
    /// The instrument log contains per-scan instrument-state values:
    /// temperatures, voltages, pressures, ion counts, etc.
    pub fn inst_log_record(&self, scan_number: u32) -> Option<&GenericRecord> {
        let first = self.run_header.sample_info.first_scan_number;
        let idx = scan_number.checked_sub(first)? as usize;
        self.inst_log.get(idx)
    }

    /// Return a typed [`StatusLogEntry`] view for the given scan number.
    ///
    /// This wraps [`Self::inst_log_record`] and provides named, type-safe
    /// accessors for common instrument-status fields.
    pub fn status_log_entry(&self, scan_number: u32) -> Option<StatusLogEntry<'_>> {
        self.inst_log_record(scan_number).map(StatusLogEntry)
    }

    /// Return the canonical Thermo scan filter string for a given scan
    /// (1-based scan number), or `None` if the scan is out of range.
    ///
    /// Example output: `"FTMS + p NSI Full ms [350.0000-1500.0000]"`.
    ///
    /// See [`crate::scan_filter`] for grammar details.
    pub fn scan_filter(&self, scan_number: u32) -> Option<String> {
        let first = self.run_header.sample_info.first_scan_number;
        let idx = scan_number.checked_sub(first)? as usize;
        let event = self.scan_events.get(idx)?;
        let entry = self.scan_index.get(idx)?;
        // Precursor m/z and activation energy come from the per-scan params
        // table (not the event body) for v66+. Fall back silently if missing.
        let params = self.scan_params(scan_number);
        let precursor = params.as_ref().and_then(|p| p.monoisotopic_mz());
        let energy = params.as_ref().and_then(|p| p.activation_energy());
        let supplemental = params
            .as_ref()
            .and_then(|p| p.supplemental_activation_energy());
        Some(crate::scan_filter::build_filter(
            event,
            entry,
            precursor,
            energy,
            supplemental,
        ))
    }

    /// Return all scan retention times (minutes) in scan order (1-based scan numbers).
    ///
    /// This is equivalent to collecting `scan_index[i].start_time` for every scan.
    /// The returned `Vec` is indexed by `scan_number - first_scan_number`.
    pub fn retention_times(&self) -> Vec<f64> {
        self.scan_index.iter().map(|e| e.start_time).collect()
    }

    /// Return a per-scan chromatogram as `(retention_time_min, tic)` pairs.
    pub fn tic_chromatogram(&self) -> Vec<(f64, f64)> {
        self.scan_index
            .iter()
            .map(|e| (e.start_time, e.total_current))
            .collect()
    }

    /// Return a per-scan base-peak chromatogram as `(retention_time_min, bpi, base_mz)` triples.
    pub fn bpc_chromatogram(&self) -> Vec<(f64, f64, f64)> {
        self.scan_index
            .iter()
            .map(|e| (e.start_time, e.base_intensity, e.base_mz))
            .collect()
    }

    /// Return the instrument method file path or name as stored in the
    /// sequence row. This is the name of the method used during acquisition
    /// (e.g. `"Standard_HCD.meth"`), not the embedded method text.
    ///
    /// See also [`Self::instrument_method_text`] for extracting the embedded
    /// XML/text method body from the file.
    pub fn instrument_method_name(&self) -> &str {
        &self.seq_row.inst_method
    }

    /// Attempt to extract the embedded instrument method text from the RAW file.
    ///
    /// Thermo RAW files embed the acquisition method as a UTF-16LE text or
    /// XML blob in the metadata region. This method scans the bytes between
    /// the start of the file and the scan data for the longest contiguous
    /// block of valid UTF-16LE text (at least 256 characters long) and returns
    /// it as a `String`.
    ///
    /// Returns `None` if no suitable text block is found or if the method was
    /// not embedded (`method_file_present == false`).
    ///
    /// Note: This is a best-effort extraction. The result is the raw text
    /// content; callers may wish to trim or parse it further.
    pub fn instrument_method_text<R: Read + Seek>(&self, source: &mut R) -> Option<String> {
        if !self.raw_file_info.preamble.method_file_present {
            return None;
        }
        // Read metadata region: from byte 0 up to (but not including) scan data.
        // Cap at 512 KB to avoid reading very large files entirely.
        const MAX_WINDOW: u64 = 512 * 1024;
        let window_len = MAX_WINDOW.min(self.data_addr) as usize;
        if window_len < 4 {
            return None;
        }
        source.seek(std::io::SeekFrom::Start(0)).ok()?;
        let mut buf = vec![0u8; window_len];
        source.read_exact(&mut buf).ok()?;

        // Scan for the longest valid UTF-16LE text block (min 256 chars = 512 bytes).
        // Strategy: find aligned 2-byte sequences where every pair decodes to a
        // printable/whitespace Unicode scalar (U+0020..U+FFFD).
        extract_utf16le_text(&buf, 256)
    }
}

/// Scan `buf` for the longest contiguous UTF-16LE text block of at least
/// `min_chars` characters and return it as a String. Returns `None` if no
/// such block exists.
fn extract_utf16le_text(buf: &[u8], min_chars: usize) -> Option<String> {
    if buf.len() < 2 {
        return None;
    }
    let mut best: Option<String> = None;
    let mut best_len = 0usize;

    // Try each even alignment (0 or 1 byte offset from start).
    for alignment in 0..2usize {
        let start = alignment;
        let usable = buf.len().saturating_sub(start);
        let n_units = usable / 2;
        if n_units < min_chars {
            continue;
        }

        let mut run_start = 0usize;
        let mut run_chars: Vec<u16> = Vec::with_capacity(min_chars);

        let flush = |run_chars: &Vec<u16>,
                     run_start: usize,
                     best: &mut Option<String>,
                     best_len: &mut usize| {
            if run_chars.len() >= min_chars {
                if let Ok(s) = String::from_utf16(run_chars) {
                    let _ = run_start; // suppress unused warning
                    if run_chars.len() > *best_len {
                        *best_len = run_chars.len();
                        *best = Some(s);
                    }
                }
            }
        };

        for i in 0..n_units {
            let off = start + i * 2;
            let u = u16::from_le_bytes([buf[off], buf[off + 1]]);
            let is_ok = matches!(u, 0x0009 | 0x000A | 0x000D | 0x0020..=0xFFFD);
            if is_ok {
                run_chars.push(u);
            } else {
                flush(&run_chars, run_start, &mut best, &mut best_len);
                run_start = i + 1;
                run_chars.clear();
            }
        }
        flush(&run_chars, run_start, &mut best, &mut best_len);
    }
    best
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
        self.0
            .get_f64("Ion Injection Time (ms):")
            .or_else(|| self.0.get_f64("Ion Inject Time (ms):"))
    }

    /// Precursor charge state (0 = unknown / MS1 scan).
    pub fn charge_state(&self) -> Option<i32> {
        self.0
            .get_i32("Charge State:")
            // Some LCQ files use UInt8 for charge state.
            .or_else(|| {
                self.0.get("Charge State:").and_then(|v| match v {
                    GenericValue::UInt8(n) => Some(*n as i32),
                    _ => None,
                })
            })
    }

    /// Monoisotopic precursor m/z (0 = not determined).
    ///
    /// Tries multiple label variants for compatibility across instrument families:
    /// - `"Monoisotopic M/Z:"` — most common (Q Exactive, Orbitrap Fusion)
    /// - `"MS2 Isolation M/Z:"` — some older LTQ firmware
    /// Returns `None` when the value is absent or zero (not determined).
    pub fn monoisotopic_mz(&self) -> Option<f64> {
        let v = self
            .0
            .get_f64("Monoisotopic M/Z:")
            .or_else(|| self.0.get_f64("MS2 Isolation M/Z:"))
            .or_else(|| self.0.get_f64("Isolation Center M/Z:"))
            .or_else(|| self.0.get_f64("Precursor M/Z:"))?;
        if v > 0.0 {
            Some(v)
        } else {
            None
        }
    }

    /// Number of micro-scans averaged into this scan.
    pub fn micro_scan_count(&self) -> Option<i32> {
        self.0.get_i32("Micro Scan Count:")
    }

    /// Scan number of the master (MS1) scan that triggered this dependent scan.
    /// Returns `None` if this is not a dependent scan.
    pub fn master_scan_number(&self) -> Option<i32> {
        self.0
            .get_i32("Master Scan Number:")
            .or_else(|| self.0.get_i32("Master Index:"))
    }

    /// Orbitrap / FT resolving power (e.g. 60000, 120000).
    pub fn ft_resolution(&self) -> Option<i32> {
        self.orbitrap_resolution()
    }

    /// Number of lock masses found / matched.
    pub fn number_of_lm_found(&self) -> Option<i32> {
        self.number_of_lock_masses()
    }

    /// Lock-mass m/z correction applied (ppm).
    pub fn lm_correction_ppm(&self) -> Option<f64> {
        self.lock_mass_correction_ppm()
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

    /// MSn isolation window width in m/z.
    ///
    /// Label varies: `"MS2 Isolation Width:"` (most common), `"MSn Isolation Width:"`,
    /// or `"Isolation Width (M/Z):"` on some firmware.
    pub fn isolation_width_mz(&self) -> Option<f64> {
        self.0
            .get_f64("MS2 Isolation Width:")
            .or_else(|| self.0.get_f64("MSn Isolation Width:"))
            .or_else(|| self.0.get_f64("Isolation Width (M/Z):"))
            .or_else(|| self.0.get_f64("MS2 Isolation Width (M/Z):"))
    }

    /// MSn isolation window target m/z (the center of the isolation window).
    ///
    /// Some instruments write this separately from the precursor m/z; when
    /// absent, callers should fall back to [`Self::monoisotopic_mz`] or to
    /// the event's first reaction `precursor_mz`.
    pub fn isolation_target_mz(&self) -> Option<f64> {
        self.0
            .get_f64("MS2 Isolation Offset:")
            .or_else(|| self.0.get_f64("Target M/Z:"))
    }

    /// Activation energy (eV or %) for the primary activation step.
    ///
    /// Tries several label variants present across instrument families:
    /// - `"HCD Energy (eV):"` — explicit eV label (Q Exactive HF-X, Exploris)
    /// - `"HCD Energy:"` — string form (e.g. "28.00" or "28%")
    /// - `"HCD Energy V:"` — Exploris variant
    /// - `"Normalized Collision Energy:"` — ion-trap CID variant
    /// - `"Collision Energy (eV):"` — ITMS CID
    /// - `"CE:"` — short form on some firmware
    pub fn activation_energy(&self) -> Option<f64> {
        if let Some(v) = self.0.get_f64("HCD Energy (eV):") {
            return Some(v);
        }
        if let Some(v) = self.0.get_f64("HCD Energy eV:") {
            return Some(v);
        }
        for label in &["HCD Energy:", "HCD Energy V:", "CE:"] {
            if let Some(s) = self.0.get_string(label) {
                if let Ok(v) = s.trim().trim_end_matches('%').parse::<f64>() {
                    return Some(v);
                }
            }
        }
        self.0
            .get_f64("Normalized Collision Energy:")
            .or_else(|| self.0.get_f64("Collision Energy (eV):"))
    }

    /// Supplemental activation energy for EThcD scans (the HCD component).
    ///
    /// Returns `None` for non-EThcD scans.
    pub fn supplemental_activation_energy(&self) -> Option<f64> {
        if let Some(v) = self.0.get_f64("Supplemental Activation CE:") {
            return Some(v);
        }
        if let Some(s) = self.0.get_string("Supplemental Activation:") {
            return s.trim().trim_end_matches('%').parse::<f64>().ok();
        }
        None
    }

    /// All possible charge states reported by the precursor selection algorithm.
    ///
    /// Returns `None` when the instrument did not report possible charges.
    /// Some firmware stores them as a space-delimited string (e.g. `"2 3"`);
    /// others use a typed integer for the single selected charge.
    pub fn possible_charge_states(&self) -> Option<Vec<u32>> {
        // String variant: "2 3 4"
        if let Some(s) = self.0.get_string("Possible Charge States:") {
            let v: Vec<u32> = s
                .split_whitespace()
                .filter_map(|t| t.parse::<u32>().ok())
                .collect();
            if !v.is_empty() {
                return Some(v);
            }
        }
        // Integer variant (single charge)
        if let Some(c) = self.charge_state() {
            if c > 0 {
                return Some(vec![c as u32]);
            }
        }
        None
    }

    /// FAIMS compensation voltage in V (Orbitrap Fusion/Lumos with FAIMS Pro).
    pub fn faims_cv(&self) -> Option<f64> {
        self.0
            .get_f64("FAIMS CV:")
            .or_else(|| self.0.get_f32("FAIMS CV:").map(f64::from))
    }

    /// Whether FAIMS voltage was active for this scan.
    pub fn faims_voltage_on(&self) -> Option<bool> {
        match self.0.get("FAIMS Voltage On:")? {
            GenericValue::Bool(b) => Some(*b),
            GenericValue::String(s) => Some(s.to_ascii_lowercase().contains("on")),
            _ => None,
        }
    }

    /// S-Lens RF level (V), typically reported on Q Exactive family.
    pub fn s_lens_rf_level(&self) -> Option<f64> {
        self.0.get_f64("S-Lens RF Level:")
    }

    /// AGC fill percentage (0.0–1.0), reported on Q Exactive HF family.
    pub fn agc_fill(&self) -> Option<f64> {
        self.0.get_f64("AGC Fill:")
    }

    /// Orbitrap analyzer temperature (°C), where available.
    pub fn analyzer_temperature(&self) -> Option<f64> {
        self.0.get_f64("Analyzer Temperature:")
    }

    /// PS injection time in milliseconds (pre-scan injection for Q Exactive).
    pub fn ps_injection_time_ms(&self) -> Option<f64> {
        self.0.get_f64("PS Inj. Time (ms):")
    }

    /// Reagent ion injection time in milliseconds (ETD reagent).
    pub fn reagent_ion_injection_time_ms(&self) -> Option<f64> {
        self.0
            .get_f32("Reagent Ion Injection Time (ms):")
            .map(f64::from)
    }

    /// Whether the reagent AGC was active.
    pub fn reagent_ion_agc(&self) -> Option<bool> {
        match self.0.get("Reagent Ion AGC:")? {
            GenericValue::Bool(b) => Some(*b),
            _ => None,
        }
    }

    /// Source CID energy applied in the ion source (eV).
    pub fn source_cid_energy_ev(&self) -> Option<f64> {
        self.0
            .get_f64("Source CID eV:")
            .or_else(|| self.0.get_f32("API Source CID Energy:").map(f64::from))
    }

    /// Dynamic retention time shift in minutes (Q Exactive HF-X AutoQC).
    pub fn dynamic_rt_shift_min(&self) -> Option<f64> {
        self.0.get_f64("Dynamic RT Shift (min):")
    }

    /// Lock mass correction applied (ppm) — tries several label variants.
    pub fn lock_mass_correction_ppm(&self) -> Option<f64> {
        self.0
            .get_f64("LM Correction (ppm):")
            .or_else(|| self.0.get_f64("LM m/z-Correction (ppm):"))
    }

    /// Number of lock masses found.
    pub fn number_of_lock_masses(&self) -> Option<i32> {
        self.0
            .get_i32("Number of LM Found:")
            .or_else(|| self.0.get_i32("Number of Lock Masses:"))
    }

    /// Orbitrap resolution setting (not measured, but requested).
    pub fn orbitrap_resolution(&self) -> Option<i32> {
        self.0
            .get_i32("Orbitrap Resolution:")
            .or_else(|| self.0.get_i32("FT Resolution:"))
    }

    /// SPS (Synchronous Precursor Selection) mass for MS3 channel N (0-based index).
    ///
    /// SPS masses are stored as `"SPS Mass 1:"`, `"SPS Mass 2:"`, ... (1-based).
    pub fn sps_mass(&self, channel: usize) -> Option<f32> {
        let label = format!("SPS Mass {}:", channel + 1);
        self.0.get_f32(&label)
    }

    /// Conversion parameter A (Orbitrap m/z conversion polynomial).
    pub fn conversion_parameter_a(&self) -> Option<f64> {
        self.0.get_f64("Conversion Parameter A:")
    }

    /// Conversion parameter B.
    pub fn conversion_parameter_b(&self) -> Option<f64> {
        self.0.get_f64("Conversion Parameter B:")
    }

    /// Conversion parameter C.
    pub fn conversion_parameter_c(&self) -> Option<f64> {
        self.0.get_f64("Conversion Parameter C:")
    }

    /// Raw over-fill time T (used for AGC computation).
    pub fn raw_ovft(&self) -> Option<f64> {
        self.0.get_f64("RawOvFtT:")
    }

    /// Error in the isotopic envelope fit (used for charge-state scoring).
    pub fn isotopic_fit_error(&self) -> Option<f64> {
        self.0.get_f64("Error in isotopic envelope fit:")
    }

    /// Scan description string (arbitrary text, set by method or real-time software).
    pub fn scan_description(&self) -> Option<&str> {
        self.0.get_string("Scan Description:")
    }

    /// Multi-inject info string (e.g. `"IT=45 "` for ion-trap fill time).
    pub fn multi_inject_info(&self) -> Option<&str> {
        self.0.get_string("Multi Inject Info:")
    }

    /// HCD energy string — raw value as stored (may be `"28.00"`, `"28%"`, or `"N/A"`).
    pub fn hcd_energy(&self) -> Option<&str> {
        self.0
            .get_string("HCD Energy:")
            .or_else(|| self.0.get_string("HCD Energy V:"))
    }
}

// ─── Status log (instrument log) typed accessor ─────────────────────────────

/// Typed accessor for a per-scan instrument-status log entry.
///
/// The instrument log records instrument-state values (temperatures, voltages,
/// pressures, etc.) at the time each scan was acquired. The schema varies
/// across instrument models.
pub struct StatusLogEntry<'a>(pub &'a GenericRecord);

impl<'a> StatusLogEntry<'a> {
    /// Return the raw record for direct field access.
    #[inline]
    pub fn record(&self) -> &GenericRecord {
        self.0
    }

    /// Ion injection time in milliseconds (present on Orbitrap family).
    pub fn ion_injection_time_ms(&self) -> Option<f64> {
        self.0
            .get_f64("Ion Injection Time (ms):")
            .or_else(|| self.0.get_f64("Ion Inject Time (ms):"))
    }

    /// Orbitrap / FT resolving power setting.
    pub fn ft_resolution(&self) -> Option<i32> {
        self.0
            .get_i32("Orbitrap Resolution:")
            .or_else(|| self.0.get_i32("FT Resolution:"))
    }

    /// FAIMS compensation voltage (V).
    pub fn faims_cv(&self) -> Option<f64> {
        self.0
            .get_f64("FAIMS CV:")
            .or_else(|| self.0.get_f32("FAIMS CV:").map(f64::from))
    }

    /// S-Lens RF level (V).
    pub fn s_lens_rf_level(&self) -> Option<f64> {
        self.0.get_f64("S-Lens RF Level:")
    }

    /// Orbitrap / analyzer temperature (°C).
    pub fn analyzer_temperature(&self) -> Option<f64> {
        self.0
            .get_f64("Analyzer Temperature:")
            .or_else(|| self.0.get_f32("Analyzer Temperature:").map(f64::from))
    }

    /// API (spray) source voltage (V).
    pub fn spray_voltage(&self) -> Option<f64> {
        self.0
            .get_f64("Spray Voltage (V):")
            .or_else(|| self.0.get_f64("Spray Voltage:"))
            .or_else(|| self.0.get_f32("Spray Voltage:").map(f64::from))
    }

    /// Lock mass reference correction (ppm).
    pub fn lock_mass_correction_ppm(&self) -> Option<f64> {
        self.0
            .get_f64("LM Correction (ppm):")
            .or_else(|| self.0.get_f64("LM m/z-Correction (ppm):"))
    }

    /// Capillary temperature (°C).
    pub fn capillary_temperature(&self) -> Option<f64> {
        self.0
            .get_f64("Capillary Temp (°C):")
            .or_else(|| self.0.get_f64("Capillary Temp:"))
            .or_else(|| self.0.get_f32("Capillary Temp:").map(f64::from))
    }

    /// Number of lock masses found.
    pub fn number_of_lock_masses(&self) -> Option<i32> {
        self.0
            .get_i32("Number of LM Found:")
            .or_else(|| self.0.get_i32("Number of Lock Masses:"))
    }

    /// Get any field by name (pass-through to the underlying record).
    pub fn get(&self, label: &str) -> Option<&GenericValue> {
        self.0.get(label)
    }

    /// Get a float64 field by name.
    pub fn get_f64(&self, label: &str) -> Option<f64> {
        self.0.get_f64(label)
    }

    /// Get an int32 field by name.
    pub fn get_i32(&self, label: &str) -> Option<i32> {
        self.0.get_i32(label)
    }

    /// Get a string field by name.
    pub fn get_string(&self, label: &str) -> Option<&str> {
        self.0.get_string(label)
    }
}
