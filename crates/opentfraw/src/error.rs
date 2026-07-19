use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("invalid magic number: expected 0xa101, got 0x{0:04x}")]
    BadMagic(u16),

    #[error("invalid signature: expected \"Finnigan\", got {0:?}")]
    BadSignature(String),

    #[error("unsupported version: {0}")]
    UnsupportedVersion(u32),

    #[error("unexpected end of file at offset {offset:#x} (needed {needed} bytes)")]
    UnexpectedEof { offset: u64, needed: usize },

    #[error("invalid UTF-16 data at offset {0:#x}")]
    InvalidUtf16(u64),

    #[error("address out of range: {0:#x}")]
    AddressOutOfRange(u64),

    #[error("invalid generic data type code: {0}")]
    InvalidTypeCode(u32),

    #[error("operation not supported for this scan-data format: {0}")]
    UnsupportedOperation(&'static str),

    #[error(
        "refusing to allocate for {requested} bytes at offset {offset:#x}: \
         only {available} bytes remain in the input"
    )]
    AllocationTooLarge {
        offset: u64,
        requested: u64,
        available: u64,
    },
}

pub type Result<T> = std::result::Result<T, Error>;
