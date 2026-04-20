use std::io::{Read, Seek};
use crate::reader::BinaryReader;
use crate::error::Result;

/// Summary information about the acquisition, embedded in RunHeader.
#[derive(Debug)]
pub struct SampleInfo {
    pub first_scan_number: u32,
    pub last_scan_number: u32,
    pub inst_log_length: u32,
    pub error_log_length: u32,
    /// 32-bit scan index address (used in pre-v64).
    pub scan_index_addr_32: u32,
    /// 32-bit data address (used in pre-v64).
    pub data_addr_32: u32,
    /// 32-bit instrument log address (used in pre-v64).
    pub inst_log_addr_32: u32,
    /// 32-bit error log address (used in pre-v64).
    pub error_log_addr_32: u32,
    pub max_ion_current: f64,
    pub low_mz: f64,
    pub high_mz: f64,
    pub start_time: f64,
    pub end_time: f64,
    pub tag1: String,
    pub tag2: String,
    pub tag3: String,
}

impl SampleInfo {
    pub const SIZE: usize = 592;

    pub(crate) fn read<R: Read + Seek>(r: &mut BinaryReader<R>) -> Result<Self> {
        let _unk1 = r.read_u32()?;
        let _unk2 = r.read_u32()?;
        let first_scan_number = r.read_u32()?;
        let last_scan_number = r.read_u32()?;
        let inst_log_length = r.read_u32()?;
        let error_log_length = r.read_u32()?;
        let _unk4 = r.read_u32()?;

        // 32-bit addresses (used in pre-v64; defunct in v64+)
        let scan_index_addr_32 = r.read_u32()?;
        let data_addr_32 = r.read_u32()?;
        let inst_log_addr_32 = r.read_u32()?;
        let error_log_addr_32 = r.read_u32()?;

        let _unk5 = r.read_u32()?;
        let max_ion_current = r.read_f64()?;
        let low_mz = r.read_f64()?;
        let high_mz = r.read_f64()?;
        let start_time = r.read_f64()?;
        let end_time = r.read_f64()?;

        // unknown_area: 56 bytes
        r.skip(56)?;

        // Tags
        let tag1 = r.read_utf16_fixed(88)?;  // 44 chars
        let tag2 = r.read_utf16_fixed(40)?;  // 20 chars
        let tag3 = r.read_utf16_fixed(320)?; // 160 chars

        Ok(Self {
            first_scan_number,
            last_scan_number,
            inst_log_length,
            error_log_length,
            scan_index_addr_32,
            data_addr_32,
            inst_log_addr_32,
            error_log_addr_32,
            max_ion_current,
            low_mz,
            high_mz,
            start_time,
            end_time,
            tag1,
            tag2,
            tag3,
        })
    }
}
