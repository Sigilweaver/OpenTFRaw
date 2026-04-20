use std::io::{Read, Seek};
use crate::reader::BinaryReader;
use crate::error::Result;

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
