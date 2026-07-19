use crate::error::Result;
use crate::reader::BinaryReader;
use std::io::{Read, Seek};

/// One entry in the scan index array (v66: 92 bytes).
#[derive(Debug)]
pub struct ScanIndexEntry {
    pub index: u32,
    pub scan_event: u16,
    pub scan_segment: u16,
    pub data_size: u32,
    pub start_time: f64,
    pub total_current: f64,
    pub base_intensity: f64,
    pub base_mz: f64,
    pub low_mz: f64,
    pub high_mz: f64,
    /// 64-bit offset relative to data stream start.
    pub offset: u64,
}

impl ScanIndexEntry {
    /// Minimum on-disk size in bytes of one entry for the given file version.
    /// Used to bound `Vec::with_capacity(num_scans)` against the file's
    /// actual remaining size before allocating.
    pub(crate) fn min_size_for_version(version: u32) -> u64 {
        // offset_32(4) + index(4) + scan_event(2) + scan_segment(2) + next(4)
        // + unk(4) + data_size(4) + 6 * f64(8) = 72 bytes, common to every
        // version (the offset_32 field is reused directly pre-v64).
        const BASE: u64 = 72;
        if version >= 66 {
            BASE + 8 /* u64 offset instead of offset_32 */ + 8 /* 2 trailing u32s */
        } else if version >= 64 {
            BASE + 8 /* u64 offset instead of offset_32 */
        } else {
            BASE
        }
    }

    pub(crate) fn read<R: Read + Seek>(r: &mut BinaryReader<R>, version: u32) -> Result<Self> {
        let offset_32 = r.read_u32()?;
        let index = r.read_u32()?;
        let scan_event = r.read_u16()?;
        let scan_segment = r.read_u16()?;
        let _next = r.read_u32()?;
        let _unk = r.read_u32()?;
        let data_size = r.read_u32()?;
        let start_time = r.read_f64()?;
        let total_current = r.read_f64()?;
        let base_intensity = r.read_f64()?;
        let base_mz = r.read_f64()?;
        let low_mz = r.read_f64()?;
        let high_mz = r.read_f64()?;

        let offset = if version >= 64 {
            r.read_u64()?
        } else {
            offset_32 as u64
        };

        if version >= 66 {
            // Two trailing unknown u32s
            let _unk1 = r.read_u32()?;
            let _unk2 = r.read_u32()?;
        }

        Ok(Self {
            index,
            scan_event,
            scan_segment,
            data_size,
            start_time,
            total_current,
            base_intensity,
            base_mz,
            low_mz,
            high_mz,
            offset,
        })
    }
}
