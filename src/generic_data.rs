use crate::error::Result;
use crate::reader::BinaryReader;
use crate::types::GenericType;
use std::io::{Read, Seek};

/// A field descriptor within a GenericDataHeader.
#[derive(Debug, Clone)]
pub struct GenericDataDescriptor {
    pub field_type: GenericType,
    pub length: u32,
    pub label: String,
}

/// Self-describing header for GenericRecord streams.
#[derive(Debug)]
pub struct GenericDataHeader {
    pub fields: Vec<GenericDataDescriptor>,
}

/// A typed value from a generic record.
#[derive(Debug, Clone)]
pub enum GenericValue {
    Gap,
    Int8(i8),
    Bool(bool),
    UInt8(u8),
    Int16(i16),
    UInt16(u16),
    Int32(i32),
    UInt32(u32),
    Float32(f32),
    Float64(f64),
    String(String),
}

/// A single record decoded using a GenericDataHeader.
#[derive(Debug)]
pub struct GenericRecord {
    pub values: Vec<(String, GenericValue)>,
}

impl GenericDataHeader {
    /// Try to read a GenericDataHeader. Returns `None` if the data at the
    /// current position does not look like a valid header (e.g. unreasonable
    /// field count or invalid type codes). The reader position is restored
    /// on failure.
    pub(crate) fn try_read<R: Read + Seek>(r: &mut BinaryReader<R>) -> Result<Option<Self>> {
        let saved_pos = r.position();
        let n = r.read_u32()?;
        if n > 1000 {
            r.seek_to(saved_pos)?;
            return Ok(None);
        }
        let mut fields = Vec::with_capacity(n as usize);
        for _ in 0..n {
            let type_code = r.read_u32()?;
            match GenericType::from_u32(type_code) {
                Some(field_type) => {
                    let length = r.read_u32()?;
                    let label = r.read_pascal_string()?;
                    fields.push(GenericDataDescriptor {
                        field_type,
                        length,
                        label,
                    });
                }
                None => {
                    r.seek_to(saved_pos)?;
                    return Ok(None);
                }
            }
        }
        Ok(Some(Self { fields }))
    }
}

impl GenericRecord {
    pub(crate) fn read<R: Read + Seek>(
        r: &mut BinaryReader<R>,
        header: &GenericDataHeader,
    ) -> Result<Self> {
        let mut values = Vec::with_capacity(header.fields.len());
        for desc in &header.fields {
            let label = desc.label.clone();
            let value = match desc.field_type {
                GenericType::Gap => GenericValue::Gap,
                GenericType::Int8 => GenericValue::Int8(r.read_i8()?),
                GenericType::Bool | GenericType::BoolYesNo | GenericType::BoolOnOff => {
                    GenericValue::Bool(r.read_u8()? != 0)
                }
                GenericType::UInt8 => GenericValue::UInt8(r.read_u8()?),
                GenericType::Int16 => GenericValue::Int16(r.read_i16()?),
                GenericType::UInt16 => GenericValue::UInt16(r.read_u16()?),
                GenericType::Int32 => GenericValue::Int32(r.read_i32()?),
                GenericType::UInt32 => GenericValue::UInt32(r.read_u32()?),
                GenericType::Float32 => GenericValue::Float32(r.read_f32()?),
                GenericType::Float64 => GenericValue::Float64(r.read_f64()?),
                GenericType::AsciiString => {
                    let s = if desc.length > 0 {
                        let bytes = r.read_bytes(desc.length as usize)?;
                        let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
                        String::from_utf8_lossy(&bytes[..end]).into_owned()
                    } else {
                        String::new()
                    };
                    GenericValue::String(s)
                }
                GenericType::WideString => {
                    let s = if desc.length > 0 {
                        r.read_utf16_fixed(desc.length as usize * 2)?
                    } else {
                        String::new()
                    };
                    GenericValue::String(s)
                }
            };
            values.push((label, value));
        }
        Ok(Self { values })
    }

    /// Look up a field by label and return a reference to its value.
    pub fn get(&self, label: &str) -> Option<&GenericValue> {
        self.values.iter().find(|(l, _)| l == label).map(|(_, v)| v)
    }

    /// Get a float64 field by label.
    pub fn get_f64(&self, label: &str) -> Option<f64> {
        match self.get(label)? {
            GenericValue::Float64(v) => Some(*v),
            GenericValue::Float32(v) => Some(*v as f64),
            _ => None,
        }
    }

    /// Get an i32 field by label.
    pub fn get_i32(&self, label: &str) -> Option<i32> {
        match self.get(label)? {
            GenericValue::Int32(v) => Some(*v),
            GenericValue::Int16(v) => Some(*v as i32),
            GenericValue::Int8(v) => Some(*v as i32),
            _ => None,
        }
    }

    /// Get a string field by label.
    pub fn get_string(&self, label: &str) -> Option<&str> {
        match self.get(label)? {
            GenericValue::String(v) => Some(v.as_str()),
            _ => None,
        }
    }
}

impl GenericValue {
    /// Get as f64, converting numeric types.
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Self::Float64(v) => Some(*v),
            Self::Float32(v) => Some(*v as f64),
            Self::Int32(v) => Some(*v as f64),
            Self::UInt32(v) => Some(*v as f64),
            Self::Int16(v) => Some(*v as f64),
            Self::UInt16(v) => Some(*v as f64),
            Self::Int8(v) => Some(*v as f64),
            Self::UInt8(v) => Some(*v as f64),
            _ => None,
        }
    }
}
