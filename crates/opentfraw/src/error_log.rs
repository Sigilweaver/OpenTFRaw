use crate::error::Result;
use crate::reader::BinaryReader;
use std::io::{Read, Seek};

/// An error log entry.
#[derive(Debug)]
pub struct ErrorEntry {
    pub time: f32,
    pub message: String,
}

impl ErrorEntry {
    pub(crate) fn read<R: Read + Seek>(r: &mut BinaryReader<R>) -> Result<Self> {
        let time = r.read_f32()?;
        let message = r.read_pascal_string()?;
        Ok(Self { time, message })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_util::pascal_string;
    use std::io::Cursor;

    fn read(bytes: Vec<u8>) -> Result<ErrorEntry> {
        let mut r = BinaryReader::new(Cursor::new(bytes));
        ErrorEntry::read(&mut r)
    }

    fn entry_bytes(time: f32, message: &str) -> Vec<u8> {
        let mut out = time.to_le_bytes().to_vec();
        out.extend_from_slice(&pascal_string(message));
        out
    }

    #[test]
    fn valid_entry_round_trips() {
        let entry = read(entry_bytes(12.5, "Reagent vial 1 configuration:")).unwrap();
        assert_eq!(entry.time, 12.5);
        assert_eq!(entry.message, "Reagent vial 1 configuration:");
    }

    #[test]
    fn empty_message_round_trips() {
        let entry = read(entry_bytes(0.0, "")).unwrap();
        assert_eq!(entry.message, "");
    }

    #[test]
    fn truncated_time_is_eof() {
        let bytes = vec![0u8; 2]; // f32 needs 4 bytes
        assert!(read(bytes).is_err());
    }

    #[test]
    fn truncated_message_is_eof() {
        let mut bytes = 1.0f32.to_le_bytes().to_vec();
        // Declare a char count that promises far more bytes than are present.
        bytes.extend_from_slice(&1000u32.to_le_bytes());
        assert!(read(bytes).is_err());
    }

    #[test]
    fn implausible_char_count_does_not_allocate_unbounded_memory() {
        // A crafted file could declare a char count far larger than the file
        // could ever contain; this must be rejected before allocating, not
        // just fail after an attempted huge allocation.
        let mut bytes = 1.0f32.to_le_bytes().to_vec();
        bytes.extend_from_slice(&u32::MAX.to_le_bytes());
        let err = read(bytes).unwrap_err();
        assert!(matches!(
            err,
            crate::error::Error::AllocationTooLarge { .. }
        ));
    }
}
