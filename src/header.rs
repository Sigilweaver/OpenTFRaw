use std::io::{Read, Seek};
use crate::reader::BinaryReader;
use crate::audit_tag::AuditTag;
use crate::error::{Error, Result};

/// The 1356-byte file header at offset 0x0000.
#[derive(Debug)]
pub struct FileHeader {
    pub magic: u16,
    pub signature: String,
    pub version: u32,
    pub audit_start: AuditTag,
    pub audit_end: AuditTag,
    pub tag: String,
}

impl FileHeader {
    pub const SIZE: usize = 1356;

    pub(crate) fn read<R: Read + Seek>(r: &mut BinaryReader<R>) -> Result<Self> {
        let magic = r.read_u16()?;
        if magic != 0xa101 {
            return Err(Error::BadMagic(magic));
        }

        // Signature: 18 bytes UTF-16-LE = 9 code units (8 chars + null)
        let signature = r.read_utf16_fixed(18)?;
        if signature != "Finnigan" {
            return Err(Error::BadSignature(signature));
        }

        // unknown_long[1..4]: 4 × u32 = 16 bytes
        r.skip(16)?;

        let version = r.read_u32()?;
        match version {
            8 | 47 | 57 | 60 | 62 | 63 | 64 | 66 => {}
            _ => return Err(Error::UnsupportedVersion(version)),
        }

        // AuditTag start (112 bytes)
        let audit_start = AuditTag::read(r)?;
        // AuditTag end (112 bytes)
        let audit_end = AuditTag::read(r)?;

        // unknown_long[5]: 4 bytes
        r.skip(4)?;

        // unknown_area: 60 bytes
        r.skip(60)?;

        // Tag: 1028 bytes UTF-16-LE (514 code units)
        let tag = r.read_utf16_fixed(1028)?;

        Ok(Self {
            magic,
            signature,
            version,
            audit_start,
            audit_end,
            tag,
        })
    }
}
