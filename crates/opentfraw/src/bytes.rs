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
