//! Tiny byte-slice helpers that surface typed errors on truncated input,
//! replacing `slice[a..b].try_into().unwrap()` chains.

use crate::error::{Error, Result};

#[inline]
fn slice_at<const N: usize>(bytes: &[u8], offset: usize) -> Result<[u8; N]> {
    bytes
        .get(offset..offset + N)
        .ok_or(Error::UnexpectedEof {
            offset: offset as u64,
            needed: N,
        })?
        .try_into()
        .map_err(|_| Error::UnexpectedEof {
            offset: offset as u64,
            needed: N,
        })
}

#[inline]
pub(crate) fn read_u32_le(bytes: &[u8], offset: usize) -> Result<u32> {
    Ok(u32::from_le_bytes(slice_at::<4>(bytes, offset)?))
}

#[inline]
pub(crate) fn read_f64_le(bytes: &[u8], offset: usize) -> Result<f64> {
    Ok(f64::from_le_bytes(slice_at::<8>(bytes, offset)?))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::Error;

    #[test]
    fn read_u32_le_decodes_little_endian() {
        let bytes = [0x78, 0x56, 0x34, 0x12];
        assert_eq!(read_u32_le(&bytes, 0).unwrap(), 0x1234_5678);
    }

    #[test]
    fn read_u32_le_reads_at_nonzero_offset() {
        let bytes = [0xff, 0x01, 0x00, 0x00, 0x00];
        assert_eq!(read_u32_le(&bytes, 1).unwrap(), 1);
    }

    #[test]
    fn read_u32_le_truncated_is_eof() {
        let bytes = [0x01, 0x02, 0x03];
        let err = read_u32_le(&bytes, 0).unwrap_err();
        assert!(matches!(
            err,
            Error::UnexpectedEof {
                offset: 0,
                needed: 4
            }
        ));
    }

    #[test]
    fn read_u32_le_offset_past_end_is_eof() {
        let bytes = [0x01, 0x02, 0x03, 0x04];
        assert!(read_u32_le(&bytes, 10).is_err());
    }

    #[test]
    fn read_f64_le_round_trips() {
        let value = -1234.5678_f64;
        let bytes = value.to_le_bytes();
        assert_eq!(read_f64_le(&bytes, 0).unwrap(), value);
    }

    #[test]
    fn read_f64_le_truncated_is_eof() {
        let bytes = [0u8; 7];
        let err = read_f64_le(&bytes, 0).unwrap_err();
        assert!(matches!(
            err,
            Error::UnexpectedEof {
                offset: 0,
                needed: 8
            }
        ));
    }

    #[test]
    fn read_f64_le_empty_slice_is_eof() {
        let bytes: [u8; 0] = [];
        assert!(read_f64_le(&bytes, 0).is_err());
    }
}
