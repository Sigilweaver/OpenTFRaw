use crate::error::Result;
use crate::reader::BinaryReader;
use std::io::{Read, Seek};

/// The critical preamble containing addresses and acquisition date.
#[derive(Debug)]
pub struct RawFileInfoPreamble {
    pub method_file_present: bool,
    pub year: u16,
    pub month: u16,
    pub day_of_week: u16,
    pub day: u16,
    pub hour: u16,
    pub minute: u16,
    pub second: u16,
    pub millisecond: u16,
    pub controller_count: u32,
    /// File offset to the scan data stream.
    pub data_addr: u64,
    /// File offset to the first RunHeader (may be a non-MS controller in multi-controller files).
    pub run_header_addr: u64,
    /// All run header addresses (one per controller). Index 0 == run_header_addr.
    pub run_header_addrs: Vec<u64>,
    /// Second RunHeader address (for multi-controller files).
    pub run_header_addr_2: u64,
}

/// RawFileInfo: preamble + label strings + computer name.
#[derive(Debug)]
pub struct RawFileInfo {
    pub preamble: RawFileInfoPreamble,
    pub label_headings: Vec<String>,
    pub computer_name: String,
}

impl RawFileInfo {
    pub(crate) fn read<R: Read + Seek>(r: &mut BinaryReader<R>, version: u32) -> Result<Self> {
        let preamble = RawFileInfoPreamble::read(r, version)?;

        // 5 label heading strings + computer name
        let mut label_headings = Vec::with_capacity(5);
        for _ in 0..5 {
            label_headings.push(r.read_pascal_string()?);
        }
        let computer_name = r.read_pascal_string()?;

        Ok(Self {
            preamble,
            label_headings,
            computer_name,
        })
    }
}

impl RawFileInfoPreamble {
    pub(crate) fn read<R: Read + Seek>(r: &mut BinaryReader<R>, version: u32) -> Result<Self> {
        let method_file_present = r.read_u32()? != 0;
        let year = r.read_u16()?;
        let month = r.read_u16()?;
        let day_of_week = r.read_u16()?;
        let day = r.read_u16()?;
        let hour = r.read_u16()?;
        let minute = r.read_u16()?;
        let second = r.read_u16()?;
        let millisecond = r.read_u16()?;

        if version >= 64 {
            // Version 66 layout
            let _unk2 = r.read_u32()?;
            let _data_addr_32 = r.read_u32()?; // defunct
            let controller_count = r.read_u32()?;
            let _controller_n2 = r.read_u32()?;
            let _unk5 = r.read_u32()?;
            let _unk6 = r.read_u32()?;
            let _run_header_addr_32 = r.read_u32()?; // defunct

            // Skip unknown_area[1]: 760 bytes
            r.skip(760)?;

            // Controller address table. Layout is:
            //   [data_addr: u64] [u32] [u32]           -- scan data entry
            //   [run_hdr[0]: u64] [u32] [u32]          -- controller 0
            //   [run_hdr[1]: u64] [u32] [u32]          -- controller 1
            //   ... (for every controller beyond 2, each entry is 16 bytes
            //        and lives at the start of what used to be "unknown_area[2]")
            //   [padding zeros to fill 1048-byte region]
            //
            // The total region (from data_addr through the skip) is 1048 bytes for
            // a 2-controller file. Each extra controller takes 16 bytes from the skip.
            let data_addr = r.read_u64()?;
            let _unk7 = r.read_u32()?;
            let _unk8 = r.read_u32()?;

            let mut run_header_addrs = Vec::new();
            // Always read at least 2 entries for compatibility
            let n_read = controller_count.max(2) as usize;

            for _ in 0..n_read {
                let addr = r.read_u64()?;
                let _unk_a = r.read_u32()?;
                let _unk_b = r.read_u32()?;
                run_header_addrs.push(addr);
            }

            let run_header_addr = run_header_addrs[0];
            let run_header_addr_2 = run_header_addrs.get(1).copied().unwrap_or(0);

            // Remaining padding to reach end of address-table region.
            // The region size differs by file version:
            //   v64: 1032 bytes (data entry + up to ~2 controller entries + padding)
            //   v66+: 1048 bytes (one extra slot for the additional controller table)
            let used_bytes = 16usize + n_read * 16;
            let total_region = if version >= 66 { 1048usize } else { 1032usize };
            let remaining = total_region.saturating_sub(used_bytes);
            r.skip(remaining)?;

            Ok(Self {
                method_file_present,
                year,
                month,
                day_of_week,
                day,
                hour,
                minute,
                second,
                millisecond,
                controller_count,
                data_addr,
                run_header_addr,
                run_header_addrs,
                run_header_addr_2,
            })
        } else {
            // Pre-v64 (32-bit addresses)
            let _unk2 = r.read_u32()?;
            let data_addr = r.read_u32()? as u64;
            let controller_count = r.read_u32()?;
            let _controller_n2 = r.read_u32()?;
            let _unk5 = r.read_u32()?;
            let _unk6 = r.read_u32()?;
            let run_header_addr = r.read_u32()? as u64;
            let _unk7 = r.read_u32()?;
            let _unk8 = r.read_u32()?;
            let run_header_addr_2 = r.read_u32()? as u64;

            // Skip unknown_area: 744 bytes
            r.skip(744)?;

            Ok(Self {
                method_file_present,
                year,
                month,
                day_of_week,
                day,
                hour,
                minute,
                second,
                millisecond,
                controller_count,
                data_addr,
                run_header_addr,
                run_header_addrs: vec![run_header_addr, run_header_addr_2],
                run_header_addr_2,
            })
        }
    }
}
