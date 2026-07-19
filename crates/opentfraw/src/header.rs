use crate::audit_tag::AuditTag;
use crate::error::{Error, Result};
use crate::reader::BinaryReader;
use std::io::{Read, Seek};

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::Error;
    use crate::test_util::file_header_bytes;
    use std::io::Cursor;

    fn read(bytes: Vec<u8>) -> Result<FileHeader> {
        let mut r = BinaryReader::new(Cursor::new(bytes));
        FileHeader::read(&mut r)
    }

    #[test]
    fn valid_header_v66_round_trips() {
        let hdr = read(file_header_bytes(66)).unwrap();
        assert_eq!(hdr.magic, 0xa101);
        assert_eq!(hdr.signature, "Finnigan");
        assert_eq!(hdr.version, 66);
        assert_eq!(hdr.audit_start.tag2, "Test Instrument");
        assert_eq!(hdr.tag.trim_end_matches('\0'), "tag");
    }

    #[test]
    fn every_supported_version_parses() {
        for &v in &[8u32, 47, 57, 60, 62, 63, 64, 66] {
            let hdr = read(file_header_bytes(v)).unwrap();
            assert_eq!(hdr.version, v);
        }
    }

    #[test]
    fn rejects_bad_magic() {
        let mut bytes = file_header_bytes(66);
        bytes[0] = 0x00;
        bytes[1] = 0x00;
        let err = read(bytes).unwrap_err();
        assert!(matches!(err, Error::BadMagic(0)));
    }

    #[test]
    fn rejects_bad_signature() {
        let mut bytes = file_header_bytes(66);
        // Corrupt the signature region (bytes 2..20).
        bytes[2] = b'X';
        let err = read(bytes).unwrap_err();
        assert!(matches!(err, Error::BadSignature(_)));
    }

    #[test]
    fn rejects_unsupported_version() {
        let bytes = file_header_bytes(999);
        let err = read(bytes).unwrap_err();
        assert!(matches!(err, Error::UnsupportedVersion(999)));
    }

    #[test]
    fn truncated_header_is_eof() {
        let mut bytes = file_header_bytes(66);
        bytes.truncate(100);
        assert!(read(bytes).is_err());
    }

    #[test]
    fn empty_input_is_eof() {
        assert!(read(Vec::new()).is_err());
    }
}
