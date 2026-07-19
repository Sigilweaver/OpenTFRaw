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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_util::audit_tag_bytes;
    use std::io::Cursor;

    fn read(bytes: Vec<u8>) -> Result<AuditTag> {
        let mut r = BinaryReader::new(Cursor::new(bytes));
        AuditTag::read(&mut r)
    }

    #[test]
    fn valid_tag_round_trips() {
        let bytes = audit_tag_bytes(1_700_000_000.0, "Xcalibur_System", "Q Exactive HF", 42);
        let tag = read(bytes).unwrap();
        assert_eq!(tag.tag1, "Xcalibur_System");
        assert_eq!(tag.tag2, "Q Exactive HF");
        assert_eq!(tag.unknown_long, 42);
        assert!((tag.time - 1_700_000_000.0).abs() < 1e-3);
    }

    #[test]
    fn zero_filetime_maps_to_zero() {
        let bytes = audit_tag_bytes(0.0, "", "", 0);
        let tag = read(bytes).unwrap();
        assert_eq!(tag.time, 0.0);
    }

    #[test]
    fn empty_tag_strings_round_trip() {
        let bytes = audit_tag_bytes(0.0, "", "", 0);
        let tag = read(bytes).unwrap();
        assert_eq!(tag.tag1, "");
        assert_eq!(tag.tag2, "");
    }

    #[test]
    fn truncated_input_is_eof() {
        let mut bytes = audit_tag_bytes(0.0, "a", "b", 0);
        bytes.truncate(50);
        assert!(read(bytes).is_err());
    }

    #[test]
    fn exact_size_is_112_bytes() {
        assert_eq!(AuditTag::SIZE, 112);
        let bytes = audit_tag_bytes(0.0, "x", "y", 0);
        assert_eq!(bytes.len(), AuditTag::SIZE);
    }
}
