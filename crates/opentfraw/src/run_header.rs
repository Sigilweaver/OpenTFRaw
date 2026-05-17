use crate::error::Result;
use crate::reader::BinaryReader;
use crate::sample_info::SampleInfo;
use std::io::{Read, Seek};

/// Run header - the secondary index structure with pointers to all data streams.
#[derive(Debug)]
pub struct RunHeader {
    pub sample_info: SampleInfo,
    pub file_names: Vec<String>,
    pub ntrailer: u32,
    pub nparams: u32,
    pub nsegs: u32,
    /// File offset to scan index array.
    pub scan_index_addr: u64,
    /// File offset to scan data stream.
    pub data_addr: u64,
    /// File offset to instrument log.
    pub inst_log_addr: u64,
    /// File offset to error log.
    pub error_log_addr: u64,
    /// Unknown address between error_log and scan_trailer (v64+ only).
    /// Hypothesis: GenericDataHeader for scan trailer / scan params schema.
    pub unk_addr: u64,
    /// File offset to scan event trailer stream.
    pub scan_trailer_addr: u64,
    /// File offset to scan parameters stream.
    pub scan_params_addr: u64,
    /// Self-address for validation.
    pub own_addr: u64,
}

impl RunHeader {
    pub(crate) fn read<R: Read + Seek>(r: &mut BinaryReader<R>, version: u32) -> Result<Self> {
        let sample_info = SampleInfo::read(r)?;

        // File name fields: 13 × UTF16LE(520) with 2 Float64 after field 6
        let mut file_names = Vec::with_capacity(13);
        for i in 0..13 {
            if i == 6 {
                // Between file_name[6] and file_name[7], there are 2 Float64s
                let _unk_d1 = r.read_f64()?;
                let _unk_d2 = r.read_f64()?;
            }
            file_names.push(r.read_utf16_fixed(520)?);
        }

        if version >= 64 {
            // Defunct 32-bit addresses
            let _scan_trailer_addr_32 = r.read_u32()?;
            let _scan_params_addr_32 = r.read_u32()?;

            let ntrailer = r.read_u32()?;
            let nparams = r.read_u32()?;
            let nsegs = r.read_u32()?;

            let _unk1 = r.read_u32()?;
            let _unk2 = r.read_u32()?;

            let _own_addr_32 = r.read_u32()?; // defunct
            let _unk3 = r.read_u32()?;
            let _unk4 = r.read_u32()?;

            // 64-bit addresses
            let scan_index_addr = r.read_u64()?;
            let data_addr = r.read_u64()?;
            let inst_log_addr = r.read_u64()?;
            let error_log_addr = r.read_u64()?;
            let unk_addr = r.read_u64()?;
            let scan_trailer_addr = r.read_u64()?;
            let scan_params_addr = r.read_u64()?;
            let _unk5 = r.read_u32()?;
            let _unk6 = r.read_u32()?;
            let own_addr = r.read_u64()?;

            // 24 unknown u32s
            r.skip(24 * 4)?;

            Ok(Self {
                sample_info,
                file_names,
                ntrailer,
                nparams,
                nsegs,
                scan_index_addr,
                data_addr,
                inst_log_addr,
                error_log_addr,
                unk_addr,
                scan_trailer_addr,
                scan_params_addr,
                own_addr,
            })
        } else {
            // Pre-v64 layout (v57/v60/v62/v63):
            // scan_trailer_addr(u32), scan_params_addr(u32),
            // unknown_length[1](u32), unknown_length[2](u32), nsegs(u32),
            // unknown[1](u32), unknown[2](u32), own_addr(u32),
            // unknown[3](u32), unknown[4](u32)
            let scan_trailer_addr = r.read_u32()? as u64;
            let scan_params_addr = r.read_u32()? as u64;
            let ntrailer = r.read_u32()?;
            let nparams = r.read_u32()?;
            let nsegs = r.read_u32()?;
            let _unk1 = r.read_u32()?;
            let _unk2 = r.read_u32()?;
            let own_addr = r.read_u32()? as u64;
            let _unk3 = r.read_u32()?;
            let _unk4 = r.read_u32()?;

            // Stream addresses from SampleInfo 32-bit fields
            let scan_index_addr = sample_info.scan_index_addr_32 as u64;
            let data_addr = sample_info.data_addr_32 as u64;
            let inst_log_addr = sample_info.inst_log_addr_32 as u64;
            let error_log_addr = sample_info.error_log_addr_32 as u64;

            Ok(Self {
                sample_info,
                file_names,
                ntrailer,
                nparams,
                nsegs,
                scan_index_addr,
                data_addr,
                inst_log_addr,
                error_log_addr,
                unk_addr: 0,
                scan_trailer_addr,
                scan_params_addr,
                own_addr,
            })
        }
    }
}
