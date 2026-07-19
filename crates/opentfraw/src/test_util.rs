//! Shared helpers for building hand-crafted binary fixtures in unit tests.
//!
//! These construct byte-for-byte valid (or deliberately broken) encodings of
//! the on-disk structures this crate decodes, without needing real `.raw`
//! files. Per the project's clean-room policy, none of this is derived from
//! vendor sources - it mirrors the encodings already documented and
//! implemented in this crate's own read functions.

#![cfg(test)]

/// Encode `s` as UTF-16LE, zero-padded (or truncated) to exactly `byte_len` bytes.
pub(crate) fn utf16_fixed(s: &str, byte_len: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(byte_len);
    for u in s.encode_utf16() {
        out.extend_from_slice(&u.to_le_bytes());
    }
    out.resize(byte_len, 0);
    out
}

/// Encode a PascalStringWin32: UInt32 char count, then that many UTF-16LE code units.
pub(crate) fn pascal_string(s: &str) -> Vec<u8> {
    let units: Vec<u16> = s.encode_utf16().collect();
    let mut out = Vec::with_capacity(4 + units.len() * 2);
    out.extend_from_slice(&(units.len() as u32).to_le_bytes());
    for u in units {
        out.extend_from_slice(&u.to_le_bytes());
    }
    out
}

/// Encode a Windows FILETIME for the given Unix-epoch seconds value.
/// Inverse of `BinaryReader::read_windows_filetime`.
pub(crate) fn windows_filetime(unix_seconds: f64) -> u64 {
    if unix_seconds == 0.0 {
        return 0;
    }
    (((unix_seconds + 11_644_473_600.0) * 10_000_000.0) as i64) as u64
}

/// A 112-byte AuditTag blob: FILETIME(8) + tag1 utf16(50) + tag2 utf16(50) + u32(4).
pub(crate) fn audit_tag_bytes(time: f64, tag1: &str, tag2: &str, unknown_long: u32) -> Vec<u8> {
    let mut out = Vec::with_capacity(112);
    out.extend_from_slice(&windows_filetime(time).to_le_bytes());
    out.extend_from_slice(&utf16_fixed(tag1, 50));
    out.extend_from_slice(&utf16_fixed(tag2, 50));
    out.extend_from_slice(&unknown_long.to_le_bytes());
    assert_eq!(out.len(), 112);
    out
}

/// A minimal valid 1356-byte FileHeader blob for the given supported version.
pub(crate) fn file_header_bytes(version: u32) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(&0xa101u16.to_le_bytes()); // magic
    out.extend_from_slice(&utf16_fixed("Finnigan", 18)); // signature
    out.extend_from_slice(&[0u8; 16]); // unknown_long[1..4]
    out.extend_from_slice(&version.to_le_bytes());
    out.extend_from_slice(&audit_tag_bytes(
        0.0,
        "Xcalibur_System",
        "Test Instrument",
        0,
    ));
    out.extend_from_slice(&audit_tag_bytes(
        0.0,
        "Xcalibur_System",
        "Test Instrument",
        0,
    ));
    out.extend_from_slice(&[0u8; 4]); // unknown_long[5]
    out.extend_from_slice(&[0u8; 60]); // unknown_area
    out.extend_from_slice(&utf16_fixed("tag", 1028)); // Tag
    assert_eq!(out.len(), 1356);
    out
}
