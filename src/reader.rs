use std::io::{Read, Seek, SeekFrom};

use crate::error::{Error, Result};
use crate::error_log::ErrorEntry;
use crate::generic_data::{GenericDataHeader, GenericRecord};
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

        // 9. Scan parameters (trailer extra) — GenericData format in v64+
        let (scan_parameters_header, scan_parameters) = if version >= 64 {
            r.seek_to(run_header.scan_params_addr)?;
            match GenericDataHeader::try_read(&mut r)? {
                Some(hdr) => {
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

        // 10. Error log
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
}
