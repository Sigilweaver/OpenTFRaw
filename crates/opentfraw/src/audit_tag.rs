use crate::error::Result;
use crate::reader::BinaryReader;
use std::io::{Read, Seek};

/// Audit tag embedded in FileHeader (112 bytes).
#[derive(Debug)]
pub struct AuditTag {
    /// Unix timestamp (seconds since epoch).
    pub time: f64,
    /// Primary tag (usually "Xcalibur_System").
    pub tag1: String,
    /// Secondary tag (instrument model, user name, etc.).
    pub tag2: String,
    pub unknown_long: u32,
}

impl AuditTag {
    pub const SIZE: usize = 112;

    pub(crate) fn read<R: Read + Seek>(r: &mut BinaryReader<R>) -> Result<Self> {
        let time = r.read_windows_filetime()?;
        let tag1 = r.read_utf16_fixed(50)?;
        let tag2 = r.read_utf16_fixed(50)?;
        let unknown_long = r.read_u32()?;
        Ok(Self {
            time,
            tag1,
            tag2,
            unknown_long,
        })
    }
}
